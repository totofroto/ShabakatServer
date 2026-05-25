pub mod arp;
pub mod deep;
pub mod digital_fence; // Expose the new passive ambient scanning sentry module
pub mod fingerprints;
pub mod network;
pub mod network_identity;
pub mod ping;
pub mod resolver;
pub mod router_api;
pub mod sys_metrics;
pub mod vulnerability;

/// Delegates to the port-fingerprint engine in `fingerprints`.
pub fn identify_by_ports(open_ports: &[u16]) -> String {
    fingerprints::identify_by_ports(open_ports)
}

pub use fingerprints::classify_from_hostname;
pub use fingerprints::classify_from_vendor;
pub use fingerprints::get_registry;
pub use fingerprints::identify_corporate_vendor;
pub use fingerprints::merge_port_and_http_likely;

use log::{debug, info, warn};
use crate::storage::AppDb;

use std::{
    collections::{HashMap, HashSet},
    net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex, OnceLock,
    },
    thread,
    time::{Duration, Instant},
};

// ── Global scan lock ──────────────────────────────────────────────────────────

static SCAN_ACTIVE: AtomicBool = AtomicBool::new(false);
static SCAN_CANCELLED: AtomicBool = AtomicBool::new(false);

pub struct ScanGuard;

impl ScanGuard {
    pub fn try_acquire() -> Option<Self> {
        SCAN_ACTIVE
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| {
                SCAN_CANCELLED.store(false, Ordering::Release);
                ScanGuard
            })
    }
}

impl Drop for ScanGuard {
    fn drop(&mut self) {
        SCAN_CANCELLED.store(false, Ordering::Release);
        SCAN_ACTIVE.store(false, Ordering::Release);
    }
}

pub fn is_scan_active() -> bool {
    SCAN_ACTIVE.load(Ordering::Acquire)
}

fn scan_cancelled() -> bool {
    SCAN_CANCELLED.load(Ordering::Acquire)
}

use crate::types::{DiscoveredDevice, ScanEvent, ScanNetworkPayload};
use futures::stream::{self as stream, StreamExt};
use ipnet::Ipv4Net;
use mdns_sd::{Receiver as MdnsReceiver, ServiceDaemon, ServiceEvent};
use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, Semaphore};

/// Channel sender for streaming scan events to the WebSocket layer (or test harness).
type EventSink = Option<mpsc::UnboundedSender<ScanEvent>>;

static XML_PROBE_SEM: OnceLock<Semaphore> = OnceLock::new();
fn xml_probe_sem() -> &'static Semaphore {
    XML_PROBE_SEM.get_or_init(|| Semaphore::new(16))
}

pub struct ScanNetworkResult {
    pub devices: Vec<DiscoveredDevice>,
    pub average_latency_ms: Option<f64>,
    pub scanned_hosts: usize,
}

#[derive(Clone, Copy)]
pub enum ScanMode {
    Silent,
    Aggressive,
    Deep,
}

impl ScanMode {
    pub fn from_str(input: &str) -> Self {
        match input.to_ascii_lowercase().as_str() {
            "aggressive" => ScanMode::Aggressive,
            "deep" => ScanMode::Deep,
            _ => ScanMode::Silent,
        }
    }
}

#[derive(Clone, Copy)]
struct ScanProfile {
    host_concurrency: usize,
    host_probe_timeout: Duration,
    use_broadcast_nudge: bool,
}

fn profile_for_mode(mode: ScanMode) -> ScanProfile {
    match mode {
        ScanMode::Silent => ScanProfile {
            host_concurrency: 20,
            host_probe_timeout: Duration::from_millis(2000),
            use_broadcast_nudge: false,
        },
        ScanMode::Aggressive => ScanProfile {
            host_concurrency: 150,
            host_probe_timeout: Duration::from_millis(500),
            use_broadcast_nudge: true,
        },
        ScanMode::Deep => ScanProfile {
            host_concurrency: 200,
            host_probe_timeout: Duration::from_millis(3000),
            use_broadcast_nudge: true,
        },
    }
}

fn concurrency_for_subnet(mode: ScanMode, host_count: usize, base: usize) -> usize {
    if host_count > 256 {
        return match mode {
            ScanMode::Silent => 40,
            ScanMode::Aggressive => 100,
            ScanMode::Deep => 150,
        };
    }
    base
}

#[derive(Default)]
struct VendorIndex {
    ma_s: HashMap<String, String>,
    ma_m: HashMap<String, String>,
    ma_l: HashMap<String, String>,
}

static VENDOR_INDEX: OnceLock<VendorIndex> = OnceLock::new();
static MDNS_CACHE: OnceLock<Mutex<HashMap<String, MdnsHostInfo>>> = OnceLock::new();
static RDNS_CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();

fn mdns_cache() -> &'static Mutex<HashMap<String, MdnsHostInfo>> {
    MDNS_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn rdns_cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    RDNS_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

static SSDP_NAME_CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn ssdp_name_cache() -> &'static Mutex<HashMap<String, String>> {
    SSDP_NAME_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn merge_persistent_mdns_cache(into: &mut HashMap<String, MdnsHostInfo>) {
    let cache = mdns_cache().lock().unwrap();
    for (ip, cached) in cache.iter() {
        let entry = into.entry(ip.clone()).or_default();
        for s in &cached.services {
            entry.services.insert(s.clone());
        }
        match (&cached.hostname, &entry.hostname) {
            (Some(ch), None) if !ch.is_empty() => entry.hostname = Some(ch.clone()),
            (Some(ch), Some(cur)) if !ch.is_empty() && ch.len() > cur.len() => {
                entry.hostname = Some(ch.clone());
            }
            _ => {}
        }
    }
}

fn persist_mdns_discovery(ip: &str, hostname: &str, tag: &str) {
    let mut c = mdns_cache().lock().unwrap();
    merge_mdns_record(&mut c, ip.to_string(), hostname, tag);
}

const PREFIX_LENGTHS: [usize; 3] = [9, 7, 6];

const SCAN_LOCAL_NETWORK_TIMEOUT: Duration = Duration::from_secs(90);

const PTR_QUERY_CONCURRENCY: usize = 32;

const HOTSPOT_IFACE_FRAGMENTS: &[&str] = &["swlan", "ap", "wlan", "tether"];

fn is_hotspot_iface(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    HOTSPOT_IFACE_FRAGMENTS.iter().any(|f| lower.contains(f))
}

pub fn get_best_local_ip() -> Option<Ipv4Addr> {
    get_best_local_network().map(|(ip, _)| ip)
}

fn netmask_to_prefix(netmask: Ipv4Addr) -> u8 {
    u32::from(netmask).leading_ones() as u8
}

pub fn get_best_local_network() -> Option<(Ipv4Addr, u8)> {
    if let Ok(ifaces) = get_if_addrs::get_if_addrs() {
        for iface in &ifaces {
            if !is_hotspot_iface(&iface.name) {
                continue;
            }
            if let get_if_addrs::IfAddr::V4(ref v4) = iface.addr {
                let ip = v4.ip;
                if !ip.is_loopback() && !ip.is_unspecified() {
                    let raw_prefix = netmask_to_prefix(v4.netmask);
                    let mut prefix = raw_prefix.clamp(16, 30);
                    if prefix < 22 {
                        info!(
                            "scan: iface {} reported broad subnet /{} — forcing /24",
                            iface.name, prefix
                        );
                        prefix = 24;
                    }
                    if raw_prefix != prefix {
                        info!(
                            "scan: iface {} netmask /{} out of safe range — clamped to /{}",
                            iface.name, raw_prefix, prefix
                        );
                    } else {
                        info!(
                            "scan: iface {} → {}/{} (netmask {})",
                            iface.name, ip, prefix, v4.netmask
                        );
                    }
                    return Some((ip, prefix));
                }
            }
        }
    }

    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let local = socket.local_addr().ok()?;
    match local.ip() {
        IpAddr::V4(v4) if !v4.is_unspecified() && !v4.is_loopback() => {
            let prefix = 24;
            info!(
                "scan: UDP-trick fallback → {} (no netmask available, using default /24)",
                v4
            );
            Some((v4, prefix))
        }
        _ => None,
    }
}

pub fn init_vendor_map() -> Result<(), String> {
    if VENDOR_INDEX.get().is_some() {
        return Ok(());
    }

    let raw = include_str!("../../resources/mac-vendors.json");
    let parsed: HashMap<String, String> =
        serde_json::from_str(raw).map_err(|err| format!("failed to parse vendor map: {err}"))?;
    let mut index = VendorIndex::default();
    for (prefix, vendor) in parsed {
        if let Some(normalized) = normalize_oui_prefix(&prefix) {
            match normalized.len() {
                9 => {
                    index.ma_s.entry(normalized).or_insert(vendor);
                }
                7 => {
                    index.ma_m.entry(normalized).or_insert(vendor);
                }
                6 => {
                    index.ma_l.entry(normalized).or_insert(vendor);
                }
                _ => {}
            }
        }
    }

    match VENDOR_INDEX.set(index) {
        Ok(()) => Ok(()),
        Err(_) => Ok(()),
    }
}

pub fn vendor_name_from_mac(mac: &str) -> String {
    let Some(vendors) = VENDOR_INDEX.get() else {
        log::warn!("Vendor index NOT initialized during lookup for {}", mac);
        return "Unknown".to_string();
    };

    let Some(mac_hex) = normalize_mac_hex(mac) else {
        return "Unknown".to_string();
    };

    let mut vendor = "Unknown".to_string();

    if mac_hex.len() >= 9 {
        if let Some(name) = vendors.ma_s.get(&mac_hex[..9]) {
            vendor = name.clone();
        }
    }
    if vendor == "Unknown" && mac_hex.len() >= 7 {
        if let Some(name) = vendors.ma_m.get(&mac_hex[..7]) {
            vendor = name.clone();
        }
    }
    if vendor == "Unknown" && mac_hex.len() >= 6 {
        if let Some(name) = vendors.ma_l.get(&mac_hex[..6]) {
            vendor = name.clone();
        }
    }

    log::info!("[OUI] MAC: {} -> Vendor: {}", mac, vendor);
    vendor
}

fn normalize_mac_hex(mac: &str) -> Option<String> {
    let hex_only: String = mac
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .map(|c| c.to_ascii_uppercase())
        .collect();

    if hex_only.len() < 6 {
        return None;
    }

    Some(hex_only)
}

fn normalize_oui_prefix(prefix: &str) -> Option<String> {
    let normalized: String = prefix
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .map(|c| c.to_ascii_uppercase())
        .collect();

    if PREFIX_LENGTHS.contains(&normalized.len()) {
        Some(normalized)
    } else {
        None
    }
}

fn is_generic_hostname(name: &str, ip: &str) -> bool {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return true;
    }

    let lowered = trimmed.to_ascii_lowercase();
    lowered == ip.to_ascii_lowercase()
        || lowered == format!("host {ip}").to_ascii_lowercase()
        || lowered.starts_with("host ")
        || lowered.starts_with("unknown")
        || lowered.contains("localhost")
}

