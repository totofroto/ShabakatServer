// src/scanner/digital_fence.rs
//
// PATCHES APPLIED:
//   Fix C — join_multicast_v4(224.0.0.251 / 239.255.255.250) so the kernel
//            delivers real multicast frames to this socket.
//   Fix D — resolve_mac_from_arp dispatched to spawn_blocking so synchronous
//            /proc/net/arp I/O never blocks the Tokio reactor.
//   Fix E — Self-IP suppression + operator-supplied + hardcoded NAS MAC
//            exclusion list prevents false Perimeter Breach alarms.
//   Fix B — Hardcoded "192.168.1.0/24" fallback replaced by env-var lookup
//            then UDP-trick, so the fence initialises on the correct subnet
//            even if local_ipv4_network() fails.

use crate::storage::AppDb;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::UdpSocket;
use tokio::sync::broadcast;
use log::{info, warn, error, debug};
use ipnet::Ipv4Net;
use rusqlite::OptionalExtension;

// ── NAS hardware identity ─────────────────────────────────────────────────────
/// Confirmed MAC address of the Asustor NAS host (J4125).
/// This is the bridge interface MAC that the ADM Avahi daemon broadcasts from.
/// Traffic originating from this MAC must never trigger a Perimeter Breach alarm.
const NAS_HOST_MAC: &str = "50:F1:4A:DE:23:D3";

// ── Multicast group addresses ─────────────────────────────────────────────────
const MDNS_MULTICAST_ADDR:  Ipv4Addr = Ipv4Addr::new(224, 0,   0,   251);
const SSDP_MULTICAST_ADDR:  Ipv4Addr = Ipv4Addr::new(239, 255, 255, 250);

#[derive(Debug, Clone)]
pub struct AmbientEvent {
    pub mac_address: String,
    pub ip_address:  String,
    pub protocol:    &'static str,
}

pub struct DigitalFence;

impl DigitalFence {
    /// Spawns the ambient broadcast capture engines for mDNS (5353) and SSDP (1900).
    pub fn start(db: AppDb, broadcast_tx: broadcast::Sender<serde_json::Value>) {
        let db_clone = db.clone();
        let tx_clone = broadcast_tx.clone();

        tokio::spawn(async move {
            // ── Fix B: subnet detection with env-var fallback ─────────────────
            // Priority: local_ipv4_network() → UDP trick → SHABAKAT_NETWORK_SUBNET
            // env var → last-resort /24 on the correct NAS subnet.
            let cidr = match crate::scanner::network::local_ipv4_network().await {
                Ok(n) => {
                    info!("[DIGITAL_FENCE] Subnet resolved via primary interface detection: {}", n.cidr);
                    n.cidr
                }
                Err(primary_err) => {
                    warn!("[DIGITAL_FENCE] Primary interface detection failed ({}), trying UDP trick...", primary_err);
                    match crate::scanner::network::fallback_ipv4_network_via_udp().await {
                        Ok(n) => {
                            info!("[DIGITAL_FENCE] Subnet resolved via UDP trick: {}", n.cidr);
                            n.cidr
                        }
                        Err(udp_err) => {
                            warn!("[DIGITAL_FENCE] UDP trick also failed ({}), checking env var...", udp_err);
                            // Fix B: read operator-configured subnet instead of hardcoded 192.168.1.0/24
                            let fallback: Ipv4Net = std::env::var("SHABAKAT_NETWORK_SUBNET")
                                .ok()
                                .and_then(|s| s.parse().ok())
                                .unwrap_or_else(|| {
                                    warn!("[DIGITAL_FENCE] SHABAKAT_NETWORK_SUBNET not set; \
                                           defaulting to 192.168.254.0/24 (NAS subnet)");
                                    "192.168.254.0/24".parse().unwrap()
                                });
                            info!("[DIGITAL_FENCE] Subnet resolved via env fallback: {}", fallback);
                            fallback
                        }
                    }
                }
            };

            info!("[DIGITAL_FENCE] Fence active — monitoring subnet {}", cidr);

            // ── Build the suppression set ─────────────────────────────────────
            // Collects all MAC addresses that must never trigger a breach alarm:
            //   1. The confirmed NAS host MAC (Fix E — hardcoded, operator-verified)
            //   2. Any additional MACs from SHABAKAT_TRUSTED_MACS env var
            let suppressed_macs = build_suppressed_mac_set();
            info!(
                "[DIGITAL_FENCE] Breach suppression active for {} MAC(s): {:?}",
                suppressed_macs.len(),
                suppressed_macs
            );

            // ── Listener 1: mDNS Ambient Sentry (Port 5353) ──────────────────
            let db_mdns   = db_clone.clone();
            let tx_mdns   = tx_clone.clone();
            let macs_mdns = suppressed_macs.clone();
            let cidr_mdns = cidr;
            tokio::spawn(async move {
                if let Err(e) = Self::listen_port(5353, "mDNS", db_mdns, tx_mdns, cidr_mdns, macs_mdns).await {
                    error!("[DIGITAL_FENCE] mDNS listener failed: {}", e);
                }
            });

            // ── Listener 2: SSDP Ambient Sentry (Port 1900) ──────────────────
            let db_ssdp   = db_clone.clone();
            let tx_ssdp   = tx_clone.clone();
            let macs_ssdp = suppressed_macs.clone();
            let cidr_ssdp = cidr;
            tokio::spawn(async move {
                if let Err(e) = Self::listen_port(1900, "SSDP", db_ssdp, tx_ssdp, cidr_ssdp, macs_ssdp).await {
                    error!("[DIGITAL_FENCE] SSDP listener failed: {}", e);
                }
            });
        });
    }

