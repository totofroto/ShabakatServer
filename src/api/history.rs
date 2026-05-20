use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Deserialize;
use serde_json::json;

use crate::{
    storage::history as hist_store,
    AppState,
};

#[derive(Deserialize)]
pub struct HistoryQuery {
    pub mac: Option<String>,
    pub limit: Option<i64>,
}

pub async fn list_history(
    State(state): State<AppState>,
    Query(q): Query<HistoryQuery>,
) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(50);
    
    // If mac is provided as a query param, use the new async get_device_history
    if let Some(mac) = q.mac {
        match hist_store::get_device_history(state.db.clone(), mac, limit).await {
            Ok(rows) => Json(rows).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
        }
    } else {
        // Fallback to global history using the new async list_history
        let conn = match state.db.connect().await {
            Ok(c) => c,
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
        };
        match hist_store::list_history(&conn, None, None, None, limit).await {
            Ok(rows) => Json(json!(rows)).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
        }
    }
}

pub async fn get_device_history(
    State(state): State<AppState>,
    Path(mac): Path<String>,
    Query(q): Query<HistoryQuery>,
) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(50);
    match hist_store::get_device_history(state.db.clone(), mac, limit).await {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

#[derive(Deserialize)]
pub struct EventsQuery {
    pub limit: Option<i64>,
}

pub async fn list_events(
    State(state): State<AppState>,
    Query(q): Query<EventsQuery>,
) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(50).min(500);
    let conn = match state.db.connect().await {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    };
    match hist_store::list_events(&conn, limit).await {
        Ok(rows) => Json(json!(rows)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}
