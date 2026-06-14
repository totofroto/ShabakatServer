use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::api::error::{ApiError, ApiResult};
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
) -> ApiResult<impl IntoResponse> {
    let providers = crate::storage::providers::get_active_providers(state.db.clone())
        .await
        .map_err(ApiError::from)?;

    if providers.is_empty() {
        return Ok(Json(DeviceDnsStats {
            ip,
            total_queries: 0,
            blocked_queries: 0,
        }));
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
        .map_err(|e| ApiError::Internal(format!("Failed to build client: {}", e)))?;

    let mut request = client.get(&url);

    if let (Some(u), Some(p)) = (&provider.username, &provider.password) {
        if !u.is_empty() {
            request = request.basic_auth(u, Some(p));
        }
    }

    let resp = request
        .send()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to reach AdGuard: {}", e)))?;

    if !resp.status().is_success() {
        let status = resp.status();
        return Err(ApiError::Internal(format!(
            "AdGuard returned error: {}",
            status
        )));
    }

    let query_log: AdGuardQueryLog = resp
        .json()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to parse AdGuard response: {}", e)))?;

    let total_queries = query_log.data.len();
    let blocked_queries = query_log
        .data
        .iter()
        .filter(|entry| is_blocked_reason(&entry.reason))
        .count();

    Ok(Json(DeviceDnsStats {
        ip,
        total_queries,
        blocked_queries,
    }))
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