    /// Core UDP listener loop.
    ///
    /// # Fix C — Multicast group membership
    /// After binding the socket we call `join_multicast_v4` so the kernel's
    /// IGMP layer adds the NIC to the multicast group and begins delivering
    /// multicast-addressed frames.  Without this join, binding 0.0.0.0:5353
    /// only receives unicast packets — the bulk of mDNS/SSDP traffic is missed.
    ///
    /// `set_multicast_loop_v4(false)` ensures our own mDNS advertisements
    /// (from the `mdns-sd` ServiceDaemon) are not looped back to this socket
    /// and do not trigger self-alarm processing.
    ///
    /// # Fix D — Non-blocking /proc/net/arp
    /// `resolve_mac_from_arp` is a synchronous filesystem read.  It is now
    /// dispatched to `tokio::task::spawn_blocking` so it never occupies a
    /// Tokio reactor thread.
    async fn listen_port(
        port: u16,
        protocol: &'static str,
        db: AppDb,
        broadcast_tx: broadcast::Sender<serde_json::Value>,
        cidr: Ipv4Net,
        suppressed_macs: Vec<String>,
    ) -> std::io::Result<()> {
        // ── Socket construction ───────────────────────────────────────────────
        let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port);

        let sock = socket2::Socket::new(
            socket2::Domain::IPV4,
            socket2::Type::DGRAM,
            Some(socket2::Protocol::UDP),
        )?;
        sock.set_reuse_address(true)?;
        #[cfg(unix)]
        sock.set_reuse_port(true)?;
        sock.bind(&bind_addr.into())?;

        // ── Fix C: Join multicast group ───────────────────────────────────────
        let multicast_addr: Option<Ipv4Addr> = match port {
            5353 => Some(MDNS_MULTICAST_ADDR),
            1900 => Some(SSDP_MULTICAST_ADDR),
            _    => None,
        };
        if let Some(mc_addr) = multicast_addr {
            sock.join_multicast_v4(&mc_addr, &Ipv4Addr::UNSPECIFIED)?;
            // Suppress our own mDNS service advertisements from re-entering the fence.
            sock.set_multicast_loop_v4(false)?;
            info!(
                "[DIGITAL_FENCE] {} listener joined multicast group {} on port {} (loop-back suppressed)",
                protocol, mc_addr, port
            );
        }

        // Promote to Tokio async socket
        let std_socket: std::net::UdpSocket = sock.into();
        std_socket.set_nonblocking(true)?;
        let socket = UdpSocket::from_std(std_socket)?;

