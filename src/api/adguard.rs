use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::AppState;

#[derive(Deserialize)]
struct AdGuardQueryLog {
    data: Vec<AdGuardQueryEntry>,
}

#[derive(Deserialize)]
struct AdGuardQueryEntry {
    reason: String,
}

#[derive(Serialize)]
pub struct DeviceDnsStats {
    pub ip: String,
    pub total_queries: usize,
    pub blocked_queries: usize,
}

pub async fn get_device_dns_stats(
    State(state): State<AppState>,
    Path(ip): Path<String>,
) -> impl IntoResponse {
    let providers = match crate::storage::providers::get_active_providers(state.db.clone()).await {
        Ok(p) => p,
        Err(e) => {
            log::error!("[ADGUARD] Failed to fetch DNS providers: {}", e);
            return Json(DeviceDnsStats {
                ip,
                total_queries: 0,
                blocked_queries: 0,
            })
            .into_response();
        }
    };

    if providers.is_empty() {
        return Json(DeviceDnsStats {
            ip,
            total_queries: 0,
            blocked_queries: 0,
        })
        .into_response();
    }

    // For now, use the first active provider.
    let provider = &providers[0];

    // AdGuard Home API URL
    let url = format!(
        "http://{}:{}/control/querylog?search={}",
        provider.ip, provider.port, ip
    );

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let mut request = client.get(&url);

    if let (Some(u), Some(p)) = (&provider.username, &provider.password) {
        if !u.is_empty() {
            request = request.basic_auth(u, Some(p));
        }
    }

    let resp = match request.send().await {
        Ok(r) => r,
        Err(e) => {
            log::warn!(
                "[FLIGHT_RECORDER] AdGuard lookup failed for IP {}: {}",
                ip,
                e
            );
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to reach AdGuard: {}", e) })),
            )
                .into_response();
        }
    };

    if !resp.status().is_success() {
        let status = resp.status();
        log::warn!(
            "[FLIGHT_RECORDER] AdGuard returned error status for IP {}: {}",
            ip,
            status
        );
        return (
            axum::http::StatusCode::from_u16(status.as_u16())
                .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
            Json(json!({ "error": format!("AdGuard returned error: {}", status) })),
        )
            .into_response();
    }

    let query_log: AdGuardQueryLog = match resp.json().await {
        Ok(log) => log,
        Err(e) => {
            log::warn!(
                "[FLIGHT_RECORDER] AdGuard parsing failed for IP {}: {}. Returning default stats.",
                ip,
                e
            );
            return Json(DeviceDnsStats {
                ip,
                total_queries: 0,
                blocked_queries: 0,
            })
            .into_response();
        }
    };

    let total_queries = query_log.data.len();
    let blocked_queries = query_log
        .data
        .iter()
        .filter(|entry| is_blocked_reason(&entry.reason))
        .count();

    Json(DeviceDnsStats {
        ip,
        total_queries,
        blocked_queries,
    })
    .into_response()
}

fn is_blocked_reason(reason: &str) -> bool {
    matches!(
        reason,
        "FilteredBlackList"
            | "FilteredSafeBrowsing"
            | "FilteredSafeSearch"
            | "FilteredParental"
            | "FilteredBlockedService"
    )
}
