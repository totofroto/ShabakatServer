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

---

## 💾 Last Verified Stable State

- **Current Date:** 2026-06-14
- **System Stability:** Stable — Authentication middleware bypassed and `/api/auth/me` mocked. Web application compiles and builds successfully without warnings.
- **Last Executed Command:** `npm run build` (web dashboard)
- **Reality Sync Details:**
  - Mocked `/api/auth/me` handler in `src/api/auth.rs` to return a static JSON payload unconditionally.
  - Commented out the outer `auth_middleware` Axum layer in `src/api/mod.rs` to make the entire API public.
  - Modified web client `AuthContext.tsx` to automatically authenticate the user locally on mount and updated the `login` function to skip Google OAuth.
  - Modified web client `login-page.tsx` button to "Connect" and updated redirect logic to point to `/devices` if `shabakat_server_mode` is enabled.
  - Disabled forced redirects to `/login` in `app-shell.tsx` and `transport.ts` when a 401 error occurs, preventing any login blockers.
  - Verified warning-free TypeScript compilation and production build via `npm run build`.

---

## 🪲 Persistent Context Log (Pending Changes / Issues)

1. **OAuth Domain Scope:** Admin tokens are bound to the `GOOGLE_REDIRECT_URI` domain; access via alternate raw IPs causes 401 lockouts by design.
2. **Docker Capability Matrix:** Container requires `NET_RAW` and `NET_ADMIN` capabilities for raw ARP socket lookups and ICMP probes.

---

## 🔄 Session Handoff Protocol

*Before ending any AI session, rewrite this file and [[HANDOFF.md]] to preserve structural state and document all modifications made.*
