//! # Shabakat Server — entry point
//!
//! Startup sequence (in order):
//!   1. `env_logger` initialised (reads `RUST_LOG`; defaults to `info`)
//!   2. Data directory created / verified  (`SHABAKAT_DATA_DIR`)
//!   3. SQLite connection opened and schema-migrated  (`Db::open`)
//!   4. `AppState` assembled and shared across the Axum router
//!   5. Background workers launched (scheduler, monitors)
//!   6. Axum HTTP server bound and running
//!
//! All blocking operations (step 3) are dispatched via `tokio::task::spawn_blocking`
//! so the Tokio reactor threads are never starved on the Celeron J4125.

// ── Module declarations ───────────────────────────────────────────────────────

mod api;           // src/api/          — REST routes + WebSocket /ws
mod config;        // src/config.rs     — env-based runtime configuration
mod dashboard;     // src/dashboard/    — scoring engine
mod middleware;    // src/middleware/   — JWT auth middleware
mod monitor;       // src/monitor/      — outage/presence/bandwidth/telemetry watchers
mod network;       // src/network/      — mDNS helpers
mod notifications; // src/notifications/— Telegram / webhook / SMTP
mod scanner;       // src/scanner/      — ARP / ICMP / mDNS / SSDP scan engine
mod scheduler;     // src/scheduler/    — watchdog + heartbeat background tasks
mod storage;       // src/storage/      — SQLite persistence layer (LIVE)
mod tools;         // src/tools/        — ping / dns / wake / portscan / whois
mod types;         // src/types.rs      — shared domain types

// ── Imports ───────────────────────────────────────────────────────────────────

use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::{extract::State, routing::get, Json, Router};
use log::{error, info, warn};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

use config::Config;
use notifications::NotificationDispatcher;
use storage::Db;
use types::{DiscoveredDevice, RouterBandwidth, SystemTelemetry};

// ── AppState ──────────────────────────────────────────────────────────────────

/// Application-wide state injected into every Axum handler via `State<AppState>`.
///
/// Every field is either:
/// - An `Arc`-wrapped value (O(1) clone, shared ownership)
/// - A `broadcast::Sender` (O(1) clone, fanout channel handle)
///
/// Axum clones `AppState` once per matched request — all clones are cheap.
#[derive(Clone)]
pub struct AppState {
    /// Thread-safe handle to the SQLite database.
    /// All query work goes through `Db::execute` → `spawn_blocking`.
    pub db: Db,

    /// Runtime configuration loaded once from environment variables.
    pub config: Arc<Config>,

    /// In-memory device cache — updated atomically after every scan.
    /// API handlers fall back to this when the DB is empty (e.g. first boot).
    pub devices: Arc<Mutex<Vec<DiscoveredDevice>>>,

    /// Notification dispatcher — wraps Telegram / SMTP / webhook providers.
    pub notifications: NotificationDispatcher,

    /// WebSocket broadcast channel.  Every connected client subscribes to
    /// a receiver; the scan engine, digital fence, and monitors publish here.
    pub broadcast_tx: broadcast::Sender<Value>,

    /// Latest router bandwidth snapshot (updated every 5 s by the bandwidth monitor).
    pub bandwidth: Arc<Mutex<Option<RouterBandwidth>>>,

    /// Latest system telemetry snapshot (updated every 2 s by the sys-metrics monitor).
    pub system_telemetry: Arc<Mutex<Option<SystemTelemetry>>>,

    /// SSE log broadcast for the debug panel (`GET /api/debug/logs/stream`).
    pub log_tx: broadcast::Sender<String>,
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    if let Err(e) = run().await {
        error!("[FLIGHT_RECORDER] !! Fatal startup error — cannot continue: {e}");
        std::process::exit(1);
    }
}

// ── Core startup logic ────────────────────────────────────────────────────────

