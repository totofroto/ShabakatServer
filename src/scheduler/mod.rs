use std::time::Duration;

use log::info;
use serde_json::json;
use tokio::sync::mpsc;

use crate::{
    scanner,
    storage::{self, devices as dev_store, networks as net_store},
    types::ScanEvent,
    AppState,
};

pub async fn run(state: AppState) {
    let interval_secs = state.config.scan_interval_secs;
    if interval_secs == 0 {
        info!("[SCHEDULER] Scan interval is 0 — auto-scan disabled");
        return;
    }

    let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
    ticker.tick().await; // skip the immediate first tick

    let mut counter = 0u64;
    loop {
        ticker.tick().await;
        counter += 1;
        let scan_id = format!("scheduled-{counter}");
        info!("[SCHEDULER] Starting scheduled scan {scan_id}");

        let guard = match scanner::ScanGuard::try_acquire() {
            Some(g) => g,
            None => {
                info!("[SCHEDULER] Skipping — scan already in progress");
                continue;
            }
        };

        // Detect current network identity before the scan.
        let network_info = scanner::network_identity::get_current_network_info().await;

        // Smart Gateway Resolution
        if let Some(ref gw_ip) = network_info.gateway {
            let db_clone = state.db.clone();
            let gw_ip_clone = gw_ip.clone();
            tokio::spawn(async move {
                let _ = scanner::resolver::resolve_gateway_name(db_clone, &gw_ip_clone).await;
            });
        }

        let network_id: Option<i64> = if let Some(ref bssid) = network_info.bssid {
            let now = storage::now_ms();
            let conn_res = state.db.connect_dedicated();
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
                        Ok(id) => {
                            info!(
                                "[FLIGHT_RECORDER] Network identity: bssid={bssid} subnet={} id={id}",
                                network_info.subnet.as_deref().unwrap_or("?")
                            );
                            Some(id)
                        }
                        Err(e) => {
                            log::warn!("[SCHEDULER] Network upsert failed: {e}");
                            None
                        }
                    }
                }
                Err(e) => {
                    log::warn!("[SCHEDULER] DB connect failed: {e}");
                    None
                }
            }
        } else {
            log::warn!("[SCHEDULER] Could not determine network identity (no BSSID/gateway MAC)");
            None
        };

        let (event_tx, event_rx) = mpsc::unbounded_channel::<ScanEvent>();
        let bcast = state.broadcast_tx.clone();
        let db = state.db.clone();

        tokio::spawn(super::api::scan::relay_scan_events(event_rx, bcast.clone()));

        let _ = bcast.send(json!({
            "event": "scan_started",
            "data": { "scanId": scan_id }
        }));

        match scanner::scan_local_network_pre_guarded(
            Some(db.clone()),
            Some(state.devices.clone()),
            guard,
            Some(event_tx),
            scanner::ScanMode::Silent,
            scan_id.clone(),
        )
        .await
        {
            Ok(result) => {
                let devices = result.devices.clone();

                match dev_store::complete_scan_persistence(db.clone(), devices.clone(), scan_id.clone(), network_id).await {
                    Ok(new_devices) => {
                        // Broadcast live intruder alerts for the UI
                        let timestamp = storage::now_ms();
                        for (name, _vendor, ip, mac) in &new_devices {
                            let _ = bcast.send(json!({
                                "type": "new-device",
                                "payload": {
                                    "timestampMs": timestamp,
                                    "mac": mac,
                                    "name": name,
                                    "ip": ip,
                                }
                            }));
                        }

                        // Send alerts for newly discovered devices.
                        for (name, vendor, ip, mac) in &new_devices {
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

                            state.notifications.broadcast_alert(&state.db, "Intruder Alert", &body).await;
                        }
                    }
                    Err(e) => log::warn!("[SCHEDULER] Persistence failed: {e}"),
                }

                let _ = bcast.send(json!({
                    "event": "scan_finished",
                    "data": {
                        "scanId": scan_id,
                        "deviceCount": result.devices.len(),
                        "scannedHosts": result.scanned_hosts,
                        "averageLatencyMs": result.average_latency_ms,
                    }
                }));

                info!(
                    "[SCHEDULER] Scan {scan_id} finished: {} devices",
                    result.devices.len()
                );
            }
            Err(e) => {
                log::warn!("[SCHEDULER] Scan {scan_id} failed: {e}");
                let _ = bcast.send(json!({
                    "event": "scan_error",
                    "data": { "scanId": scan_id, "error": e }
                }));
            }
        }
    }
}
