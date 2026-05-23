// src/monitor/compactor.rs

use std::time::Duration;
use log::{info, error};
use tokio::time::interval;
use crate::AppState;
use crate::storage::metrics::MetricStorage;

pub async fn run_metrics_compactor(state: AppState) {
    info!("[MONITOR] Starting Prometheus-style metrics compactor worker thread (4-hour cycle)");
    
    // Wake up every 4 hours to see if there is data ready for downsampling
    let mut ticker = interval(Duration::from_secs(4 * 60 * 60));
    let retention_days = 30; // Keep 30 days of long-term history trend metrics

    loop {
        ticker.tick().await;
        
        if let Err(e) = MetricStorage::run_leveled_compaction(&state.db, retention_days).await {
            error!("[MONITOR] Critical metrics retention compaction process failure: {}", e);
        }
    }
}
