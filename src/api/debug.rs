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
