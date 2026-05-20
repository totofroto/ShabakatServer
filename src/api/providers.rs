use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;

use crate::AppState;
use crate::storage::providers;
use crate::types::DnsProvider;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateProviderPayload {
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PatchProviderPayload {
    pub is_enabled: Option<bool>,
}

pub async fn list_providers(State(state): State<AppState>) -> impl IntoResponse {
    match providers::list_providers(state.db.clone()).await {
        Ok(p) => Json(p).into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e })),
        )
            .into_response(),
    }
}

pub async fn add_provider(
    State(state): State<AppState>,
    Json(payload): Json<CreateProviderPayload>,
) -> impl IntoResponse {
    log::info!("Adding DNS provider: {:?}", payload);
    let provider = DnsProvider {
        id: format!("{:x}", rand::random::<u64>()), // Simple ID generation
        name: payload.name,
        ip: payload.ip,
        port: payload.port,
        username: payload.username,
        password: payload.password,
        is_enabled: true,
        created_at: crate::storage::now_ms(),
    };

    match providers::add_provider(state.db.clone(), provider).await {
        Ok(_) => axum::http::StatusCode::CREATED.into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e })),
        )
            .into_response(),
    }
}

pub async fn patch_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    let raw_json = String::from_utf8_lossy(&body);
    log::info!("PATCH DNS provider raw JSON: {}", raw_json);

    let payload: PatchProviderPayload = match serde_json::from_slice(&body) {
        Ok(p) => p,
        Err(e) => {
            log::error!("Failed to parse PATCH payload: {}", e);
            return axum::http::StatusCode::BAD_REQUEST.into_response();
        }
    };

    if let Some(is_enabled) = payload.is_enabled {
        match providers::toggle_provider_status(state.db.clone(), id, is_enabled).await {
            Ok(_) => axum::http::StatusCode::OK.into_response(),
            Err(e) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e })),
            )
                .into_response(),
        }
    } else {
        axum::http::StatusCode::BAD_REQUEST.into_response()
    }
}

pub async fn delete_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match providers::delete_provider(state.db.clone(), id).await {
        Ok(_) => axum::http::StatusCode::OK.into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e })),
        )
            .into_response(),
    }
}