fn infer_device_name(vendor: &str, ip: &str) -> String {
    if vendor != "Unknown" && !vendor.is_empty() {
        return format!("{vendor} Device");
    }
    format!("Host {ip}")
}

fn infer_device_type(
    hostname: &str,
    vendor: &str,
    mdns_services: Option<&HashSet<String>>,
) -> String {
    let lowered_hostname = hostname.to_ascii_lowercase();
    let lowered_vendor = vendor.to_ascii_lowercase();

    if let Some(services) = mdns_services {
        if services.contains("googlecast") {
            return "tv".to_string();
        }
        if services.contains("hap") {
            return "iot".to_string();
        }
        if services.contains("printer") || services.contains("ipp") {
            return "printer".to_string();
        }
        if services.contains("spotify") || services.contains("sonos") || services.contains("raop") {
            return "audio".to_string();
        }
        if services.contains("smb") {
            return "laptop".to_string();
        }
        if services.contains("airplay")
            && (lowered_hostname.contains("tv")
                || lowered_hostname.contains("sony")
                || lowered_hostname.contains("lg"))
        {
            return "tv".to_string();
        }
    }

    if lowered_hostname.contains("tv")
        || lowered_hostname.contains("bravia")
        || lowered_hostname.contains("webos")
    {
        return "tv".to_string();
    }

    if lowered_hostname.contains("speaker")
        || lowered_hostname.contains("echo")
        || lowered_hostname.contains("sonos")
        || lowered_hostname.contains("homepod")
        || lowered_hostname.contains("nest")
        || lowered_hostname.contains("google home")
    {
        return "audio".to_string();
    }

    if lowered_hostname.contains("iphone")
        || lowered_hostname.contains("ipad")
        || lowered_hostname.contains("watch")
    {
        return "phone".to_string();
    }
    if lowered_hostname.contains("macbook")
        || lowered_hostname.contains("imac")
        || lowered_hostname.contains("desktop")
        || lowered_hostname.contains("laptop")
        || lowered_hostname.contains("pc")
    {
        return "laptop".to_string();
    }

    if lowered_vendor.contains("printer") {
        return "printer".to_string();
    }

    "iot".to_string()
}

fn infer_device_name_with_type(
    hostname: &str,
    vendor: &str,
    ip: &str,
    device_type: &str,
) -> String {
    if !is_generic_hostname(hostname, ip) {
        return hostname.to_string();
    }

    match device_type {
        "printer" => return "Network Printer".to_string(),
        "tv" => {
            if vendor != "Unknown" {
                return format!("{vendor} TV");
            }
            return "Smart TV".to_string();
        }
        "audio" => {
            if vendor != "Unknown" {
                return format!("{vendor} Speaker");
            }
            return "Smart Speaker".to_string();
        }
        "phone" => {
            if vendor != "Unknown" {
                return format!("{vendor} Mobile Device");
            }
            return "Mobile Device".to_string();
        }
        "laptop" => {
            if vendor != "Unknown" {
                return format!("{vendor} Computer");
            }
            return "Computer".to_string();
        }
        "iot" if vendor != "Unknown" => {
            return format!("{vendor} Device");
        }
        "iot" => {}
        _ => {}
    }

    infer_device_name(vendor, ip)
}

fn is_randomized_mac(mac: &str) -> bool {
    let Some(hex) = normalize_mac_hex(mac) else {
        return false;
    };
    let Some(second_nibble) = hex.chars().nth(1) else {
        return false;
    };
    matches!(second_nibble, '2' | '6' | 'A' | 'E')
}

/// Resolve the link-layer MAC for `ip`.
/// Reads /proc/net/arp directly first (no subprocess, works in Docker),
/// then falls back to `arp -n` for macOS or when the kernel entry is absent.
pub async fn get_mac_address(ip: &str) -> Option<String> {
    if let Some(mac) = arp::lookup_mac_proc(ip) {
        return Some(mac);
    }
    arp::lookup_mac(ip).await
}

/// Zeroconf browse targets for friendly hostnames.
const MDNS_BROWSE: &[(&str, &str)] = &[
    ("_googlecast._tcp.local.", "googlecast"),
    ("_hap._tcp.local.", "hap"),
    ("_printer._tcp.local.", "printer"),
    ("_spotify-connect._tcp.local.", "spotify"),
    ("_smb._tcp.local.", "smb"),
];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct MdnsHostInfo {
    hostname: Option<String>,
    services: HashSet<String>,
}

fn humanize_mdns_hostname(server: &str) -> String {
    let t = server.trim().trim_end_matches('.');
    let lower = t.to_ascii_lowercase();
    let core = if let Some(i) = lower.rfind(".local") {
        &t[..i]
    } else {
        t
    };
    core.trim_matches('.').trim().to_string()
}

fn sanitize_rdns_hostname(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_end_matches('.').trim();
    if trimmed.is_empty() {
        return None;
    }
    let lowered = trimmed.to_ascii_lowercase();

    if lowered.ends_with(".in-addr.arpa") || lowered == "in-addr.arpa" {
        return None;
    }

    const LOCAL_SUFFIXES: &[&str] = &[
        ".local",
        ".lan",
        ".home",
        ".internal",
        ".localdomain",
        ".intranet",
        ".arpa",
    ];
    let without_suffix = LOCAL_SUFFIXES
        .iter()
        .find_map(|suf| {
            lowered
                .strip_suffix(suf)
                .map(|stripped| &trimmed[..stripped.len()])
        })
        .unwrap_or(trimmed);

    let clean = without_suffix.trim().trim_matches('.').to_string();
    if clean.is_empty() {
        None
    } else {
        Some(clean)
    }
}

fn reverse_hostname_field(
    ip: &str,
    os_rdns: Option<String>,
    ptr: Option<String>,
) -> Option<String> {
    let from_os = os_rdns.and_then(|s| sanitize_rdns_hostname(&s));
    let from_ptr = ptr.and_then(|s| sanitize_rdns_hostname(&s));
    let best = from_os.or(from_ptr)?;
    let ip_trim = ip.trim();
    if best.eq_ignore_ascii_case(ip_trim) {
        return None;
    }
    if is_generic_hostname(&best, ip_trim) {
        return None;
    }
    Some(best)
}

fn reverse_dns_name_for_ip(ip: &str) -> Option<String> {
    rdns_cache().lock().unwrap().get(ip).cloned().flatten()
}

fn build_ptr_query(ip: Ipv4Addr) -> Vec<u8> {
    let [a, b, c, d] = ip.octets();
    let labels: [String; 6] = [
        d.to_string(),
        c.to_string(),
        b.to_string(),
        a.to_string(),
        "in-addr".to_string(),
        "arpa".to_string(),
    ];
    let mut buf: Vec<u8> = Vec::with_capacity(64);
    buf.extend_from_slice(&[
        0x13, 0x37, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ]);
    for label in &labels {
        buf.push(label.len() as u8);
        buf.extend_from_slice(label.as_bytes());
    }
    buf.push(0x00);
    buf.extend_from_slice(&[0x00, 0x0C]);
    buf.extend_from_slice(&[0x00, 0x01]);
    buf
}

fn dns_skip_name(buf: &[u8], mut pos: usize) -> Option<usize> {
    loop {
        let b = *buf.get(pos)?;
        if b == 0x00 {
            return Some(pos + 1);
        }
        if b & 0xC0 == 0xC0 {
            return Some(pos + 2);
        }
        pos += 1 + b as usize;
        if pos > buf.len() {
            return None;
        }
    }
}

fn dns_read_name(buf: &[u8], start: usize) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut pos = start;
    let mut jumps = 0usize;
    loop {
        let b = *buf.get(pos)?;
        if b == 0x00 {
            break;
        }
        if b & 0xC0 == 0xC0 {
            if jumps >= 10 {
                return None;
            }
            let offset = ((b as usize & 0x3F) << 8) | *buf.get(pos + 1)? as usize;
            pos = offset;
            jumps += 1;
            continue;
        }
        let len = b as usize;
        pos += 1;
        let label = std::str::from_utf8(buf.get(pos..pos + len)?).ok()?;
        parts.push(label.to_string());
        pos += len;
    }
    Some(parts.join("."))
}

fn parse_dns_ptr_response(buf: &[u8]) -> Option<String> {
    if buf.len() < 12 {
        return None;
    }
    let qdcount = u16::from_be_bytes([buf[4], buf[5]]) as usize;
    let ancount = u16::from_be_bytes([buf[6], buf[7]]) as usize;
    if ancount == 0 {
        return None;
    }

    let mut pos = 12;
    for _ in 0..qdcount {
        pos = dns_skip_name(buf, pos)?;
        pos += 4;
    }

    for _ in 0..ancount {
        pos = dns_skip_name(buf, pos)?;
        if pos + 10 > buf.len() {
            return None;
        }
        let rtype = u16::from_be_bytes([buf[pos], buf[pos + 1]]);
        let rdlength = u16::from_be_bytes([buf[pos + 8], buf[pos + 9]]) as usize;
        pos += 10;
        if rtype == 12 {
            return dns_read_name(buf, pos);
        }
        pos += rdlength;
        if pos > buf.len() {
            return None;
        }
    }
    None
}

async fn get_ptr_name(ip: &str, dns_server: Ipv4Addr) -> Option<String> {
    if let Some(cached) = rdns_cache().lock().unwrap().get(ip).cloned() {
        return cached;
    }

    let ip_addr: Ipv4Addr = ip.parse().ok()?;
    let query = build_ptr_query(ip_addr);

    let sock = tokio::net::UdpSocket::bind("0.0.0.0:0").await.ok()?;
    let dest = SocketAddr::V4(SocketAddrV4::new(dns_server, 53));
    sock.send_to(&query, dest).await.ok()?;

    let mut buf = [0u8; 512];
    let len = match tokio::time::timeout(Duration::from_millis(500), sock.recv_from(&mut buf)).await
    {
        Ok(Ok((n, _))) => n,
        _ => return None,
    };

    let raw_name = parse_dns_ptr_response(&buf[..len])?;
    let result = sanitize_rdns_hostname(&raw_name);
    rdns_cache()
        .lock()
        .unwrap()
        .insert(ip.to_string(), result.clone());
    result
}

fn merge_mdns_record(
    map: &mut HashMap<String, MdnsHostInfo>,
    ip: String,
    hostname: &str,
    tag: &str,
) {
    let human = humanize_mdns_hostname(hostname);
    let entry = map.entry(ip).or_default();
    entry.services.insert(tag.to_string());
    if human.is_empty() {
        return;
    }
    match &entry.hostname {
        None => entry.hostname = Some(human),
        Some(prev) if human.len() > prev.len() => entry.hostname = Some(human),
        _ => {}
    }
}

fn pick_mdns_primary_service(services: &HashSet<String>) -> Option<String> {
    const ORDER: &[&str] = &["googlecast", "hap", "spotify", "smb", "printer"];
    for s in ORDER {
        if services.contains(*s) {
            return Some((*s).to_string());
        }
    }
    services.iter().next().cloned()
}

