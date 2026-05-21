use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    Json,
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct AuthCallbackQuery {
    pub code: String,
    pub state: Option<String>,
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

pub async fn google_login(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    let client_id = match &state.config.google_client_id {
        Some(id) => id,
        None => return Redirect::to("/login?error=auth_not_configured").into_response(),
    };

    let redirect_uri = match &state.config.google_redirect_uri {
        Some(uri) => uri,
        None => return Redirect::to("/login?error=auth_not_configured").into_response(),
    };

    let state_token = Uuid::new_v4().to_string();
    let is_secure = state.config.google_redirect_uri.as_ref()
        .map(|uri| uri.starts_with("https://"))
        .unwrap_or(true);

    let state_cookie = Cookie::build(("oauth_state", state_token.clone()))
        .path("/")
        .http_only(true)
        .secure(is_secure)
        .same_site(axum_extra::extract::cookie::SameSite::Lax)
        .max_age(time::Duration::minutes(5))
        .build();

    let auth_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope=openid%20email&access_type=online&state={}",
        client_id, redirect_uri, state_token
    );

    (jar.add(state_cookie), Redirect::to(&auth_url)).into_response()
}

pub async fn google_callback(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<AuthCallbackQuery>,
) -> Response {
    let config = &state.config;

    // Verify state token
    let stored_state = jar.get("oauth_state").map(|c| c.value().to_string());
    if query.state.is_none() || stored_state.is_none() || query.state.unwrap() != stored_state.unwrap() {
        return (StatusCode::FORBIDDEN, "Invalid OAuth state").into_response();
    }

    let jar = jar.remove(Cookie::from("oauth_state"));

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
    let mut params = HashMap::new();
    params.insert("client_id", client_id.as_str());
    params.insert("client_secret", client_secret.as_str());
    params.insert("code", query.code.as_str());
    params.insert("redirect_uri", redirect_uri.as_str());
    params.insert("grant_type", "authorization_code");

    let token_res = match client
        .post("https://oauth2.googleapis.com/token")
        .form(&params)
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
    if user_info.email != "tarekshek@gmail.com" {
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
    let cookie = Cookie::build(("admin_token", token))
        .path("/")
        .http_only(true)
        .secure(true)
        .same_site(axum_extra::extract::cookie::SameSite::Lax)
        .expires(time::OffsetDateTime::from_unix_timestamp(expiration.timestamp()).unwrap())
        .build();

    (jar.add(cookie), Redirect::to("/settings")).into_response()
}

pub async fn logout(jar: CookieJar) -> impl IntoResponse {
    let cookie = Cookie::build(("admin_token", ""))
        .path("/")
        .max_age(time::Duration::ZERO)
        .build();
    (jar.add(cookie), Redirect::to("/login"))
}

pub async fn me(
    jar: CookieJar,
    State(state): State<AppState>,
) -> Result<Json<Claims>, (axum::http::StatusCode, String)> {
    let token = jar.get("admin_token").map(|c| c.value().to_string());

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
