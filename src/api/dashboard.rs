use axum::{extract::State, response::Json};
use serde_json::{json, Value};
use crate::AppState;
use crate::storage::system_status;

pub async fn get_system_status(State(state): State<AppState>) -> Json<Value> {
    match system_status::get_system_status(&state.db).await {
        Ok(Some(status)) => Json(json!(status)),
        Ok(None) => Json(json!({
            "score": 0,
            "performanceScore": 0,
            "latencyScore": 0,
            "securityScore": 0,
            "lastUpdated": 0
        })),
        Err(e) => Json(json!({"error": e})),
    }
}

pub async fn get_router_bandwidth(State(state): State<AppState>) -> Json<Value> {
    let b = state.bandwidth.lock().unwrap();
    match *b {
        Some(ref stats) => Json(json!(stats)),
        None => Json(json!({
            "rxBytes": 0,
            "txBytes": 0,
            "timestamp": 0
        })),
    }
}

pub async fn get_system_telemetry(State(state): State<AppState>) -> Json<Value> {
    let t = state.system_telemetry.lock().unwrap();
    match *t {
        Some(ref telemetry) => Json(json!(telemetry)),
        None => Json(json!({
            "timestamp": 0,
            "interfaces": []
        })),
    }
}
