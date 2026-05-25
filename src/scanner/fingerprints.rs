use std::collections::HashSet;
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::OnceLock;
use log::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceProfile {
    pub name: String,
    pub device_type: String,
    pub vendor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fingerprint {
    pub mac_prefixes: Option<Vec<String>>,
    pub mdns_keywords: Option<Vec<String>>,
    pub profile: DeviceProfile,
}

pub struct FingerprintRegistry {
    fingerprints: Vec<Fingerprint>,
}

impl FingerprintRegistry {
    pub fn load() -> Self {
        let path = "fingerprints.json";
        match fs::read_to_string(path) {
            Ok(content) => match serde_json::from_str::<Vec<Fingerprint>>(&content) {
                Ok(fingerprints) => {
                    info!("Loaded {} fingerprints from {}", fingerprints.len(), path);
                    FingerprintRegistry { fingerprints }
                }
                Err(e) => {
                    warn!("Failed to parse fingerprints.json: {}. Using empty registry.", e);
                    FingerprintRegistry { fingerprints: Vec::new() }
                }
            },
            Err(e) => {
                warn!("Failed to read fingerprints.json: {}. Using empty registry.", e);
                FingerprintRegistry { fingerprints: Vec::new() }
            }
        }
    }

    pub fn match_device(&self, mac: &str, mdns: &str) -> Option<DeviceProfile> {
        let mac_upper = mac.to_uppercase();
        let mdns_lower = mdns.to_lowercase();

        for fp in &self.fingerprints {
            // Match by MAC prefix
            if let Some(prefixes) = &fp.mac_prefixes {
                if prefixes.iter().any(|p| mac_upper.starts_with(&p.to_uppercase())) {
                    return Some(fp.profile.clone());
                }
            }

            // Match by mDNS keywords
            if let Some(keywords) = &fp.mdns_keywords {
                if keywords.iter().any(|k| mdns_lower.contains(&k.to_lowercase())) {
                    return Some(fp.profile.clone());
                }
            }
        }

        None
    }
}

static REGISTRY: OnceLock<FingerprintRegistry> = OnceLock::new();

pub fn get_registry() -> &'static FingerprintRegistry {
    REGISTRY.get_or_init(FingerprintRegistry::load)
}

/// Hints from HTTP `Server` / HTML / body keyword scans. Applied after port-based ID.
#[derive(Debug, Default, Clone)]
pub struct HttpLayerHints {
    /// Vendor / model line suitable for `interrogation_name` (e.g. "Bosch", title text).
    pub interrogation: Option<String>,
    /// High-confidence `likely_type` (LG webOS banner, Asustor, strong brand, …).
    pub likely: Option<String>,
}

/// Returns true for weak port-only labels that HTTP is allowed to replace.
fn is_generic_port_fingerprint(s: &str) -> bool {
    matches!(
        s,
        "" | "HTTP Device" | "HTTPS Device" | "HTTP Admin Panel" | "Network Device (Web Interface)"
    ) || s.starts_with("Network Device")
}

/// Merges port fingerprinting with optional HTTP `likely` — HTTP wins for generic
/// port classes or when a non-empty HTTP `likely` is present and the port match was empty.
pub fn merge_port_and_http_likely(port: &str, http_likely: Option<&str>) -> String {
    let h = http_likely.map(str::trim).filter(|s| !s.is_empty());
    if let Some(label) = h {
        if is_generic_port_fingerprint(port) || port.is_empty() {
            return label.to_string();
        }
    }
    if !port.is_empty() {
        return port.to_string();
    }
    h.map(|s| s.to_string()).unwrap_or_default()
}

fn contains_ci(hay: &str, needle: &str) -> bool {
    let h = hay.to_lowercase();
    h.contains(needle) || h.contains(&needle.to_lowercase())
}

