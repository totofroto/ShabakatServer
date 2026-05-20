use std::time::Duration;
use log::{info, debug};
use crate::AppState;
use crate::scanner::router_api;
use crate::types::RouterBandwidth;
use crate::storage::now_ms;

pub async fn run_bandwidth_monitor(state: AppState) {
    info!("[MONITOR] Starting router bandwidth monitor (5s interval)");
    
    // We need the gateway IP. We can get it from the config or try to infer it.
    // For now, let's assume standard .1 gateway for the subnet we are on.
    
    let mut ticker = tokio::time::interval(Duration::from_secs(5));
    loop {
        ticker.tick().await;
        
        let gateway_ip = if let Some(ip) = crate::scanner::get_best_local_ip() {
            let octets = ip.octets();
            format!("{}.{}.{}.1", octets[0], octets[1], octets[2])
        } else {
            debug!("[MONITOR] Could not determine local IP for gateway inference");
            continue;
        };

        match router_api::get_bandwidth_stats(&gateway_ip).await {
            Some((rx, tx)) => {
                let now = now_ms();
                let stats = RouterBandwidth {
                    rx_bytes: rx,
                    tx_bytes: tx,
                    timestamp: now,
                };
                
                {
                    let mut b = state.bandwidth.lock().unwrap();
                    *b = Some(stats);
                }
                
                debug!("[MONITOR] Bandwidth updated: RX={} TX={}", rx, tx);
            }
            None => {
                // Silently fail or log occasionally if the router doesn't support it
                debug!("[MONITOR] Failed to fetch bandwidth stats from {}", gateway_ip);
            }
        }
    }
}
