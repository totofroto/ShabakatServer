---
name: shabakat-server-pro
description: >
  Expert engineering assistant for the ShabakatServer project — the always-on Docker
  network scanner that runs on WADDAN (Asustor NAS, 192.168.254.18:7779). Use this skill
  for any task involving: Axum REST API, WebSocket events, SQLite schema, Rust scanner
  (server variant), Docker deployment, docker-compose, scan scheduler, Telegram alerts,
  webhook notifications, historical timeline API, or any file under ShabakatServer/src/.
  Load this skill whenever the user mentions WADDAN, docker buildx, shabakat-server,
  /api/devices, /api/scan, /ws, rusqlite, scan_history, or pastes Docker logs.
---

# ShabakatServer Engineering Skill

## What This Project Is

ShabakatServer is a **standalone Rust binary** that runs as a Docker container on WADDAN
(Asustor NAS, x86_64, IP: 192.168.254.18, port 7779). It is NOT the Tauri app.
It shares scanner DNA with the Tauri app but has NO Tauri, NO JNI, NO Android workarounds.

**Do not copy Android-specific code here. Do not add Tauri dependencies.**

---

## Project Path

```
~/Documents/ShabakatServer/
├── Cargo.toml
├── Dockerfile
├── docker-compose.yml
├── src/
│   ├── main.rs              ← Axum server + background scheduler startup
│   ├── api/                 ← REST routes + WebSocket handler
│   ├── scanner/             ← Rust scan engine (Linux variant, no Android code)
│   ├── storage/             ← SQLite via rusqlite
│   ├── scheduler/           ← watchdog + heartbeat background tasks
│   └── notify/              ← Telegram + webhook alerts
└── web/                     ← React frontend (Vite build, browser mode)
    └── src/lib/transport.ts ← HTTP/WS transport adapter (NOT Tauri invoke)
```

---

## Tech Stack

- **Web framework**: Axum (Rust async)
- **Database**: SQLite via `rusqlite` (not JSON files)
- **Ping**: TCP + real ICMP (Docker has `NET_RAW` cap — ICMP is allowed here)
- **ARP**: Read directly from `/proc/net/arp` (Linux native)
- **SSDP**: Standard multicast socket (no `SO_BINDTODEVICE` needed — wired ethernet)
- **mDNS**: Standard `ServiceDaemon::new()` (no `Box::leak`, no MulticastLock)
- **DNS**: No concurrency limit (glibc is thread-safe, unlike Android Bionic)
- **Frontend**: Same React codebase as Tauri app, built with Vite, served as static files

---

## Key Differences vs. Tauri App

| Concern | Tauri App | ShabakatServer |
|---|---|---|
| Ping | TCP-only (no CAP_NET_RAW) | TCP + real ICMP (NET_RAW cap) |
| ARP | rtnetlink / `arp -a` | `/proc/net/arp` direct read |
| SSDP | `SO_BINDTODEVICE` on wifi iface | Standard multicast (wired) |
| mDNS | `Box::leak` MulticastLock via JNI | `ServiceDaemon::new()` directly |
| DNS | Max 4 concurrent (Bionic limit) | No limit |
| Persistence | JSON files via plugin-store | SQLite database |
| Notifications | OS notification API | Telegram bot + webhook |
| Scan scheduling | Watchdog every 10 min (in-process) | Configurable cron via scheduler |
| History | `lastSeen` timestamp only | Full per-scan snapshots in SQLite |

---

## SQLite Schema (Core Tables)

```sql
-- One row per unique device (keyed by MAC)
CREATE TABLE devices (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    mac TEXT UNIQUE NOT NULL,
    first_seen INTEGER NOT NULL,   -- Unix ms
    last_seen INTEGER NOT NULL,    -- Unix ms
    last_ip TEXT,
    vendor TEXT,
    custom_name TEXT,
    likely_type TEXT,
    hostname TEXT,
    mdns_hostname TEXT,
    ssdp_server TEXT,
    interrogation_name TEXT,
    acknowledged INTEGER DEFAULT 0,
    notes TEXT
);

-- Append-only scan history (one row per device per scan)
CREATE TABLE scan_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scan_id TEXT NOT NULL,
    scanned_at INTEGER NOT NULL,   -- Unix ms
    device_id INTEGER NOT NULL REFERENCES devices(id),
    ip TEXT NOT NULL,
    is_online INTEGER NOT NULL,    -- 1 or 0
    latency_ms REAL,
    open_ports TEXT                -- JSON array
);

-- Event log (new device, offline, etc.)
CREATE TABLE device_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL,      -- "new_device" | "device_offline" | "device_online"
    device_id INTEGER REFERENCES devices(id),
    timestamp INTEGER NOT NULL,
    details TEXT                   -- JSON payload
);
```

