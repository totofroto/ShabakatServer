pub mod adguard;
pub mod assets;
pub mod auth;
pub mod dashboard;
pub mod debug;
pub mod devices;
pub mod history;
pub mod networks;
pub mod notifications;
pub mod outages;
pub mod providers;
pub mod scan;
pub mod settings;
pub mod speed_test;
pub mod tools;
pub mod ws;

use axum::{
    http::{HeaderValue, Method},
    middleware,
    routing::{delete, get, patch, post},
    Router,
};
use tower_http::cors::CorsLayer;

use crate::middleware::auth::auth_middleware;
use crate::AppState;

pub fn router(state: AppState) -> Router {
    let allowed_origins = [
        "http://localhost:3000",
        "http://localhost:5173",
        "http://localhost:8080",
        "http://127.0.0.1:3000",
        "http://127.0.0.1:5173",
        "http://127.0.0.1:8080",
    ];

    let mut cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_headers([axum::http::header::CONTENT_TYPE, axum::http::header::AUTHORIZATION])
        .allow_credentials(true);

    for origin in allowed_origins {
        if let Ok(hv) = origin.parse::<HeaderValue>() {
            cors = cors.allow_origin(hv);
        }
    }

    let auth_routes = Router::new()
        .route("/google/login", get(auth::google_login))
        .route("/google/callback", get(auth::google_callback))
        .route("/logout", post(auth::logout))
        .route("/me", get(auth::me));

    let tool_routes = Router::new()
        .route("/ping", post(tools::ping))
        .route("/tcp-ping", post(tools::tcp_ping))
        .route("/dns", post(tools::dns))
        .route("/wake", post(tools::wake))
        .route("/portscan", post(tools::portscan))
        .route("/portscan-all", post(tools::portscan_all))
        .route("/subnet-calc", post(tools::subnet_calc))
        .route("/ssl", post(tools::ssl))
        .route("/whois", post(tools::whois))
        .route("/ip-geo", post(tools::ip_geo))
        .route("/mac-lookup", post(tools::mac_lookup))
        .route("/headers", post(tools::headers))
        .route("/test-notification", post(tools::test_notification));

    let api = Router::new()
        .route("/system-status", get(dashboard::get_system_status))
        .route("/system/telemetry", get(dashboard::get_system_telemetry))
        .route("/router/bandwidth", get(dashboard::get_router_bandwidth))
        .route("/debug/state", get(debug::get_debug_state))
        .route("/debug/probe", post(debug::debug_probe))
        .route("/debug/logs/stream", get(debug::stream_logs))
        .route("/debug/terminal/run", post(debug::run_terminal_command))
        .route("/devices", get(devices::list_devices))
        .route("/devices/alias", post(devices::set_device_alias))
        .route("/devices/:mac", get(devices::get_device))
        .route("/devices/:mac", patch(devices::patch_device))
        .route("/devices/:mac", delete(devices::delete_device))
        .route("/devices/:ip/dns", get(adguard::get_device_dns_stats))
        .route("/networks", get(networks::list_networks))
        .route("/network/topology", get(networks::get_topology))
        .route("/network-info", get(networks::get_network_info))
        .route("/outages", get(outages::list_outages))
        .route("/speed-test/run", post(speed_test::run_speed_test))
        .route("/speed-test/history", get(speed_test::speed_test_history))
        .route("/scan", post(scan::trigger_scan))
        .route("/scan/status", get(scan::scan_status))
        .route("/history", get(history::get_history))
        .route("/dns/providers", get(providers::list_providers))
        .route("/dns/providers", post(providers::add_provider))
        .route("/dns/providers/:id", patch(providers::patch_provider))
        .route("/dns/providers/:id", delete(providers::delete_provider))
        .route("/assets/upload", post(assets::upload_asset))
        .route("/settings", get(settings::get_settings))
        .route("/settings", post(settings::update_setting))
        .route("/notifications/config", get(notifications::get_notification_config))
        .route("/notifications/config", post(notifications::update_notification_config))
        .nest("/tools", tool_routes)
        .nest("/auth", auth_routes)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    Router::new()
        .nest("/api", api)
        .route("/ws", get(ws::ws_handler))
        .layer(cors)
        .with_state(state)
}
