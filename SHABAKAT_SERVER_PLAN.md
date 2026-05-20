# Shabakat Server — Architecture Plan

> **The NAS-native, always-on network intelligence platform.**
> Runs as a Docker container on WADDAN (or any NAS/Linux box).
> Zero changes to the existing Tauri app.

---

## The Core Idea

Don't refactor. Don't extract crates. Don't touch the working Tauri app at all.

Instead: **copy the Rust scanner code into a new standalone project**, strip out every line that says `tauri::`, `AppHandle`, `JNI`, or `Android`, and replace the IPC layer with a proper web API. The Tauri app continues to ship independently. The server is a separate product that shares DNA but not a build pipeline.

Why this is better than a shared crate:

- **Zero risk to the Tauri app.** WADDAN doesn't need `Box::leak`, `SO_BINDTODEVICE`, or JNI. Those stay in the Tauri codebase untouched.
- **The server needs DIFFERENT code.** On a NAS with wired ethernet, you don't need Android workarounds. You CAN use real ICMP ping. You CAN read ARP directly from `/proc/net/arp`. You CAN run mDNS without a MulticastLock. The server is simpler, not a subset.
- **Independent release cycle.** You update the phone app without worrying about the server, and vice versa.
- **When scanner logic improves in either project**, you cherry-pick the specific change to the other. This happens rarely (the fingerprint rules are stable) and is easy when it does.

---

## Project Structure

```
ShabakatServer/
├── Cargo.toml                      ← single binary crate
├── Dockerfile                      ← multi-arch (x86_64 + aarch64)
├── docker-compose.yml              ← one-command deploy
├── .env.example                    ← config (scan interval, subnet override, notifications)
│
├── src/
│   ├── main.rs                     ← Axum server + background scheduler
│   │
│   ├── scanner/
│   │   ├── mod.rs                  ← scan engine (copied from Tauri, Android code removed)
│   │   ├── ping.rs                 ← TCP ping + real ICMP ping (NAS has CAP_NET_RAW)
│   │   ├── ports.rs                ← Port Guardian (identical)
│   │   ├── fingerprints.rs         ← Fingerprint engine (identical copy)
│   │   ├── arp.rs                  ← /proc/net/arp reader (Linux native, no arp -a subprocess)
│   │   ├── ssdp.rs                 ← SSDP/UPnP (no SO_BINDTODEVICE needed on wired ethernet)
│   │   ├── mdns.rs                 ← mDNS browser (no MulticastLock needed on Linux)
│   │   └── network.rs              ← Subnet detection (simplified — no Android fallbacks)
│   │
│   ├── api/
│   │   ├── mod.rs                  ← Axum router
│   │   ├── scan.rs                 ← POST /api/scan, GET /api/scan/status
│   │   ├── devices.rs              ← GET /api/devices, GET /api/devices/:id, PATCH (custom name)
│   │   ├── tools.rs                ← POST /api/tools/{ping,dns,wake,portscan,...}
│   │   ├── history.rs              ← GET /api/history (timeline data)
│   │   └── ws.rs                   ← WebSocket /ws — live scan batches, latency stream, alerts
│   │
│   ├── scheduler/
│   │   ├── mod.rs                  ← Cron-style background scan scheduler
│   │   ├── watchdog.rs             ← Periodic full scan (configurable: 5/10/30 min)
│   │   └── heartbeat.rs            ← Live device monitoring (ping alive devices every N min)
│   │
│   ├── storage/
│   │   ├── mod.rs                  ← SQLite via rusqlite (not JSON files)
│   │   ├── schema.sql              ← devices, scan_history, device_events, settings
│   │   ├── devices.rs              ← CRUD for device records
│   │   ├── history.rs              ← Append-only scan history (which devices were online when)
│   │   └── migrations.rs           ← Auto-migrate on startup
│   │
│   ├── notify/
│   │   ├── mod.rs                  ← Notification dispatcher
│   │   ├── telegram.rs             ← Telegram bot alert (you already have totofroto_bot!)
│   │   ├── webhook.rs              ← Generic webhook (Ntfy, Gotify, Slack, Discord)
│   │   └── email.rs                ← Optional SMTP alert
│   │
│   └── config.rs                   ← Env-based config (SHABAKAT_SUBNET, SHABAKAT_SCAN_INTERVAL, etc.)
│
├── web/                            ← React frontend (Vite build output — static files)
│   └── dist/                       ← copied from Shabakat main project's `npm run build`
│
├── CLAUDE.md                       ← Claude Code instructions for this project
└── README.md                       ← Setup guide
```

---

