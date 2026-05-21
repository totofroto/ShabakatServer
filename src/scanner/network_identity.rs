use std::net::Ipv4Addr;
use std::process::Stdio;
use tokio::process::Command;

pub struct NetworkInfo {
    pub ssid: Option<String>,
    /// Canonical network fingerprint: Wi-Fi BSSID or gateway MAC (on wired).
    pub bssid: Option<String>,
    pub gateway: Option<String>,
    pub subnet: Option<String>,
}

/// Detect the current network before a scan so every device and history record
/// can be tagged with a `network_id`.
///
/// Checks env vars first (`SHABAKAT_NETWORK_BSSID` etc.) — the reliable path
/// inside Docker where subprocess tools and `/proc/net/arp` may be unavailable.
/// Falls through to the `iwconfig`/`ip route`/ARP auto-detection only when
/// `SHABAKAT_NETWORK_BSSID` is not set.
pub async fn get_current_network_info() -> NetworkInfo {
    // Fast path: operator-supplied identity via environment variables.
    if let Ok(bssid) = std::env::var("SHABAKAT_NETWORK_BSSID") {
        let bssid = bssid.trim().to_string();
        if !bssid.is_empty() {
            log::info!("[FLIGHT_RECORDER] Network identity source: env");
            return NetworkInfo {
                ssid:    std::env::var("SHABAKAT_NETWORK_SSID").ok().filter(|s| !s.trim().is_empty()),
                bssid:   Some(bssid),
                gateway: std::env::var("SHABAKAT_NETWORK_GATEWAY").ok().filter(|s| !s.trim().is_empty()),
                subnet:  std::env::var("SHABAKAT_NETWORK_SUBNET").ok().filter(|s| !s.trim().is_empty()),
            };
        }
    }

    // Slow path: auto-detect from OS interfaces and ARP table.
    log::info!("[FLIGHT_RECORDER] Network identity source: auto");

    let (gateway_ip, iface) = parse_default_route().await;

    let (ssid, bssid) = match iface.as_deref() {
        Some(name) => parse_iwconfig(name).await,
        None => (None, None),
    };

    // Wired or iwconfig unavailable: use gateway MAC as network fingerprint.
    let bssid = if bssid.is_some() {
        bssid
    } else {
        gateway_ip
            .as_deref()
            .and_then(super::arp::lookup_mac_proc)
    };

    let subnet = match iface.as_deref() {
        Some(name) => parse_interface_subnet(name).await,
        None => None,
    };

    NetworkInfo { ssid, bssid, gateway: gateway_ip, subnet }
}

/// Parse `ip route show default` → (gateway_ip, interface_name).
async fn parse_default_route() -> (Option<String>, Option<String>) {
    let Ok(output) = Command::new("ip")
        .args(["route", "show", "default"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
    else {
        return (None, None);
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut gateway = None;
    let mut iface = None;

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.first() != Some(&"default") {
            continue;
        }
        if let Some(pos) = parts.iter().position(|&p| p == "via") {
            gateway = parts.get(pos + 1).map(|s| s.to_string());
        }
        if let Some(pos) = parts.iter().position(|&p| p == "dev") {
            iface = parts.get(pos + 1).map(|s| s.to_string());
        }
        break;
    }

    (gateway, iface)
}

/// Try `iwconfig <iface>` to extract SSID and BSSID. Returns (None, None) on wired.
async fn parse_iwconfig(iface: &str) -> (Option<String>, Option<String>) {
    let Ok(output) = Command::new("iwconfig")
        .arg(iface)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
    else {
        return (None, None);
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut ssid = None;
    let mut bssid = None;

    for line in stdout.lines() {
        // ESSID:"MyNetwork"
        if let Some(pos) = line.find("ESSID:\"") {
            if let Some(end) = line[pos + 7..].find('"') {
                let s = line[pos + 7..pos + 7 + end].trim().to_string();
                if !s.is_empty() && s != "off/any" {
                    ssid = Some(s);
                }
            }
        }
        // Access Point: AA:BB:CC:DD:EE:FF
        if let Some(pos) = line.find("Access Point:") {
            let rest = line[pos + 13..].trim();
            let mac = rest.split_whitespace().next().unwrap_or("");
            if mac.len() == 17 && mac != "Not-Associated" {
                bssid = Some(mac.to_uppercase());
            }
        }
    }

    (ssid, bssid)
}

/// Parse `ip addr show <iface>` to get the network CIDR (e.g. "192.168.254.0/24").
async fn parse_interface_subnet(iface: &str) -> Option<String> {
    let output = Command::new("ip")
        .args(["addr", "show", iface])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("inet ") {
            continue;
        }
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        let cidr_str = parts.get(1)?;
        let slash = cidr_str.find('/')?;
        let ip: Ipv4Addr = cidr_str[..slash].parse().ok()?;
        let prefix_len: u8 = cidr_str[slash + 1..].parse().ok()?;
        let mask = if prefix_len == 0 { 0u32 } else { !0u32 << (32 - prefix_len) };
        let net_addr = Ipv4Addr::from(u32::from(ip) & mask);
        return Some(format!("{}/{}", net_addr, prefix_len));
    }

    None
}
