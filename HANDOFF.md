---
type: project-doc
project: shabakat-server
status: active
---

# Project Handoff & Architecture Sync (V1.7.1)

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
- **Files:** [[src/api/history.rs]], `web/src/components/device-details-panel.tsx`
- **Features:** `/api/events` (device events), `/api/history` (global scan history), and `/api/devices/:mac/history` (per-device history) sync.

### 2) Passive Digital Fence Sentry Engine
- **Files:** [[src/scanner/digital_fence.rs]]
- **Logic:** Continuous background listeners for multicast chatter on ports 5353 (mDNS) and 1900 (SSDP) matching IP packets to MAC signatures via `/proc/net/arp`.

### 3) Dynamic Subnet Boundary Detection
- **Files:** [[src/scanner/network.rs]], [[src/scanner/mod.rs]]
- **Logic:** Automatic local network CIDR lookup removes hardcoded network boundaries.

### 4) Shared WebSocket Event Model
- **Logic:** Digital Fence events (`latency_update`) pushed directly to the React D3 Star-Map to flash/glow devices on activity.

### 5) Hardened Google OAuth & Admin Externalization
- **Logic:** Relaxed cookie path restrictions (`/api/auth`) and SameSite cookie properties. `SHABAKAT_ADMIN_EMAIL` fully externalized.

### 7) Code-Hardening & Reality-Sync Overhaul *(June 12, 2026)*
- **MAC Case Sanitization:** Routed all database insertions, lookups, ignores, and path parameters through `normalize_mac()` in [[src/storage/devices.rs]], [[src/storage/history.rs]], [[src/api/devices.rs]], [[src/api/history.rs]], and [[src/scanner/digital_fence.rs]] to prevent duplicate rows.
- **Unwrap Elimination:** Removed naked `.unwrap()` calls from network parsers and API handlers.
- **Build Warnings Resolution:** Clean-compiled all modules under `cargo check`. Wired background scoring thread (`dashboard::scoring::run`), fingerprints sync task (`scanner::registry::start_sync_task`), and the mDNS service daemon (`network::mdns::start_mdns_advertisement`) into [[src/main.rs]].
- **Infrastructure Realignment:** Configured true home infrastructure bindings (NAS: `192.168.254.18`, Subnet: `192.168.254.0/24`, Gateway: `192.168.254.1`) in [[.env]], and updated image tags to `shabakat-server:latest` in [[docker-compose.yml]] and [[SHABAKAT_SERVER_PLAN.md]].

### 8) Passive Digital Fence Socket Hardening *(June 12, 2026)*
- **Socket Options:** Replaced standard UDP bind with `socket2` configuration using `set_reuse_address(true)` and `set_reuse_port(true)` on the Digital Fence listeners. This resolves the `Address already in use` error for the mDNS (5353) and SSDP (1900) sentries on the host interface network mode.

---

## 💾 Deployment & Environment Profiles

- **NAS Host:** `192.168.254.18` (Asustor Lockerstor, ADM Port 8811, Portainer Port 19943)
- **Subnet Range:** `192.168.254.0/24` (via interface or explicit override)
- **Target Image:** `shabakat-server:latest`
- **OAuth Cookie Domain:** `GOOGLE_REDIRECT_URI` origin (`192.168.254.18.nip.io:7779`)
- **Container Caps:** Requires `NET_RAW` and `NET_ADMIN`

---

## 🔄 Session Handoff & Implementation Queue

### AppState fields (V1.7.1)
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
- Monitor persistent system scores and active/passive telemetry feeds under load on the live NAS stack.
- Check Telegram notifications and WebSocket feeds to verify ambient presence updates flow to the UI.
