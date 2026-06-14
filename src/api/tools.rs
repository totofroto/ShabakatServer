use std::time::{Duration, Instant};

use axum::{
    response::IntoResponse,
    http::StatusCode,
    Json,
};
use dns_lookup::{lookup_addr, lookup_host};
use ipnet::Ipv4Net;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::error::{ApiError, ApiResult};

// ── POST /api/tools/test-notification ────────────────────────────────────────

#[derive(Deserialize)]
pub struct TestNotificationReq {
    pub id: String,
    pub config: Value,
}

pub async fn test_notification(Json(body): Json<TestNotificationReq>) -> ApiResult<impl IntoResponse> {
    let provider: Option<Box<dyn crate::notifications::NotificationProvider>> = match body.id.as_str() {
        "telegram" => Some(Box::new(crate::notifications::telegram::TelegramProvider)),
        "smtp" => Some(Box::new(crate::notifications::smtp::SmtpProvider)),
        "webhook_ntfy" => Some(Box::new(crate::notifications::webhook::WebhookProvider)),
        _ => None,
    };

    if let Some(p) = provider {
        p.dispatch("Shabakat Test", "This is a connection verification message from your Shabakat Passive Sentry Hub.", &body.config)
            .await
            .map_err(|e| ApiError::Internal(format!("Provider error: {}", e)))?;
        Ok((StatusCode::OK, "Test message dispatched successfully").into_response())
    } else {
        Err(ApiError::BadRequest("Unknown provider ID".into()))
    }
}

// ── POST /api/tools/ping ─────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct PingReq { pub ip: String }

pub async fn ping(Json(body): Json<PingReq>) -> ApiResult<impl IntoResponse> {
    let target = body.ip.trim().to_string();
    let out = tokio::task::spawn_blocking(move || {
        std::process::Command::new("ping")
            .args(["-c", "4", "-W", "2", &target])
            .output()
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ApiError::Internal("'ping' utility is not installed on the server.".into())
        } else {
            ApiError::Internal(e.to_string())
        }
    })?;

    let text = if out.status.success() {
        String::from_utf8_lossy(&out.stdout).into_owned()
    } else {
        let err = String::from_utf8_lossy(&out.stderr).into_owned();
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        if err.is_empty() { stdout } else { err }
    };
    Ok(Json(json!(text)))
}

// ── POST /api/tools/tcp-ping ─────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct TcpPingReq { pub ip: String, pub port: u16 }

