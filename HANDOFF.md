# PROJECT HANDOFF & ARCHITECTURE SYNC (V1.4.1)
**Date:** May 24, 2026
**System Status:** Container-hardened Network-Agnostic Passive Digital Fence, Trait-Based Notification Hub, and Hardened Google OAuth are 100% LIVE.

## 1. Core Architecture (The Stack)
*   **Backend:** Rust (Axum, Tokio) — High-concurrency async engine.
*   **Database:** SQLite via `libSQL` crate (WAL mode + `busy_timeout = 5000` enforced).
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

## 4. Shipped Milestones (May 24, 2026)

### 1) Passive Digital Fence Sentry Engine
*   **File:** `src/scanner/digital_fence.rs`
*   **Logic:** Continuous background listeners for multicast chatter on ports 5353 (mDNS) and 1900 (SSDP).
*   **Integration:** Matches ambient IP packets to Layer 2 MAC signatures via `/proc/net/arp` tracing.
*   **Impact:** Updates `last_seen` timestamps silently without generating active probe traffic.

### 2) Dynamic Subnet Boundary Detection
*   **File:** `src/scanner/network.rs` / `src/scanner/mod.rs`
*   **Feature:** Automatically identifies the local network CIDR using UDP-connect tricks and interface lookups.
*   **Benefit:** Zero-config deployment on any network; removes all hardcoded `192.168.254.x` references.

### 3) Shared WebSocket Event Model
*   **Logic:** Digital Fence events (`latency_update` with synthetic 0.1ms flag) are pushed directly to the React D3 Star-Map.
*   **UI:** Devices "glow" or pulse on the map the moment they broadcast ambient traffic.

### 4) Hardened Google OAuth & Admin Externalization
*   **Security:** Relaxed CSRF cookie path restrictions (`/api/auth`) and enforced `SameSite::Lax` to prevent landing loop failures during cross-origin redirects.
*   **Config:** Fully externalized `SHABAKAT_ADMIN_EMAIL` allowlist, removing all hardcoded PII/Email references from the source tree.
*   **Verification:** Frontend and Backend build chains validated clean.

## 5. Deployment Verification
*   **Node IP:** Dynamic (Verified on WADDAN: `192.168.254.18`).
*   **Git Posture:** Synchronized; Google OAuth CSRF cookie restrictions relaxed via path boundaries, and `SHABAKAT_ADMIN_EMAIL` fully externalized.
*   **Container Caps:** Requires `NET_RAW` and `NET_ADMIN` for ARP tracing and ICMP.
