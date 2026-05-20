//! TR-064 / UPnP router interrogation.
//!
//! Queries the router's `Hosts:1` SOAP service for the DHCP lease table.
//! Returns `IP → (MAC, Hostname)` for every client the router knows about.
//!
//! Port/path candidates are tried in order; the first one that returns data wins.
//! All HTTP calls have a 500 ms connect timeout + 2 s total timeout, so unreachable
//! ports fail fast (TCP RST is near-instant on a LAN).

use std::{collections::HashMap, time::Duration};

use log::info;
use reqwest::Client;

const SERVICE_URN: &str = "urn:dslforum-org:service:Hosts:1";
const WAN_COMMON_URN: &str = "urn:dslforum-org:service:WANCommonInterfaceConfig:1";

/// Hard cap on index iterations — prevents runaway loops on broken routers.
const MAX_ENTRIES: u32 = 128;

// Ordered by likelihood: TR-064 default port, Fritz!Box alternate, then common HTTP ports.
const CANDIDATES: &[(u16, &str)] = &[
    (49000, "/upnp/control/hosts"),
    (49000, "/tr64/upnp/control/Hosts"),
    (5000, "/upnp/control/hosts"),
    (80, "/upnp/control/hosts"),
    (8080, "/upnp/control/hosts"),
];

const WAN_CANDIDATES: &[(u16, &str)] = &[
    (49000, "/upnp/control/WANCommonInterfaceConfig"),
    (49000, "/tr64/upnp/control/WANCommonIFConfig1"),
    (5000, "/upnp/control/WANCommonInterfaceConfig"),
    (80, "/upnp/control/WANCommonInterfaceConfig"),
];

/// Returns `IP → (MAC, Hostname)` from the router's DHCP lease table.
/// Silently returns an empty map when the router does not speak TR-064/UPnP.
pub async fn interrogate_router(gateway_ip: &str) -> HashMap<String, (String, String)> {
    let client = create_client();

    for &(port, path) in CANDIDATES {
        let map = try_fetch_hosts(gateway_ip, port, path, &client).await;
        if !map.is_empty() {
            info!(
                "TR-064: {} entries from {}:{}{}",
                map.len(),
                gateway_ip,
                port,
                path
            );
            return map;
        }
    }

    info!("TR-064: no host table found on {}", gateway_ip);
    HashMap::new()
}

/// Returns `(TotalBytesReceived, TotalBytesSent)` from the router.
pub async fn get_bandwidth_stats(gateway_ip: &str) -> Option<(u64, u64)> {
    let client = create_client();

    for &(port, path) in WAN_CANDIDATES {
        if let Some(stats) = try_fetch_bandwidth(gateway_ip, port, path, &client).await {
            return Some(stats);
        }
    }

    // Fallback: try the main candidates with WAN paths
    for &(port, _) in CANDIDATES {
        for &(_, path) in WAN_CANDIDATES {
            if let Some(stats) = try_fetch_bandwidth(gateway_ip, port, path, &client).await {
                return Some(stats);
            }
        }
    }

    None
}

fn create_client() -> Client {
    Client::builder()
        .connect_timeout(Duration::from_millis(500))
        .timeout(Duration::from_secs(2))
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap_or_else(|_| Client::new())
}

// ─────────────────────────────────────────────────────────────────────────────

async fn try_fetch_bandwidth(
    gateway_ip: &str,
    port: u16,
    path: &str,
    client: &Client,
) -> Option<(u64, u64)> {
    let url = format!("http://{}:{}{}", gateway_ip, port, path);

    let rx_resp = soap_call(
        &url,
        &format!("{}#GetTotalBytesReceived", WAN_COMMON_URN),
        &soap_envelope("GetTotalBytesReceived", "", WAN_COMMON_URN),
        client,
    )
    .await?;

    let tx_resp = soap_call(
        &url,
        &format!("{}#GetTotalBytesSent", WAN_COMMON_URN),
        &soap_envelope("GetTotalBytesSent", "", WAN_COMMON_URN),
        client,
    )
    .await?;

    let rx = extract_xml_tag(&rx_resp, "NewTotalBytesReceived")?.parse::<u64>().ok()?;
    let tx = extract_xml_tag(&tx_resp, "NewTotalBytesSent")?.parse::<u64>().ok()?;

    Some((rx, tx))
}

// ─────────────────────────────────────────────────────────────────────────────

async fn try_fetch_hosts(
    gateway_ip: &str,
    port: u16,
    path: &str,
    client: &Client,
) -> HashMap<String, (String, String)> {
    let url = format!("http://{}:{}{}", gateway_ip, port, path);

    // Probe the endpoint with GetHostNumberOfEntries.  If the HTTP request fails
    // (connection refused, timeout) the endpoint is not available and we return
    // immediately without spending time on the entry loop.
    let count_resp = soap_call(
        &url,
        &format!("{}#GetHostNumberOfEntries", SERVICE_URN),
        &soap_envelope("GetHostNumberOfEntries", "", SERVICE_URN),
        client,
    )
    .await;

    let count = match count_resp {
        // Endpoint reachable and action supported.
        Some(ref r) => {
            extract_xml_tag(r, "NewHostNumberOfEntries")
                .and_then(|v| v.parse::<u32>().ok())
                // Action may return a SOAP fault but the endpoint is alive — loop anyway.
                .unwrap_or(MAX_ENTRIES)
        }
        // Endpoint not reachable (refused / timeout) — skip this candidate.
        None => return HashMap::new(),
    };

    let limit = count.min(MAX_ENTRIES);
    info!(
        "TR-064: {}:{}{} → {} entries",
        gateway_ip, port, path, limit
    );

    let mut map = HashMap::new();
    for idx in 0..limit {
        let args = format!("<NewIndex>{}</NewIndex>", idx);
        let resp = match soap_call(
            &url,
            &format!("{}#GetGenericHostEntry", SERVICE_URN),
            &soap_envelope("GetGenericHostEntry", &args, SERVICE_URN),
            client,
        )
        .await
        {
            Some(r) => r,
            None => break, // connection dropped mid-loop
        };

        // Error 713 = SpecifiedArrayIndexInvalid → walked past the last entry.
        if resp.contains("<errorCode>713</errorCode>") {
            break;
        }

        let ip = extract_xml_tag(&resp, "NewIPAddress").unwrap_or_default();
        let mac = extract_xml_tag(&resp, "NewMACAddress").unwrap_or_default();
        let hostname = extract_xml_tag(&resp, "NewHostName").unwrap_or_default();

        if !ip.is_empty() {
            map.insert(ip, (mac, hostname));
        }
    }

    map
}

// ─────────────────────────────────────────────────────────────────────────────

fn soap_envelope(action: &str, args: &str, service_urn: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?><s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/"><s:Body><u:{action} xmlns:u="{service_urn}">{args}</u:{action}></s:Body></s:Envelope>"#
    )
}

async fn soap_call(url: &str, action: &str, body: &str, client: &Client) -> Option<String> {
    client
        .post(url)
        .header("Content-Type", r#"text/xml; charset="utf-8""#)
        .header("SOAPAction", format!(r#""{action}""#))
        .body(body.to_string())
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()
}

/// Finds `<tag>value</tag>` in an XML string without an XML parser.
fn extract_xml_tag(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)?;
    let v = xml[start..start + end].trim().to_string();
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}
