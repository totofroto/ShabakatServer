use std::time::{Duration, Instant};

use axum::{
    response::{IntoResponse, Json},
    http::StatusCode,
};
use dns_lookup::{lookup_addr, lookup_host};
use ipnet::Ipv4Net;
use serde::Deserialize;
use serde_json::json;

fn err500(msg: impl ToString) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg.to_string()})))
}

// ── POST /api/tools/ping ─────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct PingReq { pub ip: String }

pub async fn ping(Json(body): Json<PingReq>) -> impl IntoResponse {
    let target = body.ip.trim().to_string();
    match tokio::task::spawn_blocking(move || {
        std::process::Command::new("ping")
            .args(["-c", "4", "-W", "2", &target])
            .output()
    })
    .await
    {
        Ok(Ok(out)) => {
            let text = if out.status.success() {
                String::from_utf8_lossy(&out.stdout).into_owned()
            } else {
                let err = String::from_utf8_lossy(&out.stderr).into_owned();
                let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
                if err.is_empty() { stdout } else { err }
            };
            Json(json!(text)).into_response()
        }
        Ok(Err(e)) => err500(e).into_response(),
        Err(e) => err500(e).into_response(),
    }
}

// ── POST /api/tools/tcp-ping ─────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct TcpPingReq { pub ip: String, pub port: u16 }

pub async fn tcp_ping(Json(body): Json<TcpPingReq>) -> impl IntoResponse {
    let addr = format!("{}:{}", body.ip.trim(), body.port);
    match tokio::task::spawn_blocking(move || {
        use std::net::ToSocketAddrs;
        let sa = addr.to_socket_addrs()?.next()
            .ok_or_else(|| std::io::Error::other("could not resolve"))?;
        let t = Instant::now();
        std::net::TcpStream::connect_timeout(&sa, Duration::from_secs(3))?;
        Ok::<u128, std::io::Error>(t.elapsed().as_millis())
    })
    .await
    {
        Ok(Ok(ms)) => Json(json!(ms)).into_response(),
        Ok(Err(e)) => err500(e).into_response(),
        Err(e) => err500(e).into_response(),
    }
}

// ── POST /api/tools/dns ──────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct DnsReq { pub target: String }

pub async fn dns(Json(body): Json<DnsReq>) -> impl IntoResponse {
    let trimmed = body.target.trim().to_string();
    match tokio::task::spawn_blocking(move || -> Result<Vec<String>, String> {
        if let Ok(ip) = trimmed.parse::<std::net::IpAddr>() {
            let hostname = lookup_addr(&ip).map_err(|e| e.to_string())?;
            Ok(vec![hostname])
        } else {
            let addrs = lookup_host(&trimmed).map_err(|e| e.to_string())?;
            Ok(addrs.into_iter().map(|a| a.to_string()).collect())
        }
    })
    .await
    {
        Ok(Ok(results)) => Json(json!(results)).into_response(),
        Ok(Err(e)) => err500(e).into_response(),
        Err(e) => err500(e).into_response(),
    }
}

// ── POST /api/tools/wake ─────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct WakeReq { pub mac: String }

pub async fn wake(Json(body): Json<WakeReq>) -> impl IntoResponse {
    let mac = body.mac.trim().replace([':', '-', '.'], "");
    if mac.len() != 12 {
        return err500("Invalid MAC address format").into_response();
    }
    let bytes: Vec<u8> = match (0..6)
        .map(|i| u8::from_str_radix(&mac[i * 2..i * 2 + 2], 16))
        .collect::<Result<Vec<u8>, _>>() {
            Ok(b) => b,
            Err(_) => return err500("Invalid MAC address hex").into_response(),
        };
    
    if bytes.len() != 6 {
        return err500("Invalid MAC address hex").into_response();
    }
    
    let arr: [u8; 6] = match bytes.try_into() {
        Ok(a) => a,
        Err(_) => return err500("Failed to convert MAC bytes").into_response(),
    };
    let pkt = wake_on_lan::MagicPacket::new(&arr);
    match pkt.send() {
        Ok(()) => Json(json!("Wake-on-LAN packet sent successfully.")).into_response(),
        Err(e) => err500(e).into_response(),
    }
}

// ── POST /api/tools/portscan ─────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct PortscanReq { pub ip: String }

const SCAN_PORTS: &[u16] = &[21, 22, 23, 25, 53, 80, 110, 143, 443, 445, 3389, 5000, 8080, 8443];

