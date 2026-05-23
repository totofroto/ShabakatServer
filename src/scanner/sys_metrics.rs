// src/scanner/sys_metrics.rs

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::collections::HashMap;
use crate::types::{InterfaceMetrics, SystemTelemetry};

/// Holds previous tick readings to calculate accurate per-second deltas (Netdata approach)
pub struct SystemCollector {
    prev_network_stats: HashMap<String, (u64, u64)>, // Interface -> (BytesRX, BytesTX)
    prev_timestamp: i64,
}

impl SystemCollector {
    pub fn new() -> Self {
        Self {
            prev_network_stats: HashMap::new(),
            prev_timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    /// High-frequency non-blocking parse of /proc/net/dev to calculate real-time bandwidth speeds.
    /// Must be invoked inside a tokio::task::spawn_blocking thread context to secure the main loop.
    pub fn collect_telemetry(&mut self) -> Result<SystemTelemetry, std::io::Error> {
        let current_time = chrono::Utc::now().timestamp_millis();
        let file = File::open("/proc/net/dev")?;
        let reader = BufReader::new(file);
        
        let mut current_interfaces = Vec::new();
        let mut current_stats_map = HashMap::new();

        // Calculate time delta in fractional seconds, defaulting safely to 1.0s if clocks align
        let time_delta_secs = ((current_time - self.prev_timestamp) as f64 / 1000.0).max(0.1);

        // Skip the first 2 header lines of /proc/net/dev
        for line_res in reader.lines().skip(2) {
            let line = line_res?;
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 17 {
                continue;
            }

            // Interface name sits at parts[0], e.g., "eth0:" or "wlan0:"
            let interface_name = parts[0].trim_end_matches(':').to_string();
            
            // Avoid overhead from standard loopback devices
            if interface_name == "lo" {
                continue;
            }

            // Parse raw counters: Byte RX is position 1, Byte TX is position 9
            let bytes_rx: u64 = parts[1].parse().unwrap_or(0);
            let bytes_tx: u64 = parts[9].parse().unwrap_or(0);

            current_stats_map.insert(interface_name.clone(), (bytes_rx, bytes_tx));

            // Compute instant sub-second throughput differentials if historical telemetry exists
            if let Some(&(prev_rx, prev_tx)) = self.prev_network_stats.get(&interface_name) {
                // Handle edge counters wrap-around/reset gracefully
                let delta_rx = if bytes_rx >= prev_rx { bytes_rx - prev_rx } else { 0 };
                let delta_tx = if bytes_tx >= prev_tx { bytes_tx - prev_tx } else { 0 };

                current_interfaces.push(InterfaceMetrics {
                    interface: interface_name,
                    bytes_rx_per_sec: (delta_rx as f64 / time_delta_secs) as u64,
                    bytes_tx_per_sec: (delta_tx as f64 / time_delta_secs) as u64,
                });
            }
        }

        // Cache historical configurations for consecutive updates
        self.prev_network_stats = current_stats_map;
        self.prev_timestamp = current_time;

        Ok(SystemTelemetry {
            timestamp: current_time,
            interfaces: current_interfaces,
        })
    }
}
