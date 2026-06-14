use axum::{extract::State, response::IntoResponse, Json};
use serde_json::json;

use crate::api::error::{ApiError, ApiResult};
use crate::storage::system_status;
use crate::AppState;

pub async fn get_system_status(State(state): State<AppState>) -> ApiResult<impl IntoResponse> {
    match system_status::get_system_status(&state.db).await? {
        Some(status) => Ok(Json(json!(status))),
        None => Ok(Json(json!({
            "score": 0,
            "performanceScore": 0,
            "latencyScore": 0,
            "securityScore": 0,
            "lastUpdated": 0
        }))),
    }
}

pub async fn get_router_bandwidth(State(state): State<AppState>) -> ApiResult<impl IntoResponse> {
    let b = state
        .bandwidth
        .lock()
        .map_err(|e| ApiError::Internal(format!("Mutex error: {}", e)))?;
    match *b {
        Some(ref stats) => Ok(Json(json!(stats))),
        None => Ok(Json(json!({
            "rxBytes": 0,
            "txBytes": 0,
            "timestamp": 0
        }))),
    }
}

pub async fn get_system_telemetry(State(state): State<AppState>) -> ApiResult<impl IntoResponse> {
    let t = state
        .system_telemetry
        .lock()
        .map_err(|e| ApiError::Internal(format!("Mutex error: {}", e)))?;
    match *t {
        Some(ref telemetry) => Ok(Json(json!(telemetry))),
        None => Ok(Json(json!({
            "timestamp": 0,
            "interfaces": []
        }))),
    }
}