/// `Server` / HTML title / first bytes of body — keyword scan for known vendors
/// and appliance UIs. Port context avoids labelling a random 8080 as LG.
pub fn classify_from_http_artifacts(
    server: Option<&str>,
    title: Option<&str>,
    body_prefix: &str,
    open_ports: &[u16],
) -> HttpLayerHints {
    let set: HashSet<u16> = open_ports.iter().copied().collect();
    let srv = server.unwrap_or("").to_string();
    let ttl = title.unwrap_or("").to_string();
    let combined = format!("{srv} {ttl} {body_prefix}");
    let c = combined.to_lowercase();
    let mut out = HttpLayerHints::default();

    // Asustor / lighttpd on NAS
    if contains_ci(&combined, "asustor")
        || (contains_ci(&srv, "lighttpd") && contains_ci(&combined, "asustor"))
    {
        out.likely = Some("Asustor NAS".to_string());
        out.interrogation = Some("Asustor".to_string());
    }

    // LG Smart TV / appliance (port context + webOS / LGE / NetCast banners)
    let lg_port_ok = set.contains(&3000)
        || set.contains(&3001)
        || set.contains(&8080)
        || set.contains(&80)
        || set.contains(&443);
    if lg_port_ok
        && (c.contains("webos")
            || c.contains("lge")
            || c.contains("lg web")
            || c.contains("netcast")
            || c.contains("lg electronics")
            || c.contains("lg smart"))
    {
        out.likely = Some("LG Smart Appliance / TV".to_string());
    }

    // Home Assistant (UI often shows in title; port 8123 is handled in `identify_by_ports`)
    if c.contains("home assistant") && out.likely.is_none() {
        out.likely = Some("Home Assistant".to_string());
    }

    // Big-appliance & networking brands in title or body
    for (kw, display) in [
        ("bosch", "Bosch"),
        ("siemens", "Siemens"),
        ("liebherr", "Liebherr"),
        ("tp-link", "TP-Link"),
        ("tp link", "TP-Link"),
        ("xiaomi", "Xiaomi"),
        ("mi router", "Xiaomi"),
    ] {
        if c.contains(kw) {
            if out.interrogation.is_none() {
                out.interrogation = Some(format!("{display} device"));
            }
            if out.likely.is_none() {
                out.likely = Some(format!("{display} device"));
            }
            break;
        }
    }

    // Raw Server banner: treat Asustor explicitly even when lighttpd would be filtered
    if out.interrogation.is_none() && contains_ci(&srv, "asustor") {
        out.interrogation = Some(srv.chars().take(80).collect());
    }

    // Lighttpd on embedded — only promote when paired with a vendor hint
    if contains_ci(&srv, "lighttpd")
        && out.interrogation.is_none()
        && contains_ci(&combined, "asustor")
    {
        out.interrogation = Some("lighttpd (Asustor)".to_string());
    }

    out
}

// ── Phase 1: high-priority **OR** / subset rules (first match returns) ──────────

/// Multi-port **AND** rules, ordered most-specific first. Excludes port sets handled
/// in `identify_by_ports_special_or`.
static FINGERPRINTS: &[(&[u16], &str)] = &[
    // Brand-Specific
    (&[7676], "Samsung Device"),
    (&[54921], "Brother Printer / Scanner"),
    (&[54925], "Brother Printer"),
    // Smart Home & Media
    (&[32400], "Plex Media Server"),
    (&[51827], "Philips Hue Bridge"),
    (&[1883], "MQTT / IoT Hub (Home Assistant)"),
    // Printers
    (&[9100], "Network Printer (JetDirect)"),
    (&[631], "Network Printer (IPP)"),
    (&[515], "Network Printer (LPD)"),
    // Remote
    (&[22, 80], "Linux Server / Raspberry Pi"),
    // Routers
    (&[53, 80, 443], "Router / Gateway"),
    (&[53, 443], "Router / DNS Gateway"),
    (&[53, 8080], "Router / DNS Gateway"),
    // Databases
    (&[53, 80], "DNS Server / Pi-Hole"),
    (&[3306], "MySQL Database Server"),
    (&[5432], "PostgreSQL Database"),
    (&[27017], "MongoDB Server"),
    // Web
    (&[80, 443], "Network Device (Web Interface)"),
    (&[8080, 80], "Network Device (Web Interface)"),
    // Fallbacks
    (&[8008], "Chromecast / Google TV"),
    (&[5000], "Synology NAS / Plex"),
    (&[22], "Linux Server"),
    (&[23], "Embedded / Legacy Device"),
    (&[443], "HTTPS Device"),
    (&[80], "HTTP Device"),
    (&[8080], "HTTP Admin Panel"),
];