pub async fn portscan(Json(body): Json<PortscanReq>) -> impl IntoResponse {
    let ip = body.ip.trim().to_string();
    let ip_addr: std::net::IpAddr = match ip.parse() {
        Ok(a) => a,
        Err(_) => return err500("invalid IP address").into_response(),
    };
    
    let open_ports = crate::scanner::deep::scan_ports(ip_addr, SCAN_PORTS).await;
    Json(json!({ "openPorts": open_ports })).into_response()
}

// ── POST /api/tools/portscan-all ─────────────────────────────────────────────

#[derive(Deserialize)]
pub struct PortscanAllReq { pub ips: Vec<String> }

pub async fn portscan_all(Json(body): Json<PortscanAllReq>) -> impl IntoResponse {
    let mut results = Vec::new();
    
    for ip in body.ips {
        let ip_addr: std::net::IpAddr = match ip.parse() {
            Ok(a) => a,
            Err(_) => continue,
        };
        
        let open_ports = crate::scanner::deep::scan_ports(ip_addr, SCAN_PORTS).await;
        results.push(json!({ "ip": ip, "openPorts": open_ports }));
    }
    
    Json(json!(results)).into_response()
}

// ── POST /api/tools/subnet-calc ──────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SubnetReq { pub cidr: String }

pub async fn subnet_calc(Json(body): Json<SubnetReq>) -> impl IntoResponse {
    let net: Ipv4Net = match body.cidr.trim().parse() {
        Ok(n) => n,
        Err(e) => return err500(e).into_response(),
    };
    let hosts: u64 = net.hosts().count() as u64;
    Json(json!({
        "network":   net.network().to_string(),
        "broadcast": net.broadcast().to_string(),
        "mask":      net.netmask().to_string(),
        "prefix":    net.prefix_len(),
        "hosts":     hosts,
    })).into_response()
}

// ── POST /api/tools/ssl ──────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SslReq { pub domain: String }

pub async fn ssl(Json(body): Json<SslReq>) -> impl IntoResponse {
    let clean = body.domain.trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string();
    let url = format!("https://networkcalc.com/api/security/certificate/{clean}");
    proxy_get(url).await
}

// ── POST /api/tools/whois ────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct WhoisReq { pub domain: String }

pub async fn whois(Json(body): Json<WhoisReq>) -> impl IntoResponse {
    let url = format!("https://networkcalc.com/api/whois/{}", body.domain.trim());
    proxy_get(url).await
}

// ── POST /api/tools/ip-geo ───────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct GeoReq { pub ip: Option<String> }

pub async fn ip_geo(Json(body): Json<GeoReq>) -> impl IntoResponse {
    let url = match body.ip.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(ip) => format!("http://ip-api.com/json/{}", ip.trim()),
        None => "http://ip-api.com/json/".to_string(),
    };
    proxy_get(url).await
}

// ── POST /api/tools/mac-lookup ───────────────────────────────────────────────

#[derive(Deserialize)]
pub struct MacReq { pub mac: String }

pub async fn mac_lookup(Json(body): Json<MacReq>) -> impl IntoResponse {
    // First try local vendor map (fast, no rate limits)
    let local = crate::scanner::vendor_name_from_mac(&body.mac);
    if local != "Unknown" {
        return Json(json!(local)).into_response();
    }
    // Fall back to public API
    let url = format!("https://api.macvendors.com/{}", body.mac.trim());
    proxy_get(url).await
}

// ── POST /api/tools/headers ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct HeadersReq { pub url: String }

pub async fn headers(Json(body): Json<HeadersReq>) -> impl IntoResponse {
    let url = if body.url.starts_with("http://") || body.url.starts_with("https://") {
        body.url.clone()
    } else {
        format!("https://{}", body.url)
    };
    match reqwest::Client::new().get(&url).send().await {
        Ok(resp) => {
            let hdrs: Vec<(String, String)> = resp.headers().iter().map(|(k, v)| {
                (k.as_str().to_string(), v.to_str().unwrap_or("<binary>").to_string())
            }).collect();
            Json(json!(hdrs)).into_response()
        }
        Err(e) => err500(e).into_response(),
    }
}

// ── Shared HTTP proxy helper ──────────────────────────────────────────────────

async fn proxy_get(url: String) -> axum::response::Response {
    match reqwest::get(&url).await {
        Ok(resp) => match resp.text().await {
            Ok(text) => Json(json!(text)).into_response(),
            Err(e) => err500(e).into_response(),
        },
        Err(e) => err500(e).into_response(),
    }
}