async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("[FLIGHT_RECORDER] ══════════════════════════════════════════════");
    info!("[FLIGHT_RECORDER]   Shabakat Server — starting up");
    info!("[FLIGHT_RECORDER] ══════════════════════════════════════════════");

    // ── 1. Load runtime configuration ────────────────────────────────────
    let cfg = Arc::new(Config::from_env());
    info!("[FLIGHT_RECORDER] Configuration loaded (port={}, scan_interval={}s)",
          cfg.port, cfg.scan_interval_secs);

    if cfg.disable_auth {
        warn!("[FLIGHT_RECORDER] WARN: Authentication is explicitly DISABLED via environment override.");
    }

    // ── 2. Resolve and create the data directory ──────────────────────────
    let data_dir: PathBuf = PathBuf::from(&cfg.data_dir);
    let db_path = data_dir.join("shabakat.db");

    info!("[FLIGHT_RECORDER] Data directory : {}", data_dir.display());
    info!("[FLIGHT_RECORDER] Database path  : {}", db_path.display());

    tokio::fs::create_dir_all(&data_dir).await.map_err(|e| {
        io::Error::other(format!("Cannot create data directory '{}': {e}", data_dir.display()))
    })?;

    // ── 3. Open and migrate the SQLite database ───────────────────────────
    //
    // ASYNC REACTOR SAFETY: rusqlite is a synchronous C-library.  All
    // open+migrate work goes onto the dedicated blocking thread pool.
    let db_path_for_task = db_path.clone();
    let db: Db = tokio::task::spawn_blocking(move || Db::open(&db_path_for_task))
        .await
        .map_err(|join_err| {
            io::Error::other(format!("[FLIGHT_RECORDER] DB-open spawn_blocking task panicked: {join_err}"))
        })??;

    info!("[FLIGHT_RECORDER] Storage engine online — {}", db_path.display());

    // ── 4. Initialise broadcast channels ─────────────────────────────────
    let (broadcast_tx, _) = broadcast::channel::<Value>(256);
    let (log_tx, _) = broadcast::channel::<String>(512);

    // ── 5. Initialise in-memory state ─────────────────────────────────────
    let devices: Arc<Mutex<Vec<DiscoveredDevice>>> = Arc::new(Mutex::new(Vec::new()));
    let bandwidth: Arc<Mutex<Option<RouterBandwidth>>> = Arc::new(Mutex::new(None));
    let system_telemetry: Arc<Mutex<Option<SystemTelemetry>>> = Arc::new(Mutex::new(None));
    let notifications = NotificationDispatcher::new();

    // ── 6. Assemble AppState ──────────────────────────────────────────────
    let state = AppState {
        db,
        config: cfg.clone(),
        devices,
        notifications,
        broadcast_tx: broadcast_tx.clone(),
        bandwidth,
        system_telemetry,
        log_tx: log_tx.clone(),
    };

    info!("[FLIGHT_RECORDER] AppState assembled — launching background workers");

    // ── 7. Initialise debug uptime counter ────────────────────────────────
    api::debug::init_uptime();

    // ── 8. Spawn background workers ───────────────────────────────────────
    //
    // Every worker receives a cheap clone of AppState (Arc refcount bumps only).

    // Scan scheduler — runs automatic network sweeps on the configured interval.
    let sched_state = state.clone();
    tokio::spawn(async move {
        scheduler::run(sched_state).await;
    });
    info!("[FLIGHT_RECORDER] Scan scheduler spawned (interval={}s)", cfg.scan_interval_secs);

    // Outage detector — polls 8.8.8.8:53 every 60 s for WAN connectivity.
    let outage_state = state.clone();
    tokio::spawn(async move {
        monitor::outage_detector::start_outage_monitor(outage_state).await;
    });
    info!("[FLIGHT_RECORDER] Outage detector spawned");

    // Presence monitor — active heartbeat pings every 20 s per known device.
    let presence_state = state.clone();
    tokio::spawn(async move {
        monitor::presence::run_presence_monitor(presence_state).await;
    });
    info!("[FLIGHT_RECORDER] Presence monitor spawned");

    // Router bandwidth monitor — TR-064 / SNMP probe every 5 s.
    let bandwidth_state = state.clone();
    tokio::spawn(async move {
        monitor::router::run_bandwidth_monitor(bandwidth_state).await;
    });
    info!("[FLIGHT_RECORDER] Bandwidth monitor spawned");

    // System telemetry monitor — /proc/net/dev parser every 2 s.
    let telemetry_state = state.clone();
    tokio::spawn(async move {
        monitor::sys_metrics::run_sys_metrics_monitor(telemetry_state).await;
    });
    info!("[FLIGHT_RECORDER] System telemetry monitor spawned");

    // Digital Fence sentry — passive ambient mDNS/SSDP listener (always-on).
    let fence_db = state.db.clone();
    let fence_bcast = state.broadcast_tx.clone();
    scanner::digital_fence::DigitalFence::start(fence_db, fence_bcast);
    info!("[FLIGHT_RECORDER] Digital Fence sentry started");

    // Metrics compactor — hourly roll-up and purge of raw scan_history rows.
    storage::compactor::MetricsCompactor::start(state.db.clone());
    info!("[FLIGHT_RECORDER] Metrics compactor spawned (hourly interval)");

    // Dashboard scoring task — runs every 60 s.
    let scoring_state = state.clone();
    tokio::spawn(async move {
        dashboard::scoring::run(scoring_state).await;
    });
    info!("[FLIGHT_RECORDER] Dashboard scoring task spawned (60s interval)");

    // Fingerprints sync task — downloads the latest fingerprints from GitHub periodically.
    scanner::registry::start_sync_task();
    info!("[FLIGHT_RECORDER] Fingerprints sync task spawned");

    // mDNS service advertisement — registers ShabakatServer._http._tcp service.
    let _mdns_daemon = network::mdns::start_mdns_advertisement();


    // ── 9. Build the Axum router ──────────────────────────────────────────
    let app = build_router(state.clone());

    // ── 10. Bind the HTTP server ──────────────────────────────────────────
    let bind_addr = format!("0.0.0.0:{}", cfg.port);

    let listener = TcpListener::bind(&bind_addr).await.map_err(|e| {
        io::Error::new(
            io::ErrorKind::AddrInUse,
            format!("[FLIGHT_RECORDER] Cannot bind to {bind_addr}: {e}"),
        )
    })?;

    info!("[FLIGHT_RECORDER] HTTP server listening on http://{}", bind_addr);
    info!("[FLIGHT_RECORDER] ── Ready to accept connections ─────────────────");

    axum::serve(listener, app).await?;
    Ok(())
}