const SSDP_MSEARCH: &str = concat!(
    "M-SEARCH * HTTP/1.1\r\n",
    "HOST: 239.255.255.250:1900\r\n",
    "MAN: \"ssdp:discover\"\r\n",
    "MX: 2\r\n",
    "ST: ssdp:all\r\n",
    "\r\n",
);

struct SsdpHeaders {
    server: Option<String>,
    location: Option<String>,
}

fn parse_ssdp_headers(payload: &[u8]) -> SsdpHeaders {
    let mut out = SsdpHeaders {
        server: None,
        location: None,
    };
    let Ok(s) = std::str::from_utf8(payload) else {
        return out;
    };
    for line in s.lines() {
        let line = line.trim();
        let Some((key, rest)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let val = rest.trim();
        if val.is_empty() {
            continue;
        }
        if key.eq_ignore_ascii_case("server") && out.server.is_none() {
            out.server = Some(val.to_string());
        } else if key.eq_ignore_ascii_case("location") && out.location.is_none() {
            out.location = Some(
                format!("{}:{}", key, rest)
                    .trim_start_matches("location:")
                    .trim_start_matches("LOCATION:")
                    .trim()
                    .to_string(),
            );
        }
    }
    if out.location.is_none() {
        for line in s.lines() {
            let line = line.trim();
            if let Some(loc) = line
                .strip_prefix("LOCATION:")
                .or_else(|| line.strip_prefix("location:"))
                .or_else(|| {
                    let lower = line.to_ascii_lowercase();
                    if lower.starts_with("location:") {
                        Some(&line[9..])
                    } else {
                        None
                    }
                })
            {
                let v = loc.trim().to_string();
                if !v.is_empty() {
                    out.location = Some(v);
                    break;
                }
            }
        }
    }
    out
}

fn extract_xml_tag(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let lower_xml = xml.to_ascii_lowercase();
    let lower_open = open.to_ascii_lowercase();
    let lower_close = close.to_ascii_lowercase();
    let start = lower_xml.find(&lower_open)? + lower_open.len();
    let end = lower_xml[start..].find(&lower_close)?;
    let value = xml[start..start + end].trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn parse_netbios_response(data: &[u8]) -> Option<String> {
    for &rdata_offset in &[62usize, 56, 50] {
        if data.len() <= rdata_offset {
            continue;
        }
        let num_names = data[rdata_offset] as usize;
        if num_names == 0 || num_names > 25 {
            continue;
        }
        let names_start = rdata_offset + 1;
        if data.len() < names_start + num_names * 18 {
            continue;
        }
        for i in 0..num_names {
            let off = names_start + i * 18;
            if data[off + 15] != 0x00 {
                continue;
            }
            let raw = &data[off..off + 15];
            let name = std::str::from_utf8(raw)
                .unwrap_or("")
                .trim_end_matches('\0')
                .trim()
                .to_string();
            if !name.is_empty() && name.chars().all(|c| c.is_ascii_graphic() || c == ' ') {
                return Some(name);
            }
        }
    }
    None
}

async fn get_netbios_name(ip: &str) -> Option<String> {
    let sock = tokio::net::UdpSocket::bind("0.0.0.0:0").await.ok()?;
    let target: std::net::SocketAddr = format!("{ip}:137").parse().ok()?;

    #[rustfmt::skip]
    let packet: &[u8] = &[
        0x00, 0x00,
        0x00, 0x00,
        0x00, 0x01,
        0x00, 0x00,
        0x00, 0x00,
        0x00, 0x00,
        0x20,
        0x43, 0x4b, 0x41, 0x41, 0x41, 0x41, 0x41, 0x41,
        0x41, 0x41, 0x41, 0x41, 0x41, 0x41, 0x41, 0x41,
        0x41, 0x41, 0x41, 0x41, 0x41, 0x41, 0x41, 0x41,
        0x41, 0x41, 0x41, 0x41, 0x41, 0x41, 0x41, 0x41,
        0x00,
        0x00, 0x21,
        0x00, 0x01,
    ];

    sock.send_to(packet, target).await.ok()?;

    let mut buf = [0u8; 1024];
    let len = match tokio::time::timeout(Duration::from_millis(300), sock.recv_from(&mut buf)).await
    {
        Ok(Ok((n, _))) => n,
        _ => return None,
    };

    if len >= 57 + 15 {
        let name_bytes = &buf[57..57 + 15];
        if let Ok(raw) = std::str::from_utf8(name_bytes) {
            let name = raw
                .trim_end_matches(' ')
                .trim_end_matches('\0')
                .trim()
                .to_string();
            if !name.is_empty() && name.chars().all(|c| c.is_ascii_graphic() || c == ' ') {
                return Some(name);
            }
        }
    }

    parse_netbios_response(&buf[..len])
}

async fn fetch_device_identity(
    ip: String,
    location_url: Option<String>,
    client: &reqwest::Client,
) -> Option<(String, String)> {
    if let Some(url) = location_url {
        if let Ok(resp) = client.get(&url).send().await {
            if let Ok(body) = resp.text().await {
                if let Some(name) = extract_xml_tag(&body, "friendlyName")
                    .or_else(|| extract_xml_tag(&body, "modelName"))
                {
                    return Some((ip, name));
                }
            }
        }
    }
    get_netbios_name(&ip).await.map(|name| (ip, name))
}

fn extract_html_title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<title>")? + 7;
    let end = lower[start..].find("</title>")? + start;
    let title = html[start..end].trim().to_string();
    if title.is_empty() {
        None
    } else {
        Some(title)
    }
}

fn is_useful_http_identity(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.len() < 3 || trimmed.len() > 100 {
        return false;
    }
    let l = trimmed.to_ascii_lowercase();
    for prefix in &[
        "apache",
        "nginx",
        "microsoft-iis",
        "lighttpd",
        "openresty",
        "cloudflare",
    ] {
        if l.starts_with(prefix) {
            return false;
        }
    }
    for exact in &[
        "untitled",
        "document",
        "index of /",
        "403 forbidden",
        "404 not found",
        "400 bad request",
        "500 internal server error",
    ] {
        if l.as_str() == *exact {
            return false;
        }
    }
    true
}

async fn probe_http_identity(ip: &str, client: &reqwest::Client) -> Option<String> {
    let url = format!("http://{ip}/");
    let resp = tokio::time::timeout(Duration::from_secs(2), client.get(&url).send())
        .await
        .ok()?
        .ok()?;

    let server_val = resp
        .headers()
        .get("server")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string());

    if let Some(ref s) = server_val {
        if is_useful_http_identity(s) {
            return Some(s.chars().take(80).collect());
        }
    }

    let body = tokio::time::timeout(Duration::from_secs(2), resp.text())
        .await
        .ok()?
        .ok()?;

    let title = extract_html_title(&body)?;
    if is_useful_http_identity(&title) {
        Some(title.chars().take(80).collect())
    } else {
        None
    }
}

fn build_mdns_device_info_query() -> Vec<u8> {
    let mut buf = vec![
        0x00, 0x00,
        0x00, 0x00,
        0x00, 0x01,
        0x00, 0x00,
        0x00, 0x00,
        0x00, 0x00,
    ];
    for label in &[b"_device-info" as &[u8], b"_tcp", b"local"] {
        buf.push(label.len() as u8);
        buf.extend_from_slice(label);
    }
    buf.push(0x00);
    buf.extend_from_slice(&[0x00, 0x0C]);
    buf.extend_from_slice(&[0x80, 0x01]);
    buf
}

