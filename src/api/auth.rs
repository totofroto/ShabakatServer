use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect},
    Json,
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::api::error::{ApiError, ApiResult};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct AuthCallbackQuery {
    pub code: String,
    pub _state: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub exp: i64,
    pub iss: String,
    pub aud: String,
}

#[derive(Debug, Deserialize)]
struct GoogleTokenResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct GoogleUserInfo {
    email: String,
}

pub async fn google_login(State(state): State<AppState>) -> ApiResult<impl IntoResponse> {
    let client_id = state
        .config
        .google_client_id
        .as_ref()
        .ok_or_else(|| ApiError::Internal("GOOGLE_CLIENT_ID not set".to_string()))?;
    let redirect_uri = state
        .config
        .google_redirect_uri
        .as_ref()
        .ok_or_else(|| ApiError::Internal("GOOGLE_REDIRECT_URI not set".to_string()))?;

    let auth_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?response_type=code&client_id={}&redirect_uri={}&scope=openid%20email&state={}",
        client_id,
        redirect_uri,
        Uuid::new_v4()
    );

    Ok(Redirect::to(&auth_url))
}

pub async fn google_callback(
    jar: CookieJar,
    State(state): State<AppState>,
    Query(query): Query<AuthCallbackQuery>,
) -> ApiResult<impl IntoResponse> {
    let client_id = state
        .config
        .google_client_id
        .as_ref()
        .ok_or_else(|| ApiError::Internal("GOOGLE_CLIENT_ID not set".to_string()))?;
    let client_secret = state
        .config
        .google_client_secret
        .as_ref()
        .ok_or_else(|| ApiError::Internal("GOOGLE_CLIENT_SECRET not set".to_string()))?;
    let redirect_uri = state
        .config
        .google_redirect_uri
        .as_ref()
        .ok_or_else(|| ApiError::Internal("GOOGLE_REDIRECT_URI not set".to_string()))?;

    let client = reqwest::Client::new();
    let params = [
        ("code", query.code.as_str()),
        ("client_id", client_id.as_str()),
        ("client_secret", client_secret.as_str()),
        ("redirect_uri", redirect_uri.as_str()),
        ("grant_type", "authorization_code"),
    ];

    let token_res = client
        .post("https://oauth2.googleapis.com/token")
        .form(&params)
        .send()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch token: {}", e)))?
        .json::<GoogleTokenResponse>()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to parse token: {}", e)))?;

    let user_info = client
        .get("https://www.googleapis.com/oauth2/v3/userinfo")
        .bearer_auth(token_res.access_token)
        .send()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch user info: {}", e)))?
        .json::<GoogleUserInfo>()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to parse user info: {}", e)))?;

    // Check if user is admin
    if let Some(admin_email) = &state.config.admin_email {
        if user_info.email != *admin_email {
            return Err(ApiError::Unauthorized("Unauthorized email".to_string()));
        }
    }

    let exp = Utc::now() + Duration::days(7);
    let claims = Claims {
        sub: user_info.email.clone(),
        email: user_info.email,
        exp: exp.timestamp(),
        iss: "shabakat-server".to_string(),
        aud: "shabakat-admin".to_string(),
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.config.jwt_secret.as_bytes()),
    )
    .map_err(|e| ApiError::Internal(format!("Failed to encode token: {}", e)))?;

    let cookie = Cookie::build(("admin_token", token))
        .path("/")
        .http_only(true)
        .secure(false) // Set to true in production
        .same_site(axum_extra::extract::cookie::SameSite::Lax)
        .max_age(time::Duration::days(7))
        .build();

    let jar = jar.add(cookie);

    Ok((jar, Redirect::to("/")))
}

pub async fn logout(jar: CookieJar) -> ApiResult<impl IntoResponse> {
    let cookie = Cookie::build(("admin_token", ""))
        .path("/")
        .max_age(time::Duration::ZERO)
        .build();

    Ok((jar.add(cookie), Json(HashMap::from([("status", "ok")]))))
}

pub async fn me() -> impl IntoResponse {
    Json(serde_json::json!({
        "sub": "local-admin",
        "email": "admin@shabakat.local",
        "role": "admin"
    }))
}
