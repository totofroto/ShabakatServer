use axum::{
    extract::State,
    response::IntoResponse,
    Json,
};
use serde_json::json;

use crate::api::error::{ApiError, ApiResult};
use crate::AppState;

pub async fn list_outages(State(state): State<AppState>) -> ApiResult<impl IntoResponse> {
    let results = state
        .db
        .execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, started_at, ended_at, duration_ms
             FROM outages
             ORDER BY started_at DESC
             LIMIT 50",
            )?;

            let rows = stmt.query_map([], |row| {
                let id: i64 = row
                    .get::<_, Option<i64>>(0)
                    .unwrap_or_default()
                    .unwrap_or_default();
                let started_at: i64 = row
                    .get::<_, Option<i64>>(1)
                    .unwrap_or_default()
                    .unwrap_or_default();
                let ended_at: Option<i64> = row.get::<_, Option<i64>>(2).unwrap_or_default();
                let duration_ms: Option<i64> = row.get::<_, Option<i64>>(3).unwrap_or_default();

                Ok(json!({
                    "id":         id,
                    "startedAt":  started_at,
                    "endedAt":    ended_at,
                    "durationMs": duration_ms,
                    "ongoing":    ended_at.is_none()
                }))
            })?;

            let mut results = Vec::new();
            for i in rows {
                results.push(i?);
            }
            Ok::<_, rusqlite::Error>(results)
        })
        .await.map_err(ApiError::from)?;

    Ok(Json(json!(results)))
}