---

## API Design

### REST Endpoints

```
GET    /api/devices              → all devices with latest status
GET    /api/devices/:mac         → single device with history
PATCH  /api/devices/:mac         → update custom_name, notes, acknowledged
DELETE /api/devices/:mac         → remove from registry

POST   /api/scan                 → trigger manual scan { mode: "silent" | "aggressive" }
GET    /api/scan/status          → { isScanning, scanId, progress }

GET    /api/history?from=&to=&mac=  → scan history timeline
GET    /api/events?limit=50         → recent device events

POST   /api/tools/ping     { ip }
POST   /api/tools/dns      { hostname }
POST   /api/tools/wake     { mac }
POST   /api/tools/portscan { ip }
```

### WebSocket `/ws` — Same Event Model as Tauri

```json
{ "event": "scan_started",      "data": { "scanId": "scheduled-42" } }
{ "event": "device_discovered", "data": { "scanId": "scheduled-42", "batchSeq": 1, "devices": [...] } }
{ "event": "scan_finished",     "data": { "scanId": "scheduled-42", "devices": [...] } }
{ "event": "latency_update",    "data": { "ip": "192.168.254.34", "latencyMs": 1.2 } }
{ "event": "new_device",        "data": { "mac": "AA:BB:CC:DD:EE:FF", "ip": "..." } }
{ "event": "device_offline",    "data": { "mac": "AA:BB:CC:DD:EE:FF" } }
```

---

## Transport Adapter (Frontend)

`web/src/lib/transport.ts` detects whether it's running in Tauri or browser and routes
calls accordingly. In browser mode: REST for commands, WebSocket for events.
**Never use `invoke()` from `@tauri-apps/api` in the server frontend.**

---

## Logging — Every New Function Must Include This

```rust
info!("[FLIGHT_RECORDER] [module_name] action | key=value");
info!("[SCAN_LIFECYCLE] description | scan_id={}", scan_id);
warn!("[NOTIFY] telegram alert sent | event={}", event_type);
```

---

## Deploy to WADDAN

**Always use `--platform linux/amd64`** — M1/M4 Mac builds ARM by default, NAS is x86_64.

```bash
cd ~/Documents/ShabakatServer
docker buildx build --platform linux/amd64 -t shabakat-server:latest .
docker save shabakat-server:latest | gzip > /tmp/shabakat-server.tar.gz
scp -o IdentitiesOnly=yes -o PreferredAuthentications=password \
  /tmp/shabakat-server.tar.gz totofroto@192.168.254.18:/tmp/
ssh -t -o IdentitiesOnly=yes -o PreferredAuthentications=password \
  totofroto@192.168.254.18 \
  'sudo docker load -i /tmp/shabakat-server.tar.gz && \
   sudo docker stop shabakat-server && sudo docker rm shabakat-server && \
   cd /volume1/Docker/shabakat-server && sudo docker compose up -d && \
   sudo docker logs --tail 10 shabakat-server'
```

Always verify the last 10 log lines after deploy.

### Watch live logs from WADDAN

```bash
ssh -t -o IdentitiesOnly=yes -o PreferredAuthentications=password \
  totofroto@192.168.254.18 'sudo docker logs -f shabakat-server 2>&1'
```

---

## docker-compose.yml Requirements

```yaml
network_mode: host       # CRITICAL — raw LAN access for SSDP/mDNS/ARP
cap_add:
  - NET_RAW              # real ICMP ping
  - NET_ADMIN            # ARP/network interface access
```

Without `network_mode: host`, SSDP and mDNS multicast will not reach the container.

---

## Notification System

Telegram alerts via `totofroto_bot` (already configured):

```
SHABAKAT_TELEGRAM_BOT_TOKEN=<token>
SHABAKAT_TELEGRAM_CHAT_ID=<chat_id>
```

Alert format:
```
🔴 New device on your network!
MAC: AA:BB:CC:DD:EE:FF
IP: 192.168.254.99
Vendor: Unknown
Shabakat · http://192.168.254.18:7779
```

---

## What NOT to Touch

| File / Pattern | Reason |
|---|---|
| `web/src/lib/transport.ts` | Transport adapter — edit carefully, both projects depend on the pattern |
| SQLite schema migrations | Always add new columns via `ALTER TABLE`, never recreate tables |
| `network_mode: host` in docker-compose | Required for multicast — do not change to bridge |
| Scanner fingerprint rule order | OR-rules before AND-rules; order matters |
| `[FLIGHT_RECORDER]` log lines | Debug observability |
