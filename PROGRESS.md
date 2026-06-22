---
type: project-doc
project: shabakat-server
status: active
---

# Shabakat Server Project Progress

---

## 🗺️ Workspace Architecture Tree

```
shabakat-server/
├── Cargo.toml                      # Single binary crate configuration
├── docker-compose.yml              # Host networking & capability configuration
├── Dockerfile                      # Multi-arch Linux build script
├── data/
│   └── shabakat.db                 # Persistent SQLite database file
└── src/
    ├── main.rs                     # Runtime entry point & background worker orchestration
    ├── api/                        # REST & WebSocket route handlers (17 files)
    ├── config.rs                   # Environment configuration (12-factor mapping)
    ├── dashboard/                  # Live background status scoring task
    ├── middleware/                  # JWT auth — ACTIVE and protecting all /api/* routes
    ├── monitor/                    # Telemetry: outage_detector, presence, router, sys_metrics
    ├── network/                    # mDNS advertisement helpers
    ├── notifications/              # Trait-based Telegram & webhook dispatcher
    ├── scanner/                    # Core engine: ARP, ICMP, fingerprints, Digital Fence
    ├── scheduler/                  # Core periodic scan orchestration loop
    ├── storage/                    # Database connections, schema migrations, and sub-modules
    ├── tools/                      # Direct ping & lookup tool utilities
    └── types.rs                    # Shared type declarations & serialization definitions
```

---

## 🛠️ Active Implementation Targets

- [x] **Active Target 1:** E2E Integration Testing via local container deployment.
- [x] **Active Target 2:** Validate live backend health visibility through the `/api/health` lookup endpoint.
- [x] **Active Target 3:** Add optional authentication bypass variables (`SHABAKAT_DISABLE_AUTH=true`) to simplify development.
- [x] **Active Target 4:** Verify D3 star-map layout data synchronization with `/api/history` metrics.
- [x] **Active Target 5:** Verify production Docker build architecture and capability matrix.
- [x] **Active Target 6:** Overhaul project documentation, sync home profile, and execute code hardening sweep.
- [x] **Active Target 7:** Authentication middleware stripped and session endpoint mocked for local-only access.
- [x] **Active Target 8:** Audited and bypassed client-side login UI blockers in the web client application.
- [x] **Active Target 9:** Fix database duplication bug by refining the SQLite UPSERT logic and verifying uniqueness constraint.
- [x] **Active Target 10:** Implement a Stale State Sweeper background task to prevent ghost devices.
- [x] **Active Target 11:** Full maintenance pass — Phase 0 reality audit, Phase 1 safety sweep, Phase 2 bug fixes (2026-06-15).
- [x] **Active Target 12:** Coordinator decision batch executed (2026-06-15) — speed test severed, build-apkg.sh deleted, cargo update (42 pkg), RSA risk accepted, auth middleware re-enabled, WS scan_finished patched to fetch GET /api/devices.
- [x] **Active Target 13:** Phase A completed — Speed Test Subsystem: POST `/api/speed-test/run` (Cloudflare 5MB payload sampling) and GET `/api/speed-test/history` endpoint returning latest 10 database records.
- [x] **Active Target 14:** Phase B completed — Alerts Acknowledgment System: Optimized backend architecture—transition from client-side array filtering to a dedicated GET `/api/alerts` endpoint running an efficient SQLite JOIN targeting `d.acknowledged = 0`.

---

## 💾 Last Verified Stable State

- **Current Date:** 2026-06-15
- **System Stability:** Stable — 0 cargo check errors, 0 clippy warnings, 78/78 tests passing.
- **Last Executed Commands:** `cargo update` (42 packages), `cargo check`, `cargo test` (78/78), `cargo clippy --all-targets -- -D warnings`
- **Coordinator Decision Batch (2026-06-15):**
  - D1: (Superseded by Phase A) Speed Test restored. Functional `POST /api/speed-test/run` execution loop implemented with Cloudflare 5MB payload sampling. `GET /api/speed-test/history` endpoint returns latest 10 database records.
  - D2: `build-apkg.sh` deleted. App Central deployments out of scope — Docker + host networking is the canonical deployment.
  - D3: `cargo update` applied (42 packages updated). `cargo check` → 0 errors. `cargo test` → 78/78 passed. `cargo clippy --all-targets -- -D warnings` → 0 warnings.
  - D4: RSA Marvin Attack (RUSTSEC-2023-0071) — local risk explicitly accepted. Logged in QA_LOG.md as `[FLIGHT_RECORDER]` entry. Server operates behind NAT perimeter only.
  - D5: Auth middleware re-enabled. `axum::middleware::from_fn_with_state` wired in `src/api/mod.rs`. All `/api/*` routes now protected. `/api/health` is on the outer `health_router` and remains unprotected. `SHABAKAT_DISABLE_AUTH=true` bypasses for local dev.
  - D6: Frontend WS `scan_finished` handler patched. `useNetworkScan.ts` now fires `GET /api/devices` on `scan_finished` instead of reading a `devices` array from the WS payload. `transport.ts` type and resolver also updated. Fallback to in-memory accumulation if fetch fails.

---

## ⚠️ Pending Decisions (from Coordinator)

*All 6 pending decisions resolved by Coordinator on 2026-06-15. No open items.*

---

## 🪲 Persistent Context Log (Pending Changes / Issues)

1. **OAuth Domain Scope:** Auth middleware is now ACTIVE. When `SHABAKAT_DISABLE_AUTH=false` (default), all `/api/*` routes require a valid JWT in `Authorization: Bearer` header or `admin_token` cookie. The `/api/health` route and `/api/auth/google/login`, `/api/auth/google/callback` routes are unprotected.
2. **Docker Capability Matrix:** Container requires `NET_RAW` and `NET_ADMIN` capabilities for raw ARP socket lookups and ICMP probes.
3. **speed_test.rs:** Module source file preserved but the module declaration and routes are commented out per Coordinator order. Do not re-enable without explicit coordinator approval.

---

## 🔄 Session Handoff Protocol

*Before ending any AI session, rewrite this file and [[HANDOFF.md]] to preserve structural state and document all modifications made.*
