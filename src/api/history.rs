use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use crate::AppState;
use crate::api::error::{ApiResult, ApiError};

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
    pub open_ports: Option<String>,
    pub mac: String,
}

/// Fetches paginated unified historical logs from the event log registry
pub async fn get_events(
    State(state): State<AppState>,
    Query(query): Query<HistoryQuery>,
) -> ApiResult<impl IntoResponse> {
    let limit = query.limit;
    let offset = query.offset;

    let events: Vec<HistoricalEvent> = state.db.execute(move |conn| {
        // Join with devices table to enrich the history log with hardware identity contexts
        let sql = "
            SELECT 
                e.id, e.event_type, e.device_id, e.timestamp, e.details,
                d.mac, d.last_ip, d.hostname
            FROM device_events e
            LEFT JOIN devices d ON e.device_id = d.id
            ORDER BY e.timestamp DESC
            LIMIT ?1 OFFSET ?2";

        let mut stmt = conn.prepare(sql)?;

        let rows = stmt.query_map(rusqlite::params![limit, offset], |row| {
            Ok(HistoricalEvent {
                id: row.get::<_, i64>(0).unwrap_or(0),
                event_type: row.get::<_, String>(1).unwrap_or_default(),
                device_id: row.get::<_, Option<i64>>(2).ok().flatten(),
                timestamp: row.get::<_, i64>(3).unwrap_or(0),
                details: row.get::<_, String>(4).unwrap_or_default(),
                mac: row.get::<_, Option<String>>(5).ok().flatten(),
                ip: row.get::<_, Option<String>>(6).ok().flatten(),
                hostname: row.get::<_, Option<String>>(7).ok().flatten(),
            })
        })?;

        Ok::<_, rusqlite::Error>(rows.flatten().collect())
    }).await.map_err(ApiError::from)?;

    Ok(Json(events))
}

/// Fetches paginated scan history
pub async fn get_history(
    State(state): State<AppState>,
    Query(query): Query<HistoryQuery>,
) -> ApiResult<impl IntoResponse> {
    let limit = query.limit;
    let offset = query.offset;

    let entries: Vec<ScanHistoryEntry> = state.db.execute(move |conn| {
        let sql = "
            SELECT
                h.scan_id, h.scanned_at, h.ip, h.is_online, h.latency_ms, h.open_ports, d.mac
            FROM scan_history h
            JOIN devices d ON h.device_id = d.id
            ORDER BY h.scanned_at DESC
            LIMIT ?1 OFFSET ?2";

        let mut stmt = conn.prepare(sql)?;

        let rows = stmt.query_map(rusqlite::params![limit, offset], |row| {
            Ok(ScanHistoryEntry {
                scan_id: row.get::<_, String>(0).unwrap_or_default(),
                scanned_at: row.get::<_, i64>(1).unwrap_or(0),
                ip: row.get::<_, String>(2).unwrap_or_default(),
                is_online: row.get::<_, i32>(3).unwrap_or(0) != 0,
                latency_ms: row.get::<_, Option<f64>>(4).ok().flatten(),
                open_ports: row.get::<_, Option<String>>(5).ok().flatten(),
                mac: row.get::<_, String>(6).unwrap_or_default(),
            })
        })?;

        Ok::<_, rusqlite::Error>(rows.flatten().collect())
    }).await.map_err(ApiError::from)?;

    Ok(Json(entries))
}

/// Fetches scan history for a specific device by MAC
pub async fn get_device_history(
    State(state): State<AppState>,
    Path(mac): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> ApiResult<impl IntoResponse> {
    let limit = query.limit;
    let offset = query.offset;
    let normalized = crate::scanner::arp::normalize_mac(&mac);

    let entries: Vec<ScanHistoryEntry> = state.db.execute(move |conn| {
        let sql = "
            SELECT
                h.scan_id, h.scanned_at, h.ip, h.is_online, h.latency_ms, h.open_ports, d.mac
            FROM scan_history h
            JOIN devices d ON h.device_id = d.id
            WHERE d.mac = ?1
            ORDER BY h.scanned_at DESC
            LIMIT ?2 OFFSET ?3";

        let mut stmt = conn.prepare(sql)?;

        let rows = stmt.query_map(rusqlite::params![normalized, limit, offset], |row| {
            Ok(ScanHistoryEntry {
                scan_id: row.get::<_, String>(0).unwrap_or_default(),
                scanned_at: row.get::<_, i64>(1).unwrap_or(0),
                ip: row.get::<_, String>(2).unwrap_or_default(),
                is_online: row.get::<_, i32>(3).unwrap_or(0) != 0,
                latency_ms: row.get::<_, Option<f64>>(4).ok().flatten(),
                open_ports: row.get::<_, Option<String>>(5).ok().flatten(),
                mac: row.get::<_, String>(6).unwrap_or_default(),
            })
        })?;

        Ok::<_, rusqlite::Error>(rows.flatten().collect())
    }).await.map_err(ApiError::from)?;

    Ok(Json(entries))
}


