use std::time::Duration;
use log::{info, debug, error};
use crate::AppState;
use crate::scanner::sys_metrics::SystemCollector;

pub async fn run_sys_metrics_monitor(state: AppState) {
    info!("[MONITOR] Starting system metrics monitor (2s interval)");
    
    let mut collector = SystemCollector::new();
    let mut ticker = tokio::time::interval(Duration::from_secs(2));

    loop {
        ticker.tick().await;
        
        // Use spawn_blocking for file I/O as per recommendation in sys_metrics.rs
        let mut collector_clone = collector;
        let res = tokio::task::spawn_blocking(move || {
            let stats = collector_clone.collect_telemetry();
            (stats, collector_clone)
        }).await;

        match res {
            Ok((Ok(telemetry), updated_collector)) => {
                collector = updated_collector;
                
                {
                    let mut t = state.system_telemetry.lock().unwrap();
                    *t = Some(telemetry.clone());
                }

                // Optional: broadcast via websocket if needed
                let _ = state.broadcast_tx.send(serde_json::json!({
                    "type": "system_telemetry",
                    "payload": telemetry
                }));
            }
            Ok((Err(e), updated_collector)) => {
                collector = updated_collector;
                // Only log error if not on Linux (where /proc/net/dev might be missing)
                #[cfg(target_os = "linux")]
                error!("[MONITOR] Failed to collect system metrics: {}", e);
                #[cfg(not(target_os = "linux"))]
                debug!("[MONITOR] System metrics not available on this OS: {}", e);
            }
            Err(e) => {
                error!("[MONITOR] Blocking task join error: {}", e);
                break;
            }
        }
    }
}