fn parse_txt_model(rdata: &[u8]) -> Option<String> {
    let mut pos = 0;
    while pos < rdata.len() {
        let len = *rdata.get(pos)? as usize;
        pos += 1;
        let end = pos + len;
        if end > rdata.len() {
            break;
        }
        if let Ok(s) = std::str::from_utf8(&rdata[pos..end]) {
            if s.len() > 6 && s[..6].eq_ignore_ascii_case("model=") {
                let v = s[6..].trim().to_string();
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
        pos = end;
    }
    None
}

fn parse_mdns_identity_response(buf: &[u8]) -> Option<String> {
    if buf.len() < 12 {
        return None;
    }
    if buf[2] & 0x80 == 0 {
        return None;
    }

    let qdcount = u16::from_be_bytes([buf[4], buf[5]]) as usize;
    let ancount = u16::from_be_bytes([buf[6], buf[7]]) as usize;
    let nscount = u16::from_be_bytes([buf[8], buf[9]]) as usize;
    let arcount = u16::from_be_bytes([buf[10], buf[11]]) as usize;

    let total_rr = ancount + nscount + arcount;
    if total_rr == 0 {
        return None;
    }

    let mut pos = 12;
    for _ in 0..qdcount {
        pos = dns_skip_name(buf, pos)?;
        pos = pos.checked_add(4)?;
        if pos > buf.len() {
            return None;
        }
    }

    let mut ptr_instance: Option<String> = None;

    for _ in 0..total_rr {
        if pos >= buf.len() {
            break;
        }
        pos = dns_skip_name(buf, pos)?;
        if pos + 10 > buf.len() {
            break;
        }
        let rtype = u16::from_be_bytes([buf[pos], buf[pos + 1]]);
        let rdlen = u16::from_be_bytes([buf[pos + 8], buf[pos + 9]]) as usize;
        pos += 10;
        let rdata_end = match pos.checked_add(rdlen) {
            Some(e) if e <= buf.len() => e,
            _ => break,
        };

        match rtype {
            16 => {
                if let Some(model) = parse_txt_model(&buf[pos..rdata_end]) {
                    return Some(model);
                }
            }
            12 if ptr_instance.is_none() => {
                if let Some(full) = dns_read_name(buf, pos) {
                    if let Some(label) = full.split('.').next() {
                        let s = label.trim_start_matches('_').to_string();
                        if s.len() > 2 {
                            ptr_instance = Some(s);
                        }
                    }
                }
            }
            _ => {}
        }

        pos = rdata_end;
    }

    ptr_instance
}

pub async fn unicast_mdns_query(ip: &str) -> Option<String> {
    let sock = tokio::net::UdpSocket::bind("0.0.0.0:0").await.ok()?;
    let target: std::net::SocketAddr = format!("{ip}:5353").parse().ok()?;

    let query = build_mdns_device_info_query();
    sock.send_to(&query, target).await.ok()?;

    let mut buf = [0u8; 4096];
    let len = match tokio::time::timeout(Duration::from_secs(1), sock.recv_from(&mut buf)).await {
        Ok(Ok((n, _))) => n,
        _ => return None,
    };

    parse_mdns_identity_response(&buf[..len])
}

pub async fn resolve_hostname_via_ptr(ip: String) -> Option<String> {
    let ip_addr: Ipv4Addr = ip.parse().ok()?;
    let o = ip_addr.octets();
    let dns_server = Ipv4Addr::new(o[0], o[1], o[2], 1);
    get_ptr_name(&ip, dns_server).await
}

/// Runs UPnP XML and unicast identity fetches (NetBIOS + HTTP) concurrently.
async fn run_identity_fetches(
    location_map: HashMap<String, String>,
    all_ips: Vec<String>,
    state: &Arc<Mutex<ScanProgressState>>,
) {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
    {
        Ok(c) => c,
        Err(_) => return,
    };

    let (tx, mut rx) = mpsc::channel::<(String, String)>(64);

    for (ip, url) in location_map {
        let tx = tx.clone();
        let client = client.clone();
        tokio::spawn(async move {
            if let Some(result) = fetch_device_identity(ip, Some(url), &client).await {
                let _ = tx.send(result).await;
            }
        });
    }

    for ip in all_ips {
        if ssdp_name_cache().lock().unwrap().contains_key(&ip) {
            continue;
        }
        let tx1 = tx.clone();
        let ip1 = ip.clone();
        tokio::spawn(async move {
            if let Some(name) = get_netbios_name(&ip1).await {
                let _ = tx1.send((ip1, name)).await;
            }
        });
        let tx2 = tx.clone();
        let client2 = client.clone();
        tokio::spawn(async move {
            if let Some(name) = probe_http_identity(&ip, &client2).await {
                let _ = tx2.send((ip, name)).await;
            }
        });
    }

    drop(tx);

    while let Some((ip, name)) = rx.recv().await {
        ssdp_name_cache()
            .lock()
            .unwrap()
            .entry(ip.clone())
            .or_insert(name);
        // Progress event emission removed — callers get the final result at scan end.
        let _ = state; // suppress unused warning
    }
}

/// Collects IPv4 peers' SERVER: banners and LOCATION: URLs from SSDP responses (~4s window).
pub async fn ssdp_discover(
    local_ip: Ipv4Addr,
) -> (HashMap<String, String>, HashMap<String, String>) {
    let mut out = HashMap::<String, String>::new();
    let mut locations = HashMap::<String, String>::new();

    let s2 = match Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)) {
        Ok(s) => s,
        Err(err) => {
            warn!("SSDP: socket creation failed ({err}) — SSDP disabled (non-fatal)");
            return (out, locations);
        }
    };

    s2.set_reuse_address(true).ok();

    let bind_sa = socket2::SockAddr::from(SocketAddr::V4(SocketAddrV4::new(local_ip, 0)));
    if let Err(err) = s2.bind(&bind_sa) {
        warn!("SSDP: bind {local_ip}:0 failed ({err}) — SSDP disabled (non-fatal)");
        return (out, locations);
    }

    if let Err(err) = s2.set_nonblocking(true) {
        warn!("SSDP: set_nonblocking failed ({err}) — SSDP disabled (non-fatal)");
        return (out, locations);
    }

    if let Err(err) = s2.set_multicast_if_v4(&local_ip) {
        warn!("SSDP: set_multicast_if_v4 failed ({err}) — falling back to default interface");
    }

    let ssdp_group = std::net::Ipv4Addr::new(239, 255, 255, 250);
    if let Err(err) = s2.join_multicast_v4(&ssdp_group, &local_ip) {
        warn!(
            "SSDP: join_multicast_v4 failed ({err}) — \
             NOTIFY unsolicited messages suppressed; M-SEARCH responses unaffected"
        );
    }

    let socket = match UdpSocket::from_std(std::net::UdpSocket::from(s2)) {
        Ok(s) => s,
        Err(err) => {
            warn!("SSDP: UdpSocket::from_std failed ({err}) — SSDP disabled (non-fatal)");
            return (out, locations);
        }
    };

    let dest: SocketAddr = match "239.255.255.250:1900".parse() {
        Ok(a) => a,
        Err(_) => return (out, locations),
    };

    if let Err(err) = socket.send_to(SSDP_MSEARCH.as_bytes(), dest).await {
        warn!(
            "SSDP: M-SEARCH send failed ({err}) — multicast blocked on this network (non-fatal)"
        );
        return (out, locations);
    }
    info!("[FLIGHT_RECORDER] SSDP: M-SEARCH burst 1 sent to {dest}");

    let start = Instant::now();
    let deadline = start + Duration::from_secs(4);
    let mut buf = vec![0u8; 8192];
    let mut second_burst_sent = false;

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }

        if !second_burst_sent && start.elapsed() >= Duration::from_millis(1000) {
            second_burst_sent = true;
            if let Err(err) = socket.send_to(SSDP_MSEARCH.as_bytes(), dest).await {
                warn!("SSDP: M-SEARCH burst 2 failed ({err}) — continuing with burst 1 replies");
            } else {
                info!("[FLIGHT_RECORDER] SSDP: M-SEARCH burst 2 sent");
            }
        }

        let poll_cap = remaining.min(Duration::from_millis(120));
        match tokio::time::timeout(poll_cap, socket.recv_from(&mut buf)).await {
            Ok(Ok((len, addr))) => {
                let IpAddr::V4(v4) = addr.ip() else { continue };
                let peer_ip = v4.to_string();
                let headers = parse_ssdp_headers(&buf[..len]);
                if let Some(server) = headers.server {
                    info!("[FLIGHT_RECORDER] SSDP: {} → SERVER: {}", peer_ip, server);
                    out.entry(peer_ip.clone())
                        .and_modify(|prev| {
                            if server.len() > prev.len() {
                                *prev = server.clone();
                            }
                        })
                        .or_insert(server);
                }
                if let Some(loc) = headers.location {
                    locations.entry(peer_ip).or_insert(loc);
                }
            }
            Ok(Err(err)) => {
                warn!("SSDP: recv_from error: {err}");
                break;
            }
            Err(_) => {}
        }
    }

    info!(
        "[FLIGHT_RECORDER] SSDP: discover complete — {} devices responded",
        out.len()
    );
    (out, locations)
}

fn truncate_ssdp_vendor_banner(server: &str) -> String {
    server.trim().chars().take(120).collect()
}

fn ssdp_friendly_device_title(server: &str) -> Option<String> {
    let t = server.trim();
    let upper = t.to_ascii_uppercase();
    if let Some(idx) = upper.find("UPNP/") {
        let tail = t[idx + 5..].trim_start();
        let mut parts = tail.split_whitespace();
        let mut tok = parts.next()?;
        if tok.chars().all(|c| c.is_ascii_digit() || c == '.') {
            tok = parts.next()?;
        }
        let candidate = tok;
        if (3..=48).contains(&candidate.len()) && candidate.contains('/') {
            return Some(candidate.to_string());
        }
    }

    let first = t.split_whitespace().next()?;
    if !(3..=48).contains(&first.len()) || !first.contains('/') {
        return None;
    }
    let lower = first.to_ascii_lowercase();
    if lower.starts_with("linux/") || lower.starts_with("unix/") || lower.starts_with("windows/") {
        return None;
    }
    Some(first.to_string())
}

fn apply_ssdp_fingerprint(d: &mut DiscoveredDevice, ssdp: &HashMap<String, String>) {
    {
        let cache = ssdp_name_cache().lock().unwrap();
        if let Some(friendly) = cache.get(&d.ip) {
            if !friendly.trim().is_empty() && d.mdns_hostname.is_none() {
                let n = d.name.trim();
                if is_generic_hostname(n, &d.ip) || n == d.ip || n == d.vendor_name.trim() {
                    d.name = friendly.clone();
                }
                return;
            }
        }
    }

    let Some(server) = ssdp.get(&d.ip) else {
        return;
    };
    let server = server.trim();
    if server.is_empty() {
        return;
    }

    d.ssdp_server = Some(server.chars().take(120).collect());

    if d.vendor_name.trim().eq_ignore_ascii_case("unknown") {
        let label = truncate_ssdp_vendor_banner(server);
        if let Some(corporate) = identify_corporate_vendor("", &label) {
            d.vendor_name = corporate.clone();
            d.vendor = corporate;
        } else {
            d.vendor_name = label.clone();
            d.vendor = label;
        }
    }

    if d.mdns_hostname.is_some() {
        return;
    }

    let n = d.name.trim();
    if !(is_generic_hostname(n, &d.ip) || n == d.ip) {
        return;
    }

    if let Some(title) = ssdp_friendly_device_title(server) {
        d.name = title;
    }
}

#[derive(Default)]
struct ScanProgressState {
    mdns: HashMap<String, MdnsHostInfo>,
    ping: HashMap<String, (DiscoveredDevice, f64)>,
    ssdp: HashMap<String, String>,
    scanned_hosts: usize,
    total_hosts: usize,
}

