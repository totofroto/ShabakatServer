use axum::{
    extract::State,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::api::error::{ApiError, ApiResult};
use crate::AppState;

#[derive(Serialize, Deserialize)]
pub struct ProviderPayload {
    pub id: String,
    pub enabled: bool,
    pub config_json: Value,
}

/// Retrieves saved channels configuration state
pub async fn get_notification_config(
    State(state): State<AppState>,
) -> ApiResult<impl IntoResponse> {
    let list = state
        .db
        .execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, enabled, config_json FROM notification_providers",
            )?;

            let rows = stmt.query_map(rusqlite::params![], |row| {
                let id: String = row.get(0).unwrap_or_default();
                let name: String = row.get(1).unwrap_or_default();
                let enabled_int: i64 = row.get(2).unwrap_or(0);
                let config_str: String = row.get(3).unwrap_or_default();
                let config_json: Value = serde_json::from_str(&config_str).unwrap_or(Value::Null);

                Ok(serde_json::json!({
                    "id": id,
                    "name": name,
                    "enabled": enabled_int == 1,
                    "config": config_json
                }))
            })?;

            let mut list = Vec::new();
            for i in rows {
                list.push(i?);
            }
            Ok::<_, rusqlite::Error>(list)
        })
        .await.map_err(ApiError::from)?;

    Ok(Json(list))
}

/// Registers, modifies, or tests connection states dynamically
pub async fn update_notification_config(
    State(state): State<AppState>,
    Json(payload): Json<ProviderPayload>,
) -> ApiResult<impl IntoResponse> {
    let enabled_int = if payload.enabled { 1 } else { 0 };
    let config_str = payload.config_json.to_string();
    let provider_id = payload.id.clone();

    let affected = state
        .db
        .execute(move |conn| {
            let affected = conn.execute(
                "UPDATE notification_providers SET enabled = ?1, config_json = ?2 WHERE id = ?3",
                rusqlite::params![enabled_int, config_str, provider_id],
            )?;
            Ok::<_, rusqlite::Error>(affected)
        })
        .await.map_err(ApiError::from)?;

    if affected > 0 {
        log::info!(
            "[FLIGHT_RECORDER] Updated alert registration parameters for target provider: {}",
            payload.id
        );
        Ok(Json(serde_json::json!({"status": "success", "message": "Configuration successfully saved"})))
    } else {
        Err(ApiError::NotFound("Target notification provider missing".to_string()))
    }
}

