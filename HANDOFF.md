---
type: project-doc
project: shabakat-server
status: active
---

# Project Handoff & Architecture Sync (V1.9.0)

---

## 🖥️ Core Architecture & Stack

- **Backend:** Rust (Axum, Tokio) — High-concurrency async engine.
- **Database:** SQLite via `rusqlite` crate v0.32.1 (bundled, WAL mode + `busy_timeout = 5000` enforced). All queries dispatched through `tokio::task::spawn_blocking` for Intel Celeron J4125 reactor safety.
- **Frontend:** React + TypeScript (Vite) — GPU-accelerated Canvas for topology mapping.
- **Network Engine:** ARP-Scan + ICMP-Ping + Passive Digital Fence (mDNS/SSDP).

---

## 🔐 Security & Safety Mandates

- **RULE 1: OUT-OF-BAND ONLY.** No inline bridging or spoofing. The server acts as a passive observer and active prober.
- **RULE 2: ATOMIC PERSISTENCE.** All DB writes during scans must be fire-and-forget (`tokio::spawn`) to prevent UI blocking.
- **RULE 3: KERNEL INTEGRITY.** Direct file access to `/proc` is preferred over spawning shell binaries.
- **RULE 4: NETWORK AGNOSTICISM.** Avoid hardcoded IP ranges. Always use dynamic subnet detection for filtering.
- **RULE 5: REACTOR SPARK PROTECTION.** Every `rusqlite` database call MUST be wrapped in a blocking context (`tokio::task::spawn_blocking` or `Db::execute`/`Db::interact`/`Db::interact_mut`) to protect the low-power J4125 CPU from async thread starvation.
- **RULE 6: CASE SANITIZATION.** Normalize all MAC addresses using `normalize_mac()` before doing database insertions or lookups to prevent duplicate rows.
- **RULE 7: STRIP UNWRAPS.** Do not use naked `.unwrap()` or `.expect()` in API routes, network parsers, or database lookups. Use structured error handling.

---

## 🌐 Infrastructure Telemetry Phases

| Component | Status | Implementation Detail |
|---|---|---|
| **Phase 1: Real-time Metrics** | LIVE | High-frequency system telemetry via [[src/monitor/sys_metrics.rs]]. |
| **Phase 2: Uptime & Detection** | LIVE | Trait-Based Notification Hub. Active Verification Route `/api/tools/test-notification`. |
| **Phase 3: Persistence & Viz** | LIVE | Leveled time-series metrics aging via [[src/storage/compactor.rs]] worker thread. |
| **Phase 4: Digital Fence** | LIVE | Network-Agnostic Passive Digital Fence tracking mDNS (5353) and SSDP (1900) via [[src/scanner/digital_fence.rs]]. |

---

## 🛠️ Shipped Milestones & Reality Sync

### 6) SQLite Persistence Layer — Integrated & Verified
- **Files:** [[src/storage/mod.rs]], [[src/storage/schema.sql]], [[src/main.rs]]
- **Storage Driver:** Thread-safe `Arc<Mutex<rusqlite::Connection>>` handle (`Db` struct). Auto-migration via embedded versioning. WAL mode, `synchronous=NORMAL`, `busy_timeout=5000` configured on every connection open. All blocking SQL work is dispatched through `Db::execute` or `Db::interact` wrappers.

### 1) Unified History & Events API Sync
- **Files:** [[src/api/history.rs]]
- **Features:** `/api/events` (device events), `/api/history` (global scan history), and `/api/devices/:mac/history` (per-device history) sync.

### 2) Passive Digital Fence Sentry Engine
- **Files:** [[src/scanner/digital_fence.rs]]
- **Logic:** Continuous background listeners for multicast chatter on ports 5353 (mDNS) and 1900 (SSDP) matching IP packets to MAC signatures via `/proc/net/arp`.

### 3) Dynamic Subnet Boundary Detection
- **Files:** [[src/scanner/network.rs]], [[src/scanner/mod.rs]]
- **Logic:** Automatic local network CIDR lookup removes hardcoded network boundaries.

### 4) Shared WebSocket Event Model
- **Logic:** Digital Fence events (`latency_update`) pushed directly to the React D3 Star-Map to flash/glow devices on activity.

### 5) Auth Middleware — **ACTIVE AND ENFORCING**
- **Reality:** Google OAuth routes exist (`/api/auth/google/login`, `/api/auth/google/callback`, `/api/auth/me`, `/api/auth/logout`) and the auth middleware layer is **now active** in `api/mod.rs` via `axum::middleware::from_fn_with_state`. All `/api/*` routes require a valid JWT in the `Authorization: Bearer` header or `admin_token` cookie. `/api/health` is on the outer `health_router` (outside the nested `/api` tree) and remains intentionally unprotected. Set `SHABAKAT_DISABLE_AUTH=true` in `.env` to bypass during local development.

### 7) Code-Hardening & Reality-Sync Overhaul *(June 12, 2026)*
- **MAC Case Sanitization:** Routed all database insertions, lookups, ignores, and path parameters through `normalize_mac()` in [[src/storage/devices.rs]], [[src/storage/history.rs]], [[src/api/devices.rs]], [[src/api/history.rs]], and [[src/scanner/digital_fence.rs]] to prevent duplicate rows.
- **Unwrap Elimination:** Removed naked `.unwrap()` calls from network parsers and API handlers.
- **Build Warnings Resolution:** Clean-compiled all modules under `cargo check`. Wired background scoring thread (`dashboard::scoring::run`), fingerprints sync task (`scanner::registry::start_sync_task`), and the mDNS service daemon (`network::mdns::start_mdns_advertisement`) into [[src/main.rs]].
- **Infrastructure Realignment:** Configured true home infrastructure bindings (NAS: `192.168.254.18`, Subnet: `192.168.254.0/24`, Gateway: `192.168.254.1`) in [[.env]], and updated image tags to `shabakat-server:latest` in [[docker-compose.yml]] and [[SHABAKAT_SERVER_PLAN.md]].

