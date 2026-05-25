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
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct AuthCallbackQuery { pub code: String, pub _state: Option<String> }

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims { pub sub: String, pub email: String, pub exp: i64, pub iss: String, pub aud: String }

#[derive(Debug, Deserialize)]
struct GoogleTokenResponse { access_token: String }

#[derive(Debug, Deserialize)]
struct GoogleUserInfo { email: String }

pub async fn google_login(State(state): State<AppState>) -> impl IntoResponse {
    let client_id = state.config.google_client_id.as_ref().expect("GOOGLE_CLIENT_ID not set");
    let redirect_uri = state.config.google_redirect_uri.as_ref().expect("GOOGLE_REDIRECT_URI not set");

    let auth_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?response_type=code&client_id={}&redirect_uri={}&scope=openid%20email&state={}",
        client_id,
        redirect_uri,
        Uuid::new_v4().to_string()
    );

    Redirect::to(&auth_url)
}

pub async fn google_callback(
    jar: CookieJar,
    State(state): State<AppState>,
    Query(query): Query<AuthCallbackQuery>,
) -> impl IntoResponse {
    let client_id = state.config.google_client_id.as_ref().expect("GOOGLE_CLIENT_ID not set");
    let client_secret = state.config.google_client_secret.as_ref().expect("GOOGLE_CLIENT_SECRET not set");
    let redirect_uri = state.config.google_redirect_uri.as_ref().expect("GOOGLE_REDIRECT_URI not set");

    let client = reqwest::Client::new();
    let params = [
        ("code", query.code.as_str()),
        ("client_id", client_id.as_str()),
        ("client_secret", client_secret.as_str()),
        ("redirect_uri", redirect_uri.as_str()),
        ("grant_type", "authorization_code"),
    ];

    let token_res = match client
        .post("https://oauth2.googleapis.com/token")
        .form(&params)
        .send()
        .await {
            Ok(res) => match res.json::<GoogleTokenResponse>().await {
                Ok(token) => token,
                Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to parse token: {}", e)).into_response(),
            },
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to fetch token: {}", e)).into_response(),
        };

    let user_info = match client
        .get("https://www.googleapis.com/oauth2/v3/userinfo")
        .bearer_auth(token_res.access_token)
        .send()
        .await {
            Ok(res) => match res.json::<GoogleUserInfo>().await {
                Ok(info) => info,
                Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to parse user info: {}", e)).into_response(),
            },
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to fetch user info: {}", e)).into_response(),
        };

    // Check if user is admin
    if let Some(admin_email) = &state.config.admin_email {
        if user_info.email != *admin_email {
            return (StatusCode::UNAUTHORIZED, "Unauthorized email").into_response();
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

    let token = match encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.config.jwt_secret.as_bytes()),
    ) {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to encode token: {}", e)).into_response(),
    };

    let cookie = Cookie::build(("admin_token", token))
        .path("/")
        .http_only(true)
        .secure(false) // Set to true in production
        .same_site(axum_extra::extract::cookie::SameSite::Lax)
        .max_age(time::Duration::days(7))
        .build();

    let jar = jar.add(cookie);

    (jar, Redirect::to("/")).into_response()
}

pub async fn logout(jar: CookieJar) -> impl IntoResponse {
    let cookie = Cookie::build(("admin_token", ""))
        .path("/")
        .max_age(time::Duration::ZERO)
        .build();

    (jar.add(cookie), Json(HashMap::from([("status", "ok")]))).into_response()
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

    // AUTH IS NOW MANDATORY: No bypass here
    Err((axum::http::StatusCode::UNAUTHORIZED, "No token".to_string()))
}
