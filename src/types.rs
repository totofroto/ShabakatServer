use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredDevice {
    pub status: String,
    pub name: String,
    pub ip: String,
    pub mac: String,
    /// OUI / IEEE registry company name.
    pub vendor: String,
    pub vendor_name: String,
    pub device_type: String,
    pub is_randomized: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mdns_hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mdns_primary_service: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub likely_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssdp_server: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_ports: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ScanNetworkPayload {
    pub devices: Vec<DiscoveredDevice>,
    pub average_latency_ms: Option<f64>,
    pub scanned_hosts: usize,
    pub total_hosts: usize,
    pub scan_id: String,
    pub batch_seq: u32,
}

/// Events streamed from the scanner engine to the WebSocket layer.
pub enum ScanEvent {
    /// A batch of newly discovered devices during a scan sweep.
    DeviceDiscovered(ScanNetworkPayload),
    /// Full current device snapshot (for progress bar updates).
    Progress(ScanNetworkPayload),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DnsProvider {
    pub id: String,
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub is_enabled: bool,
    pub created_at: i64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NetworkRecord {
    pub id: i64,
    pub ssid: Option<String>,
    pub bssid: String,
    pub gateway: Option<String>,
    pub subnet: Option<String>,
    pub first_seen: i64,
    pub last_seen: i64,
    pub device_count: i64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeviceRecord {
    pub id: i64,
    pub mac: String,
    pub first_seen: i64,
    pub last_seen: i64,
    pub last_ip: Option<String>,
    pub vendor: Option<String>,
    pub custom_name: Option<String>,
    pub likely_type: Option<String>,
    pub hostname: Option<String>,
    pub mdns_hostname: Option<String>,
    pub ssdp_server: Option<String>,
    pub interrogation_name: Option<String>,
    pub acknowledged: bool,
    pub notes: Option<String>,
    pub display_name: Option<String>,
    pub is_online: bool,
    pub custom_icon: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SystemStatus {
    pub score: i32,
    pub performance_score: i32,
    pub latency_score: i32,
    pub security_score: i32,
    pub last_updated: i64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RouterBandwidth {
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub timestamp: i64,
}
