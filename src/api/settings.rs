use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Deserialize;
use crate::{
    storage::settings as settings_store,
    AppState,
};

pub async fn get_settings(
    State(state): State<AppState>,
) -> impl IntoResponse {
    match settings_store::get_all_settings(state.db.clone()).await {
        Ok(settings) => Json(settings).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

#[derive(Deserialize)]
pub struct SettingUpdate {
    pub key: String,
    pub value: String,
}

pub async fn update_setting(
    State(state): State<AppState>,
    Json(body): Json<SettingUpdate>,
) -> impl IntoResponse {
    match settings_store::set_setting(state.db.clone(), body.key, body.value).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}
