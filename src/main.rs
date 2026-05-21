mod api;
mod config;
mod dashboard;
mod middleware;
mod monitor;
mod notifications;
mod scanner;
mod scheduler;
mod storage;
mod types;

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use log::info;
use tokio::sync::broadcast;
use tower_http::services::{ServeDir, ServeFile};

use crate::config::Config;
use crate::types::DiscoveredDevice;

#[derive(Clone)]
pub struct AppState {
    pub db: storage::AppDb,
    pub config: Arc<Config>,
    pub broadcast_tx: broadcast::Sender<serde_json::Value>,
    pub devices: Arc<Mutex<Vec<DiscoveredDevice>>>,
    pub bandwidth: Arc<Mutex<Option<crate::types::RouterBandwidth>>>,
}

#[tokio::main]
async fn main() {
    env_logger::init();
    info!("[FLIGHT_RECORDER] Shabakat Server starting…");

    let config = Arc::new(Config::from_env());

    let db = storage::AppDb::new("./shabakat_server.db").await;
    scanner::init_vendor_map().expect("failed to load vendor map");

    let (broadcast_tx, _) = broadcast::channel(256);

    let state = AppState {
        db,
        config: Arc::clone(&config),
        broadcast_tx,
        devices: Arc::new(Mutex::new(Vec::new())),
        bandwidth: Arc::new(Mutex::new(None)),
    };

    // Background scan scheduler
    tokio::spawn(scheduler::run(state.clone()));

    // Background scoring task
    tokio::spawn(dashboard::scoring::run(state.clone()));

    // Internet outage monitor
    tokio::spawn(monitor::outage_detector::start_outage_monitor(state.clone()));

    // Router bandwidth monitor
    tokio::spawn(monitor::router::run_bandwidth_monitor(state.clone()));

    // Passive mDNS presence monitor
    tokio::spawn(monitor::presence::run_presence_monitor(state.db.clone()));

    let mut app = api::router(state);

    // Serve custom assets
    app = app.nest_service("/uploads", ServeDir::new("data/assets"));

    // Serve React frontend if SHABAKAT_WEB_DIR is set
    if let Some(ref web_dir) = config.web_dir {
        let index = format!("{web_dir}/index.html");
        let serve = ServeDir::new(web_dir).fallback(ServeFile::new(index));
        app = app.fallback_service(serve);
    }

    let addr = format!("0.0.0.0:{}", config.port);
    info!("[FLIGHT_RECORDER] Listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("bind failed");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("server error");
}