fn discover_mdns_background_thread(
    timeout: Duration,
    state: Arc<Mutex<ScanProgressState>>,
    notify: mpsc::Sender<()>,
) {
    let mdns = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(ServiceDaemon::new)) {
        Ok(Ok(daemon)) => daemon,
        Ok(Err(err)) => {
            warn!("mDNS daemon unavailable: {err}");
            return;
        }
        Err(_) => {
            warn!("mDNS daemon panicked during init; skipping mDNS discovery");
            return;
        }
    };

    let known_types: HashSet<&str> = MDNS_BROWSE.iter().map(|(t, _)| *t).collect();
    let mut receivers: Vec<(String, MdnsReceiver<ServiceEvent>)> = Vec::new();
    for (service_ty, tag) in MDNS_BROWSE {
        match mdns.browse(service_ty) {
            Ok(rx) => receivers.push((tag.to_string(), rx)),
            Err(err) => warn!("mDNS browse {service_ty} skipped: {err}"),
        }
    }

    let meta_rx = mdns.browse("_services._dns-sd._udp.local.").ok();
    let mut dynamic_browsed: HashSet<String> = HashSet::new();

    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some(ref meta) = meta_rx {
            while let Ok(event) = meta.try_recv() {
                if let ServiceEvent::ServiceFound(_type_name, fullname) = event {
                    let svc_type = if fullname.ends_with('.') {
                        fullname.clone()
                    } else {
                        format!("{fullname}.")
                    };
                    if known_types.contains(svc_type.as_str()) {
                        continue;
                    }
                    if dynamic_browsed.contains(&svc_type) {
                        continue;
                    }
                    dynamic_browsed.insert(svc_type.clone());
                    let tag = svc_type
                        .trim_end_matches('.')
                        .trim_start_matches('_')
                        .split("._")
                        .next()
                        .unwrap_or("dynamic")
                        .to_string();
                    match mdns.browse(&svc_type) {
                        Ok(rx) => receivers.push((tag, rx)),
                        Err(err) => warn!("mDNS dynamic browse {svc_type} skipped: {err}"),
                    }
                }
            }
        }

        for (tag, receiver) in &receivers {
            while let Ok(event) = receiver.try_recv() {
                if let ServiceEvent::ServiceResolved(info) = event {
                    let host = info.get_hostname().to_string();
                    let instance_name = info
                        .get_fullname()
                        .split('.')
                        .next()
                        .unwrap_or("")
                        .to_string();
                    let mut changed = false;
                    for scoped in info.get_addresses().iter() {
                        let ip = scoped.to_ip_addr();
                        if let IpAddr::V4(v4) = ip {
                            let ip_s = v4.to_string();
                            let mut g = state.lock().unwrap();
                            let before = g.mdns.get(&ip_s).cloned();
                            merge_mdns_record(&mut g.mdns, ip_s.clone(), &host, tag);
                            let human_host = humanize_mdns_hostname(&host);
                            if is_generic_hostname(&human_host, &ip_s) && !instance_name.is_empty()
                            {
                                merge_mdns_record(&mut g.mdns, ip_s.clone(), &instance_name, tag);
                            }
                            let after = g.mdns.get(&ip_s).cloned();
                            drop(g);
                            persist_mdns_discovery(&ip_s, &host, tag);
                            if before != after {
                                changed = true;
                            }
                        }
                    }
                    if changed {
                        let _ = notify.blocking_send(());
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(18));
    }

    drop(receivers);
    if let Ok(shutdown_rx) = mdns.shutdown() {
        let _ = shutdown_rx.recv();
    }
}

fn sync_mdns_fields_from_map(
    mdns_map: &HashMap<String, MdnsHostInfo>,
    mut device: DiscoveredDevice,
) -> DiscoveredDevice {
    if let Some(info) = mdns_map.get(&device.ip) {
        if let Some(raw) = info.hostname.as_ref() {
            let h = humanize_mdns_hostname(raw);
            if !h.is_empty() {
                device.mdns_hostname = Some(h);
            }
        }
        if device.mdns_primary_service.is_none() {
            device.mdns_primary_service = pick_mdns_primary_service(&info.services);
        }
    }
    device
}

fn apply_mdns_hostname_as_primary_title(d: &mut DiscoveredDevice) {
    if let Some(ref raw) = d.mdns_hostname {
        let h = humanize_mdns_hostname(raw);
        if !h.is_empty() {
            d.mdns_hostname = Some(h.clone());
            d.name = h;
        }
    }
}

fn apply_oui_manufacturer_when_title_is_host_prefix(d: &mut DiscoveredDevice) {
    let t = d.name.trim();
    if !t.to_ascii_lowercase().starts_with("host ") {
        return;
    }
    if d.vendor_name != "Unknown" && !d.vendor_name.trim().is_empty() {
        d.name = d.vendor_name.clone();
    }
}

fn apply_aggressive_oui_display_name(d: &mut DiscoveredDevice) {
    if d.mdns_hostname.is_some() {
        return;
    }
    if d.vendor_name == "Unknown" || d.vendor_name.trim().is_empty() {
        return;
    }
    let n = d.name.trim();
    if n == d.ip || n.to_lowercase() == "unknown" || is_generic_hostname(n, &d.ip) {
        d.name = d.vendor_name.clone();
    }
}

fn mdns_stub_device(ip: &str, info: &MdnsHostInfo) -> Option<DiscoveredDevice> {
    let hostname = info.hostname.as_ref()?.trim();
    if hostname.is_empty() {
        return None;
    }
    let human = humanize_mdns_hostname(hostname);
    if human.is_empty() {
        return None;
    }
    let mut vendor_name = "Unknown".to_string();
    if let Some(corporate) = identify_corporate_vendor(&human, "") {
        vendor_name = corporate;
    }
    let vendor = vendor_name.clone();
    let mdns_services = Some(&info.services);
    let device_type = infer_device_type(&human, &vendor, mdns_services);
    let mdns_primary_opt = pick_mdns_primary_service(&info.services);
    Some(DiscoveredDevice {
        status: "Online".to_string(),
        name: human.clone(),
        ip: ip.to_string(),
        mac: "Unknown".to_string(),
        vendor,
        vendor_name,
        device_type,
        is_randomized: false,
        mdns_hostname: Some(human),
        mdns_primary_service: mdns_primary_opt,
        likely_type: None,
        hostname: None,
        ssdp_server: None,
        latency_ms: None,
        open_ports: None,
        suggested_names: None,
    })
}

fn build_scan_payload(
    state: &ScanProgressState,
) -> (Vec<DiscoveredDevice>, Option<f64>, usize) {
    let mut devices: Vec<DiscoveredDevice> = state
        .ping
        .iter()
        .map(|(_ip, (d, lat))| {
            let mut dev = sync_mdns_fields_from_map(&state.mdns, d.clone());
            dev.latency_ms = Some(*lat);
            dev
        })
        .collect();
    for (ip, info) in &state.mdns {
        if state.ping.contains_key(ip) {
            continue;
        }
        if let Some(stub) = mdns_stub_device(ip, info) {
            devices.push(stub);
        }
    }

    for d in &mut devices {
        apply_mdns_hostname_as_primary_title(d);
        let services = state.mdns.get(&d.ip).map(|i| &i.services);
        if d.mdns_hostname.is_some() {
            let new_type = infer_device_type(&d.name, &d.vendor, services);
            d.device_type = new_type;
        }

        apply_ssdp_fingerprint(d, &state.ssdp);

        if d.mdns_hostname.is_none() && is_generic_hostname(d.name.trim(), &d.ip) {
            if let Some(rdns_name) = reverse_dns_name_for_ip(&d.ip) {
                if !is_generic_hostname(&rdns_name, &d.ip) {
                    d.name = rdns_name.clone();
                    if d.hostname.is_none() {
                        d.hostname = Some(rdns_name);
                    }
                }
            }
        }

        apply_oui_manufacturer_when_title_is_host_prefix(d);
        apply_aggressive_oui_display_name(d);
    }

    devices.sort_by(|a, b| a.ip.cmp(&b.ip));
    let average_latency_ms = if state.ping.is_empty() {
        None
    } else {
        Some(state.ping.values().map(|(_, ms)| *ms).sum::<f64>() / state.ping.len() as f64)
    };
    (
        devices,
        average_latency_ms,
        state.scanned_hosts,
    )
}

fn emit_scan_progress(
    tx: &mpsc::UnboundedSender<ScanEvent>,
    state: &Arc<Mutex<ScanProgressState>>,
) {
    let snapshot = {
        let g = state.lock().unwrap();
        build_scan_payload(&g)
    };
    let payload = ScanNetworkPayload {
        devices: snapshot.0,
        average_latency_ms: snapshot.1,
        scanned_hosts: snapshot.2,
        scan_id: String::new(),
        batch_seq: 0,
    };
    let _ = tx.send(ScanEvent::Progress(payload));
}

async fn probe_host(
    ip: Ipv4Addr,
    state: Arc<Mutex<ScanProgressState>>,
    profile: ScanProfile,
) -> Option<(String, DiscoveredDevice, f64)> {
    let ip_string = ip.to_string();
    if scan_cancelled() {
        return None;
    }

    // --- Phase 1: Passive / Cache identification (PRIORITY) ---
    let mac_opt = get_mac_address(&ip_string).await;
    let mac = mac_opt.clone().unwrap_or_else(|| "Unknown".to_string());

    let mdns_info = {
        let g = state.lock().unwrap();
        g.mdns.get(&ip_string).cloned()
    };
    let mdns_hostname_cached = mdns_info.as_ref().and_then(|m| m.hostname.clone());
    let mdns_str = mdns_hostname_cached.as_deref().unwrap_or("");

    let registry_match = get_registry().match_device(&mac, mdns_str);
    if let Some(ref p) = registry_match {
        info!("[REGISTRY] Pre-probe match for {}: {} ({})", ip_string, p.name, p.device_type);
    }

    info!("[FLIGHT_RECORDER] Probing IP: {:?} | Method: TCP+ICMP", ip);
    let probe_result = tokio::time::timeout(profile.host_probe_timeout, ping::ping_host_latency_ms(ip))
        .await
        .ok()
        .flatten();

    if probe_result.is_none() {
        let mdns_identity = if mdns_hostname_cached.is_none() {
            tokio::time::timeout(Duration::from_millis(350), unicast_mdns_query(&ip_string))
                .await
                .ok()
                .flatten()
        } else {
            mdns_hostname_cached.clone()
        };

        if mac_opt.is_none() && mdns_identity.is_none() && registry_match.is_none() {
            return None;
        }

        let mac = mac_opt.unwrap_or_else(|| "Unknown".to_string());
        let mdns_label = mdns_identity;
        let mdns_str = mdns_label.as_deref().unwrap_or("");
        
        let mut vendor_name;
        let device_type;
        let resolved_name;

        if let Some(p) = registry_match.or_else(|| get_registry().match_device(&mac, mdns_str)) {
            vendor_name = p.vendor;
            device_type = p.device_type;
            resolved_name = p.name;
        } else {
            vendor_name = vendor_name_from_mac(&mac);
            if vendor_name.eq_ignore_ascii_case("unknown") {
                if let Some(corporate) = identify_corporate_vendor(mdns_str, "") {
                    vendor_name = corporate;
                }
            }
            let vendor = vendor_name.clone();
            let label_for_infer = if mdns_str.is_empty() { &vendor } else { mdns_str };
            device_type = infer_device_type(label_for_infer, &vendor, None);
            resolved_name = infer_device_name_with_type(label_for_infer, &vendor, &ip_string, &device_type);
        }

        let is_randomized = is_randomized_mac(&mac);
        log::info!("[SCAN] Stealth device found via ARP/mDNS/Registry: {} [{}] vendor={}", ip_string, mac, vendor_name);

        return Some((
            ip_string.clone(),
            DiscoveredDevice {
                status: "Online".to_string(),
                name: resolved_name,
                ip: ip_string,
                mac,
                vendor: vendor_name.clone(),
                vendor_name,
                device_type,
                is_randomized,
                mdns_hostname: mdns_label,
                mdns_primary_service: Some("stealth".to_string()),
                likely_type: None,
                hostname: None,
                ssdp_server: None,
                latency_ms: Some(1.0),
                open_ports: None,
                suggested_names: None,
            },
            1.0,
        ));
    }

    let probe = probe_result.unwrap();
    let latency_ms = probe.latency_ms;
    let tcp_ports = probe.tcp_ports;

    let (raw_rdns, ptr_hostname) = tokio::join!(network::reverse_dns(ip), async {
        tokio::time::timeout(
            Duration::from_millis(400),
            resolve_hostname_via_ptr(ip_string.clone()),
        )
        .await
        .unwrap_or(None)
    },);

    let api_hostname = reverse_hostname_field(&ip_string, raw_rdns.clone(), ptr_hostname.clone());

    if let Some(ref name) = raw_rdns {
        rdns_cache()
            .lock()
            .unwrap()
            .entry(ip_string.clone())
            .or_insert_with(|| sanitize_rdns_hostname(name));
    }
    let reverse_name = raw_rdns.unwrap_or_else(|| format!("Host {}", ip));

    let vendor_name_oui = vendor_name_from_mac(&mac);
    log::info!("[SCAN] Host {} [{}] vendor={}", ip_string, mac, vendor_name_oui);

    let (mdns_hostname_opt, mdns_primary_opt, mdns_services_for_infer) = {
        let g = state.lock().unwrap();
        let mdns_opt = g.mdns.get(&ip_string);
        (
            mdns_opt.and_then(|m| m.hostname.clone()),
            mdns_opt.and_then(|m| pick_mdns_primary_service(&m.services)),
            mdns_opt.map(|m| m.services.clone()),
        )
    };

    let effective_mdns_hostname = mdns_hostname_opt.or(ptr_hostname);
    let mdns_str = effective_mdns_hostname.as_deref().unwrap_or("");

    // Re-check registry if we have new mDNS info
    let final_registry_match = registry_match.or_else(|| get_registry().match_device(&mac, mdns_str));

    let mut vendor_name;
    let device_type;
    let resolved_name;

    if let Some(ref p) = final_registry_match {
        vendor_name = p.vendor.clone();
        device_type = p.device_type.clone();
        resolved_name = p.name.clone();
    } else {
        let v_tmp = vendor_name_oui.clone();
        let label_for_infer = if mdns_str.is_empty() { reverse_name.as_str() } else { mdns_str };
        device_type = infer_device_type(label_for_infer, &vendor_name_oui, mdns_services_for_infer.as_ref());
        resolved_name = match &effective_mdns_hostname {
            Some(h) if !is_generic_hostname(h, &ip_string) => h.clone(),
            Some(h) => infer_device_name_with_type(h, &vendor_name_oui, &ip_string, &device_type),
            None => infer_device_name_with_type(&reverse_name, &vendor_name_oui, &ip_string, &device_type),
        };
        vendor_name = v_tmp;
    }

    let is_randomized = is_randomized_mac(&mac);

    // Skip aggressive probes if matched via registry
    let mut effective_open_ports: Vec<u16> = tcp_ports;
    if final_registry_match.is_none() {
        const TELL_TALE_PORTS: &[u16] = &[80, 443, 515, 554, 631, 3689, 8008, 8060, 9100];
        if effective_open_ports.is_empty() {
            let ip_addr: std::net::IpAddr = ip_string.parse().ok().unwrap_or(IpAddr::V4(ip));
            let probes: Vec<_> = TELL_TALE_PORTS
                .iter()
                .copied()
                .map(|port| {
                    let addr = SocketAddr::new(ip_addr, port);
                    async move {
                        match tokio::time::timeout(
                            Duration::from_millis(200),
                            tokio::net::TcpStream::connect(addr),
                        )
                        .await
                        {
                            Ok(Ok(_)) => Some(port),
                            _ => None,
                        }
                    }
                })
                .collect();
            effective_open_ports = futures::future::join_all(probes)
                .await
                .into_iter()
                .flatten()
                .collect();
        }
    }

    let port_label = identify_by_ports(&effective_open_ports);

    const GENERIC_PORT_LABELS: &[&str] = &[
        "", "HTTP Device", "HTTPS Device", "HTTP Admin Panel", "Network Device (Web Interface)", "Network Device",
    ];
    
    let mut http_server_banner = None;
    let mut http_hints = None;

    if final_registry_match.is_none() {
        let http_port = if GENERIC_PORT_LABELS.contains(&port_label.as_str()) {
            if effective_open_ports.contains(&80) {
                Some(80u16)
            } else if effective_open_ports.contains(&8080) {
                Some(8080u16)
            } else if effective_open_ports.contains(&443) {
                Some(443u16)
            } else {
                None
            }
        } else {
            None
        };

        if let Some(port) = http_port {
            let banner = quick_http_server_header(&ip_string, port).await;
            let hints = fingerprints::classify_from_http_artifacts(
                banner.as_deref(),
                None,
                "",
                &effective_open_ports,
            );
            http_server_banner = banner;
            http_hints = Some(hints);
        }
    }

    // --- Automated Vendor Overrides ---
    if final_registry_match.is_none() && vendor_name.eq_ignore_ascii_case("unknown") {
        if let Some(corporate) = identify_corporate_vendor(
            mdns_str,
            http_server_banner.as_deref().unwrap_or(""),
        ) {
            vendor_name = corporate;
        }
    }

    // --- XML Model Extraction (Google Cast / SSDP) ---
    let mut xml_model_name = None;
    if final_registry_match.is_none() && (effective_open_ports.contains(&8008) || effective_open_ports.contains(&8060)) {
        let port = if effective_open_ports.contains(&8008) { 8008 } else { 8060 };
        let ip_cp = ip_string.clone();
        
        xml_model_name = match tokio::time::timeout(
            Duration::from_millis(3200),
            tokio::spawn(async move {
                probe_model_xml(&ip_cp, port).await
            })
        ).await {
            Ok(Ok(res)) => res,
            _ => None,
        };
    }

    let mut final_display_name = resolved_name;
    if let Some(model) = xml_model_name {
        final_display_name = model;
    }

    let quick_likely_type = if final_registry_match.is_some() {
        None
    } else {
        let merged = merge_port_and_http_likely(
            &port_label,
            http_hints.as_ref().and_then(|h| h.likely.as_deref()),
        );
        let from_ports = if merged.is_empty() { None } else { Some(merged) };

        const GENERIC_TYPES: &[&str] = &["Network Device", "Router / Gateway"];
        let is_specific = |s: &str| !s.is_empty() && !GENERIC_TYPES.contains(&s);

        if from_ports.as_deref().map(is_specific).unwrap_or(false) {
            from_ports
        } else {
            let by_hostname = [
                effective_mdns_hostname.as_deref(),
                api_hostname.as_deref(),
                Some(final_display_name.as_str()),
            ]
            .into_iter()
            .flatten()
            .find_map(classify_from_hostname);

            by_hostname
                .or_else(|| classify_from_vendor(&vendor_name, &effective_open_ports, from_ports.as_deref().unwrap_or("")))
                .or(from_ports)
        }
    };

    let open_ports_str = if effective_open_ports.is_empty() {
        None
    } else {
        Some(
            effective_open_ports
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(","),
        )
    };

    Some((
        ip_string.clone(),
        DiscoveredDevice {
            status: "Online".to_string(),
            name: final_display_name,
            ip: ip_string,
            mac,
            vendor: vendor_name.clone(),
            vendor_name,
            device_type,
            is_randomized,
            mdns_hostname: effective_mdns_hostname,
            mdns_primary_service: mdns_primary_opt,
            likely_type: quick_likely_type,
            hostname: api_hostname,
            ssdp_server: http_server_banner,
            latency_ms: Some(latency_ms),
            open_ports: open_ports_str,
            suggested_names: None,
        },
        latency_ms,
    ))
}

async fn probe_model_xml(ip: &str, port: u16) -> Option<String> {
    let _permit = xml_probe_sem().acquire().await.ok();
    
    let path = if port == 8008 { "/ssdescription.xml" } else { "/device-desc.xml" };
    let url = format!("http://{}:{}{}", ip, port, path);
    
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_millis(1500))
        .danger_accept_invalid_certs(true)
        .build()
        .ok()?;
    
    let result = tokio::time::timeout(Duration::from_millis(3000), async {
        let resp = match tokio::time::timeout(
            Duration::from_millis(1500),
            client.get(&url).send()
        ).await {
            Ok(Ok(r)) => r,
            _ => return None,
        };
        
        if !resp.status().is_success() {
            return None;
        }

        let body = match tokio::time::timeout(
            Duration::from_millis(1500),
            resp.text()
        ).await {
            Ok(Ok(b)) => b,
            _ => return None,
        };
        
        extract_xml_tag(&body, "friendlyName")
            .or_else(|| extract_xml_tag(&body, "modelName"))
    }).await.ok().flatten();

    result
}

