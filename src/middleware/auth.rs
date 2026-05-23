use axum::{
    body::Body,
    extract::{ConnectInfo, State},
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use axum_extra::extract::cookie::CookieJar;
use jsonwebtoken::{decode, DecodingKey, Validation};
use std::net::SocketAddr;

use crate::api::auth::Claims;
use crate::AppState;

pub async fn auth_middleware(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    jar: CookieJar,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = req.uri().path();

    // Whitelist authentication routes - MUST happen before token check
    if !is_protected_route(path) {
        return Ok(next.run(req).await);
    }

    // Check if auth is disabled globally
    if state.config.disable_auth {
        return Ok(next.run(req).await);
    }

    // Always allow OPTIONS for CORS preflight
    if req.method() == axum::http::Method::OPTIONS {
        return Ok(next.run(req).await);
    }

    // Check for local bypass if enabled
    if state.config.auth_bypass_local && is_local_ip(addr.ip()) {
        return Ok(next.run(req).await);
    }

    // Check cookie or authorization header
    let mut token = jar
        .get("admin_token")
        .map(|c| c.value().to_string())
        .or_else(|| {
            req.headers()
                .get("Authorization")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.strip_prefix("Bearer "))
                .map(|s| s.to_string())
        });

    // Re-hydration layer: check database if token is missing
    if token.is_none() {
        if let Ok(Some(db_token)) = crate::storage::settings::get_setting(state.db.clone(), "active_admin_session").await {
            if !db_token.is_empty() {
                token = Some(db_token);
            }
        }
    }

    let token = match token {
        Some(t) => t,
        None => return Err(StatusCode::UNAUTHORIZED),
    };

    let decoding_key = DecodingKey::from_secret(state.config.jwt_secret.as_bytes());
    let mut validation = Validation::default();
    validation.set_issuer(&["shabakat-server"]);
    validation.set_audience(&["shabakat-admin"]);

    match decode::<Claims>(&token, &decoding_key, &validation) {
        Ok(_) => Ok(next.run(req).await),
        Err(_) => Err(StatusCode::UNAUTHORIZED),
    }
}

fn is_protected_route(path: &str) -> bool {
    let path = path.to_lowercase();
    // Bulletproof whitelist: catch the auth paths regardless of how Axum strips the prefix
    if path.contains("/auth") || path.contains("/google/login") || path.contains("/google/callback") {
        return false; // NOT protected
    }
    true
}

fn is_local_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ipv4) => {
            ipv4.is_loopback() || ipv4.is_private() || ipv4.is_link_local()
        }
        std::net::IpAddr::V6(ipv6) => {
            ipv6.is_loopback() || (ipv6.segments()[0] & 0xff00) == 0xfe00
        }
    }
}
