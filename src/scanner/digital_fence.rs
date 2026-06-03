// src/scanner/digital_fence.rs

use crate::storage::AppDb;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::UdpSocket;
use tokio::sync::broadcast;
use log::{info, error};
use ipnet::Ipv4Net;
use rusqlite::OptionalExtension;

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
        
        let std_socket = std::net::UdpSocket::bind(addr)?;
        std_socket.set_nonblocking(true)?;
        let socket = UdpSocket::from_std(std_socket)?;
        
        let mut buf = [0u8; 1024];
        info!("[FLIGHT_RECORDER] Digital Fence tracking ambient {} chatter on port {}.", protocol, port);

        loop {
            let (bytes_read, peer_addr) = match socket.recv_from(&mut buf).await {
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
                    return Some(super::arp::normalize_mac(mac));
                }
            }
        }
        None
    }

    async fn process_ambient_match(event: AmbientEvent, db: &AppDb, broadcast_tx: &broadcast::Sender<serde_json::Value>) {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let mac_address = event.mac_address.clone();
        let ip_address = event.ip_address.clone();
        let _protocol = event.protocol;


        let res = db.execute(move |conn| {
            // Check if this physical layer signature already exists in your table
            let device_info: Option<(bool, bool)> = {
                let mut stmt = conn.prepare("SELECT 1, is_ignored FROM devices WHERE mac = ?1").map_err(|e| e.to_string())?;
                stmt.query_row(rusqlite::params![mac_address], |row| Ok((true, row.get::<_, i64>(1)? != 0))).optional().map_err(|e| e.to_string())?
            };

            let (device_exists, is_ignored) = match device_info {
                Some((exists, ignored)) => (exists, ignored),
                None => (false, false),
            };

            // Atomic Database Sync: Update presence footprints silently. 
            // We use INSERT ON CONFLICT to ensure even unrecognized devices are registered.
            let affected_rows = conn.execute(
                "INSERT INTO devices (mac, first_seen, last_seen, last_ip, likely_type) 
                 VALUES (?1, ?2, ?2, ?3, 'Digital Fence Discovery')
                 ON CONFLICT(mac) DO UPDATE SET last_seen = ?2, last_ip = ?3",
                rusqlite::params![mac_address, now_ms, ip_address],
            ).map_err(|e| e.to_string())?;

            Ok::<_, String>((device_exists, is_ignored, affected_rows))
        }).await;

        if let Ok((device_exists, is_ignored, affected_rows)) = res {
            if affected_rows > 0 {
                // 🌟 Hub Execution Vector: Target is an intruder/unrecognized newcomer on the fence loop!
                if !device_exists && !is_ignored {
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
