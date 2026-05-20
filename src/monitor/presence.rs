use std::time::{Duration, Instant};
use log::{info, warn, debug, error};
use mdns_sd::{ServiceDaemon, ServiceEvent, Receiver as MdnsReceiver};
use std::collections::HashSet;
use crate::storage::AppDb;
use crate::scanner::arp;
use crate::storage::now_ms;
use libsql::params;

const FLUSH_INTERVAL: Duration = Duration::from_secs(30);

pub async fn run_presence_monitor(db: AppDb) {
    info!("[PRESENCE] Starting passive presence monitor...");
    
    let mdns = match ServiceDaemon::new() {
        Ok(d) => d,
        Err(e) => {
            warn!("[PRESENCE] Failed to start mDNS daemon: {e}");
            return;
        }
    };

    let initial_services = [
        "_googlecast._tcp.local.",
        "_hap._tcp.local.",
        "_printer._tcp.local.",
        "_spotify-connect._tcp.local.",
        "_smb._tcp.local.",
        "_airplay._tcp.local.",
        "_raop._tcp.local.",
        "_workstation._tcp.local.",
        "_ssh._tcp.local.",
        "_http._tcp.local.",
    ];

    let mut receivers: Vec<MdnsReceiver<ServiceEvent>> = Vec::new();
    for svc in initial_services {
        if let Ok(rx) = mdns.browse(svc) {
            receivers.push(rx);
        }
    }

    let meta_rx = mdns.browse("_services._dns-sd._udp.local.").ok();
    let mut dynamic_browsed: HashSet<String> = HashSet::new();
    let mut seen_ips = HashSet::new();
    
    let mut last_flush = Instant::now();

    loop {
        // Check for new service types
        if let Some(ref meta) = meta_rx {
            while let Ok(event) = meta.try_recv() {
                if let ServiceEvent::ServiceFound(_type_name, fullname) = event {
                    let svc_type = if fullname.ends_with('.') {
                        fullname.clone()
                    } else {
                        format!("{fullname}.")
                    };
                    if !dynamic_browsed.contains(&svc_type) {
                        dynamic_browsed.insert(svc_type.clone());
                        if let Ok(rx) = mdns.browse(&svc_type) {
                            debug!("[PRESENCE] Now browsing for {}", svc_type);
                            receivers.push(rx);
                        }
                    }
                }
            }
        }

        // Check all service receivers
        for rx in &receivers {
            while let Ok(event) = rx.try_recv() {
                if let ServiceEvent::ServiceResolved(info) = event {
                    for addr in info.get_addresses() {
                        if let std::net::IpAddr::V4(v4) = addr.to_ip_addr() {
                            seen_ips.insert(v4.to_string());
                        }
                    }
                }
            }
        }

        // Flush to DB
        if last_flush.elapsed() >= FLUSH_INTERVAL && !seen_ips.is_empty() {
            let ips: Vec<String> = seen_ips.drain().collect();
            let db_clone = db.clone();
            tokio::spawn(async move {
                if let Err(e) = update_devices_presence(db_clone, ips).await {
                    error!("[PRESENCE] DB update failed: {e}");
                }
            });
            last_flush = Instant::now();
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

async fn update_devices_presence(db: AppDb, ips: Vec<String>) -> Result<(), String> {
    let now = now_ms();
    let conn = db.connect().await?;
    
    conn.execute("BEGIN", ()).await.map_err(|e| e.to_string())?;
    
    for ip in ips {
        // Nudge to ensure it's in ARP table
        arp::nudge_neighbor(&ip);
        
        // Wait a tiny bit for ARP to resolve if it was missing
        // (In a passive monitor this is fine)
        
        if let Some(mac) = arp::lookup_mac(&ip).await {
            let _ = conn.execute(
                "UPDATE devices SET last_seen = ?1, is_online = 1, last_ip = ?2 WHERE mac = ?3",
                params![now, ip.clone(), mac.clone()],
            ).await;
            debug!("[PRESENCE] Device {} ({}) seen via mDNS", mac, ip);
        }
    }
    
    conn.execute("COMMIT", ()).await.map_err(|e| e.to_string())?;
    Ok(())
}
