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
    
    // Spawn the Stale State Sweeper task
    let sweeper_state = state.clone();
    tokio::spawn(async move {
        run_stale_state_sweeper(sweeper_state).await;
    });

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

async fn run_stale_state_sweeper(state: AppState) {
    info!("[MONITOR] Starting Stale State Sweeper loop (60s interval)");
    let mut ticker = interval(Duration::from_secs(60));
    
    loop {
        ticker.tick().await;
        debug!("[MONITOR] Stale State Sweeper executing cleanup check...");
        
        let db = state.db.clone();
        let now_ms = crate::storage::now_ms();
        
        let stale_devices_res = db.execute(move |conn| -> Result<_, String> {
            let mut stmt = conn.prepare(
                "SELECT mac, last_ip, hostname FROM devices WHERE is_online = 1 AND (?1 - last_seen) > 300000"
            ).map_err(|e| e.to_string())?;
            
            let stale_list = stmt.query_map(rusqlite::params![now_ms], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            }).map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect::<Vec<(String, Option<String>, Option<String>)>>();
            
            if !stale_list.is_empty() {
                conn.execute(
                    "UPDATE devices SET is_online = 0 WHERE is_online = 1 AND (?1 - last_seen) > 300000",
                    rusqlite::params![now_ms],
                ).map_err(|e| e.to_string())?;
            }
            
            Ok(stale_list)
        }).await;
        
        match stale_devices_res {
            Ok(stale_list) => {
                for (mac, ip, hostname) in stale_list {
                    let name = hostname.clone().unwrap_or_else(|| ip.clone().unwrap_or_else(|| mac.clone()));
                    info!(
                        "[MONITOR] Device {} (MAC: {}) has gone stale. Transitioning to offline.",
                        name, mac
                    );
                    
                    let _ = state.broadcast_tx.send(serde_json::json!({
                        "type": "latency_update",
                        "payload": {
                            "mac": mac,
                            "ip": ip.unwrap_or_default(),
                            "isOnline": false,
                            "latencyMs": serde_json::Value::Null,
                            "timestamp": now_ms
                        }
                    }));
                }
            }
            Err(e) => {
                error!("[MONITOR] Stale State Sweeper DB query failed: {}", e);
            }
        }
    }
}
