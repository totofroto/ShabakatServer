#![allow(dead_code)]

use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Request},
    middleware::Next,
    response::Response,
};
use axum_extra::extract::cookie::CookieJar;
use jsonwebtoken::{decode, DecodingKey, Validation};

use crate::api::auth::Claims;
use crate::api::error::ApiError;
use crate::AppState;

/// Decode and validate a JWT, returning the embedded claims on success.
/// Returns `None` for any decode/signature/issuer/audience/expiry failure.
pub fn decode_claims(token: &str, secret: &str) -> Option<Claims> {
    let decoding_key = DecodingKey::from_secret(secret.as_bytes());
    let mut validation = Validation::default();
    validation.set_issuer(&["shabakat-server"]);
    validation.set_audience(&["shabakat-admin"]);

    decode::<Claims>(token, &decoding_key, &validation)
        .ok()
        .map(|data| data.claims)
}

pub async fn auth_middleware(
    jar: CookieJar,
    headers: HeaderMap,
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let path = req.uri().path();

    // Whitelist authentication routes
    if !is_protected_route(path) {
        return Ok(next.run(req).await);
    }

    // Development bypass: SHABAKAT_DISABLE_AUTH=true skips all token validation.
    // Still inject a synthetic admin identity so handlers that depend on the
    // validated Claims extension (e.g. /api/auth/me) keep working locally.
    if state.config.disable_auth {
        req.extensions_mut().insert(Claims {
            sub: "dev-bypass".to_string(),
            email: state
                .config
                .admin_email
                .clone()
                .unwrap_or_else(|| "dev@shabakat.local".to_string()),
            exp: 0,
            iss: "shabakat-server".to_string(),
            aud: "shabakat-admin".to_string(),
        });
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
        if let Some(claims) = decode_claims(&t, &state.config.jwt_secret) {
            req.extensions_mut().insert(claims);
            return Ok(next.run(req).await);
        }
        log::warn!("[AUTH_DEBUG] Invalid token or cookie for path: {}", path);
    }

    Err(ApiError::Unauthorized("Access denied. Invalid or missing authentication token.".to_string()))
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

