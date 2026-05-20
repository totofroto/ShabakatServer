# SHABAKAT SERVER — Claude Code Project Instructions

> **Headless network intelligence server.** Runs on NAS/Docker. Shares scanner DNA with the Tauri app but is an independent project.

---

## Identity

You are the engineer building Shabakat Server — a 24/7 network monitoring service that runs as a Docker container on an Asustor NAS (or any Linux box). The scanning logic was born in the Shabakat Tauri app but this project is independent. You do not modify the Tauri app. You do not share a Cargo workspace with it. You copy what you need and evolve it for the server context.

---

---

## Required Reading (Every New Session)

At the start of every new session, read:
- **`HANDOFF.md`** — full project context, current state, what's done, what's pending. Read this before anything else.
- **`SHABAKAT_SERVER_PLAN.md`** — server architecture and implementation plan.


## Tech Stack

| Layer | Technology |
|---|---|
| Backend | Rust + Tokio + Axum |
| Database | SQLite via rusqlite |
| API | REST (JSON) + WebSocket (live events) |
| Frontend | React + TypeScript + Vite (static build served by Axum) |
| Deployment | Docker (multi-arch: x86_64 + aarch64) |
| Notifications | Telegram Bot API, generic webhook |

---

## Architecture

```
Docker container (--network host, --cap-add=NET_RAW)
├── Axum HTTP server (:8080)
│   ├── /api/*          REST endpoints
│   ├── /ws             WebSocket (live scan events, latency, alerts)
│   └── /*              Static React frontend
├── Background scheduler
│   ├── Full scan       Every N minutes (configurable)
│   └── Heartbeat       Ping known devices every M minutes
├── Scanner engine
│   ├── TCP + ICMP ping
│   ├── Port Guardian
│   ├── SSDP/UPnP
│   ├── mDNS/Zeroconf
│   ├── ARP (/proc/net/arp)
│   └── Fingerprint engine
├── Notification dispatcher
│   ├── Telegram
│   └── Webhook
└── SQLite database (/data/shabakat.db)
```

---

## Key Differences from Tauri App

This is a Linux server. No Android. No macOS. No GUI framework. This means:

- **No Tauri, no AppHandle, no invoke, no emit.** All Tauri imports must be removed.
- **No JNI, no MulticastLock.** Linux doesn't need it.
- **No SO_BINDTODEVICE.** Wired ethernet on NAS has no dual-NIC routing issue.
- **No Box::leak.** No Android hot-reload to worry about.
- **No OS_DNS_SEM.** glibc's resolver is thread-safe. No Bionic mutex crash.
- **No Android permission dance.** Docker `--network host` provides full access.
- **Real ICMP ping allowed.** Docker `--cap-add=NET_RAW` grants it.
- **ARP via /proc/net/arp.** Direct file read, not rtnetlink or subprocess.
- **No concurrency cap at 64.** NAS has more resources than a phone.

When copying Rust code from the Tauri project (`~/Documents/Shabakat/src-tauri/src/`), strip every `#[cfg(target_os = "android")]` block and every `use tauri::` import. Keep the core logic.

---

## How We Work

Same debug relay as the Tauri project. Tareg does not write code. He deploys, tests, and pastes logs.

For the server, logs come from Docker:
```bash
docker logs -f shabakat-server
```

Or during development:
```bash
cargo run 2>&1
```

When Tareg pastes logs, you analyze and fix. Same protocol as the Tauri project CLAUDE.md.

---

## Code Standards

- All async functions use Tokio.
- All errors return proper HTTP status codes (400/404/500) with JSON error bodies.
- Every scan, alert, and significant action logs with `[FLIGHT_RECORDER]` prefix (same convention).
- SQLite operations use rusqlite with connection pooling (r2d2-sqlite or deadpool-sqlite).
- WebSocket messages use the same event names and payload shapes as the Tauri app for frontend compatibility.
- Config via environment variables (12-factor app style).
- No unwrap() in production paths. All errors handled and logged.

---

## Build Environment

| Item | Value |
|---|---|
| Machine | MacBook Pro M1 Pro (cross-compiles for x86_64-unknown-linux-gnu) |
| Target NAS | Asustor Lockerstor Gen 1 (Intel Celeron J4125, x86_64) |
| Docker on NAS | Accessible via SSH: `ssh totofroto@192.168.254.18` |
| Tailscale IP | 100.82.32.61 |
| Claude Code | `~/.local/bin/claude` |

---

## Reference Files

The Tauri app's scanner code lives at:
```
~/Documents/Shabakat/src-tauri/src/
├── scanner/
│   ├── mod.rs           ← main scan engine
│   ├── ping.rs          ← TCP ping
│   ├── ports.rs         ← Port Guardian
│   ├── arp.rs           ← ARP lookup
│   └── ...
├── fingerprints.rs      ← device fingerprint rules
├── network.rs           ← subnet detection
├── tools.rs             ← ping/dns/wake/whois/etc.
├── monitor.rs           ← watchdog + live-watch
├── commands.rs          ← Tauri IPC commands (DO NOT COPY — replace with Axum routes)
├── mdns_scanner.rs      ← mDNS browser
└── lib.rs               ← Tauri bootstrap (DO NOT COPY)
```

**Copy:** scanner/, fingerprints.rs, network.rs, arp.rs, tools.rs, monitor.rs, mdns_scanner.rs
**Do NOT copy:** commands.rs, lib.rs (these are Tauri-specific)
**Strip from copied files:** all `tauri::` imports, `AppHandle` parameters, `#[cfg(target_os = "android")]` blocks, JNI code, `emit()` calls

---

## Mandatory Checklist

1. ☐ Never modify files in ~/Documents/Shabakat/ (the Tauri app)
2. ☐ All scanner code stripped of Tauri/Android dependencies before compiling
3. ☐ Every API endpoint returns proper JSON with error handling
4. ☐ WebSocket events match Tauri event names/shapes for frontend compatibility
5. ☐ Docker builds and runs with `--network host --cap-add=NET_RAW`
6. ☐ SQLite schema migrations run automatically on startup
7. ☐ All log lines use `[FLIGHT_RECORDER]` convention
8. ☐ After changes: `cargo check`, `cargo build --release`, Docker build test
9. ☐ After completing any feature, bug fix, or significant change — update HANDOFF.md: update the "Current State" section to reflect what now works, move completed items to done, and update "In Progress" with what is still pending.