        let mut buf = [0u8; 4096]; // mDNS packets can be up to ~9000 bytes; 4 KB covers 99%
        info!(
            "[DIGITAL_FENCE] {} sentry active on port {} — monitoring subnet {}",
            protocol, port, cidr
        );

        loop {
            let (bytes_read, peer_addr) = match socket.recv_from(&mut buf).await {
                Ok(res)  => res,
                Err(e)   => {
                    debug!("[DIGITAL_FENCE] recv_from error on port {}: {}", port, e);
                    continue;
                }
            };

            if bytes_read == 0 { continue; }

            let SocketAddr::V4(v4_addr) = peer_addr else { continue; };
            let src_ip = v4_addr.ip();

            // Gate: only process packets sourced within our LAN subnet.
            if !cidr.contains(src_ip) { continue; }

            // ── Fix D: Dispatch synchronous /proc/net/arp read off-reactor ────
            let src_ip_str = src_ip.to_string();
            let db_clone = db.clone();
            let tx_clone = broadcast_tx.clone();
            let macs_clone = suppressed_macs.clone();

            tokio::spawn(async move {
                let ip_for_task = src_ip_str.clone();
                let mac_result = tokio::task::spawn_blocking(move || {
                    Self::resolve_mac_from_arp(&ip_for_task)
                }).await;

                let mac = match mac_result {
                    Ok(Some(m)) => m,
                    Ok(None)    => return, // MAC not in ARP table yet — packet arrived before ARP resolved
                    Err(e)      => {
                        warn!("[DIGITAL_FENCE] spawn_blocking for ARP lookup panicked: {}", e);
                        return;
                    }
                };

                let event = AmbientEvent {
                    mac_address: mac,
                    ip_address:  src_ip_str,
                    protocol,
                };

                Self::process_ambient_match(event, &db_clone, &tx_clone, &macs_clone).await;
            });
        }
    }

    /// Reads `/proc/net/arp` directly to find the MAC address matching a given IP.
    /// This is a **synchronous** function — always call it from within `spawn_blocking`.
    fn resolve_mac_from_arp(target_ip: &str) -> Option<String> {
        let contents = std::fs::read_to_string("/proc/net/arp").ok()?;

        for line in contents.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                let ip  = parts[0];
                let mac = parts[3];
                if ip == target_ip && mac != "00:00:00:00:00:00" {
                    return Some(super::arp::normalize_mac(mac));
                }
            }
        }
        None
    }

    /// Core event processor — runs after a valid MAC has been resolved.
    ///
    /// # Fix E — Self-suppression logic
    /// Before touching the database or firing any alert, we check the incoming
    /// MAC against the `suppressed_macs` set.  This set includes:
    ///   • The NAS host MAC `50:F1:4A:DE:23:D3` (hardcoded — confirmed hardware)
    ///   • Any MACs listed in `SHABAKAT_TRUSTED_MACS` (operator-configurable)
    ///
    /// Suppressed events are logged at DEBUG level so the Operator can still
    /// observe them without generating alarm noise.
    async fn process_ambient_match(
        event: AmbientEvent,
        db: &AppDb,
        broadcast_tx: &broadcast::Sender<serde_json::Value>,
        suppressed_macs: &[String],
    ) {
        let now_ms      = chrono::Utc::now().timestamp_millis();
        let mac_address = super::arp::normalize_mac(&event.mac_address);
        let ip_address  = event.ip_address.clone();

        // ── Fix E: Suppression gate ───────────────────────────────────────────
        // Normalise for comparison (both uppercase, colon-separated).
        if suppressed_macs.contains(&mac_address) {
            debug!(
                "[DIGITAL_FENCE] Suppressed ambient {} packet from NAS/trusted MAC {} ({})",
                event.protocol, mac_address, ip_address
            );
            return;
        }

        // ── DB upsert: update presence footprint ──────────────────────────────
        let mac_for_db  = mac_address.clone();
        let ip_for_db   = ip_address.clone();
        let res = db.execute(move |conn| {
            // Determine current knowledge of this device
            let device_info: Option<(bool, bool)> = {
                let mut stmt = conn
                    .prepare("SELECT 1, is_ignored FROM devices WHERE mac = ?1")
                    .map_err(|e| e.to_string())?;
                stmt.query_row(rusqlite::params![mac_for_db], |row| {
                    Ok((true, row.get::<_, i64>(1)? != 0))
                })
                .optional()
                .map_err(|e| e.to_string())?
            };

            let (device_exists, is_ignored) = device_info.unwrap_or((false, false));

            // Upsert: register unknown devices and refresh presence timestamps.
            let affected_rows = conn.execute(
                "INSERT INTO devices (mac, first_seen, last_seen, last_ip, likely_type, is_online, is_active)
                 VALUES (?1, ?2, ?2, ?3, 'Digital Fence Discovery', 1, 1)
                 ON CONFLICT(mac) DO UPDATE SET last_seen = ?2, last_ip = ?3, is_online = 1, is_active = 1",
                rusqlite::params![mac_for_db, now_ms, ip_for_db],
            )
            .map_err(|e| e.to_string())?;

            Ok::<_, String>((device_exists, is_ignored, affected_rows))
        }).await;

        let (device_exists, is_ignored, affected_rows) = match res {
            Ok(tuple) => tuple,
            Err(e) => {
                warn!("[DIGITAL_FENCE] DB upsert failed for {}: {}", mac_address, e);
                return;
            }
        };

        if affected_rows == 0 { return; }

        // ── Perimeter Breach logic ────────────────────────────────────────────
        if !device_exists && !is_ignored {
            warn!(
                "[DIGITAL_FENCE] ⚠ Perimeter Breach — unknown device: MAC={} IP={} via={}",
                mac_address, ip_address, event.protocol
            );

            // Log into the historical event registry
            let db_clone  = db.clone();
            let mac_clone = mac_address.clone();
            tokio::spawn(async move {
                crate::storage::history::log_event(
                    &db_clone,
                    "intruder",
                    None,
                    &format!("Breach detected! Unknown address: {}", mac_clone),
                ).await;
            });

            let title = "🚨 [SHABAKAT PERIMETER ALARM]";
            let body  = format!(
                "An unrecognized device has breached your passive surveillance grid!\n\n\
                 MAC Frame: {}\n\
                 IP Layer:  {}\n\
                 Vector:    {}\n\
                 Time:      {}",
                event.mac_address,
                event.ip_address,
                event.protocol,
                chrono::Utc::now().to_rfc3339()
            );

            let db_clone = db.clone();
            tokio::spawn(async move {
                crate::notifications::broadcast_alert(&db_clone, title, &body).await;
            });
        }

        info!(
            "[DIGITAL_FENCE] Passive Presence Verified: {} [IP: {}] via {}",
            mac_address, ip_address, event.protocol
        );

        // Push a real-time presence update to the Star-Map UI
        let _ = broadcast_tx.send(serde_json::json!({
            "type": "latency_update",
            "payload": {
                "mac":       event.mac_address,
                "ip":        event.ip_address,
                "latencyMs": 0.1,
                "via":       event.protocol,
                "timestamp": now_ms
            }
        }));
    }
}

// ── Suppression set builder ───────────────────────────────────────────────────

/// Builds the complete set of MACs that the Digital Fence should never alarm on.
///
/// Sources (in order):
///  1. `NAS_HOST_MAC` — the confirmed Asustor J4125 NIC (hardcoded, verified)
///  2. `SHABAKAT_TRUSTED_MACS` env var — comma-separated list for additional
///     operator-controlled trusted devices (router, management laptop, etc.)
///
/// All MACs are normalised to UPPERCASE colon-separated form before storage.
fn build_suppressed_mac_set() -> Vec<String> {
    let mut set = Vec::new();

    // 1. Hardcoded NAS host MAC (Fix E — primary self-alarm suppression)
    set.push(super::arp::normalize_mac(NAS_HOST_MAC));

    // 2. Operator-supplied trusted MACs via environment variable
    if let Ok(env_val) = std::env::var("SHABAKAT_TRUSTED_MACS") {
        for raw in env_val.split(',') {
            let normalised = super::arp::normalize_mac(raw.trim());
            if !normalised.is_empty() && !set.contains(&normalised) {
                set.push(normalised);
            }
        }
    }

    set
}
