use axum::{extract::State, response::IntoResponse, Json};
use serde_json::json;
use std::time::Instant;
use std::sync::OnceLock;

use crate::{scanner, AppState};

static START_TIME: OnceLock<Instant> = OnceLock::new();

pub fn init_uptime() {
    let _ = START_TIME.get_or_init(Instant::now);
}

pub async fn get_debug_state(State(state): State<AppState>) -> impl IntoResponse {
    let uptime_secs = START_TIME.get().map(|t| t.elapsed().as_secs()).unwrap_or(0);
    
    // Check DB status
    let db_status = match state.db.connect_dedicated() {
        Ok(_) => "connected",
        Err(e) => {
            log::error!("[DEBUG] DB connection check failed: {}", e);
            "error"
        }
    };

    // Scanner state
    let is_scanning = scanner::is_scan_active();

    // Memory footprint (basic)
    let memory_usage = get_memory_usage();

    Json(json!({
        "database": {
            "status": db_status,
        },
        "scanner": {
            "is_active": is_scanning,
        },
        "system": {
            "uptime_secs": uptime_secs,
            "memory_bytes": memory_usage,
        }
    }))
}

fn get_memory_usage() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        if let Ok(statm) = fs::read_to_string("/proc/self/statm") {
            let parts: Vec<&str> = statm.split_whitespace().collect();
            if let Some(rss_pages) = parts.get(1).and_then(|p| p.parse::<u64>().ok()) {
                // Return in bytes (usually 4KB per page on most Linux systems)
                return Some(rss_pages * 4096);
            }
        }
    }
    None
}
