// src/scanner/digital_fence.rs

use crate::storage::AppDb;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::UdpSocket;
use tokio::sync::broadcast;
use log::{info, error};
use ipnet::Ipv4Net;

#[derive(Debug, Clone)]
pub struct AmbientEvent {
    pub mac_address: String,
    pub ip_address: String,
    pub protocol: &'static str,
}

pub struct DigitalFence;

impl DigitalFence {
    /// Spawns the ambient broadcast capture engines for mDNS and SSDP
    pub fn start(db: AppDb, broadcast_tx: broadcast::Sender<serde_json::Value>) {
        let db_clone = db.clone();
        let tx_clone = broadcast_tx.clone();
        
        tokio::spawn(async move {
            let cidr = match crate::scanner::network::local_ipv4_network().await {
                Ok(n) => n.cidr,
                Err(_) => {
                    // Fallback to UDP trick if primary detection fails
                    crate::scanner::network::fallback_ipv4_network_via_udp().await
                        .map(|n| n.cidr)
                        .unwrap_or_else(|_| "192.168.1.0/24".parse().unwrap())
                }
            };

            info!("[FLIGHT_RECORDER] Digital Fence initialized with dynamic subnet: {}", cidr);

            let db_mdns = db_clone.clone();
            let tx_mdns = tx_clone.clone();
            let cidr_mdns = cidr;
            // Listener 1: mDNS Ambient Sentry (Port 5353)
            tokio::spawn(async move {
                if let Err(e) = Self::listen_port(5353, "mDNS", db_mdns, tx_mdns, cidr_mdns).await {
                    error!("[FLIGHT_RECORDER] Digital Fence mDNS listener failed: {}", e);
                }
            });

            let db_ssdp = db_clone.clone();
            let tx_ssdp = tx_clone.clone();
            let cidr_ssdp = cidr;
            // Listener 2: SSDP Ambient Sentry (Port 1900)
            tokio::spawn(async move {
                if let Err(e) = Self::listen_port(1900, "SSDP", db_ssdp, tx_ssdp, cidr_ssdp).await {
                    error!("[FLIGHT_RECORDER] Digital Fence SSDP listener failed: {}", e);
                }
            });
        });
    }

    async fn listen_port(
        port: u16, 
        protocol: &'static str, 
        db: AppDb, 
        broadcast_tx: broadcast::Sender<serde_json::Value>,
        cidr: Ipv4Net
    ) -> std::io::Result<()> {
        // Bind to INADDR_ANY over Host Network mode to capture global multicast frames
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port);
        
        // Use socket2 for more control like SO_REUSEADDR and SO_REUSEPORT
        let socket = socket2::Socket::new(
            socket2::Domain::IPV4,
            socket2::Type::DGRAM,
            Some(socket2::Protocol::UDP),
        )?;

        socket.set_reuse_address(true)?;
        #[cfg(all(unix, not(target_os = "solaris"), not(target_os = "illumos")))]
        socket.set_reuse_port(true)?;
        
        socket.bind(&addr.into())?;
        
        let socket = UdpSocket::from_std(socket.into())?;
        
        let mut buffer = [0u8; 1024];
        info!("[FLIGHT_RECORDER] Digital Fence tracking ambient {} chatter on port {}.", protocol, port);

