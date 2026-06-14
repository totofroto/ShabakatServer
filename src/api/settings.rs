use axum::{
    extract::State,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use crate::{
    api::error::{ApiError, ApiResult},
    storage::settings as settings_store,
    AppState,
};

pub async fn get_settings(
    State(state): State<AppState>,
) -> ApiResult<impl IntoResponse> {
    let settings = settings_store::get_all_settings(state.db.clone())
        .await
        .map_err(ApiError::Internal)?;
    
    Ok(Json(settings))
}

#[derive(Deserialize)]
pub struct SettingUpdate {
    pub key: String,
    pub value: String,
}

pub async fn update_setting(
    State(state): State<AppState>,
    Json(body): Json<SettingUpdate>,
) -> ApiResult<impl IntoResponse> {
    settings_store::set_setting(state.db.clone(), body.key, body.value)
        .await
        .map_err(ApiError::Internal)?;
        
    Ok(axum::http::StatusCode::NO_CONTENT)
}
