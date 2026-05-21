use std::time::Duration;
use log::{info, debug};
use crate::storage::AppDb;
use crate::scanner::arp;
use crate::storage::now_ms;
use libsql::params;

/// Passive Presence Monitor
/// 
/// This monitor purely parses the system neighbor table (ARP cache) to update 
/// device statuses. It never generates new network traffic or runs sweeps.
pub async fn run_presence_monitor(db: AppDb) {
    info!("[PRESENCE] Starting passive ARP presence monitor...");
    
    let mut ticker = tokio::time::interval(Duration::from_secs(30));

    loop {
        ticker.tick().await;
        debug!("[PRESENCE] Polling system neighbor table...");
        
        // Purely passive: read the system neighbor table
        #[cfg(target_os = "linux")]
        let neighbors = arp::parse_proc_arp();
        
        #[cfg(target_os = "macos")]
        let neighbors = arp::dump_arp_table_macos().await;
        
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        let neighbors: Vec<(std::net::Ipv4Addr, String)> = Vec::new();

        if neighbors.is_empty() {
            debug!("[PRESENCE] No neighbors found in system table");
            continue;
        }

        let db_clone = db.clone();
        tokio::spawn(async move {
            if let Err(e) = update_devices_presence(db_clone, neighbors).await {
                debug!("[PRESENCE] Passive update failed: {e}");
            }
        });
    }
}

async fn update_devices_presence(db: AppDb, neighbors: Vec<(std::net::Ipv4Addr, String)>) -> Result<(), String> {
    let now = now_ms();
    
    // Perform DB updates in a focused transaction
    let conn = db.connect().await?;
    conn.execute("BEGIN", ()).await.map_err(|e| e.to_string())?;
    
    for (ip, mac) in neighbors {
        let ip_str = ip.to_string();
        debug!("[PRESENCE] Device {} ({}) confirmed via ARP cache", mac, ip_str);
        let _ = conn.execute(
            "UPDATE devices SET last_seen = ?1, is_online = 1, last_ip = ?2 WHERE mac = ?3",
            params![now, ip_str, mac],
        ).await;
    }
    
    conn.execute("COMMIT", ()).await.map_err(|e| e.to_string())?;
    Ok(())
}
