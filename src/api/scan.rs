use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc;

use crate::{
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
) -> impl IntoResponse {
    let mode_str = req.mode.as_deref().unwrap_or("silent");
    let mode = scanner::ScanMode::from_str(mode_str);
    let scan_id = format!("manual-{}", storage::now_ms());

    let guard = match scanner::ScanGuard::try_acquire() {
        Some(g) => g,
        None => {
            return (
                StatusCode::CONFLICT,
                Json(json!({"error": "SCAN_IN_PROGRESS"})),
            )
                .into_response()
        }
    };

    let (event_tx, event_rx) = mpsc::unbounded_channel::<ScanEvent>();
    let broadcast_tx = state.broadcast_tx.clone();
    let db = state.db.clone();
    let scan_id_out = scan_id.clone();

    // Relay: scanner events → WebSocket broadcast
    let bcast = broadcast_tx.clone();
    tokio::spawn(relay_scan_events(event_rx, bcast));

    // Scan task
    tokio::spawn(async move {
        let _ = broadcast_tx.send(json!({
            "event": "scan_started",
            "data": { "scanId": scan_id }
        }));

        // Detect current network identity before the scan.
        let network_info = scanner::network_identity::get_current_network_info().await;
        let network_id: Option<i64> = if let Some(ref bssid) = network_info.bssid {
            let now = storage::now_ms();
            let conn_res = db.connect_dedicated();
            match conn_res {
                Ok(conn) => {
                    match net_store::upsert_network(
                        &conn,
                        network_info.ssid.as_deref(),
                        bssid,
                        network_info.gateway.as_deref(),
                        network_info.subnet.as_deref(),
                        now,
                    ).await {
                        Ok(id) => Some(id),
                        Err(e) => {
                            log::warn!("[SCAN] Network upsert failed: {e}");
                            None
                        }
                    }
                }
                Err(e) => {
                    log::warn!("[SCAN] DB connect failed: {e}");
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
                let _ = broadcast_tx.send(json!({
                    "event": "scan_finished",
                    "data": {
                        "scanId": scan_id,
                        "devices": result.devices,
                        "deviceCount": result.devices.len(),
                        "scannedHosts": result.scanned_hosts,
                        "averageLatencyMs": result.average_latency_ms,
                    }
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
                                let payload = crate::notifications::AlertPayload {
                                    title: "Intruder Alert".to_string(),
                                    mac: mac.clone(),
                                    ip: ip.clone(),
                                    vendor: vendor_str.to_string(),
                                    hostname: Some(name.clone()),
                                    timestamp: chrono::Utc::now().to_rfc3339(),
                                };
                                let n_dispatcher = state.notifications.clone();
                                let n_config = state.config.clone();
                                tokio::spawn(async move {
                                    n_dispatcher.broadcast_alert(&n_config, &payload).await;
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
                // Use "scan_failed" — matches the frontend's unlistenScanFailed handler.
                let _ = broadcast_tx.send(json!({
                    "event": "scan_failed",
                    "data": { "scanId": scan_id, "error": e }
                }));
            }
        }
    });

    Json(json!({"scanId": scan_id_out, "status": "started"})).into_response()
}

pub async fn relay_scan_events(
    mut rx: mpsc::UnboundedReceiver<ScanEvent>,
    tx: tokio::sync::broadcast::Sender<serde_json::Value>,
) {
    while let Some(event) = rx.recv().await {
        let msg = match event {
            ScanEvent::DeviceDiscovered(payload) => json!({
                "event": "device_discovered",
                "data": payload,
            }),
            ScanEvent::Progress(payload) => json!({
                "event": "scan_progress",
                "data": payload,
            }),
        };
        let _ = tx.send(msg);
    }
}

pub async fn scan_status(State(_state): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({
        "isScanning": scanner::is_scan_active(),
    }))
}
