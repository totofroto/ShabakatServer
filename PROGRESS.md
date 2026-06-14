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
    ├── api/                        # REST & WebSocket route handlers
    ├── config.rs                   # Environment configuration (12-factor mapping)
    ├── dashboard/                  # Live background status scoring task
    ├── middleware/                 # JWT validation & authorization layer
    ├── monitor/                    # Telemetry: sys_metrics, presence, bandwidth
    ├── network/                    # mDNS & subnet processing utilities
    ├── notifications/              # Trait-based Telegram & webhook dispatcher
    ├── scanner/                    # Core engine: ARP, ICMP, and Passive Digital Fence
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

---

## 💾 Last Verified Stable State

- **Current Date:** 2026-06-14
- **System Stability:** Stable — Fixed device duplication by checking existence before upserting, added database migration v3 for `is_active` column, and implemented a periodic stale state sweeper task to transition devices offline after 5 minutes of inactivity.
- **Last Executed Command:** `cargo test`
- **Reality Sync Details:**
  - Added SQLite schema migration v3 to add `is_active` column to the `devices` table, ensuring the table has the required fields.
  - Refactored `upsert_discovered_device` in `src/storage/devices.rs` to check MAC existence before inserting to prevent reporting existing devices as new.
  - Added `run_stale_state_sweeper` task running every 60 seconds to mark devices offline after 300 seconds of inactivity and broadcast `latency_update` websocket updates.
  - Updated `DeviceRecord::is_online` dynamic check in `src/types.rs` to return the database column value directly.
  - Updated `list_devices` in `src/storage/devices.rs` to query online devices using `is_online = 1`.

---

## 🪲 Persistent Context Log (Pending Changes / Issues)

1. **OAuth Domain Scope:** Admin tokens are bound to the `GOOGLE_REDIRECT_URI` domain; access via alternate raw IPs causes 401 lockouts by design.
2. **Docker Capability Matrix:** Container requires `NET_RAW` and `NET_ADMIN` capabilities for raw ARP socket lookups and ICMP probes.

---

## 🔄 Session Handoff Protocol

*Before ending any AI session, rewrite this file and [[HANDOFF.md]] to preserve structural state and document all modifications made.*

