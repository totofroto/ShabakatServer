use axum::{
    extract::State,
    response::{
        sse::{Event, Sse},
        IntoResponse,
    },
    Json,
};
use futures::stream::{self, Stream};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{convert::Infallible, sync::OnceLock, time::Instant};

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

    let probe_future = async move {
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
                        raw_output: format!("Failed to execute ping: {}", e),
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
                    format!(
                        "UDP Trick Success - Kernel resolved MAC:\nIP: {}\nMAC: {}",
                        target, m
                    )
                } else {
                    format!(
                        "UDP Trick sent packet to {}:9, but no ARP response received.",
                        target
                    )
                };
                Json(ProbeResponse { online, raw_output })
            }
            _ => Json(ProbeResponse {
                online: false,
                raw_output: format!("Unknown probe type: {}", probe_type),
            }),
        }
    };

    match tokio::time::timeout(std::time::Duration::from_secs(5), probe_future).await {
        Ok(response) => response,
        Err(_) => Json(ProbeResponse {
            online: false,
            raw_output: "Error: Diagnostic probe timed out after 5 seconds. Connection dropped."
                .to_string(),
        }),
    }
}

pub async fn stream_logs(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.log_tx.subscribe();

    let stream = stream::unfold(rx, |mut rx| async move {
        match rx.recv().await {
            Ok(msg) => Some((Ok(Event::default().data(msg)), rx)),
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                Some((Ok(Event::default().data("[SYSTEM] Log stream lagged, some messages lost")), rx))
            }
            Err(_) => None,
        }
    });

    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}

#[derive(Deserialize)]
pub struct TerminalRequest {
    pub command: String,
}

#[derive(Serialize)]
pub struct TerminalResponse {
    pub output: String,
}

pub async fn run_terminal_command(Json(payload): Json<TerminalRequest>) -> impl IntoResponse {
    let command_parts: Vec<&str> = payload.command.split_whitespace().collect();
    if command_parts.is_empty() {
        return Json(TerminalResponse {
            output: "Error: No command provided".to_string(),
        });
    }

    let cmd = command_parts[0];
    let args = &command_parts[1..];

    let whitelist = ["ls", "cat", "arp", "ping", "df", "ps"];
    if !whitelist.contains(&cmd) {
        return Json(TerminalResponse {
            output: format!(
                "Error: Command '{}' is not whitelisted. Allowed: {:?}",
                cmd, whitelist
            ),
        });
    }

    match tokio::process::Command::new(cmd)
        .args(args)
        .output()
        .await
    {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let output = if out.status.success() {
                stdout.to_string()
            } else {
                format!(
                    "Error (exit code {}):\n{}{}",
                    out.status.code().unwrap_or(-1),
                    stdout,
                    stderr
                )
            };
            Json(TerminalResponse { output })
        }
        Err(e) => Json(TerminalResponse {
            output: format!("Failed to execute command: {}", e),
        }),
    }
}
