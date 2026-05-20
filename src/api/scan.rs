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
                let devices = result.devices.clone();
                log::info!("[SCAN] Scan finished. Found {} devices.", devices.len());

                if let Err(e) = dev_store::complete_scan_persistence(db.clone(), devices, scan_id.clone(), network_id).await {
                    log::error!("[SCAN] Persistence failed: {}", e);
                }

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
            }
            Err(e) => {
                let _ = broadcast_tx.send(json!({
                    "event": "scan_error",
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
