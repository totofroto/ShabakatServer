use log::{info, warn};
use std::time::Duration;
use crate::storage::AppDb;
use crate::storage::devices as dev_store;

/// Attempts to resolve a friendly name for the gateway IP.
/// This targets common router ports and protocols.
pub async fn resolve_gateway_name(db: AppDb, gateway_ip: &str) -> Option<String> {
    info!("[GATEWAY_RESOLVER] Identifying gateway: {}", gateway_ip);

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .danger_accept_invalid_certs(true)
        .build()
    {
        Ok(c) => c,
        Err(_) => return None,
    };

    // 1. HTTP Probe for Title
    if let Some(name) = probe_http_title(gateway_ip, &client).await {
        info!("[GATEWAY_RESOLVER] Resolved via HTTP: {}", name);
        update_gateway_alias(db, gateway_ip, &name).await;
        return Some(name);
    }

    // 2. mDNS Unicast Probe
    if let Some(name) = super::unicast_mdns_query(gateway_ip).await {
        info!("[GATEWAY_RESOLVER] Resolved via mDNS: {}", name);
        update_gateway_alias(db, gateway_ip, &name).await;
        return Some(name);
    }

    warn!("[GATEWAY_RESOLVER] Could not identify gateway name for {}", gateway_ip);
    None
}

async fn probe_http_title(ip: &str, client: &reqwest::Client) -> Option<String> {
    for port in &[80, 443, 8080] {
        let scheme = if *port == 443 { "https" } else { "http" };
        let url = format!("{}://{}/", scheme, ip);
        
        match client.get(&url).send().await {
            Ok(resp) => {
                if let Ok(body) = resp.text().await {
                    if let Some(title) = extract_title(&body) {
                        if is_useful_title(&title) {
                            return Some(title);
                        }
                    }
                }
            }
            Err(_) => continue,
        }
    }
    None
}

fn extract_title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let start_tag = "<title>";
    let end_tag = "</title>";
    
    if let Some(start_idx) = lower.find(start_tag) {
        let content_start = start_idx + start_tag.len();
        if let Some(end_idx) = lower[content_start..].find(end_tag) {
            let title = &html[content_start..content_start + end_idx];
            return Some(title.trim().to_string());
        }
    }
    None
}

fn is_useful_title(title: &str) -> bool {
    let t = title.to_lowercase();
    let is_generic = t.is_empty() 
        || t.contains("404") 
        || t.contains("403") 
        || t.contains("index of") 
        || t.contains("unauthorized")
        || t == "login"
        || t == "home";
    
    !is_generic && title.len() > 2 && title.len() < 64
}

async fn update_gateway_alias(db: AppDb, ip: &str, name: &str) {
    info!("[GATEWAY_RESOLVER] Updating alias for {}: {}", ip, name);
    let _ = dev_store::upsert_device_alias(db, ip.to_string(), name.to_string()).await;
}