// ── Router construction ───────────────────────────────────────────────────────

fn build_router(state: AppState) -> Router {
    // The health route needs AppState injected separately because
    // api::router() already calls .with_state() and returns Router<()>.
    // We apply state to the health route first, then merge both Router<()> trees.
    let health_router: Router = Router::new()
        .route("/api/health", get(health_handler))
        .with_state(state.clone());

    health_router
        // Mount the full REST + WebSocket API sub-router (state already applied inside).
        .merge(api::router(state))
        // CORS — permissive at the outermost layer.
        .layer(CorsLayer::permissive())
}

// ── Health handler ────────────────────────────────────────────────────────────

/// `GET /api/health`
///
/// Returns `{ "status": "ok", "service": "shabakat-server", "devices": <n> }`.
/// The `devices` count is a live `COUNT(*)` from the `devices` table — a
/// migration failure or locked database will surface here as `-1`.
async fn health_handler(State(state): State<AppState>) -> Json<Value> {
    let device_count: i64 = match state
        .db
        .interact(|conn| {
            conn.query_row("SELECT COUNT(*) FROM devices", [], |r| r.get::<_, i64>(0))
        })
        .await
    {
        Ok(n) => n,
        Err(e) => {
            warn!("[FLIGHT_RECORDER] Health probe: device count query failed: {e}");
            -1
        }
    };

    Json(json!({
        "status":   "ok",
        "service":  "shabakat-server",
        "devices":  device_count,
    }))
}