/// Priority OR / exclusive rules: evaluated before `FINGERPRINTS` (AND) table.
fn identify_by_ports_special_or(set: &HashSet<u16>) -> Option<String> {
    // Samsung pair before Asustor (8001 alone → Asustor in separate branch)
    if set.contains(&8001) && set.contains(&8002) {
        return Some("Samsung Smart TV".to_string());
    }
    if set.contains(&445) && set.contains(&548) {
        return Some("Mac / Apple File Server".to_string());
    }
    if set.contains(&8080) && set.contains(&8443) {
        return Some("Ubiquiti UniFi Gateway / Web Admin".to_string());
    }
    if set.contains(&8008) && set.contains(&8009) {
        return Some("Chromecast / Google TV".to_string());
    }
    if set.contains(&5000) && set.contains(&5001) {
        return Some("Synology NAS".to_string());
    }
    if set.contains(&3389) {
        return Some("Windows PC".to_string());
    }
    // Asustor: 8000, or 8001 without 8002 (Samsung needs both 8001+8002)
    if set.contains(&8000) || (set.contains(&8001) && !set.contains(&8002)) {
        return Some("Asustor NAS".to_string());
    }
    if set.contains(&8123) {
        return Some("Home Assistant".to_string());
    }
    if set.contains(&554) {
        return Some("RTSP Security Camera".to_string());
    }
    if set.contains(&1400) {
        return Some("Sonos Audio Device".to_string());
    }
    // Apple — HomeKit / Continuity / AirPlay (user-specified 7668 label for HomeKit)
    if set.contains(&7668) {
        return Some("Apple Device / iPhone / Mac".to_string());
    }
    if set.contains(&62078) {
        return Some("Apple Device (iPhone/iPad/Mac)".to_string());
    }
    if set.contains(&3689) {
        return Some("Apple TV / HomePod (iTunes)".to_string());
    }
    if set.contains(&7000) {
        return Some("Apple AirPlay Receiver".to_string());
    }
    // LG WebOS: any of 3000 or 3001 (HTTP refines 8080 via `classify_from_http_artifacts`)
    if set.contains(&3000) || set.contains(&3001) {
        return Some("LG Smart Appliance / TV".to_string());
    }
    // NetBIOS or SMB
    if set.contains(&139) || set.contains(&445) {
        return Some("Windows / Lenovo PC".to_string());
    }
    // AFP single (no SMB pair)
    if set.contains(&548) {
        return Some("Mac / Apple Device".to_string());
    }
    None
}

/// Returns a `likely_type` string based on patterns in the device's hostname or mDNS name.
/// Strips a trailing `.local` before matching. Returns `None` when no pattern matches.
pub fn classify_from_hostname(hostname: &str) -> Option<String> {
    let h = hostname.trim();
    if h.is_empty() {
        return None;
    }
    let stripped = h.strip_suffix(".local").unwrap_or(h);

    // Case-sensitive prefix/substring patterns (model numbers, Windows auto-names)
    if stripped.contains("RX-") {
        return Some("AV Receiver".to_string());
    }
    if stripped.contains("SM-") {
        return Some("Android Phone".to_string());
    }
    if stripped.contains("DESKTOP-") || stripped.contains("LAPTOP-") {
        return Some("Windows PC".to_string());
    }
    if stripped.starts_with("PS5-") || stripped.starts_with("PS4-") {
        return Some("PlayStation".to_string());
    }
    if stripped.contains("V-NBRADREMOTE") {
        return Some("TV Remote / Smart Remote".to_string());
    }

    // Case-insensitive keyword matching
    let l = stripped.to_lowercase();

    if l.contains("macbook") {
        return Some("Mac / MacBook".to_string());
    }
    if l.contains("iphone") || l.contains("ipad") {
        return Some("Apple iPhone / iPad".to_string());
    }
    if l.contains("android") || l.contains("galaxy") || l.contains("pixel") {
        return Some("Android Phone".to_string());
    }
    if l.contains("chromecast") {
        return Some("Chromecast".to_string());
    }
    if l.contains("echo") || l.contains("alexa") {
        return Some("Amazon Echo".to_string());
    }
    if l.contains("yeelink") || l.contains("mibedsidelamp") || l.contains("miio") {
        return Some("Smart Light".to_string());
    }
    if l.contains("bravia") || l.contains("tizen") || l.contains("samsung-tv") {
        return Some("Smart TV".to_string());
    }
    // "tv" matched last — short substring so put common false-positive words first
    if l.contains("tv") {
        return Some("Smart TV".to_string());
    }

    None
}

