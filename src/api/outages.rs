use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde_json::json;

use crate::AppState;

pub async fn list_outages(State(state): State<AppState>) -> impl IntoResponse {
    let conn = match state.db.connect().await {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    };

    let mut rows = match conn
        .query(
            "SELECT id, started_at, ended_at, duration_ms
             FROM outages
             ORDER BY started_at DESC
             LIMIT 50",
            (),
        ).await {
            Ok(r) => r,
            Err(e) => {
                log::error!("Outages query failed: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
            }
        };

    let mut results = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
        let id: i64 = row.get::<Option<i64>>(0).unwrap_or_default().unwrap_or_default();
        let started_at: i64 = row.get::<Option<i64>>(1).unwrap_or_default().unwrap_or_default();
        let ended_at: Option<i64> = row.get::<Option<i64>>(2).unwrap_or_default();
        let duration_ms: Option<i64> = row.get::<Option<i64>>(3).unwrap_or_default();

        results.push(json!({
            "id":         id,
            "startedAt":  started_at,
            "endedAt":    ended_at,
            "durationMs": duration_ms,
            "ongoing":    ended_at.is_none()
        }));
    }

    Json(json!(results)).into_response()
}