pub async fn tcp_ping(Json(body): Json<TcpPingReq>) -> ApiResult<impl IntoResponse> {
    let addr = format!("{}:{}", body.ip.trim(), body.port);
    let ms = tokio::task::spawn_blocking(move || {
        use std::net::ToSocketAddrs;
        let sa = addr.to_socket_addrs()?.next()
            .ok_or_else(|| std::io::Error::other("could not resolve"))?;
        let t = Instant::now();
        std::net::TcpStream::connect_timeout(&sa, Duration::from_secs(3))?;
        Ok::<u128, std::io::Error>(t.elapsed().as_millis())
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(json!(ms)))
}

// ── POST /api/tools/dns ──────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct DnsReq { pub target: String }

pub async fn dns(Json(body): Json<DnsReq>) -> ApiResult<impl IntoResponse> {
    let trimmed = body.target.trim().to_string();
    let results = tokio::task::spawn_blocking(move || -> Result<Vec<String>, String> {
        if let Ok(ip) = trimmed.parse::<std::net::IpAddr>() {
            let hostname = lookup_addr(&ip).map_err(|e| e.to_string())?;
            Ok(vec![hostname])
        } else {
            let addrs = lookup_host(&trimmed).map_err(|e| e.to_string())?;
            Ok(addrs.into_iter().map(|a| a.to_string()).collect())
        }
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .map_err(ApiError::Internal)?;

    Ok(Json(json!(results)))
}

// ── POST /api/tools/wake ─────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct WakeReq { pub mac: String }

pub async fn wake(Json(body): Json<WakeReq>) -> ApiResult<impl IntoResponse> {
    let mac = body.mac.trim().replace([':', '-', '.'], "");
    if mac.len() != 12 {
        return Err(ApiError::BadRequest("Invalid MAC address format".into()));
    }
    let bytes: Vec<u8> = (0..6)
        .map(|i| u8::from_str_radix(&mac[i * 2..i * 2 + 2], 16))
        .collect::<Result<Vec<u8>, _>>()
        .map_err(|_| ApiError::BadRequest("Invalid MAC address hex".into()))?;
    
    if bytes.len() != 6 {
        return Err(ApiError::BadRequest("Invalid MAC address hex".into()));
    }
    
    let arr: [u8; 6] = bytes.try_into()
        .map_err(|_| ApiError::Internal("Failed to convert MAC bytes".into()))?;

    let pkt = wake_on_lan::MagicPacket::new(&arr);
    pkt.send().map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(json!("Wake-on-LAN packet sent successfully.")))
}

// ── POST /api/tools/portscan ─────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct PortscanReq { pub ip: String }

const SCAN_PORTS: &[u16] = &[21, 22, 23, 25, 53, 80, 110, 143, 443, 445, 3389, 5000, 8080, 8443];

pub async fn portscan(Json(body): Json<PortscanReq>) -> ApiResult<impl IntoResponse> {
    let ip = body.ip.trim().to_string();
    let ip_addr: std::net::IpAddr = ip.parse()
        .map_err(|_| ApiError::BadRequest("invalid IP address".into()))?;
    
    let open_ports = crate::scanner::deep::scan_ports(ip_addr, SCAN_PORTS).await;
    Ok(Json(json!({ "openPorts": open_ports })))
}

// ── POST /api/tools/portscan-all ─────────────────────────────────────────────

#[derive(Deserialize)]
pub struct PortscanAllReq { pub ips: Vec<String> }

pub async fn portscan_all(Json(body): Json<PortscanAllReq>) -> ApiResult<impl IntoResponse> {
    let mut results = Vec::new();
    
    for ip in body.ips {
        let ip_addr: std::net::IpAddr = match ip.parse() {
            Ok(a) => a,
            Err(_) => continue,
        };
        
        let open_ports = crate::scanner::deep::scan_ports(ip_addr, SCAN_PORTS).await;
        results.push(json!({ "ip": ip, "openPorts": open_ports }));
    }
    
    Ok(Json(json!(results)))
}

// ── POST /api/tools/subnet-calc ──────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SubnetReq { pub cidr: String }

pub async fn subnet_calc(Json(body): Json<SubnetReq>) -> ApiResult<impl IntoResponse> {
    let net: Ipv4Net = body.cidr.trim().parse()
        .map_err(|e: ipnet::AddrParseError| ApiError::BadRequest(e.to_string()))?;
    let hosts: u64 = net.hosts().count() as u64;
    Ok(Json(json!({
        "network":   net.network().to_string(),
        "broadcast": net.broadcast().to_string(),
        "mask":      net.netmask().to_string(),
        "prefix":    net.prefix_len(),
        "hosts":     hosts,
    })))
}

// ── POST /api/tools/ssl ──────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SslReq { pub domain: String }

pub async fn ssl(Json(body): Json<SslReq>) -> ApiResult<impl IntoResponse> {
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

pub async fn whois(Json(body): Json<WhoisReq>) -> ApiResult<impl IntoResponse> {
    let url = format!("https://networkcalc.com/api/whois/{}", body.domain.trim());
    proxy_get(url).await
}

// ── POST /api/tools/ip-geo ───────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct GeoReq { pub ip: Option<String> }

pub async fn ip_geo(Json(body): Json<GeoReq>) -> ApiResult<impl IntoResponse> {
    let url = match body.ip.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(ip) => format!("http://ip-api.com/json/{}", ip.trim()),
        None => "http://ip-api.com/json/".to_string(),
    };
    proxy_get(url).await
}

// ── POST /api/tools/mac-lookup ───────────────────────────────────────────────

#[derive(Deserialize)]
pub struct MacReq { pub mac: String }

pub async fn mac_lookup(Json(body): Json<MacReq>) -> ApiResult<impl IntoResponse> {
    // First try local vendor map (fast, no rate limits)
    let local = crate::scanner::vendor_name_from_mac(&body.mac);
    if local != "Unknown" {
        return Ok(Json(json!(local)).into_response());
    }
    // Fall back to public API
    let url = format!("https://api.macvendors.com/{}", body.mac.trim());
    proxy_get(url).await
}

// ── POST /api/tools/headers ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct HeadersReq { pub url: String }

pub async fn headers(Json(body): Json<HeadersReq>) -> ApiResult<impl IntoResponse> {
    let url = if body.url.starts_with("http://") || body.url.starts_with("https://") {
        body.url.clone()
    } else {
        format!("https://{}", body.url)
    };
    let resp = reqwest::Client::new().get(&url).send().await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let hdrs: Vec<(String, String)> = resp.headers().iter().map(|(k, v)| {
        (k.as_str().to_string(), v.to_str().unwrap_or("<binary>").to_string())
    }).collect();
    Ok(Json(json!(hdrs)))
}

// ── Shared HTTP proxy helper ──────────────────────────────────────────────────

async fn proxy_get(url: String) -> ApiResult<axum::response::Response> {
    let resp = reqwest::get(&url).await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let text = resp.text().await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(json!(text)).into_response())
}