## What Changes vs. Tauri App

| Component | Tauri App | Server |
|---|---|---|
| IPC: UI → Backend | `invoke("scan_network")` | `POST /api/scan` |
| IPC: Backend → UI | `AppHandle::emit("device_discovered")` | WebSocket message |
| Ping | TCP-only (no CAP_NET_RAW) | TCP + real ICMP (Docker `--cap-add=NET_RAW`) |
| ARP table | Android: rtnetlink. macOS: `arp -a` | Linux: `/proc/net/arp` direct read |
| SSDP socket | `SO_BINDTODEVICE` on wifi interface | Standard multicast (wired ethernet, no routing issues) |
| mDNS | `Box::leak` MulticastLock via JNI | Standard `ServiceDaemon::new()` (no Android Scudo issues) |
| DNS resolver | `os_dns_sem()` max 4 (Bionic mutex) | No limit needed (glibc is thread-safe) |
| Persistence | JSON files via plugin-store | SQLite database |
| Notifications | OS notification API | Telegram bot / webhook / email |
| Permissions | Android location permission dance | None (Docker `--network host` handles everything) |
| Scan scheduling | Watchdog every 10 min (in-process) | Configurable cron (5/10/30/60 min) |
| History | `lastSeen` timestamp on device | Full timeline: `scan_history` table with per-scan snapshots |

---

## SQLite Schema

This is the big upgrade over JSON files. Proper relational history.

```sql
-- Core device registry (one row per unique device)
CREATE TABLE devices (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    mac         TEXT UNIQUE NOT NULL,               -- XX:XX:XX:XX:XX:XX
    first_seen  INTEGER NOT NULL,                   -- Unix ms
    last_seen   INTEGER NOT NULL,                   -- Unix ms
    last_ip     TEXT,                                -- most recent IP
    vendor      TEXT,                                -- OUI lookup
    custom_name TEXT,                                -- user-assigned
    likely_type TEXT,                                -- fingerprint label
    hostname    TEXT,                                -- rDNS
    mdns_hostname TEXT,                              -- Zeroconf
    ssdp_server TEXT,                                -- UPnP banner
    interrogation_name TEXT,                         -- HTTP/UPnP display name
    acknowledged INTEGER DEFAULT 0,                  -- user marked as known
    notes       TEXT                                 -- user notes
);

-- Append-only scan history (one row per device per scan)
CREATE TABLE scan_history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    scan_id     TEXT NOT NULL,                       -- "scheduled-42" or "manual-5"
    scanned_at  INTEGER NOT NULL,                    -- Unix ms
    device_id   INTEGER NOT NULL REFERENCES devices(id),
    ip          TEXT NOT NULL,
    is_online   INTEGER NOT NULL,                    -- 1 or 0
    latency_ms  REAL,                                -- TCP ping result
    open_ports  TEXT                                  -- JSON array of open port numbers
);
CREATE INDEX idx_scan_history_device ON scan_history(device_id, scanned_at);
CREATE INDEX idx_scan_history_time ON scan_history(scanned_at);

-- Events log (intruder alerts, device offline, etc.)
CREATE TABLE device_events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type  TEXT NOT NULL,                       -- "new_device" | "device_offline" | "device_online" | "port_change"
    device_id   INTEGER REFERENCES devices(id),
    timestamp   INTEGER NOT NULL,
    details     TEXT                                  -- JSON payload
);

-- App settings
CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

This gives you the **historical timeline view** that's been in the roadmap backlog — `SELECT * FROM scan_history WHERE device_id = ? ORDER BY scanned_at` shows exactly when each device was online or offline, with latency data, going back weeks/months.

---

## API Design

### REST Endpoints

```
GET    /api/devices                    → all known devices with latest status
GET    /api/devices/:mac               → single device with full history
PATCH  /api/devices/:mac               → update custom_name, notes, acknowledged
DELETE /api/devices/:mac               → remove from registry

POST   /api/scan                       → trigger manual scan { mode: "silent" | "aggressive" }
GET    /api/scan/status                → { isScanning, scanId, progress }

GET    /api/history?from=&to=&mac=     → scan history entries (timeline data)
GET    /api/events?limit=50            → recent device events

POST   /api/tools/ping     { ip }
POST   /api/tools/dns      { hostname }
POST   /api/tools/wake     { mac }
POST   /api/tools/portscan { ip, ports? }
POST   /api/tools/whois    { domain }

