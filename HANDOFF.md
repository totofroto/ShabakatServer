# PROJECT HANDOFF & ARCHITECTURE SYNC (V1.7.0)
**Date:** June 3, 2026
**System Status:** Full persistence integration complete. All modules wired into `main.rs`. `AppState` carries db, config, devices, notifications, broadcast_tx, bandwidth, system_telemetry, log_tx. All background workers (scheduler, outage detector, presence monitor, bandwidth monitor, sys-metrics monitor, digital fence, metrics compactor) spawn at startup. `cargo build --release` passes clean — **binary: 15 MB**.

## 1. Core Architecture (The Stack)
*   **Backend:** Rust (Axum, Tokio) — High-concurrency async engine.
*   **Database:** SQLite via `rusqlite` crate v0.32.1 (bundled, WAL mode + `busy_timeout = 5000` enforced). All queries dispatched through `tokio::task::spawn_blocking` for Celeron J4125 reactor safety.
*   **Frontend:** React + TypeScript (Vite) — GPU-accelerated Canvas for topology.
*   **Network Engine:** ARP-Scan + ICMP-Ping + **Passive Digital Fence (mDNS/SSDP)**.

## 2. Security Mandates (Non-Negotiable)
*   **RULE 1: OUT-OF-BAND ONLY.** No inline bridging or spoofing. The server acts as a passive observer and active prober.
*   **RULE 2: ATOMIC PERSISTENCE.** All DB writes during scans must be fire-and-forget (`tokio::spawn`) to prevent UI blocking.
*   **RULE 3: KERNEL INTEGRITY.** Direct file access to `/proc` is preferred over spawning shell binaries.
*   **RULE 4: NETWORK AGNOSTICISM.** Avoid hardcoded IP ranges. Always use dynamic subnet detection for filtering.

## 3. Infrastructure Telemetry — Phases 1-4 (100% Complete)
**Goal:** Elimination of external dependencies (Uptime Kuma, Netdata, Prometheus).

| Component | Status | Implementation Detail |
|---|---|---|
| **Phase 1: Real-time Metrics** | LIVE | High-frequency system telemetry via `src/monitor/sys_metrics.rs`. |
| **Phase 2: Uptime & Detection** | LIVE | Trait-Based Notification Hub, Active Verification Route `/api/tools/test-notification`. |
| **Phase 3: Persistence & Viz** | LIVE | Leveled time-series metrics aging via `compactor.rs` worker thread. |
| **Phase 4: Digital Fence** | LIVE | Network-Agnostic Passive Digital Fence with Dynamic Subnet Detection tracking mDNS (5353) and SSDP (1900). |

## 4. Shipped Milestones

### 6) SQLite Persistence Layer — Integrated & Verified *(June 1, 2026)*
*   **Files:** `src/storage/mod.rs`, `src/storage/schema.sql`, `src/main.rs`

#### Storage Module (`src/storage/`)
*   **Schema** (`schema.sql`): Four-table relational design — `devices` (MAC-keyed registry), `scan_history` (append-only per-scan-per-device timeline), `device_events` (lifecycle/alert ledger), `settings` (key-value runtime config). All `CREATE TABLE IF NOT EXISTS` for idempotent migrations. Foreign key constraints with `ON DELETE CASCADE` / `ON DELETE SET NULL`. Six targeted indexes on MAC lookups, timestamps, and event types.
*   **Driver** (`mod.rs`): Thread-safe `Arc<Mutex<rusqlite::Connection>>` handle (`Db` struct). Cloneable across Axum handlers and background Tokio tasks at zero cost. Auto-migration via embedded `include_str!("schema.sql")` and `PRAGMA user_version` versioning. WAL mode, `synchronous=NORMAL`, `busy_timeout=5000`, `foreign_keys=ON`, `mmap_size=256MiB` configured on every connection open. `Db::interact()` and `Db::interact_mut()` wrappers dispatch all blocking SQL work through `tokio::task::spawn_blocking`, preserving the async reactor for network I/O.

#### Application Wiring (`src/main.rs`)
*   **Module declaration:** `mod storage;` registered.
*   **`AppState` struct:** `pub db: Db` — the single, cloneable database handle shared across the entire router tree. Documented with fields to add as future subsystems come online (scanner state, WebSocket broadcast channel, runtime config).
*   **Startup lifecycle (in order):**
    1. `env_logger` initialised (defaults to `info`; overridden by `RUST_LOG`).
    2. `SHABAKAT_DATA_DIR` resolved (defaults to `./data/`; Docker volume mounted at `/data`).
    3. Target directory auto-created with `tokio::fs::create_dir_all` (idempotent, async-safe).
    4. `Db::open` dispatched via `spawn_blocking` — WAL setup + schema migration off the reactor.
    5. `AppState` assembled and injected via `.with_state()`.
    6. `CorsLayer::permissive()` applied as outer middleware.
    7. `SHABAKAT_PORT` resolved (defaults to 7779), `TcpListener` bound, `axum::serve` started.
*   **`GET /api/health`** endpoint live: returns `{ status, service, devices }` where `devices` is a live `COUNT(*)` from the `devices` table — doubles as a storage layer sanity check.
*   **`cargo check` result:** ✅ `Finished dev profile` — zero errors, zero warnings.

### 1) Unified History & Events API Sync
*   **File:** `src/api/history.rs` / `web/src/components/device-details-panel.tsx`
*   **Feature:** Synchronized backend API with frontend expectations. Implemented `/api/events` (device events), `/api/history` (global scan history), and `/api/devices/:mac/history` (per-device history).
*   **Benefit:** Enables functional 'Activity' timeline and per-device historical views in the web UI. Fixed camelCase/snake_case inconsistencies between React components.

