use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use crate::AppState;

#[derive(Deserialize)]
pub struct HistoryQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default = "default_offset")]
    pub offset: i64,
}

fn default_limit() -> i64 { 20 }
fn default_offset() -> i64 { 0 }

#[derive(Serialize)]
pub struct HistoricalEvent {
    pub id: i64,
    pub event_type: String,
    pub device_id: Option<i64>,
    pub mac: Option<String>,
    pub ip: Option<String>,
    pub hostname: Option<String>,
    pub timestamp: i64,
    pub details: String,
}

/// Fetches paginated unified historical logs from the event log registry
pub async fn get_history(
    State(state): State<AppState>,
    Query(query): Query<HistoryQuery>,
) -> impl IntoResponse {
    let conn = match state.db.connect().await {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("DB Connection error: {}", e)).into_response(),
    };

    // Join with devices table to enrich the history log with hardware identity contexts
    let sql = "
        SELECT 
            e.id, e.event_type, e.device_id, e.timestamp, e.details,
            d.mac, d.last_ip, d.hostname
        FROM device_events e
        LEFT JOIN devices d ON e.device_id = d.id
        ORDER BY e.timestamp DESC
        LIMIT ?1 OFFSET ?2";

    let mut stmt = match conn.prepare(sql).await {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Statement error: {}", e)).into_response(),
    };

    let mut rows = match stmt.query(libsql::params![query.limit, query.offset]).await {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Query error: {}", e)).into_response(),
    };

    let mut events = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
        events.push(HistoricalEvent {
            id: row.get(0).unwrap_or(0),
            event_type: row.get(1).unwrap_or_default(),
            device_id: row.get(2).ok(),
            timestamp: row.get(3).unwrap_or(0),
            details: row.get(4).unwrap_or_default(),
            mac: row.get(5).ok(),
            ip: row.get(6).ok(),
            hostname: row.get(7).ok(),
        });
    }

    Json(events).into_response()
}