GET    /api/config                     → current settings
PATCH  /api/config                     → update settings
```

### WebSocket `/ws`

Same event model as Tauri, but over WebSocket:

```json
{ "event": "scan_started",      "data": { "scanId": "scheduled-42" } }
{ "event": "device_discovered",  "data": { "scanId": "scheduled-42", "batchSeq": 1, "devices": [...] } }
{ "event": "scan_finished",      "data": { "scanId": "scheduled-42", "devices": [...] } }
{ "event": "latency_update",     "data": { "ip": "192.168.254.34", "latencyMs": 1.2 } }
{ "event": "new_device",         "data": { "mac": "AA:BB:CC:DD:EE:FF", "ip": "192.168.254.99" } }
{ "event": "device_offline",     "data": { "mac": "AA:BB:CC:DD:EE:FF" } }
```

---

## Frontend Adapter

The React frontend is the same codebase. You build it once with Vite, copy `dist/` into the Docker image. The only change is a transport adapter in `useNetworkScan.ts`:

```typescript
// transport.ts — one file that abstracts Tauri invoke vs HTTP fetch

const isTauri = typeof window !== 'undefined' && '__TAURI__' in window;

export async function rpcCall<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(command, args);
  }
  const res = await fetch(`/api/${command}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(args ?? {}),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export function subscribeEvents(handler: (event: string, data: any) => void): () => void {
  if (isTauri) {
    // Use Tauri listen()
    const { listen } = await import('@tauri-apps/api/event');
    // ... existing Tauri event listeners
  } else {
    // WebSocket
    const ws = new WebSocket(`ws://${location.host}/ws`);
    ws.onmessage = (e) => {
      const msg = JSON.parse(e.data);
      handler(msg.event, msg.data);
    };
    return () => ws.close();
  }
}
```

This means the same React code works in both the Tauri app AND the browser. You add this one file to the main Shabakat repo, and `npm run build` produces a `dist/` that works everywhere.

---

## Docker Deployment

### Dockerfile (multi-arch)

```dockerfile
# ── Stage 1: Build Rust backend ──
FROM rust:1.80-slim-bookworm AS rust-builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
RUN cargo build --release

# ── Stage 2: Build React frontend ──
FROM node:20-slim AS web-builder
WORKDIR /web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ .
RUN npm run build

# ── Stage 3: Final image ──
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=rust-builder /build/target/release/shabakat-server /usr/local/bin/
COPY --from=web-builder /web/dist /srv/web
ENV SHABAKAT_WEB_DIR=/srv/web
ENV SHABAKAT_DATA_DIR=/data
EXPOSE 8080
VOLUME /data
CMD ["shabakat-server"]
```

### docker-compose.yml (for WADDAN)

```yaml
version: "3.8"
services:
  shabakat:
    image: ghcr.io/totofroto/shabakat-server:latest
    container_name: shabakat-server
    restart: unless-stopped
    network_mode: host          # CRITICAL: raw LAN access for SSDP/mDNS/ARP
    cap_add:
      - NET_RAW                 # real ICMP ping
    environment:
      - SHABAKAT_SCAN_INTERVAL=600        # seconds between auto-scans (10 min)
      - SHABAKAT_HEARTBEAT_INTERVAL=120   # seconds between device heartbeats (2 min)
      - SHABAKAT_SUBNET=auto              # auto-detect or override: 192.168.254.0/24
      - SHABAKAT_PORT=8080                # web UI port
      - SHABAKAT_TELEGRAM_BOT_TOKEN=      # your totofroto_bot token
      - SHABAKAT_TELEGRAM_CHAT_ID=        # your Telegram chat ID
    volumes:
      - shabakat-data:/data     # SQLite DB + mac-vendors.json persist here

volumes:
  shabakat-data:
```

### Deploy on WADDAN

```bash
# SSH into NAS
ssh totofroto@192.168.254.34

# Create project directory
sudo mkdir -p /volume1/Docker/shabakat-server

# Copy docker-compose.yml
# Then:
sudo docker-compose up -d

# Access from any device on the network:
# http://192.168.254.34:8080
```

---

## Notification System

The killer feature of always-on monitoring. When a new device appears at 2am, you get a Telegram message immediately.

### Telegram (via totofroto_bot — already exists!)

```rust
async fn send_telegram_alert(config: &Config, msg: &str) {
    if let (Some(token), Some(chat_id)) = (&config.telegram_bot_token, &config.telegram_chat_id) {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
        let _ = reqwest::Client::new()
            .post(&url)
            .json(&serde_json::json!({
                "chat_id": chat_id,
                "text": msg,
                "parse_mode": "HTML"
            }))
            .send()
            .await;
    }
}
```

Alert message format:
```
🔴 New device on your network!