### 2) Passive Digital Fence Sentry Engine
*   **File:** `src/scanner/digital_fence.rs`
*   **Logic:** Continuous background listeners for multicast chatter on ports 5353 (mDNS) and 1900 (SSDP).
*   **Integration:** Matches ambient IP packets to Layer 2 MAC signatures via `/proc/net/arp` tracing.
*   **Impact:** Updates `last_seen` timestamps silently without generating active probe traffic.

### 3) Dynamic Subnet Boundary Detection
*   **File:** `src/scanner/network.rs` / `src/scanner/mod.rs`
*   **Feature:** Automatically identifies the local network CIDR using UDP-connect tricks and interface lookups.
*   **Benefit:** Zero-config deployment on any network; removes all hardcoded `192.168.254.x` references.

### 4) Shared WebSocket Event Model
*   **Logic:** Digital Fence events (`latency_update` with synthetic 0.1ms flag) are pushed directly to the React D3 Star-Map.
*   **UI:** Devices "glow" or pulse on the map the moment they broadcast ambient traffic.

### 5) Hardened Google OAuth & Admin Externalization
*   **Security:** Relaxed CSRF cookie path restrictions (`/api/auth`) and enforced `SameSite::Lax` to prevent landing loop failures during cross-origin redirects.
*   **Config:** Fully externalized `SHABAKAT_ADMIN_EMAIL` allowlist, removing all hardcoded PII/Email references from the source tree.
*   **Verification:** Frontend and Backend build chains validated clean.

### 7) Device Deduplication — MAC Case Normalization *(June 3, 2026)*
*   **Root Cause:** Active scanner wrote MAC addresses uppercase; Digital Fence wrote them lowercase. SQLite's case-sensitive `ON CONFLICT(mac)` never fired → one duplicate row per fence-discovered device (51 rows for 33 real devices).
*   **Fix:** Added `normalize_mac()` in `src/scanner/arp.rs`; routed all DB MAC writes through it (`digital_fence.rs`, `storage/devices.rs`). Deployed as `shabakat-server:v4`.
*   **Cleanup Migration:** One-time SQL merged 20 lowercase duplicate rows into their uppercase survivors, repointing `scan_history.device_id` foreign keys (0 orphaned rows), preserving `custom_name`/`acknowledged`. Device count corrected: 51 → 33.

### 8) OAuth Cookie Origin Clarification *(June 3, 2026)*
*   **Finding:** The live `admin_token` cookie is scoped to the `GOOGLE_REDIRECT_URI` origin (`192.168.254.18.nip.io:7779`). Accessing the UI from a different origin (direct IP, Tailscale address) causes a 401 lockout because the cookie domain won't match.
*   **Action:** Documented — no code change required. Always browse via the redirect URI origin after OAuth login.

## 5. Deployment Verification
*   **Node IP:** Dynamic (Verified on WADDAN: `192.168.254.18`).
*   **Git Posture:** Synchronized; Google OAuth CSRF cookie restrictions relaxed via path boundaries, and `SHABAKAT_ADMIN_EMAIL` fully externalized.
*   **Container Caps:** Requires `NET_RAW` and `NET_ADMIN` for ARP tracing and ICMP.

## 6. Next Session — Implementation Queue

### All modules wired (V1.7.0)
Every module is now declared in `main.rs` and compiles cleanly:
- `mod api` ✅ — full REST + WebSocket router (auth, devices, scan, history, tools, settings, etc.)
- `mod config` ✅ — `Config::from_env()` (port, scan interval, JWT, OAuth, Telegram)
- `mod dashboard` ✅ — background scoring task
- `mod middleware` ✅ — JWT auth middleware
- `mod monitor` ✅ — outage detector, presence, bandwidth, sys-metrics
- `mod network` ✅ — mDNS helpers
- `mod notifications` ✅ — Telegram / SMTP / webhook, `NotificationDispatcher`
- `mod scanner` ✅ — full scan engine (ARP, ICMP, mDNS, SSDP, fingerprints, registry, digital fence)
- `mod scheduler` ✅ — auto-scan loop wired to persistence
- `mod storage` ✅ — `Db`, `AppDb`, `execute()`, `now_ms()`, all 9 sub-modules, v2 schema
- `mod tools` ✅ — ping utility
- `mod types` ✅ — `DiscoveredDevice`, `DeviceRecord`, `NetworkRecord`, etc.

### AppState fields (V1.7.0)
```rust
pub struct AppState {
    pub db:               Db,                                   // SQLite handle
    pub config:           Arc<Config>,                          // env config
    pub devices:          Arc<Mutex<Vec<DiscoveredDevice>>>,    // in-memory cache
    pub notifications:    NotificationDispatcher,               // alert broker
    pub broadcast_tx:     broadcast::Sender<Value>,             // WebSocket fanout
    pub bandwidth:        Arc<Mutex<Option<RouterBandwidth>>>,  // bandwidth snapshot
    pub system_telemetry: Arc<Mutex<Option<SystemTelemetry>>>,  // telemetry snapshot
    pub log_tx:           broadcast::Sender<String>,            // debug SSE logs
}
```

### Next Steps
- Test full end-to-end: `docker compose up --build`, verify `/api/health`, trigger a scan, check `/api/devices`
- Set required env vars: `JWT_SECRET`, `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`, `GOOGLE_REDIRECT_URI`, `SHABAKAT_ADMIN_EMAIL`
- Optional: `SHABAKAT_DISABLE_AUTH=true` or `SHABAKAT_AUTH_BYPASS_LOCAL=true` for local dev