/// Returns a `likely_type` string based on the OUI vendor name.
///
/// Yamaha and Raspberry Pi are always classified. Samsung and Apple are 
/// promoted even with open ports if the port-based label is generic.
pub fn classify_from_vendor(vendor: &str, open_ports: &[u16], current_label: &str) -> Option<String> {
    let v = vendor.trim();
    if v.is_empty() || v.eq_ignore_ascii_case("unknown") {
        return None;
    }
    let l = v.to_lowercase();

    if l.contains("yamaha") {
        return Some("AV Receiver".to_string());
    }
    if l.contains("raspberry") {
        return Some("Raspberry Pi".to_string());
    }
    if l.contains("lumi united") || l.contains("aqara") {
        return Some("Smart Home Device".to_string());
    }
    if l.contains("reolink") {
        return Some("IP Camera".to_string());
    }
    if l.contains("xiaomi") || l.contains("beijing xiaomi") {
        return Some("Xiaomi Device".to_string());
    }
    if l.contains("lg innotek") || l.contains("lg electronics") {
        return Some("Smart TV".to_string());
    }
    if l.contains("fiio") {
        return Some("Audio DAC / Player".to_string());
    }
    if l.contains("liebherr") {
        return Some("Smart Appliance".to_string());
    }
    if l.contains("zhen shi") {
        return Some("Smart Device".to_string());
    }

    // Port-data guard: promote vendor when there is no port signal, or if the port signal is generic
    if open_ports.is_empty() || is_generic_port_fingerprint(current_label) {
        if l.contains("samsung") {
            return Some("Samsung Device".to_string());
        }
        if l.contains("apple") {
            return Some("Apple Device".to_string());
        }
        if l.contains("google") {
            return Some("Google Device".to_string());
        }
    }

    None
}

/// If the MAC OUI is unknown, we can often infer the vendor from hostname or HTTP banners.
pub fn identify_corporate_vendor(hostname: &str, http_banner: &str) -> Option<String> {
    let combined = format!("{} {}", hostname, http_banner).to_lowercase();
    
    if combined.contains("samsung") || combined.contains("tizen") {
        return Some("Samsung".to_string());
    }
    if combined.contains("apple") || combined.contains("iphone") || combined.contains("ipad") || combined.contains("macbook") || combined.contains("iwatch") {
        return Some("Apple".to_string());
    }
    if combined.contains("google") || combined.contains("pixel") || combined.contains("chromecast") {
        return Some("Google".to_string());
    }
    if combined.contains("sony") || combined.contains("bravia") || combined.contains("playstation") {
        return Some("Sony".to_string());
    }
    if combined.contains("microsoft") || combined.contains("windows") {
        return Some("Microsoft".to_string());
    }
    if combined.contains("amazon") || combined.contains("echo") || combined.contains("alexa") || combined.contains("kindle") {
        return Some("Amazon".to_string());
    }
    if combined.contains("lg electronics") || combined.contains("webos") {
        return Some("LG".to_string());
    }

    None
}

