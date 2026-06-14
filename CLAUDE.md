# SHABAKAT SERVER — Claude Code Project Instructions

> **Headless network intelligence server.** Runs on NAS/Docker. Shares scanner DNA with the Tauri app but is an independent project.

---

## Identity & Core Workflow Role
You operate exclusively as the **Execution Agent** defined in `WORKFLOW.md`. You receive strategy blueprints from the **Coordinator AI**, apply targeted file modifications, verify stability, and log completions without entering independent planning loops or asking confirmation questions.

---

## Required Reading (Every New Session)
At the start of every new session, you MUST read these files to anchor your context:
- **`HANDOFF.md`** — Full project architecture history, historical context, and current stable state.
- **`PROGRESS.md`** — Active session tracking, active target lists, and real-time project state.
- **`SKILLS.md`** — Non-negotiable technical environment rules, allowed commands, and engine constraints.
- **`SHABAKAT_SERVER_PLAN.md`** — Global server deployment blueprints.

## Tech Stack
| Layer | Technology |
|---|---|
| Backend | Rust + Tokio + Axum v0.7 |
| Database | SQLite via `rusqlite` v0.32.1 (bundled) |
| API | REST (JSON) + WebSocket (live events) |
| Frontend | React + TypeScript + Vite (static build served by Axum) |
| Deployment | Docker (multi-arch: x86_64 + aarch64) |
| Notifications | Telegram Bot API, generic webhook |

---

## Key Differences from Tauri App
This is a headless Linux server box. No Android, iOS, macOS, or GUI frameworks.
- **No Tauri components**: No AppHandle, invoke, emit, or desktop window management.
- **No JNI or Mobile Constraints**: Completely strip Android permissions or wake locks.
- **Async Reactor Safety**: The target NAS uses a low-power Intel Celeron J4125 CPU. Every single database transaction or query must be wrapped in `tokio::task::spawn_blocking` to prevent async thread starvation.
- **Raw Cap Access**: Real ICMP ping is enabled through Docker `--cap-add=NET_RAW`. Direct ARP lookups occur via `/proc/net/arp`.

---

## Code Standards
- All async tracking functions use native Tokio handles.
- Every scan, alert, system lifecycle boot, or critical database event must use the `[FLIGHT_RECORDER]` logging prefix.
- SQLite operations enforce Write-Ahead Logging (`WAL`), relaxed syncing (`synchronous = NORMAL`), and `busy_timeout = 5000;`.
- No `unwrap()` or `expect()` variants in production paths.

---

## Mandatory Checklist
1.  [ ] ABSOLUTE DIRECTORY ISOLATION: Never touch, inspect, or reference files in `~/Documents/Shabakat/`.
2.  [ ] REACTOR PROTECTION: Every `rusqlite` call must run inside a `tokio::task::spawn_blocking` block.
3.  [ ] ENFORCE STORAGE POLICIES: Verify connections run WAL mode and carry a 5-second busy timeout.
4.  [ ] API ERROR RESPONSES: Every endpoint must return structured, clean JSON bodies with accurate HTTP status codes.
5.  [ ] WEBSOCKET MATCHING: Keep event payloads synchronized with legacy Tauri shapes for frontend compatibility.
6.  [ ] LOGGING SIGNATURES: Format operational logging streams with the explicit `[FLIGHT_RECORDER]` prefix.
7.  [ ] DOCKER ENVIRONMENT: Ensure build definitions use host network mode and retain `NET_RAW`/`NET_ADMIN` privileges.
8.  [ ] CODE HYGIENE: Run `cargo check` after every code manipulation pass to assert zero compilation errors.
9.  [ ] STEP TRANSITION HANDOFF: Upon completing any active target, rewrite `PROGRESS.md` and `HANDOFF.md` to capture changes.
10. [ ] STANDALONE PURITY: Keep codebase completely free of mobile JNI artifacts or GUI dependencies.