async fn quick_http_server_header(ip: &str, port: u16) -> Option<String> {
    let scheme = if port == 443 { "https" } else { "http" };
    let url = format!("{scheme}://{ip}:{port}/");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(300))
        .connect_timeout(Duration::from_millis(200))
        .danger_accept_invalid_certs(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .ok()?;
    let resp = tokio::time::timeout(
        Duration::from_millis(300),
        client.head(&url).send(),
    )
    .await
    .ok()?
    .ok()?;
    resp.headers()
        .get("server")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().chars().take(80).collect())
}

async fn run_with_guard(
    db: Option<AppDb>,
    shared_devices: Option<Arc<Mutex<Vec<DiscoveredDevice>>>>,
    guard: ScanGuard,
    app: EventSink,
    mode: ScanMode,
    scan_id: String,
) -> Result<ScanNetworkResult, String> {
    let state = Arc::new(Mutex::new(ScanProgressState::default()));
    let state_for_timeout = Arc::clone(&state);
    let outcome = match tokio::time::timeout(
        SCAN_LOCAL_NETWORK_TIMEOUT,
        scan_local_network_inner(db, shared_devices.clone(), app, state, mode, scan_id),
    )
    .await
    {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(err)) => {
            warn!("scan_local_network failed: {err}");
            Err(err)
        }
        Err(_) => {
            warn!(
                "scan_local_network: timed out after {:?} — returning partial results",
                SCAN_LOCAL_NETWORK_TIMEOUT
            );
            let (devices, average_latency_ms, scanned_hosts) = {
                let g = state_for_timeout.lock().unwrap();
                build_scan_payload(&g)
            };
            Ok(ScanNetworkResult {
                devices,
                average_latency_ms,
                scanned_hosts,
            })
        }
    };

    if let Ok(ref result) = outcome {
        if let Some(ref sd) = shared_devices {
            let mut d_lock = sd.lock().unwrap();
            *d_lock = result.devices.clone();
            info!(
                "[SCAN] Shared App State explicitly updated with {} devices.",
                result.devices.len()
            );
        }
    }

    drop(guard);
    outcome
}

pub async fn scan_local_network_pre_guarded(
    db: Option<AppDb>,
    shared_devices: Option<Arc<Mutex<Vec<DiscoveredDevice>>>>,
    guard: ScanGuard,
    app: EventSink,
    mode: ScanMode,
    scan_id: String,
) -> Result<ScanNetworkResult, String> {
    info!("[SCAN_LIFECYCLE] scan_local_network_pre_guarded — discovery starting | scan_id={scan_id}");
    run_with_guard(db, shared_devices, guard, app, mode, scan_id).await
}

fn empty_scan_result() -> ScanNetworkResult {
    ScanNetworkResult {
        devices: Vec::new(),
        average_latency_ms: None,
        scanned_hosts: 0,
    }
}

