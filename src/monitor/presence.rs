// src/monitor/presence.rs

use std::time::Duration;
use log::{info, error, debug};
use tokio::time::interval;
use crate::AppState;
use crate::storage::devices::DeviceStorage;
use crate::storage::metrics::{MetricStorage, MetricEntry};

/// High-frequency active heartbeat execution engine (Uptime Kuma subsystem)
pub async fn run_presence_monitor(state: AppState) {
    info!("[MONITOR] Starting high-frequency Uptime Kuma heartbeat loop (20s interval)");
    
    // Check devices every 20 seconds for rapid latency graphing updates
    let mut ticker = interval(Duration::from_secs(20));
    let scan_id_prefix = "heartbeat-";

    loop {
        ticker.tick().await;
        debug!("[MONITOR] Heartbeat worker executing active latency diagnostics...");

        // 1. Recover all known hardware endpoints from our database registry
        let devices_res = DeviceStorage::get_all_devices(&state.db).await;
        
        match devices_res {
            Ok(devices) => {
                let timestamp = chrono::Utc::now().timestamp_millis();
                let unique_scan_id = format!("{}{}", scan_id_prefix, timestamp);
                let mut heartbeat_batch = Vec::new();

                for device in devices {
                    // We can only ping targets that possess a tracked local network IP address
                    if let Some(ip) = device.last_ip {
                        let device_id = device.id;
                        
                        // 2. Execute a lean raw ping diagnostic using our engine's tools subsystem
                        // We use the system's existing ping mechanism to leverage CAP_NET_RAW capabilities
                        let ping_res = crate::tools::ping::ping_device(&ip, 1, 500).await;
                        
                        let (is_online, latency_ms) = match ping_res {
                            Ok(latency) => (true, Some(latency)),
                            Err(_) => (false, None),
                        };

                        // 3. Assemble historical metric context block
                        heartbeat_batch.push(MetricEntry {
                            id: None,
                            scan_id: unique_scan_id.clone(),
                            scanned_at: timestamp,
                            device_id,
                            ip: ip.clone(),
                            is_online,
                            latency_ms,
                            open_ports: None,
                        });

                        // 4. Stream real-time diagnostic telemetry through our active WebSocket hub
                        let _ = state.broadcast_tx.send(serde_json::json!({
                            "type": "latency_update",
                            "payload": {
                                "mac": device.mac,
                                "ip": ip,
                                "isOnline": is_online,
                                "latencyMs": latency_ms,
                                "timestamp": timestamp
                            }
                        }));
                    }
                }

                // 5. Commit historical records directly to SQLite using Rule 2 (Strict Transaction Batching)
                if !heartbeat_batch.is_empty() {
                    if let Err(e) = MetricStorage::log_heartbeat_batch(&state.db, heartbeat_batch).await {
                        error!("[MONITOR] Failed to batch-persist active heartbeat metrics: {}", e);
                    }
                }
            }
            Err(e) => {
                error!("[MONITOR] Failed to extract device registry context for heartbeats: {}", e);
            }
        }
    }
}
