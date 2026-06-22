use axum::{
    extract::State,
    response::IntoResponse,
    Json,
};
use crate::AppState;
use crate::api::error::{ApiResult, ApiError};
use crate::api::history::HistoricalEvent;

/// Fetches unacknowledged new device alerts
pub async fn get_alerts(State(state): State<AppState>) -> ApiResult<impl IntoResponse> {
    let events: Vec<HistoricalEvent> = state.db.execute(move |conn| {
        let sql = "
            SELECT 
                e.id, e.event_type, e.device_id, e.timestamp, e.details,
                d.mac, d.last_ip, d.hostname
            FROM device_events e
            JOIN devices d ON e.device_id = d.id
            WHERE e.event_type = 'new_device' AND d.acknowledged = 0
            ORDER BY e.timestamp DESC
        ";
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map([], |row| {
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
