use axum::{
    extract::State,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc;

use crate::{
    api::error::{ApiError, ApiResult},
    scanner,
    storage::{self, devices as dev_store, networks as net_store},
    types::ScanEvent,
    AppState,
};

#[derive(Deserialize)]
pub struct ScanRequest {
    pub mode: Option<String>,
}

pub async fn trigger_scan(
    State(state): State<AppState>,
    Json(req): Json<ScanRequest>,
) -> ApiResult<impl IntoResponse> {
    let mode_str = req.mode.as_deref().unwrap_or("silent");
    let mode = scanner::ScanMode::from_str(mode_str);
    let scan_id = format!("manual-{}", storage::now_ms());

    let guard = scanner::ScanGuard::try_acquire()
        .ok_or_else(|| ApiError::ScanInProgress("A scan is already running".into()))?;

    let (event_tx, event_rx) = mpsc::unbounded_channel::<ScanEvent>();
    let broadcast_tx = state.broadcast_tx.clone();
    let db = state.db.clone();
    let scan_id_out = scan_id.clone();

    // Relay: scanner events → WebSocket broadcast
    let bcast = broadcast_tx.clone();
    tokio::spawn(relay_scan_events(event_rx, bcast));

    // Scan task
    tokio::spawn(async move {
        let lifecycle = scanner::ScanLifecycle::new(scan_id.clone(), broadcast_tx.clone());

        // Detect current network identity before the scan.
        let network_info = scanner::network_identity::get_current_network_info().await;
        let network_id: Option<i64> = if let Some(ref bssid) = network_info.bssid {
            let now = storage::now_ms();
            let bssid_clone = bssid.clone();
            let ssid_clone = network_info.ssid.clone();
            let gateway_clone = network_info.gateway.clone();
            let subnet_clone = network_info.subnet.clone();

            match db.execute(move |conn| {
                net_store::upsert_network(
                    conn,
                    ssid_clone.as_deref(),
                    &bssid_clone,
                    gateway_clone.as_deref(),
                    subnet_clone.as_deref(),
                    now,
                )
            }).await {
                Ok(id) => Some(id),
                Err(e) => {
                    log::warn!("[SCAN] Network upsert failed: {e}");
                    None
                }
            }
        } else {
            None
        };


        match scanner::scan_local_network_pre_guarded(Some(state.db.clone()), Some(state.devices.clone()), guard, Some(event_tx), mode, scan_id.clone())
            .await
        {
            Ok(result) => {
                log::info!(
                    "[FLIGHT_RECORDER] Scan finished. Found {} device(s). Broadcasting scan_finished immediately.",
                    result.devices.len()
                );

                // Broadcast scan_finished FIRST — the frontend releases its isScanning
                // lock here. Never block this broadcast on a DB write; persistence
                // failures must not freeze the UI.
                let mut devices_with_suggestions = result.devices.clone();
                for d in &mut devices_with_suggestions {
                    d.generate_suggested_names();
                }

                lifecycle.finish(json!({
                    "devices": devices_with_suggestions,
                    "deviceCount": result.devices.len(),
                    "scannedHosts": result.scanned_hosts,
                    "averageLatencyMs": result.average_latency_ms,
                }));

                // Persist asynchronously — never block the broadcast above.
                let db_persist = db.clone();
                let devices_persist = result.devices.clone();
                let scan_id_persist = scan_id.clone();
                tokio::spawn(async move {
                    match dev_store::complete_scan_persistence(
                        db_persist,
                        devices_persist,
                        scan_id_persist,
                        network_id,
                    )
                    .await
                    {
                        Ok(new_devices) => {
                            log::info!(
                                "[FLIGHT_RECORDER] DB persistence complete — {} new device(s) recorded.",
                                new_devices.len()
                            );

                            // Broadcast live intruder alerts for the UI
                            let timestamp = crate::storage::now_ms();
                            for (name, vendor, ip, mac) in new_devices {
                                let _ = broadcast_tx.send(json!({
                                    "type": "new-device",
                                    "payload": {
                                        "timestampMs": timestamp,
                                        "mac": mac,
                                        "name": name,
                                        "ip": ip,
                                    }
                                }));

                                // Send professional structured alerts
                                let vendor_str = if vendor.is_empty() { "Unknown" } else { vendor.as_str() };
                                let body = format!(
                                    "MAC: {}\nIP: {}\nVendor: {}\nName: {}\nDetected At: {}",
                                    mac, ip, vendor_str, name, chrono::Utc::now().to_rfc3339()
                                );
                                
                                // 1. Log the breach into the historical event registry
                                let db_clone = state.db.clone();
                                let mac_clone = mac.clone();
                                tokio::spawn(async move {
                                    crate::storage::history::log_event(
                                        &db_clone, 
                                        "intruder", 
                                        None, 
                                        &format!("Breach detected! Unknown address: {}", mac_clone)
                                    ).await;
                                });

                                let n_dispatcher = state.notifications.clone();
                                let db = state.db.clone();
                                tokio::spawn(async move {
                                    n_dispatcher.broadcast_alert(&db, "Intruder Alert", &body).await;
                                });
                            }
                        }
                        Err(e) => {
                            log::error!(
                                "[FLIGHT_RECORDER] DB persistence failed (scan data unaffected, UI already unblocked): {}",
                                e
                            );
                        }
                    }
                });
            }
            Err(e) => {
                log::error!("[FLIGHT_RECORDER] Scan engine error: {}", e);
                lifecycle.fail(e);
            }
        }
    });

    Ok(Json(json!({"scanId": scan_id_out, "status": "started"})))
}

pub async fn relay_scan_events(
    mut rx: mpsc::UnboundedReceiver<ScanEvent>,
    tx: tokio::sync::broadcast::Sender<serde_json::Value>,
) {
    while let Some(event) = rx.recv().await {
        let msg = match event {
            ScanEvent::DeviceDiscovered(mut payload) => {
                for d in &mut payload.devices {
                    d.generate_suggested_names();
                }
                json!({
                    "event": "device_discovered",
                    "data": payload,
                })
            }
            ScanEvent::Progress(mut payload) => {
                for d in &mut payload.devices {
                    d.generate_suggested_names();
                }
                json!({
                    "event": "scan_progress",
                    "data": payload,
                })
            }
        };
        let _ = tx.send(msg);
    }
}

pub async fn scan_status(State(_state): State<AppState>) -> ApiResult<impl IntoResponse> {
    Ok(Json(json!({
        "isScanning": scanner::is_scan_active(),
    })))
}

pub async fn abort_scan() -> ApiResult<impl IntoResponse> {
    scanner::abort_scan();
    Ok(axum::http::StatusCode::OK)
}
