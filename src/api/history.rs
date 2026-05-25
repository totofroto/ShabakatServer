use axum::{
    extract::{Path, Query, State},
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
#[serde(rename_all = "camelCase")]
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanHistoryEntry {
    pub scan_id: String,
    pub scanned_at: i64,
    pub ip: String,
    pub is_online: bool,
    pub latency_ms: Option<f64>,
    pub mac: String,
}

/// Fetches paginated unified historical logs from the event log registry
pub async fn get_events(
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

/// Fetches paginated scan history
pub async fn get_history(
    State(state): State<AppState>,
    Query(query): Query<HistoryQuery>,
) -> impl IntoResponse {
    let conn = match state.db.connect().await {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("DB Connection error: {}", e)).into_response(),
    };

    let sql = "
        SELECT 
            h.scan_id, h.scanned_at, h.ip, h.is_online, h.latency_ms, d.mac
        FROM scan_history h
        JOIN devices d ON h.device_id = d.id
        ORDER BY h.scanned_at DESC
        LIMIT ?1 OFFSET ?2";

    let mut stmt = match conn.prepare(sql).await {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Statement error: {}", e)).into_response(),
    };

    let mut rows = match stmt.query(libsql::params![query.limit, query.offset]).await {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Query error: {}", e)).into_response(),
    };

    let mut entries = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
        entries.push(ScanHistoryEntry {
            scan_id: row.get(0).unwrap_or_default(),
            scanned_at: row.get(1).unwrap_or(0),
            ip: row.get(2).unwrap_or_default(),
            is_online: row.get::<i32>(3).unwrap_or(0) != 0,
            latency_ms: row.get(4).ok(),
            mac: row.get(5).unwrap_or_default(),
        });
    }

    Json(entries).into_response()
}

/// Fetches scan history for a specific device by MAC
pub async fn get_device_history(
    State(state): State<AppState>,
    Path(mac): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> impl IntoResponse {
    let conn = match state.db.connect().await {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("DB Connection error: {}", e)).into_response(),
    };

    let sql = "
        SELECT 
            h.scan_id, h.scanned_at, h.ip, h.is_online, h.latency_ms, d.mac
        FROM scan_history h
        JOIN devices d ON h.device_id = d.id
        WHERE d.mac = ?1
        ORDER BY h.scanned_at DESC
        LIMIT ?2 OFFSET ?3";

    let mut stmt = match conn.prepare(sql).await {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Statement error: {}", e)).into_response(),
    };

    let mut rows = match stmt.query(libsql::params![mac, query.limit, query.offset]).await {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Query error: {}", e)).into_response(),
    };

    let mut entries = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
        entries.push(ScanHistoryEntry {
            scan_id: row.get(0).unwrap_or_default(),
            scanned_at: row.get(1).unwrap_or(0),
            ip: row.get(2).unwrap_or_default(),
            is_online: row.get::<i32>(3).unwrap_or(0) != 0,
            latency_ms: row.get(4).ok(),
            mac: row.get(5).unwrap_or_default(),
        });
    }

    Json(entries).into_response()
}
