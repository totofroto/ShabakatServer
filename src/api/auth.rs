use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
    Json,
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use time;
use libsql;

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct CallbackParams {
    pub code: String,
    pub state: String,
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

pub async fn google_login(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let client_id = match &state.config.google_client_id {
        Some(id) => id,
        None => return Redirect::to("/login?error=auth_not_configured").into_response(),
    };
    
    let redirect_uri = match &state.config.google_redirect_uri {
        Some(uri) => uri,
        None => return Redirect::to("/login?error=auth_not_configured").into_response(),
    };

    let csrf_state = Uuid::new_v4().to_string();

    // 🌟 PERSISTENCE PASS: Write the state directly to SQLite to guarantee safety across proxy layers
    let conn = match state.db.connect().await {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Database access error: {}", e)).into_response(),
    };

    if let Err(e) = conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES ('pending_oauth_state', ?1)",
        libsql::params![csrf_state.clone()],
    ).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to record secure runtime state: {}", e)).into_response();
    }

    let auth_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope=openid%20email&access_type=online&state={}",
        client_id, redirect_uri, csrf_state
    );

    Redirect::to(&auth_url).into_response()
}

pub async fn google_callback(
    State(state): State<AppState>,
    Query(params): Query<CallbackParams>,
    jar: CookieJar,
) -> impl IntoResponse {
    // LOGGING: This will appear in your 'docker logs'
    log::info!("[AUTH_DEBUG] Callback received. Params state: '{}'", params.state);

    let conn = match state.db.connect().await {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e)).into_response(),
    };

    // 🌟 EXTRACTION PASS: Read the pending verification state back out from our persistent setting layer
    let mut stmt = match conn.prepare("SELECT value FROM settings WHERE key = 'pending_oauth_state'").await {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Query error: {}", e)).into_response(),
    };

    let mut rows = match stmt.query(()).await {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Execution error: {}", e)).into_response(),
    };

    let saved_state: String = match rows.next().await {
        Ok(Some(row)) => row.get::<String>(0).unwrap_or_default(),
        _ => {
            log::error!("[AUTH_DEBUG] No pending_oauth_state found in database!");
            return (StatusCode::FORBIDDEN, "OAuth session lost").into_response();
        }
    };

    log::info!("[AUTH_DEBUG] Database state: '{}'", saved_state);

    if params.state != saved_state || saved_state.is_empty() {
        log::warn!("[AUTH_DEBUG] State mismatch! Recv: '{}', Saved: '{}'", params.state, saved_state);
        return (StatusCode::FORBIDDEN, "Invalid OAuth state").into_response();
    }

    // Clear the consumed state token immediately to prevent reuse replay threats
    let _ = conn.execute("DELETE FROM settings WHERE key = 'pending_oauth_state'", ()).await;

    let config = &state.config;

    let client_id = match &config.google_client_id {
        Some(id) => id,
        None => return Redirect::to("/login?error=auth_not_configured").into_response(),
    };

    let client_secret = match &config.google_client_secret {
        Some(secret) => secret,
        None => return Redirect::to("/login?error=auth_not_configured").into_response(),
    };

    let redirect_uri = match &config.google_redirect_uri {
        Some(uri) => uri,
        None => return Redirect::to("/login?error=auth_not_configured").into_response(),
    };

    // Exchange code for token
    let client = reqwest::Client::new();
    let mut params_map = HashMap::new();
    params_map.insert("client_id", client_id.as_str());
    params_map.insert("client_secret", client_secret.as_str());
    params_map.insert("code", params.code.as_str());
    params_map.insert("redirect_uri", redirect_uri.as_str());
    params_map.insert("grant_type", "authorization_code");

    let token_res = match client
        .post("https://oauth2.googleapis.com/token")
        .form(&params_map)
        .send()
        .await
    {
        Ok(res) => res,
        Err(_) => return Redirect::to("/login?error=token_exchange_failed").into_response(),
    };

    let token_data: GoogleTokenResponse = match token_res.json().await {
        Ok(data) => data,
        Err(_) => return Redirect::to("/login?error=token_parsing_failed").into_response(),
    };

    // Get user info
    let user_info_res = match client
        .get("https://www.googleapis.com/oauth2/v3/userinfo")
        .bearer_auth(token_data.access_token)
        .send()
        .await
    {
        Ok(res) => res,
        Err(_) => return Redirect::to("/login?error=user_info_failed").into_response(),
    };

    let user_info: GoogleUserInfo = match user_info_res.json().await {
        Ok(info) => info,
        Err(_) => return Redirect::to("/login?error=user_info_parsing_failed").into_response(),
    };

    // Enforce admin email allowlist
    let is_authorized = match &config.admin_email {
        Some(admin_email) => user_info.email == *admin_email,
        None => true, // If no admin email is configured, allow any (or you might want to default to false)
    };

    if !is_authorized {
        return (StatusCode::FORBIDDEN, "Unauthorized email").into_response();
    }

    // Generate JWT
    let expiration = Utc::now() + Duration::days(7);
    let claims = Claims {
        sub: user_info.email.clone(),
        email: user_info.email,
        exp: expiration.timestamp(),
        iss: "shabakat-server".to_string(),
        aud: "shabakat-admin".to_string(),
    };

    let token = match encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    ) {
        Ok(t) => t,
        Err(_) => return Redirect::to("/login?error=token_creation_failed").into_response(),
    };

    // Set cookie
    let is_secure = redirect_uri.starts_with("https");
    let cookie = Cookie::build(("session", token.clone()))
        .path("/")
        .http_only(true)
        .secure(is_secure)
        .same_site(axum_extra::extract::cookie::SameSite::Lax)
        .expires(time::OffsetDateTime::from_unix_timestamp(expiration.timestamp()).unwrap())
        .build();

    // Persist session for re-hydration
    if let Err(e) = crate::storage::settings::set_setting(
        state.db.clone(),
        "active_admin_session".to_string(),
        token.clone(),
    ).await {
        log::error!("Failed to persist admin session: {}", e);
    }

    let redirect_url = format!("/dashboard#token={}", token);
    (jar.add(cookie), Redirect::to(&redirect_url)).into_response()
}

