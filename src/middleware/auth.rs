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
        log::info!("[AUTH_DEBUG] Auth disabled globally. Allowing request to {}", path);
        return Ok(next.run(req).await);
    }

    // Always allow OPTIONS for CORS preflight
    if req.method() == axum::http::Method::OPTIONS {
        return Ok(next.run(req).await);
    }

    // Check for local bypass if enabled
    if state.config.auth_bypass_local && is_local_ip(addr.ip()) {
        log::info!("[AUTH_DEBUG] Local bypass active for {}. Allowing request to {}", addr.ip(), path);
        return Ok(next.run(req).await);
    }

    // Check cookie or authorization header
    let mut token = jar
        .get("session")
        .map(|c| {
            log::debug!("[AUTH_DEBUG] Found session cookie for {}", path);
            c.value().to_string()
        })
        .or_else(|| {
            req.headers()
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.strip_prefix("Bearer "))
                .map(|s| {
                    log::info!("[AUTH_DEBUG] Found Authorization Bearer token for {}", path);
                    s.to_string()
                })
        });

    // Re-hydration layer: check database if token is missing
    if token.is_none() {
        if let Ok(Some(db_token)) = crate::storage::settings::get_setting(state.db.clone(), "active_admin_session").await {
            if !db_token.is_empty() {
                log::info!("[AUTH_DEBUG] Re-hydrating session from DB for {}", path);
                token = Some(db_token);
            }
        }
    }

    let token = match token {
        Some(t) => t,
        None => {
            log::warn!("[AUTH_DEBUG] No authentication token found for {}", path);
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    let decoding_key = DecodingKey::from_secret(state.config.jwt_secret.as_bytes());
    let mut validation = Validation::default();
    validation.set_issuer(&["shabakat-server"]);
    validation.set_audience(&["shabakat-admin"]);

    match decode::<Claims>(&token, &decoding_key, &validation) {
        Ok(_) => Ok(next.run(req).await),
        Err(e) => {
            log::error!("[AUTH_DEBUG] JWT decoding failed for {}: {:?}", path, e);
            Err(StatusCode::UNAUTHORIZED)
        }
    }
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
    // but we want to be careful not to whitelist everything under /api/auth
    if path.contains("/auth") && (path.contains("/login") || path.contains("/callback")) {
        return false;
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