/// Returns a human-readable device-type guess based on open TCP ports.
/// Returns `"Network Device"` when ports were probed but no specific rule matched,
/// or an empty string when `open_ports` is empty (port scan not yet run).
pub fn identify_by_ports(open_ports: &[u16]) -> String {
    if open_ports.is_empty() {
        return String::new();
    }
    let set: HashSet<u16> = open_ports.iter().copied().collect();
    if let Some(s) = identify_by_ports_special_or(&set) {
        return s;
    }
    for (required, description) in FINGERPRINTS {
        if required.iter().all(|p| set.contains(p)) {
            return description.to_string();
        }
    }
    "Network Device".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plex_via_32400() {
        assert_eq!(identify_by_ports(&[32400, 443]), "Plex Media Server");
    }

    #[test]
    fn chromecast_via_8008_and_8009() {
        assert_eq!(identify_by_ports(&[8008, 8009]), "Chromecast / Google TV");
    }

    #[test]
    fn chromecast_fallback_8008_only() {
        assert_eq!(identify_by_ports(&[8008, 80]), "Chromecast / Google TV");
    }

    #[test]
    fn sonos_via_1400() {
        assert_eq!(identify_by_ports(&[1400]), "Sonos Audio Device");
    }

    #[test]
    fn hue_bridge_via_51827() {
        assert_eq!(identify_by_ports(&[51827]), "Philips Hue Bridge");
    }

    #[test]
    fn daap_via_3689() {
        assert_eq!(identify_by_ports(&[3689]), "Apple TV / HomePod (iTunes)");
    }

    #[test]
    fn airplay_via_7000() {
        assert_eq!(identify_by_ports(&[7000]), "Apple AirPlay Receiver");
    }

    #[test]
    fn homekit_7668() {
        assert_eq!(identify_by_ports(&[7668]), "Apple Device / iPhone / Mac");
    }

    #[test]
    fn mqtt_via_1883() {
        assert_eq!(
            identify_by_ports(&[1883]),
            "MQTT / IoT Hub (Home Assistant)"
        );
    }

    #[test]
    fn home_assistant_8123() {
        assert_eq!(identify_by_ports(&[8123, 80]), "Home Assistant");
    }

    #[test]
    fn rtsp_554() {
        assert_eq!(identify_by_ports(&[554]), "RTSP Security Camera");
    }

    #[test]
    fn asustor_8000() {
        assert_eq!(identify_by_ports(&[8000, 80]), "Asustor NAS");
    }

    #[test]
    fn asustor_8001_without_8002() {
        assert_eq!(identify_by_ports(&[8001, 80]), "Asustor NAS");
    }

    #[test]
    fn jetdirect_printer_via_9100() {
        assert_eq!(identify_by_ports(&[9100]), "Network Printer (JetDirect)");
    }

    #[test]
    fn ipp_printer_via_631() {
        assert_eq!(identify_by_ports(&[631]), "Network Printer (IPP)");
    }

    #[test]
    fn lpd_printer_via_515() {
        assert_eq!(identify_by_ports(&[515]), "Network Printer (LPD)");
    }

    #[test]
    fn synology_nas_via_5000_and_5001() {
        assert_eq!(identify_by_ports(&[5000, 5001]), "Synology NAS");
    }

    #[test]
    fn synology_fallback_5000_only() {
        assert_eq!(identify_by_ports(&[5000, 443]), "Synology NAS / Plex");
    }

    #[test]
    fn apple_file_server_via_445_and_548() {
        assert_eq!(identify_by_ports(&[445, 548]), "Mac / Apple File Server");
    }

    #[test]
    fn smb_139() {
        assert_eq!(identify_by_ports(&[139, 80]), "Windows / Lenovo PC");
    }

    #[test]
    fn smb_file_server_via_445_only() {
        assert_eq!(identify_by_ports(&[445, 80]), "Windows / Lenovo PC");
    }

    #[test]
    fn lg_3000_only() {
        assert_eq!(identify_by_ports(&[3000, 443]), "LG Smart Appliance / TV");
    }

    #[test]
    fn pihole_via_53_and_80() {
        assert_eq!(identify_by_ports(&[53, 80]), "DNS Server / Pi-Hole");
    }

    #[test]
    fn mysql_via_3306() {
        assert_eq!(identify_by_ports(&[3306]), "MySQL Database Server");
    }

    #[test]
    fn postgres_via_5432() {
        assert_eq!(identify_by_ports(&[5432]), "PostgreSQL Database");
    }

    #[test]
    fn mongodb_via_27017() {
        assert_eq!(identify_by_ports(&[27017]), "MongoDB Server");
    }

    #[test]
    fn unifi_via_8080_and_8443() {
        assert_eq!(
            identify_by_ports(&[8080, 8443]),
            "Ubiquiti UniFi Gateway / Web Admin"
        );
    }

    #[test]
    fn windows_pc_via_rdp() {
        assert_eq!(identify_by_ports(&[3389, 445]), "Windows PC");
    }

    #[test]
    fn linux_server_via_ssh_and_http() {
        assert_eq!(identify_by_ports(&[22, 80]), "Linux Server / Raspberry Pi");
    }

    #[test]
    fn linux_server_fallback_ssh_only() {
        assert_eq!(identify_by_ports(&[22]), "Linux Server");
    }

    #[test]
    fn legacy_device_via_telnet() {
        assert_eq!(identify_by_ports(&[23]), "Embedded / Legacy Device");
    }

    #[test]
    fn http_classify_bosch() {
        let h = classify_from_http_artifacts(None, Some("Bosch Home Connect"), "", &[80, 443]);
        assert_eq!(h.likely.as_deref(), Some("Bosch device"));
    }

    #[test]
    fn http_classify_lg_webos() {
        let h = classify_from_http_artifacts(
            Some("LGE Proprietary"), // would be filtered as identity but we still classify
            None,
            "",
            &[8080],
        );
        assert_eq!(h.likely.as_deref(), Some("LG Smart Appliance / TV"));
    }

    #[test]
    fn merge_generic_http_replaces() {
        assert_eq!(
            merge_port_and_http_likely("HTTP Device", Some("Bosch device")),
            "Bosch device"
        );
    }

    #[test]
    fn merge_strong_port_kept() {
        assert_eq!(
            merge_port_and_http_likely("Plex Media Server", Some("Bosch device")),
            "Plex Media Server"
        );
    }

    #[test]
    fn empty_ports_returns_empty() {
        assert_eq!(identify_by_ports(&[]), "");
    }

    #[test]
    fn no_match_returns_network_device() {
        assert_eq!(identify_by_ports(&[1234, 5678]), "Network Device");
    }

    // ── classify_from_hostname ────────────────────────────────────────────────

    #[test]
    fn hostname_macbook() {
        assert_eq!(
            classify_from_hostname("Tareg's-MacBook-Pro.local"),
            Some("Mac / MacBook".to_string())
        );
    }

    #[test]
    fn hostname_macbook_mdns_no_suffix() {
        assert_eq!(
            classify_from_hostname("MacBook-Air"),
            Some("Mac / MacBook".to_string())
        );
    }

    #[test]
    fn hostname_iphone() {
        assert_eq!(
            classify_from_hostname("iPhone"),
            Some("Apple iPhone / iPad".to_string())
        );
    }

    #[test]
    fn hostname_ipad() {
        assert_eq!(
            classify_from_hostname("iPad-Pro.local"),
            Some("Apple iPhone / iPad".to_string())
        );
    }

    #[test]
    fn hostname_android_galaxy() {
        assert_eq!(
            classify_from_hostname("Galaxy-S24"),
            Some("Android Phone".to_string())
        );
    }

    #[test]
    fn hostname_samsung_sm_prefix() {
        assert_eq!(
            classify_from_hostname("SM-G998B"),
            Some("Android Phone".to_string())
        );
    }

    #[test]
    fn hostname_pixel() {
        assert_eq!(
            classify_from_hostname("pixel-7-pro"),
            Some("Android Phone".to_string())
        );
    }

    #[test]
    fn hostname_yamaha_rx() {
        assert_eq!(
            classify_from_hostname("RX-V6A"),
            Some("AV Receiver".to_string())
        );
    }

    #[test]
    fn hostname_bravia() {
        assert_eq!(
            classify_from_hostname("BRAVIA-KD55X90K"),
            Some("Smart TV".to_string())
        );
    }

    #[test]
    fn hostname_tizen_tv() {
        assert_eq!(
            classify_from_hostname("TIZEN-TV-UE65"),
            Some("Smart TV".to_string())
        );
    }

    #[test]
    fn hostname_tv_suffix() {
        assert_eq!(
            classify_from_hostname("samsung-tv"),
            Some("Smart TV".to_string())
        );
    }

    #[test]
    fn hostname_chromecast() {
        assert_eq!(
            classify_from_hostname("Chromecast-HD"),
            Some("Chromecast".to_string())
        );
    }

    #[test]
    fn hostname_echo() {
        assert_eq!(
            classify_from_hostname("Echo-Dot-4th-Gen"),
            Some("Amazon Echo".to_string())
        );
    }

    #[test]
    fn hostname_alexa() {
        assert_eq!(
            classify_from_hostname("Alexa-Kitchen"),
            Some("Amazon Echo".to_string())
        );
    }

    #[test]
    fn hostname_desktop_windows() {
        assert_eq!(
            classify_from_hostname("DESKTOP-AB12CD"),
            Some("Windows PC".to_string())
        );
    }

    #[test]
    fn hostname_laptop_windows() {
        assert_eq!(
            classify_from_hostname("LAPTOP-XYZ789"),
            Some("Windows PC".to_string())
        );
    }

    #[test]
    fn hostname_empty_returns_none() {
        assert_eq!(classify_from_hostname(""), None);
        assert_eq!(classify_from_hostname("  "), None);
    }

    #[test]
    fn hostname_no_match_returns_none() {
        assert_eq!(classify_from_hostname("myserver"), None);
        assert_eq!(classify_from_hostname("192.168.1.42"), None);
    }

    // ── classify_from_vendor ─────────────────────────────────────────────────

    #[test]
    fn vendor_yamaha_always() {
        assert_eq!(
            classify_from_vendor("Yamaha Corporation", &[80, 443], "HTTP Device"),
            Some("AV Receiver".to_string())
        );
        assert_eq!(
            classify_from_vendor("Yamaha Corporation", &[], ""),
            Some("AV Receiver".to_string())
        );
    }

    #[test]
    fn vendor_raspberry_always() {
        assert_eq!(
            classify_from_vendor("Raspberry Pi Trading Ltd", &[22, 80], "Linux Server"),
            Some("Raspberry Pi".to_string())
        );
    }

    #[test]
    fn vendor_samsung_no_ports() {
        assert_eq!(
            classify_from_vendor("Samsung Electronics", &[], ""),
            Some("Samsung Device".to_string())
        );
    }

    #[test]
    fn vendor_samsung_with_generic_port_label() {
        assert_eq!(
            classify_from_vendor("Samsung Electronics", &[80, 443], "HTTP Device"),
            Some("Samsung Device".to_string())
        );
    }

    #[test]
    fn vendor_samsung_with_specific_port_no_override() {
        assert_eq!(
            classify_from_vendor("Samsung Electronics", &[80, 443], "Plex Media Server"),
            None
        );
    }

    #[test]
    fn vendor_apple_no_ports() {
        assert_eq!(
            classify_from_vendor("Apple, Inc.", &[], ""),
            Some("Apple Device".to_string())
        );
    }

    #[test]
    fn vendor_apple_with_generic_port_label() {
        assert_eq!(
            classify_from_vendor("Apple, Inc.", &[62078], "Network Device"),
            Some("Apple Device".to_string())
        );
    }

    #[test]
    fn vendor_unknown_returns_none() {
        assert_eq!(classify_from_vendor("Unknown", &[], ""), None);
        assert_eq!(classify_from_vendor("", &[], ""), None);
    }

    #[test]
    fn vendor_unrecognized_returns_none() {
        assert_eq!(classify_from_vendor("Generic Corp", &[], ""), None);
    }
}
