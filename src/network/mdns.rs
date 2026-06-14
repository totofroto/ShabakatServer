use mdns_sd::{ServiceDaemon, ServiceInfo};
use log::{info, error};
use std::collections::HashMap;

/// Starts the mDNS advertisement for ShabakatServer.
/// This broadcasts the _http._tcp service on port 7779.
/// Returns the ServiceDaemon handle which must be kept alive.
pub fn start_mdns_advertisement() -> Option<ServiceDaemon> {
    info!("[MDNS] Initializing mDNS advertisement...");

    // Create a new mDNS daemon
    let mdns = match ServiceDaemon::new() {
        Ok(d) => d,
        Err(e) => {
            error!("[MDNS] Failed to create mDNS daemon: {}", e);
            return None;
        }
    };

    // Service details
    let service_type = "_http._tcp.local."; 
    let instance_name = "ShabakatServer";
    let host_name = "shabakat-server.local.";
    let port = 7779;
    
    let mut properties = HashMap::new();
    properties.insert("path".to_string(), "/".to_string());
    properties.insert("version".to_string(), "1.0.0".to_string());

    // Create service info
    let service_info = match ServiceInfo::new(
        service_type,
        instance_name,
        host_name,
        "", // IP will be auto-detected
        port,
        Some(properties),
    ) {
        Ok(info) => info,
        Err(e) => {
            error!("[MDNS] Failed to create service info: {}", e);
            return None;
        }
    };

    // Register the service
    match mdns.register(service_info) {
        Ok(_) => {
            info!("[MDNS] Registered mDNS service: {}._http._tcp.local on port {}", instance_name, port);
            Some(mdns)
        }
        Err(e) => {
            error!("[MDNS] Failed to register mDNS service: {}", e);
            None
        }
    }
}
