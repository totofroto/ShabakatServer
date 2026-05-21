use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Instant;
use std::sync::OnceLock;

use crate::{scanner, AppState};

static START_TIME: OnceLock<Instant> = OnceLock::new();

pub fn init_uptime() {
    let _ = START_TIME.get_or_init(Instant::now);
}

pub async fn get_debug_state(State(state): State<AppState>) -> impl IntoResponse {
    let _uptime_secs = START_TIME.get().map(|t| t.elapsed().as_secs()).unwrap_or(0);
    
    // Check DB status
    let db_connected = state.db.connect_dedicated().is_ok();

    // Scanner state
    let is_scanning = scanner::is_scan_active();

    // File descriptors (Linux only)
    let open_fds = get_open_fds();

    Json(json!({
        "is_scanner_active": is_scanning,
        "db_connected": db_connected,
        "open_fds": open_fds,
    }))
}

fn get_open_fds() -> usize {
    #[cfg(target_os = "linux")]
    {
        if let Ok(entries) = std::fs::read_dir("/proc/self/fd") {
            return entries.count();
        }
    }
    0
}

#[derive(Deserialize)]
pub struct ProbeRequest {
    pub target_ip: String,
    pub probe_type: String, // "ping" | "arp" | "udp_trick"
}

#[derive(Serialize)]
pub struct ProbeResponse {
    pub online: bool,
    pub raw_output: String,
}

pub async fn debug_probe(Json(payload): Json<ProbeRequest>) -> impl IntoResponse {
    let target = payload.target_ip.trim().to_string();
    let probe_type = payload.probe_type.to_lowercase();

    match probe_type.as_str() {
        "ping" => {
            // Use the system ping command for raw output
            match tokio::process::Command::new("ping")
                .args(["-c", "1", "-W", "1", &target])
                .output()
                .await
            {
                Ok(out) => {
                    let online = out.status.success();
                    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                    let raw_output = if stdout.is_empty() { stderr } else { stdout };
                    Json(ProbeResponse { online, raw_output })
                }
                Err(e) => Json(ProbeResponse { 
                    online: false, 
                    raw_output: format!("Failed to execute ping: {}", e) 
                }),
            }
        }
        "arp" => {
            let mac = scanner::arp::lookup_mac(&target).await;
            let online = mac.is_some();
            let raw_output = if let Some(m) = mac {
                format!("ARP Table match found:\nIP: {}\nMAC: {}", target, m)
            } else {
                format!("No entry found in ARP table for {}", target)
            };
            Json(ProbeResponse { online, raw_output })
        }
        "udp_trick" => {
            scanner::arp::nudge_neighbor(&target);
            // Wait a tiny bit for the OS to update the ARP table
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let mac = scanner::arp::lookup_mac(&target).await;
            let online = mac.is_some();
            let raw_output = if let Some(m) = mac {
                format!("UDP Trick Success - Kernel resolved MAC:\nIP: {}\nMAC: {}", target, m)
            } else {
                format!("UDP Trick sent packet to {}:9, but no ARP response received.", target)
            };
            Json(ProbeResponse { online, raw_output })
        }
        _ => Json(ProbeResponse {
            online: false,
            raw_output: format!("Unknown probe type: {}", probe_type),
        }),
    }
}
