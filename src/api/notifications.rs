use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
) -> impl IntoResponse {
    let conn = match state.db.connect().await {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Database connection failure: {}", e)).into_response(),
    };

    let mut stmt = match conn.prepare("SELECT id, name, enabled, config_json FROM notification_providers").await {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Statement preparation failure: {}", e)).into_response(),
    };

    let mut rows = match stmt.query(libsql::params![]).await {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Query execution error: {}", e)).into_response(),
    };

    let mut list = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
        let id: String = row.get(0).unwrap_or_default();
        let name: String = row.get(1).unwrap_or_default();
        let enabled_int: i64 = row.get(2).unwrap_or(0);
        let config_str: String = row.get(3).unwrap_or_default();
        let config_json: Value = serde_json::from_str(&config_str).unwrap_or(Value::Null);

        list.push(serde_json::json!({
            "id": id,
            "name": name,
            "enabled": enabled_int == 1,
            "config": config_json
        }));
    }

    Json(list).into_response()
}

/// Registers, modifies, or tests connection states dynamically
pub async fn update_notification_config(
    State(state): State<AppState>,
    Json(payload): Json<ProviderPayload>
) -> impl IntoResponse {
    let conn = match state.db.connect().await {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Database connection failure: {}", e)).into_response(),
    };

    let enabled_int = if payload.enabled { 1 } else { 0 };
    let config_str = payload.config_json.to_string();

    let result = conn.execute(
        "UPDATE notification_providers SET enabled = ?1, config_json = ?2 WHERE id = ?3",
        libsql::params![enabled_int, config_str, payload.id.clone()],
    ).await;

    match result {
        Ok(affected) if affected > 0 => {
            log::info!("[FLIGHT_RECORDER] Updated alert registration parameters for target provider: {}", payload.id);
            (StatusCode::OK, "Configuration successfully saved").into_response()
        }
        Ok(_) => (StatusCode::NOT_FOUND, "Target notification provider missing").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Data corruption exception: {}", e)).into_response(),
    }
}