MAC: AA:BB:CC:DD:EE:FF
IP: 192.168.254.99
Vendor: Unknown
First seen: 2026-05-06 02:17:33

Shabakat Server · http://192.168.254.34:8080
```

### Webhook (Ntfy / Gotify / Discord / Slack)

```env
SHABAKAT_WEBHOOK_URL=https://ntfy.sh/shabakat-alerts
```

One POST to the URL with the alert payload. Works with any notification service.

---

## Server Advantages Over Phone App

| Capability | Phone App | Server on NAS |
|---|---|---|
| Scan when you're asleep | No | Yes — every 10 min, 24/7 |
| Historical timeline | lastSeen only | Full per-scan snapshots in SQLite |
| Alert on new device | Next time you open app | Telegram/webhook within seconds |
| Scan speed | Limited by WiFi, battery | Wired ethernet, no battery concerns |
| ICMP ping | Blocked (no CAP_NET_RAW) | Real ICMP (Docker NET_RAW) |
| Multicast reliability | Android driver drops packets | Wired ethernet, 100% reliable |
| DNS resolution | Bionic mutex limits to 4 concurrent | glibc, no limit |
| Concurrent probes | Capped at 64 (Android) | 256+ (NAS has resources) |
| Access | One device | Any browser on the network |
| Storage | Device local storage | SQLite on NAS (persistent, queryable) |
| Uptime | When app is open | 24/7/365 |

---

## Implementation Order

### Phase 1: Minimal Working Server (1 week with Claude Code)
1. Copy scanner/, fingerprints.rs, tools.rs, network.rs, arp.rs from Tauri project
2. Strip all `#[cfg(target_os = "android")]` blocks and Tauri dependencies
3. Build Axum server with `/api/scan`, `/api/devices`, `/ws`
4. SQLite schema + basic CRUD
5. Background scan scheduler (fixed interval)
6. Serve static React frontend from `dist/`
7. Dockerfile + docker-compose.yml
8. Deploy on WADDAN, verify scan works

### Phase 2: Notifications + History (1 week)
1. Telegram bot integration
2. Webhook support
3. Historical timeline API (`/api/history`)
4. Timeline UI component (new page or tab)
5. Device event log

### Phase 3: Frontend Adapter (1 week)
1. Add `transport.ts` adapter to main Shabakat React codebase
2. Build shared `dist/` that works in both Tauri and browser
3. Single `npm run build` produces universal frontend

### Phase 4: Polish + Release
1. Multi-arch Docker image (x86_64 + aarch64)
2. Asustor ADM App Center package (optional)
3. README with setup guide
4. Push to GitHub Container Registry

---

## Claude Code Project Setup

Create the new project:

```bash
mkdir ~/Documents/ShabakatServer
cd ~/Documents/ShabakatServer
```

Copy the CLAUDE.md for the server project (different from the Tauri app's CLAUDE.md):

The first prompt to Claude Code:

```
Read CLAUDE.md. This is a new project: Shabakat Server — a headless network scanner 
that runs as a Docker container on a NAS. Start Phase 1, Step 1: initialize the Cargo 
project, copy scanner code from ~/Documents/Shabakat/src-tauri/src/ (scanner/, 
fingerprints.rs, network.rs, arp.rs, tools.rs), strip all Tauri/Android dependencies, 
and get cargo check passing. Do not modify anything in ~/Documents/Shabakat/.
```

---

## Addendum: Telemetry Architecture Shift (May 2026)
During initial deployment, a critical architecture decision was made regarding network telemetry (bandwidth monitoring and presence detection).

**The Constraint:** The NAS (WADDAN) sleeps at night. Therefore, any "Man-in-the-Middle" (ARP Spoofing) or "Inline Gateway" (Physical Bridging) techniques cannot be used, as they would sever internet access for the entire house when the NAS goes offline. Furthermore, ARP spoofing introduces unacceptable latency (jitter) for competitive gaming.

**The Solution (Out-of-Band Telemetry):**
To achieve Zero-Trust visibility without impacting network performance, the Server employs strictly passive and API-driven techniques:
1.  **Bandwidth:** The Rust backend polls the TP-Link 5G CPE via the TR-064/UPnP `WANCommonInterfaceConfig` API to read raw RX/TX byte counters.
2.  **Presence:** A continuous background mDNS/Bonjour listener monitors the network for ambient broadcasts (e.g., Apple devices, Chromecasts) to silently update `last_seen` timestamps without active pinging.
3.  **UI:** The topology map was upgraded to a D3/Force-directed physics graph to handle the dense device data visually.
