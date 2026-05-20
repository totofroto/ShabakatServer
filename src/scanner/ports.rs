//! Port Guardian — on-demand TCP exposure sweep for a single host.

use std::{net::SocketAddr, time::Duration};

use futures::future::join_all;
use tokio::net::TcpStream;

/// Curated audit surface covering remote access, web, file sharing, printers, media,
/// databases, and IoT hubs — every port that feeds a fingerprint rule plus common
/// exposure indicators (SSH, RDP, cleartext HTTP).
const GUARDIAN_PORTS: &[u16] = &[
    22,    // SSH
    23,    // Telnet
    53,    // DNS (Pi-Hole)
    80,    // HTTP
    139,   // NetBIOS Session
    443,   // HTTPS
    445,   // SMB
    515,   // LPD
    548,   // AFP
    554,   // RTSP
    631,   // IPP
    1400,  // Sonos
    1883,  // MQTT
    3000,  // LG WebOS TV
    3001,  // LG WebOS TV
    3306,  // MySQL
    3389,  // RDP
    3689,  // Apple DAAP (iTunes / AirPlay)
    5000,  // Synology DSM HTTP / Plex (old)
    5001,  // Synology DSM HTTPS
    5432,  // PostgreSQL
    7000,  // AirPlay
    7668,  // AirPlay 2 / HomeKit accessory port
    7676,  // Samsung Device
    8000,  // Asustor / HTTP alt
    8001,  // Samsung / Asustor
    8002,  // Samsung Smart TV
    8008,  // Chromecast HTTP API
    8009,  // Chromecast TLS
    8080,  // HTTP Alt / UniFi
    8123,  // Home Assistant
    8443,  // HTTPS Alt / UniFi
    9100,  // JetDirect (raw printing)
    27017, // MongoDB
    32400, // Plex Media Server
    51827, // Philips Hue Bridge
    54921, // Brother Printer / Scanner
    54925, // Brother Printer
    62078, // Apple Device (iPhone/iPad/Mac)
];

const CONNECT_TIMEOUT: Duration = Duration::from_millis(500);

/// Concurrent TCP connect probes; wall-clock stays under ~1s with 500ms per-port budget.
pub async fn scan_device_ports(ip: String) -> Result<Vec<u16>, String> {
    let ip_addr: std::net::IpAddr = ip
        .parse()
        .map_err(|_| format!("invalid IP address for port scan: '{ip}'"))?;

    let checks: Vec<_> = GUARDIAN_PORTS
        .iter()
        .copied()
        .map(|port| {
            let socket = SocketAddr::new(ip_addr, port);
            async move {
                match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(socket)).await {
                    Ok(Ok(_)) => Some(port),
                    _ => None,
                }
            }
        })
        .collect();

    let mut open: Vec<u16> = join_all(checks).await.into_iter().flatten().collect();
    open.sort_unstable();
    Ok(open)
}
