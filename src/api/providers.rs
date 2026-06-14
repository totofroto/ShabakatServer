use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use crate::api::error::{ApiError, ApiResult};
use crate::storage::providers;
use crate::types::DnsProvider;
use crate::AppState;

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

pub async fn list_providers(State(state): State<AppState>) -> ApiResult<impl IntoResponse> {
    let p = providers::list_providers(state.db.clone())
        .await
        .map_err(ApiError::from)?;
    Ok(Json(p))
}

pub async fn add_provider(
    State(state): State<AppState>,
    Json(payload): Json<CreateProviderPayload>,
) -> ApiResult<impl IntoResponse> {
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

    providers::add_provider(state.db.clone(), provider)
        .await
        .map_err(ApiError::from)?;
    Ok(axum::http::StatusCode::CREATED)
}

pub async fn patch_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<PatchProviderPayload>,
) -> ApiResult<impl IntoResponse> {
    log::info!("PATCH DNS provider payload: {:?}", payload);

    if let Some(is_enabled) = payload.is_enabled {
        providers::toggle_provider_status(state.db.clone(), id, is_enabled)
            .await
            .map_err(ApiError::from)?;
        Ok(axum::http::StatusCode::OK)
    } else {
        Err(ApiError::BadRequest("Field 'isEnabled' is required".to_string()))
    }
}

pub async fn delete_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<impl IntoResponse> {
    providers::delete_provider(state.db.clone(), id)
        .await
        .map_err(ApiError::from)?;
    Ok(axum::http::StatusCode::OK)
}