pub async fn logout(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    // Clear persisted session
    let _ = crate::storage::settings::set_setting(
        state.db.clone(),
        "active_admin_session".to_string(),
        "".to_string(),
    ).await;

    let cookie = Cookie::build(("session", ""))
        .path("/")
        .max_age(time::Duration::ZERO)
        .build();
    (jar.add(cookie), Redirect::to("/login"))
}

pub async fn me(
    headers: axum::http::HeaderMap,
    jar: CookieJar,
    State(state): State<AppState>,
) -> Result<Json<Claims>, (axum::http::StatusCode, String)> {
    let mut token = jar.get("session").map(|c| c.value().to_string());

    // Check Authorization header if cookie is missing
    if token.is_none() {
        token = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .map(|s| s.to_string());
    }

    // Re-hydration layer: check database if token is still missing
    if token.is_none() {
        if let Ok(Some(db_token)) = crate::storage::settings::get_setting(state.db.clone(), "active_admin_session").await {
            if !db_token.is_empty() {
                token = Some(db_token);
            }
        }
    }

    if let Some(token) = token {
        let decoding_key = DecodingKey::from_secret(state.config.jwt_secret.as_bytes());
        let mut validation = Validation::default();
        validation.set_issuer(&["shabakat-server"]);
        validation.set_audience(&["shabakat-admin"]);

        if let Ok(token_data) = decode::<Claims>(&token, &decoding_key, &validation) {
            return Ok(Json(token_data.claims));
        }
    }

    // If no token or invalid token, check if auth is disabled
    if state.config.disable_auth {
        return Ok(Json(Claims {
            sub: "admin@local".to_string(),
            email: "admin@local".to_string(),
            exp: Utc::now().timestamp() + 3600,
            iss: "shabakat-server".to_string(),
            aud: "shabakat-admin".to_string(),
        }));
    }

    Err((axum::http::StatusCode::UNAUTHORIZED, "No token".to_string()))
}
