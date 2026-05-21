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