fn send_broadcast_nudge(local_network: &network::LocalNetwork) {
    let broadcast_ip = local_network.cidr.broadcast();
    let payload = b"shabakat-broadcast-nudge";

    let bind_addr = SocketAddrV4::new(local_network.interface_ip, 0);
    let directed_dest = SocketAddrV4::new(broadcast_ip, 9);

    let directed_ok = (|| -> bool {
        let Ok(sock) = std::net::UdpSocket::bind(bind_addr) else {
            debug!("[SCAN_TRACE] Broadcast nudge: bind to {} failed", bind_addr);
            return false;
        };
        if sock.set_broadcast(true).is_err() {
            return false;
        }
        match sock.send_to(payload, directed_dest) {
            Ok(_) => {
                info!(
                    "[SCAN_TRACE] Broadcast nudge → {} (directed)",
                    directed_dest
                );
                true
            }
            Err(err) => {
                info!(
                    "[SCAN_TRACE] Directed broadcast to {} denied ({err}) \
                     — falling back to 255.255.255.255",
                    directed_dest
                );
                false
            }
        }
    })();

    if directed_ok {
        return;
    }

    let limited_dest = SocketAddrV4::new(Ipv4Addr::new(255, 255, 255, 255), 9);
    let Ok(sock2) = std::net::UdpSocket::bind("0.0.0.0:0") else {
        debug!("[SCAN_TRACE] Broadcast nudge: fallback bind(0.0.0.0:0) failed");
        return;
    };
    if sock2.set_broadcast(true).is_err() {
        debug!("[SCAN_TRACE] Broadcast nudge: fallback set_broadcast failed");
        return;
    }
    match sock2.send_to(payload, limited_dest) {
        Ok(_) => debug!("[SCAN_TRACE] Broadcast nudge → 255.255.255.255 (limited)"),
        Err(err) => debug!("[SCAN_TRACE] Broadcast nudge: limited broadcast failed: {err}"),
    }
}