        loop {
            let (bytes_read, peer_addr) = match socket.recv_from(&mut buffer).await {
                Ok(res) => res,
                Err(_) => continue,
            };

            if bytes_read == 0 { continue; }

            if let SocketAddr::V4(v4_addr) = peer_addr {
                let src_ip = v4_addr.ip();

                // Constraint Enforcement: Isolate processing to local subnet boundary
                if cidr.contains(src_ip) {
                    let src_ip_str = src_ip.to_string();
                    // Resolve physical Layer 2 MAC signature securely via kernel table parsing
                    if let Some(mac) = Self::resolve_mac_from_arp(&src_ip_str) {
                        let event = AmbientEvent {
                            mac_address: mac.clone(),
                            ip_address: src_ip_str,
                            protocol,
                        };

                        Self::process_ambient_match(event, &db, &broadcast_tx).await;
                    }
                }
            }
        }
    }

    /// Reads /proc/net/arp directly to find the MAC address matching a given IP address
    fn resolve_mac_from_arp(target_ip: &str) -> Option<String> {
        let file = File::open("/proc/net/arp").ok()?;
        let reader = BufReader::new(file);

        for line in reader.lines().skip(1).flatten() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                let ip = parts[0];
                let mac = parts[3];

                if ip == target_ip && mac != "00:00:00:00:00:00" {
                    return Some(mac.to_string().to_lowercase());
                }
            }
        }
        None
    }

    async fn process_ambient_match(event: AmbientEvent, db: &AppDb, broadcast_tx: &broadcast::Sender<serde_json::Value>) {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let conn = db.conn.lock().await;

        // Check if this physical layer signature already exists in your table
        let device_exists: bool = {
            let mut stmt = match conn.prepare("SELECT COUNT(*) FROM devices WHERE mac = ?1").await {
                Ok(s) => s,
                Err(_) => return,
            };
            let mut rows = match stmt.query(libsql::params![event.mac_address.clone()]).await {
                Ok(r) => r,
                Err(_) => return,
            };
            match rows.next().await {
                Ok(Some(row)) => row.get::<i64>(0).unwrap_or(0) > 0,
                _ => false,
            }
        };

        // Atomic Database Sync: Update presence footprints silently. 
        // We use INSERT ON CONFLICT to ensure even unrecognized devices are registered.
        let update_res = conn.execute(
            "INSERT INTO devices (mac, first_seen, last_seen, last_ip, likely_type) 
             VALUES (?1, ?2, ?2, ?3, 'Digital Fence Discovery')
             ON CONFLICT(mac) DO UPDATE SET last_seen = ?2, last_ip = ?3",
            libsql::params![event.mac_address.clone(), now_ms, event.ip_address.clone()],
        ).await;

        // Release DB lock early to prevent out-of-band blocking threats
        drop(conn);

        if let Ok(affected_rows) = update_res {
            if affected_rows > 0 {
                // 🌟 Hub Execution Vector: Target is an intruder/unrecognized newcomer on the fence loop!
                if !device_exists {
                    log::warn!("[FLIGHT_RECORDER] Perimeter Breach! Processing multi-channel intruder alerts for MAC: {}", event.mac_address);

                    // 1. Log the breach into the historical event registry
                    let db_clone = db.clone();
                    let mac_clone = event.mac_address.clone();
                    tokio::spawn(async move {
                        crate::storage::history::log_event(
                            &db_clone, 
                            "intruder", 
                            None, 
                            &format!("Breach detected! Unknown address: {}", mac_clone)
                        ).await;
                    });
                    
                    let title = "🚨 [SHABAKAT PERIMETER ALARM]";
                    let body = format!(
                        "An unrecognized device has breached your passive surveillance grid!\n\n\
                         MAC Frame: {}\n\
                         IP Layer: {}\n\
                         Vector: {}\n\
                         Time Trace: {}",
                        event.mac_address, event.ip_address, event.protocol, chrono::Utc::now().to_rfc3339()
                    );

                    // Dispatch out-of-band across all enabled channels seamlessly
                    let db_clone = db.clone();
                    tokio::spawn(async move {
                        crate::notifications::broadcast_alert(&db_clone, title, &body).await;
                    });
                }

                info!(
                    "[FLIGHT_RECORDER] Passive Presence Verified: {} [IP: {}] via {}",
                    event.mac_address, event.ip_address, event.protocol
                );

                // Instantly alert the Star-Map UI over our shared WebSocket channel
                let _ = broadcast_tx.send(serde_json::json!({
                    "type": "latency_update",
                    "payload": {
                        "mac": event.mac_address,
                        "ip": event.ip_address,
                        "latencyMs": 0.1, // Near-zero artificial latency value to signify instant local wakefulness
                        "via": event.protocol,
                        "timestamp": now_ms
                    }
                }));
            }
        }
    }
}
