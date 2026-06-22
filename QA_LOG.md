# QA Log

This file tracks testing, quality assurance checks, and verification procedures for the Shabakat Server project.

## Quality Assurance & Verification Logs

| Date | Target / Change | Verified By | Status | Notes / Command Executed |
|------|-----------------|-------------|--------|--------------------------| 
| 2026-06-14 | Setup Metadata Protocol & Initial Analysis | Antigravity | PASSED | Project mapping completed; auth middleware and routes analysed. |
| 2026-06-14 | Strip Authentication & Mock /api/auth/me | Antigravity | PROVEN | Removed auth layer and mocked me endpoint; successfully verified warning-free router type checks via cargo check. |
| 2026-06-14 | Deep Clean & Zero-Bug Verification Build | Antigravity | PROVEN | Fresh npm build & cargo build --release passed warning-free; fixed missing networks table schema and clippy warnings. |
| 2026-06-14 | Fix database duplication & stale ghosting | Antigravity | PASSED | Refactored upsert to prevent duplicates; created Stale State Sweeper to mark offline after 300s & broadcast via WS; ran cargo test. |
| 2026-06-15 | Phase 0 Reality Audit | Antigravity | PROVEN | `cargo check` → 0 errors. All subsystems confirmed live in code and wired into main.rs. Previous claim of "skeleton main.rs" was fiction — corrected. |
| 2026-06-15 | Phase 1 Safety Sweep — unwrap/normalize_mac/spawn_blocking | Antigravity | PROVEN | `cargo clippy --all-targets -- -D warnings` → 0 warnings. `grep -r '.unwrap()' src/` → 0 results. `grep -r 'normalize_mac' src/` → 28 call sites covering all read/write paths. One production-path `lock().unwrap()` found in api/devices.rs and fixed. |
| 2026-06-15 | Phase 1 — Test suite | Antigravity | PROVEN | `cargo test` → 78 passed; 0 failed; 0 ignored. Output: `test result: ok. 78 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 20.25s` |
| 2026-06-15 | Phase 1 — cargo audit | Antigravity | DOCUMENTED | 1 vulnerability: rsa 0.9.10 Marvin Attack (RUSTSEC-2023-0071, severity 5.9 medium) via jsonwebtoken. No fix available upstream. Auth middleware is currently bypassed. 2 unmaintained warnings (gcc, paste) from transitive deps — no CVE. |
| 2026-06-15 | Phase 2 — WebSocket device duplication fix | Antigravity | PROVEN | Root cause traced: scan_finished emitted full device array PLUS device_discovered batches already sent during scan → 2× duplication on frontend accumulation. Fix: removed `devices` from scan_finished payload. Added [FLIGHT_RECORDER] count logs at each emit. `cargo check` → 0 errors post-fix. |
| 2026-06-15 | Phase 2 — fix_shabakat.sh rewrite | Antigravity | PROVEN | Old script used PORT=8080 and AUTH_DEBUG/JWT_SECRET (dead scheme). Rewritten to port 7779, curl /api/health, [FLIGHT_RECORDER] grep. |
| 2026-06-15 | Phase 2 — COMMANDS.md §6 and §8 corrections | Antigravity | PROVEN | §6: netstat not in debian:bookworm-slim image → replaced with ss. §8: /app/data/ wrong path → correct volume path documented; sqlite3-not-in-image caveat added with api/health alternative. |
| 2026-06-15 | Phase 2 — Deploy script reconciliation | Antigravity | PROVEN | deploy_to_nas.sh marked DEPRECATED (binary-rsync strategy, .env transfer risk). deploy.sh confirmed canonical. COMMANDS.md §4 updated with hierarchy. |
| 2026-06-15 | Phase 2 — build-apkg.sh vs philosophy | Antigravity | FLAGGED | Contradiction flagged via COORDINATOR DECISION NEEDED block in script header. Not deleted — awaiting coordinator decision. |
| 2026-06-15 | Post-fix verification | Antigravity | PROVEN | `cargo check` → 0 errors; `cargo clippy --all-targets -- -D warnings` → 0 warnings; `cargo test` → 78/78 passed. All checks clean after all Phase 1+2 changes. |
| 2026-06-15 | [FLIGHT_RECORDER] RSA Marvin Attack — ACCEPTED LOCAL RISK | Coordinator | ACCEPTED | RUSTSEC-2023-0071: `rsa 0.9.10` timing side-channel (Marvin Attack, severity 5.9 medium) via `jsonwebtoken`. No upstream fix available. **Coordinator ruling (2026-06-15):** Risk accepted. Shabakat Server operates exclusively behind a local NAT perimeter. There is no public TLS termination and no external exposure of RSA-encrypted traffic. The risk of a timing-based private key recovery attack in this deployment envelope is negligible. Do NOT attempt to patch or replace `rsa`/`jsonwebtoken` without a new coordinator decision. |
| 2026-06-15 | Coordinator Decision Batch — 6 pending decisions resolved | Coordinator+Antigravity | PROVEN | D1: speed-test/run route commented out (uplink saturation). D2: build-apkg.sh deleted (App Central out of scope). D3: cargo update (42 packages) + cargo check/test/clippy run. D4: RSA Marvin Attack risk accepted (see above). D5: Auth middleware re-enabled and wired. D6: Frontend WS scan_finished updated to fetch GET /api/devices. |
