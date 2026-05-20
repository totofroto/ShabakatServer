use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket as StdUdpSocket};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

/// Look up a single IPv4 address in /proc/net/arp and return its MAC.
/// No subprocess required — safe inside Docker with no extra tools installed.
pub fn lookup_mac_proc(ip: &str) -> Option<String> {
    let target: Ipv4Addr = ip.parse().ok()?;
    parse_proc_arp()
        .into_iter()
        .find_map(|(found_ip, mac)| (found_ip == target).then_some(mac))
}

/// Parse `/proc/net/arp` and return all reachable neighbors as `(ip, mac)` pairs.
///
/// Works on Linux without any special privileges.
pub fn parse_proc_arp() -> Vec<(Ipv4Addr, String)> {
    let Ok(contents) = std::fs::read_to_string("/proc/net/arp") else {
        return Vec::new();
    };

    let mut results = Vec::new();
    for line in contents.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 4 {
            continue;
        }

        let Ok(ip) = fields[0].parse::<Ipv4Addr>() else {
            continue;
        };
        let flags = u32::from_str_radix(fields[2].trim_start_matches("0x"), 16).unwrap_or(0);
        if flags == 0 {
            continue;
        }

        let mac_raw = fields[3];
        let mac_hex: String = mac_raw
            .chars()
            .filter(|c| c.is_ascii_hexdigit())
            .map(|c| c.to_ascii_uppercase())
            .collect();
        if mac_hex.len() != 12 || mac_hex == "000000000000" || mac_hex == "FFFFFFFFFFFF" {
            continue;
        }

        let mac_norm = mac_hex
            .as_bytes()
            .chunks(2)
            .map(|pair| std::str::from_utf8(pair).unwrap_or("00"))
            .collect::<Vec<_>>()
            .join(":");

        results.push((ip, mac_norm));
    }

    results
}

/// Not Android — always returns an empty list.
pub async fn dump_all_neighbours() -> Vec<(Ipv4Addr, String)> {
    Vec::new()
}

/// macOS: read the entire system ARP table via `arp -a` once.
#[cfg(target_os = "macos")]
pub async fn dump_arp_table_macos() -> Vec<(Ipv4Addr, String)> {
    let Ok(output) = Command::new("arp")
        .arg("-a")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
    else {
        return Vec::new();
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.lines().filter_map(parse_arp_a_line_macos).collect()
}

#[cfg(not(target_os = "macos"))]
pub async fn dump_arp_table_macos() -> Vec<(Ipv4Addr, String)> {
    Vec::new()
}

#[cfg(target_os = "macos")]
fn parse_arp_a_line_macos(line: &str) -> Option<(Ipv4Addr, String)> {
    let lp = line.find('(')?;
    let rp = line[lp..].find(')')? + lp;
    let ip: Ipv4Addr = line[lp + 1..rp].parse().ok()?;

    let after_rp = &line[rp + 1..];
    let at_idx = after_rp.find(" at ")?;
    let mac_raw = after_rp[at_idx + 4..].split_whitespace().next()?;

    if mac_raw.starts_with('(') {
        return None;
    }

    let mac = normalize_macos_mac(mac_raw)?;
    Some((ip, mac))
}

#[cfg(target_os = "macos")]
fn normalize_macos_mac(raw: &str) -> Option<String> {
    let parts: Vec<&str> = raw.split(':').collect();
    if parts.len() != 6 {
        return None;
    }
    let mut octets = [0u8; 6];
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() || part.len() > 2 || !part.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        octets[i] = u8::from_str_radix(part, 16).ok()?;
    }
    if octets == [0u8; 6] || octets == [0xff; 6] {
        return None;
    }
    Some(format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        octets[0], octets[1], octets[2], octets[3], octets[4], octets[5]
    ))
}

/// Always `false` — this is a Linux/macOS server, not Android.
pub const fn is_android_target() -> bool {
    false
}

/// No-op on server: neighbour MAC via the kernel table is not used outside Android.
pub(crate) async fn neighbour_table_mac_for_ip(_ip: &str) -> Option<String> {
    None
}

/// Neighbor nudge: dummy UDP to Discard (9) so the kernel resolves the MAC before neighbour lookup.
pub fn nudge_neighbor(ip: &str) {
    if let Ok(socket) = StdUdpSocket::bind("0.0.0.0:0") {
        let _ = socket.set_read_timeout(Some(Duration::from_millis(10)));
        let target: SocketAddr = format!("{ip}:9")
            .parse()
            .unwrap_or_else(|_| SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9));
        let _ = socket.send_to(&[0], target);
    }
}

pub async fn nudge_neighbor_async(ip: Ipv4Addr) {
    let ip = ip.to_string();
    let _ = tokio::task::spawn_blocking(move || nudge_neighbor(&ip)).await;
}

/// No-op on server (was Android-only preflight).
pub async fn preflight_udp_nudges(_hosts: &[Ipv4Addr]) {}

/// Resolve MAC for `ip`.
/// On Linux (Docker), read /proc/net/arp directly — no `arp` binary required.
/// On other platforms, fall back to the `arp -n` subprocess.
pub async fn lookup_mac(ip: &str) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        lookup_mac_proc(ip)
    }
    #[cfg(not(target_os = "linux"))]
    {
        lookup_mac_arp(ip).await
    }
}

async fn lookup_mac_arp(ip: &str) -> Option<String> {
    let output = Command::new("arp")
        .args(["-n", ip])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8(output.stdout).ok()?;
    parse_mac_from_arp(&stdout)
}

fn parse_mac_from_arp(raw: &str) -> Option<String> {
    raw.split_whitespace().find_map(normalize_mac_token)
}

fn normalize_mac_token(token: &str) -> Option<String> {
    let trimmed = token.trim_matches(|c: char| c == '(' || c == ')' || c == ',');
    let candidate = trimmed.replace('-', ":");
    let parts: Vec<&str> = candidate.split(':').collect();

    if parts.len() != 6 {
        return None;
    }

    let mut octets = [0u8; 6];
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() || part.len() > 2 || !part.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        octets[i] = u8::from_str_radix(part, 16).ok()?;
    }

    if octets == [0u8; 6] || octets == [0xff; 6] {
        return None;
    }

    Some(format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        octets[0], octets[1], octets[2], octets[3], octets[4], octets[5]
    ))
}
