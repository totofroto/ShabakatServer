# PROJECT HANDOFF & ARCHITECTURE SYNC
**Date:** May 21, 2026
**System Status:** V1 Audit Remediations Pending

## 1. Core Architecture (The Stack)
* **Backend:** Rust (Axum, Tokio) running as a Docker container.
* **Database:** SQLite via `libSQL` crate (WAL mode enabled).
* **Frontend:** React + TypeScript (Vite).

## 2. Infrastructure Constraints & Security Mandates (DO NOT VIOLATE)
* **RULE 1: NO INLINE BRIDGING / SPOOFING.** Must remain out-of-band.
* **RULE 2: STRICT DB BATCHING.** All database writes during a scan must be wrapped in an explicit transaction block (`BEGIN` / `COMMIT`). Double-writing inside the scanning loop and the scheduler is strictly forbidden. Background tasks must not hold the global DB connection lock open while performing async operations (`yield`).
* **RULE 3: AUTHENTICATION INTEGRITY.** - **Google OAuth:** MUST enforce a strict admin email allowlist (`tarekshek@gmail.com`). 
  - **CSRF Protection:** State parameter MUST be generated, passed, and validated during OAuth exchanges.
  - **No Header Bypasses:** local auth bypass logic must NEVER trust forgeable HTTP headers like `X-Forwarded-For` or `X-Real-IP`.
  - **JWT Hardening:** Default fallback secrets are banned; the server must panic at startup if `JWT_SECRET` is missing. Cookies must enforce `secure: true` when running outside development.

## 3. Current Focus: Remediation Phase
* We are systemically fixing the 4 Critical and 5 High-severity issues discovered during the V1 Audit Report to secure and stabilize the Command Center.

## 4. Scan Pipeline Regression — Diagnosed & Fixed (May 21, 2026)

**Symptom:** Scan hung at 95%, returned 0 devices, `TypeError: n.slice is not a function` in browser console, UI completely frozen (no cancel, no logout).

**Root Causes Found:**

| # | File | Bug | Fix |
|---|---|---|---|
| 1 | `src/api/scan.rs` | `scan_finished` broadcast was awaited AFTER `complete_scan_persistence` — any DB lag caused a 140s UI freeze | Broadcast `scan_finished` first; run persistence in `tokio::spawn` (fire-and-forget) |
| 2 | `src/api/scan.rs` | Backend emitted `"scan_error"` event, frontend listened for `"scan_failed"` — engine errors never unblocked the UI | Renamed event to `"scan_failed"` |
| 3 | `src/storage/devices.rs` | Dedicated DB connection had no `busy_timeout` pragma — WAL write contention caused immediate `SQLITE_BUSY` failure | Added `PRAGMA busy_timeout = 5000` as first statement on dedicated conns |
| 4 | `web/src/lib/transport.ts` | `scan_network` Promise had no `scan_failed` listener — engine errors left it unresolved until 140s timeout | Added `wsListen("scan_failed")` that immediately rejects with the error |
| 5 | `web/src/lib/transport.ts` | `scan_status` command had no REST mapping — browser-mode hydration silently skipped the scan-active guard | Added `scan_status` → `GET /api/scan/status` |
| 6 | `web/src/hooks/useNetworkScan.ts` | `finally` block did not call `setProgressPct(0)` — UI could freeze at 95% on any abort path | Added `setProgressPct(0)` to `finally` |
| 7 | `web/src/hooks/useNetworkScan.ts` / `transport.ts` | `data.devices ?? []` could pass an error object to `.slice`/`.map` — caused the `TypeError` | Replaced with `Array.isArray(data.devices) ? data.devices : []` everywhere |

**Commit:** `21765f7` — `cargo check` and `npm run build` pass clean.