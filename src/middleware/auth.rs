use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Request, StatusCode},
    middleware::Next,
    response::Response,
};
use axum_extra::extract::cookie::CookieJar;
use jsonwebtoken::{decode, DecodingKey, Validation};

use crate::api::auth::Claims;
use crate::AppState;

pub fn validate_token(token: &str, secret: &str) -> bool {
    let decoding_key = DecodingKey::from_secret(secret.as_bytes());
    let mut validation = Validation::default();
    validation.set_issuer(&["shabakat-server"]);
    validation.set_audience(&["shabakat-admin"]);

    decode::<Claims>(token, &decoding_key, &validation).is_ok()
}

pub async fn auth_middleware(
    jar: CookieJar,
    headers: HeaderMap,
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = req.uri().path();

    // Whitelist authentication routes
    if !is_protected_route(path) {
        return Ok(next.run(req).await);
    }

    // Always allow OPTIONS for CORS preflight
    if req.method() == axum::http::Method::OPTIONS {
        return Ok(next.run(req).await);
    }

    // 1. Try Authorization header first
    let mut token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(|s| s.to_string());

    // 2. Fallback to admin_token cookie from CookieJar
    if token.is_none() {
        token = jar.get("admin_token").map(|c| c.value().to_string());
    }

    // 3. Last resort: Manual Cookie header extraction (bypass CookieJar if it fails)
    if token.is_none() {
        if let Some(cookie_header) = headers.get(axum::http::header::COOKIE).and_then(|h| h.to_str().ok()) {
            for cookie in cookie_header.split(';') {
                let cookie = cookie.trim();
                if let Some(val) = cookie.strip_prefix("admin_token=") {
                    token = Some(val.to_string());
                    log::info!("[AUTH_DEBUG] Recovered token from raw Cookie header");
                    break;
                }
            }
        }
    }

    // Debug print
    if token.is_none() {
        let cookie_header = headers.get(axum::http::header::COOKIE).and_then(|h| h.to_str().ok());
        log::warn!(
            "[AUTH_DEBUG] Unauthorized access attempt: Path={}, CookiesPresent={}, CookieHeader={:?}",
            path,
            jar.iter().count() > 0,
            cookie_header
        );
    }

    if let Some(t) = token {
        if validate_token(&t, &state.config.jwt_secret) {
            return Ok(next.run(req).await);
        }
        log::warn!("[AUTH_DEBUG] Invalid token or cookie for path: {}", path);
    }

    Err(StatusCode::UNAUTHORIZED)
}

fn is_protected_route(path: &str) -> bool {
    let path = path.to_lowercase();

    // Explicitly protect the /me endpoint even if it contains /auth
    if path.ends_with("/auth/me") {
        return true;
    }

    // Whitelist login and callback routes
    if path.contains("/google/login") || path.contains("/google/callback") || path.contains("/auth/google") {
        return false; // NOT protected
    }

    // General auth routes (like login/callback if they don't match above)
    if path.contains("/auth") && (path.contains("/login") || path.contains("/callback")) {
        return false;
    }

    true
}