async fn scan_local_network_inner(
    _db: Option<AppDb>,
    shared_devices: Option<Arc<Mutex<Vec<DiscoveredDevice>>>>,
    app: EventSink,
    state: Arc<Mutex<ScanProgressState>>,
    mode: ScanMode,
    scan_id: String,
) -> Result<ScanNetworkResult, String> {
    debug!("[SCAN_TRACE] scan_local_network_inner: entry");

    if let Ok(ifaces) = get_if_addrs::get_if_addrs() {
        for iface in ifaces {
            debug!("[SCAN_DEBUG] Available System Interface: name={}, addr={:?}", iface.name, iface.addr);
        }
    }

    tokio::task::spawn_blocking(init_vendor_map)
        .await
        .map_err(|join_err| format!("vendor map init panic: {join_err}"))??;

    debug!("[SCAN_TRACE] Fetching local IP/Subnet...");

    const IFACE_RETRY_ATTEMPTS: u32 = 4;
    const IFACE_RETRY_DELAY: Duration = Duration::from_millis(500);

    let local_network = {
        let mut resolved: Option<network::LocalNetwork> = None;

        for attempt in 0..IFACE_RETRY_ATTEMPTS {
            info!(
                "[SCAN_TRACE] Iface resolution attempt {}/{}",
                attempt + 1,
                IFACE_RETRY_ATTEMPTS
            );

            if let Some((ip, prefix)) = get_best_local_network() {
                if let Ok(cidr) = Ipv4Net::new(ip, prefix) {
                    let ln = network::LocalNetwork {
                        interface_ip: ip,
                        cidr,
                    };
                    info!(
                        "[SCAN_TRACE] getifaddrs OK on attempt {} → {}/{}",
                        attempt + 1,
                        ln.interface_ip,
                        ln.cidr.prefix_len(),
                    );
                    resolved = Some(ln);
                    break;
                }
            } else {
                info!(
                    "[SCAN_TRACE] getifaddrs returned None on attempt {}",
                    attempt + 1
                );
            }

            match network::local_ipv4_network().await {
                Ok(n) => {
                    info!(
                        "[SCAN_TRACE] Netlink OK on attempt {} → {}/{}",
                        attempt + 1,
                        n.interface_ip,
                        n.cidr.prefix_len(),
                    );
                    resolved = Some(n);
                    break;
                }
                Err(primary) => {
                    info!(
                        "[SCAN_TRACE] Netlink failed on attempt {}: {}",
                        attempt + 1,
                        primary
                    );
                    match network::fallback_ipv4_network_via_udp().await {
                        Ok(n) => {
                            info!(
                                "[SCAN_TRACE] UDP fallback OK on attempt {} → {}/{}",
                                attempt + 1,
                                n.interface_ip,
                                n.cidr.prefix_len(),
                            );
                            resolved = Some(n);
                            break;
                        }
                        Err(fb) => {
                            info!(
                                "[SCAN_TRACE] ERROR: All 3 iface methods failed on attempt {}/{} \
                                 (getifaddrs=None, Netlink={primary}, UDP={fb}); \
                                 sleeping {:?} before retry",
                                attempt + 1,
                                IFACE_RETRY_ATTEMPTS,
                                IFACE_RETRY_DELAY,
                            );
                            if attempt + 1 < IFACE_RETRY_ATTEMPTS {
                                tokio::time::sleep(IFACE_RETRY_DELAY).await;
                            }
                        }
                    }
                }
            }
        }

        match resolved {
            Some(ln) => {
                info!(
                    "[SCAN_TRACE] Local IP/Subnet resolved → {}/{}",
                    ln.interface_ip,
                    ln.cidr.prefix_len(),
                );
                ln
            }
            None => {
                info!(
                    "[SCAN_TRACE] ERROR: Could not determine local IP/Subnet after {} attempts — \
                     returning empty device list",
                    IFACE_RETRY_ATTEMPTS,
                );
                return Ok(empty_scan_result());
            }
        }
    };

    let interface_ip = local_network.interface_ip;
    let subnet_prefix = local_network.cidr.prefix_len();
    let profile = profile_for_mode(mode);

    if profile.use_broadcast_nudge {
        send_broadcast_nudge(&local_network);
    }

    let scan_hosts: Vec<Ipv4Addr> = network::host_ips(&local_network.cidr)
        .into_iter()
        .filter(|ip| *ip != interface_ip)
        .collect();
    {
        let mut g = state.lock().unwrap();
        g.total_hosts = scan_hosts.len();
        g.scanned_hosts = 0;
    }

    let o = interface_ip.octets();
    let gateway_str = format!("{}.{}.{}.1", o[0], o[1], o[2]);
    let router_handle =
        tokio::spawn(async move { router_api::interrogate_router(&gateway_str).await });

    let ssdp_handle = tokio::spawn(ssdp_discover(interface_ip));

    let mdns_channel_cap: usize = 128;
    let mdns_window = Duration::from_secs(5);
    let (mdns_notify_tx, mut mdns_notify_rx) = mpsc::channel::<()>(mdns_channel_cap);

    {
        let mut g = state.lock().unwrap();
        merge_persistent_mdns_cache(&mut g.mdns);
    }

    let state_mdns = Arc::clone(&state);
    let mdns_tx_clone = mdns_notify_tx.clone();
    let _mdns_thread = thread::spawn(move || {
        discover_mdns_background_thread(mdns_window, state_mdns, mdns_tx_clone);
    });
    drop(mdns_notify_tx);

    let host_concurrency = concurrency_for_subnet(mode, scan_hosts.len(), profile.host_concurrency);
    info!(
        "[SCAN_TRACE] Starting ping loop — {} hosts on subnet {} (/{}) concurrency {}, mode={}",
        scan_hosts.len(),
        local_network.cidr,
        subnet_prefix,
        host_concurrency,
        match mode {
            ScanMode::Silent => "silent",
            ScanMode::Aggressive => "aggressive",
            ScanMode::Deep => "deep",
        },
    );
    let (ping_tx, mut ping_rx) =
        mpsc::channel::<Option<(String, DiscoveredDevice, f64)>>(host_concurrency.max(32));
    let state_ping = Arc::clone(&state);
    let hosts_for_ping = scan_hosts.clone();
    let probe_count = Arc::new(AtomicUsize::new(0));
    tokio::spawn(async move {
        stream::iter(hosts_for_ping)
            .map(|ip| {
                let st = Arc::clone(&state_ping);
                let pc = Arc::clone(&probe_count);
                tokio::spawn(async move {
                    let count = pc.fetch_add(1, Ordering::SeqCst);
                    if count < 3 {
                        debug!("[SCAN_DEBUG] Firing probe at target IP: {}", ip);
                    }
                    probe_host(ip, st, profile).await
                })
            })
            .buffer_unordered(host_concurrency)
            .for_each(|row| {
                let tx = ping_tx.clone();
                async move {
                    let _ = tx.send(row.ok().flatten()).await;
                }
            })
            .await;
    });

    let mut mdns_done = false;
    let mut ping_done = scan_hosts.is_empty();

    const BATCH_SIZE: usize = 10;
    const BATCH_FLUSH_INTERVAL: Duration = Duration::from_millis(300);
    let mut device_batch: Vec<DiscoveredDevice> = Vec::with_capacity(BATCH_SIZE);
    let mut batch_scanned: usize = 0;
    let mut batch_seq: u32 = 0;
    let mut batch_last_flush = Instant::now();

    macro_rules! flush_batch {
        ($tx:expr) => {
            if !device_batch.is_empty() {
                batch_seq += 1;
                let payload = ScanNetworkPayload {
                    devices: std::mem::take(&mut device_batch),
                    average_latency_ms: None,
                    scanned_hosts: batch_scanned,
                    scan_id: scan_id.clone(),
                    batch_seq,
                };
                let _ = $tx.send(ScanEvent::DeviceDiscovered(payload));
                #[allow(unused_assignments)]
                {
                    batch_last_flush = Instant::now();
                }
            }
        };
    }

    loop {
        if scan_cancelled() {
            debug!("[SCAN_TRACE] scan cancelled — returning partial results");
            if let Some(ref tx) = app {
                flush_batch!(tx);
            }
            break;
        }
        if mdns_done && ping_done {
            break;
        }

        tokio::select! {
            mdns_tick = mdns_notify_rx.recv(), if !mdns_done => {
                match mdns_tick {
                    Some(()) => {
                        if let Some(ref tx) = app {
                            emit_scan_progress(tx, &state);
                        }
                    }
                    None => {
                        mdns_done = true;
                        if let Some(ref tx) = app {
                            flush_batch!(tx);
                        }
                    }
                }
            }
            ping_row = ping_rx.recv(), if !ping_done => {
                match ping_row {
                    Some(Some((ip, dev, lat))) => {
                        info!(
                            "[FLIGHT_RECORDER] Device Found! IP: {:?} | MAC: {:?}",
                            ip,
                            dev.mac
                        );
                        let discovered_device = dev.clone();

                        let scanned_hosts = {
                            let mut g = state.lock().unwrap();
                            g.scanned_hosts = g.scanned_hosts.saturating_add(1);
                            g.ping.insert(ip, (dev, lat));
                            g.scanned_hosts
                        };

                        batch_scanned = scanned_hosts;
                        device_batch.push(discovered_device);

                        if device_batch.len() >= BATCH_SIZE
                            || batch_last_flush.elapsed() >= BATCH_FLUSH_INTERVAL
                        {
                            if let Some(ref tx) = app {
                                flush_batch!(tx);
                            }
                        }
                    }
                    Some(None) => {
                        let scanned_hosts = {
                            let mut g = state.lock().unwrap();
                            g.scanned_hosts = g.scanned_hosts.saturating_add(1);
                            g.scanned_hosts
                        };
                        batch_scanned = scanned_hosts;

                        if batch_last_flush.elapsed() >= BATCH_FLUSH_INTERVAL {
                            if let Some(ref tx) = app {
                                flush_batch!(tx);
                            }
                        }
                    }
                    None => {
                        ping_done = true;
                        if let Some(ref tx) = app {
                            flush_batch!(tx);
                        }
                    }
                }
            }
        }
    }

    // ── ARP table harvest (the "Fing trick") ─────────────────────────────────
    {
        let mut arp_neighbors: std::collections::HashMap<Ipv4Addr, String> =
            std::collections::HashMap::new();

        for (ip, mac) in arp::parse_proc_arp() {
            arp_neighbors.entry(ip).or_insert(mac);
        }

        #[cfg(target_os = "macos")]
        for (ip, mac) in arp::dump_arp_table_macos().await {
            arp_neighbors.entry(ip).or_insert(mac);
        }

        if !arp_neighbors.is_empty() {
            let self_ip = interface_ip;
            let subnet_cidr = local_network.cidr;
            let mut g = state.lock().unwrap();

            for (ip, mac) in arp_neighbors {
                if ip == self_ip || !subnet_cidr.contains(&ip) {
                    continue;
                }
                let ip_str = ip.to_string();
                if g.ping.contains_key(&ip_str) {
                    continue;
                }

                let vendor_name = vendor_name_from_mac(&mac);
                let vendor = vendor_name.clone();
                let is_randomized = is_randomized_mac(&mac);
                let device_type = infer_device_type("", &vendor, None);
                let name = infer_device_name(&vendor, &ip_str);

                info!(
                    "[SCAN_TRACE] ARP harvest: {} ({}) found in neighbour cache (AP-isolated?)",
                    ip_str, mac
                );

                let dev = DiscoveredDevice {
                    status: "Online".to_string(),
                    name,
                    ip: ip_str.clone(),
                    mac,
                    vendor,
                    vendor_name,
                    device_type,
                    is_randomized,
                    mdns_hostname: None,
                    mdns_primary_service: None,
                    likely_type: None,
                    hostname: None,
                    ssdp_server: None,
                    latency_ms: Some(1.0),
                    open_ports: None,
                    suggested_names: None,
                };

                g.ping.insert(ip_str.clone(), (dev.clone(), 1.0));

                if let Some(ref tx) = app {
                    batch_seq += 1;
                    let payload = ScanNetworkPayload {
                        devices: vec![dev],
                        average_latency_ms: None,
                        scanned_hosts: g.scanned_hosts,
                        scan_id: scan_id.clone(),
                        batch_seq,
                    };
                    let _ = tx.send(ScanEvent::DeviceDiscovered(payload));
                }
            }
        }
    }

    // ── PTR (reverse-DNS) pass ────────────────────────────────────────────────
    let o = interface_ip.octets();
    let dns_server = Ipv4Addr::new(o[0], o[1], o[2], 1);

    let all_active_ips: Vec<String> = state.lock().unwrap().ping.keys().cloned().collect();
    let ptr_handle = tokio::spawn(async move {
        stream::iter(all_active_ips)
            .map(|ip| async move {
                let name = get_ptr_name(&ip, dns_server).await;
                name.map(|n| (ip, n))
            })
            .buffer_unordered(PTR_QUERY_CONCURRENCY)
            .filter_map(|r| async move { r })
            .collect::<Vec<(String, String)>>()
            .await
    });

    // ── NetBIOS pass ──────────────────────────────────────────────────────────
    {
        let unnamed_ips: Vec<String> = {
            let g = state.lock().unwrap();
            g.ping
                .iter()
                .filter(|(ip, (dev, _))| {
                    dev.mdns_hostname.is_none() && is_generic_hostname(dev.name.trim(), ip)
                })
                .map(|(ip, _)| ip.clone())
                .collect()
        };

        let netbios_results: Vec<(String, String)> = stream::iter(unnamed_ips)
            .map(|ip| async move { get_netbios_name(&ip).await.map(|n| (ip, n)) })
            .buffer_unordered(host_concurrency)
            .filter_map(|r| async move { r })
            .collect()
            .await;

        let mut g = state.lock().unwrap();
        for (ip, name) in netbios_results {
            if let Some((dev, _)) = g.ping.get_mut(&ip) {
                dev.name = name;
            }
        }
    }

    // ── SSDP identity fetches ─────────────────────────────────────────────────
    match ssdp_handle.await {
        Ok((server_map, location_map)) => {
            let all_ips: Vec<String> = {
                let mut g = state.lock().unwrap();
                g.ssdp = server_map;
                g.ping.keys().cloned().collect()
            };
            run_identity_fetches(location_map, all_ips, &state).await;
        }
        Err(err) => {
            warn!("SSDP discovery task panicked or was cancelled: {err}");
        }
    }

    // ── Apply PTR results ─────────────────────────────────────────────────────
    if let Ok(ptr_results) = ptr_handle.await {
        let mut g = state.lock().unwrap();
        for (ip, name) in ptr_results {
            if let Some((dev, _)) = g.ping.get_mut(&ip) {
                if dev.mdns_hostname.is_none() && is_generic_hostname(&dev.name, &ip) {
                    dev.name = name.clone();
                    if dev.hostname.is_none() {
                        if let Some(h) = sanitize_rdns_hostname(&name) {
                            dev.hostname = Some(h);
                        }
                    }
                }
            }
        }
    }

    // ── Apply TR-064 router data ──────────────────────────────────────────────
    if let Ok(router_map) = router_handle.await {
        if !router_map.is_empty() {
            let mut g = state.lock().unwrap();
            for (ip, (mac, hostname)) in router_map {
                if let Some((dev, _)) = g.ping.get_mut(&ip) {
                    if (dev.mac == "Unknown" || dev.mac == "MAC Restricted") && !mac.is_empty() {
                        let vn = vendor_name_from_mac(&mac);
                        dev.mac = mac;
                        dev.vendor = vn.clone();
                        dev.vendor_name = vn;
                        dev.is_randomized = is_randomized_mac(&dev.mac);
                    }
                    let hn = hostname.trim().to_string();
                    if !hn.is_empty()
                        && dev.mdns_hostname.is_none()
                        && is_generic_hostname(&dev.name, &ip)
                    {
                        dev.name = hn.clone();
                        dev.mdns_hostname = Some(hn.clone());
                        if dev.hostname.is_none() {
                            dev.hostname = Some(hn);
                        }
                    }
                }
            }
        }
    }

    // ── ARP enrichment pass — backfill MAC for any device still "Unknown" ────
    {
        let arp_table: HashMap<Ipv4Addr, String> = arp::parse_proc_arp().into_iter().collect();
        if !arp_table.is_empty() {
            let mut g = state.lock().unwrap();
            for (ip_str, (dev, _)) in g.ping.iter_mut() {
                if dev.mac != "Unknown" && dev.mac != "MAC Restricted" {
                    continue;
                }
                let Ok(ip_addr) = ip_str.parse::<Ipv4Addr>() else {
                    continue;
                };
                if let Some(mac) = arp_table.get(&ip_addr) {
                    let vn = vendor_name_from_mac(mac);
                    info!(
                        "[FLIGHT_RECORDER] ARP enrichment: {} → MAC {}",
                        ip_str, mac
                    );
                    dev.vendor = vn.clone();
                    dev.vendor_name = vn;
                    dev.is_randomized = is_randomized_mac(mac);
                    dev.mac = mac.clone();
                }
            }
        }
    }

    let (only_devices, average_latency_ms, scanned_hosts) = {
        let g = state.lock().unwrap();
        build_scan_payload(&g)
    };
    debug!("[SCAN_DEBUG] Discovery phase concluded. Total raw objects found: {}", only_devices.len());

    if let Some(ref sd) = shared_devices {
        let mut d_lock = sd.lock().unwrap();
        *d_lock = only_devices.clone();
        info!("[SCAN] Shared App State populated with {} devices.", only_devices.len());
    }

    if only_devices.is_empty() {
        info!(
            "[FLIGHT_RECORDER] Scan completed with 0 devices on subnet: {}",
            local_network.cidr
        );
    }

    Ok(ScanNetworkResult {
        devices: only_devices,
        average_latency_ms,
        scanned_hosts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn oui_vendor_lookup_oracle_prefix() {
        init_vendor_map().expect("mac-vendors.json");
        let name = vendor_name_from_mac("00:00:17:aa:bb:cc");
        assert!(
            name.to_ascii_lowercase().contains("oracle"),
            "expected Oracle for OUI 000017, got {name:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn arp_scanner_finds_at_least_three_devices() {
        let has_network = network::local_ipv4_network().await.is_ok()
            || network::fallback_ipv4_network_via_udp().await.is_ok();
        if !has_network {
            eprintln!(
                "SKIP arp_scanner_finds_at_least_three_devices: \
                 no local IPv4 network found — test requires LAN connectivity"
            );
            return;
        }

        let start = Instant::now();
        // Since scan_local_network was removed, we use scan_local_network_pre_guarded with a guard
        let guard = ScanGuard::try_acquire().expect("failed to acquire scan guard for test");
        let result = scan_local_network_pre_guarded(None, None, guard, None, ScanMode::Silent, "test-scan".to_string())
            .await
            .expect("scan_local_network returned Err in test");
        let elapsed = start.elapsed();

        let test_budget = SCAN_LOCAL_NETWORK_TIMEOUT + Duration::from_secs(5);
        assert!(
            elapsed < test_budget,
            "scan did not return within {test_budget:?} — possible hang: elapsed {elapsed:?}",
        );

        let found: Vec<String> = result
            .devices
            .iter()
            .map(|d| format!("{} mac={} type={}", d.ip, d.mac, d.device_type))
            .collect();

        assert!(
            result.devices.len() >= 3,
            "ARP/ping scanner found {} device(s) — expected ≥ 3.\n\
             Devices seen: {found:#?}",
            result.devices.len(),
        );

        let arp_resolved = result
            .devices
            .iter()
            .any(|d| d.mac != "Unknown" && d.mac != "MAC Restricted");

        assert!(
            arp_resolved,
            "No device had a resolved MAC address.\nDevices seen: {found:#?}",
        );

        let oui_resolved = result.devices.iter().any(|d| {
            d.mac != "Unknown"
                && d.mac != "MAC Restricted"
                && d.vendor_name != "Unknown"
                && !d.vendor_name.is_empty()
        });
        assert!(
            oui_resolved,
            "No device received an OUI vendor_name.\nSample: {:?}",
            result
                .devices
                .iter()
                .filter(|d| d.mac != "Unknown" && d.mac != "MAC Restricted")
                .take(5)
                .map(|d| (&d.ip, &d.mac, &d.vendor_name))
                .collect::<Vec<_>>()
        );
    }
}