### 8) Passive Digital Fence Socket Hardening *(June 12, 2026)*
- **Socket Options:** Replaced standard UDP bind with `socket2` configuration using `set_reuse_address(true)` and `set_reuse_port(true)` on the Digital Fence listeners. This resolves the `Address already in use` error for the mDNS (5353) and SSDP (1900) sentries on the host interface network mode.

### 9) Full Maintenance Pass *(June 15, 2026)*
- **Phase 0 audit:** Confirmed ALL subsystems are live and wired. Previous session docs that described main.rs as a skeleton were incorrect fiction — corrected here.
- **WebSocket device-duplication fix:** Removed `devices` array from `scan_finished` payload. The backend was emitting devices twice: once in `device_discovered` batches during the scan, and again in `scan_finished`. Frontend must now call `GET /api/devices` after receiving `scan_finished`. Added `[FLIGHT_RECORDER]` count logs at every batch emit for future diagnostics.
- **Production-path unwrap fixed:** `api/devices.rs` `lock().unwrap()` on in-memory cache → `lock().unwrap_or_else(|p| p.into_inner())`.
- **fix_shabakat.sh rewritten:** Was targeting port 8080 + AUTH_DEBUG/JWT_SECRET (dead scheme). Now uses port 7779, curl /api/health, [FLIGHT_RECORDER] grep.
- **COMMANDS.md fixed:** §6 netstat → ss; §8 wrong DB path + sqlite3-not-in-image caveat; §4 deploy hierarchy clarified.
- **deploy_to_nas.sh marked DEPRECATED.** Canonical path is `deploy.sh` (source-build-on-NAS).
- **build-apkg.sh marked DEPRECATED.** Canonical path is `deploy.sh` (source-build-on-NAS).
- **build-apkg.sh flagged:** Contradicts "avoid App Central" philosophy. Coordinator decision required.
- **cargo audit:** rsa 0.9.10 Marvin Attack (via jsonwebtoken) — no fix available. Auth middleware is currently commented out. Documented.

### 10) Coordinator Decision Batch *(June 15, 2026)*
- **D1 — Speed Test SEVERED:** *(Superseded by Phase A)*. Speed Test has been restored via Cloudflare 5MB payload sampling to avoid uplink saturation.
- **D2 — build-apkg.sh DELETED:** App Central deployments officially out of scope. Docker + host networking on the NAS is the canonical deployment strategy. File is gone.
- **D3 — cargo update EXECUTED:** 42 packages updated. Verified: `cargo check` → 0 errors; `cargo test` → 78/78 passed; `cargo clippy --all-targets -- -D warnings` → 0 warnings. No transitive async/rusqlite breakages.
- **D4 — RSA Marvin Attack ACCEPTED LOCAL RISK:** RUSTSEC-2023-0071 (`rsa 0.9.10` timing side-channel, severity 5.9 medium, via `jsonwebtoken`). No upstream fix. `[FLIGHT_RECORDER]` entry added to `QA_LOG.md`. Server operates behind NAT perimeter only — no public TLS termination, no external RSA exposure. Do NOT patch without new coordinator decision.
- **D5 — Auth Middleware RE-ENABLED:** `axum::middleware::from_fn_with_state` wired back into `src/api/mod.rs`. All `/api/*` routes are now protected. `/api/health` exempt (outer router). `SHABAKAT_DISABLE_AUTH=true` available for local dev bypass.
- **D6 — Frontend WS Protocol PATCHED:** `useNetworkScan.ts` `scan_finished` handler now fires `GET /api/devices` instead of reading a `devices` array from the WS payload (which no longer exists). `transport.ts` `ScanFinishedData` type updated (field removed). Browser-mode resolver changed to resolve with empty array and delegate reconciliation to the hook's `GET /api/devices` fetch. Fallback to in-memory accumulation if the fetch fails.

### 11) Phase A: Speed Test Subsystem *(June 22, 2026)*
- **Endpoints:** Functional `POST /api/speed-test/run` execution loop (Cloudflare 5MB payload sampling) and upgraded `GET /api/speed-test/history` endpoint returning the latest 10 database records.

### 12) Phase B: Alerts Acknowledgment System *(June 22, 2026)*
- **Architecture:** Transitioned from client-side array filtering to a dedicated `GET /api/alerts` endpoint running an efficient SQLite `JOIN` targeting `d.acknowledged = 0`.

---

## 💾 Deployment & Environment Profiles

- **NAS Host:** `192.168.254.18` (Asustor Lockerstor, ADM Port 8811, Portainer Port 19943)
- **Subnet Range:** `192.168.254.0/24` (via interface or explicit override)
- **Target Image:** `shabakat-server:latest`
- **OAuth Cookie Domain:** `GOOGLE_REDIRECT_URI` origin (`192.168.254.18.nip.io:7779`) — auth middleware currently bypassed
- **Container Caps:** Requires `NET_RAW` and `NET_ADMIN`
- **Canonical Deploy:** `./deploy.sh` (rsyncs source, builds on NAS via docker compose)

---

## 🔄 Session Handoff & Implementation Queue

### AppState fields (V1.8.0) — verified present
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

### Pending Coordinator Decisions
*All 6 decisions resolved on 2026-06-15. No open items.*

### Audit Findings
- **Finding A**: Resolved
- **Finding B**: Resolved
- **Finding C**: Resolved
- **Finding D**: Resolved
- **Finding E**: Resolved
