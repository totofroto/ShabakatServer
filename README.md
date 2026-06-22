# Shabakat Server

Shabakat is a highly performant network scanning and telemetry server built with Rust and Axum, paired with a React frontend.

## Architecture and Configuration

### Network Architecture
Production deployment **requires** Host Network Mode (`network_mode: "host"`). This is critical to allow the Rust engine to escape the Docker virtualization bubble and accurately sweep the true physical LAN subnet (e.g., `192.168.254.x`). Without host networking, passive mDNS detection and ARP sweeps will be trapped within Docker's isolated bridge network.

### Authentication
Authentication middleware and session tokens have been completely **stripped** from this application. It is designed for a local-first, zero-friction deployment on a secure physical LAN. Do not expose this service directly to the public internet without an external reverse proxy providing its own authentication layer.

### UI Improvements
The frontend dashboard features several stability and performance enhancements:
- **Null-Safe Sorting:** Device lists and tables implement robust null-safe sorting using `.localeCompare` with null-coalescing fallbacks. This prevents rendering crashes during POST-scan reconciliation.
- **WebSocket Streaming:** Real-time state streaming includes deduplication mechanisms to ensure efficient UI updates without overwhelming the React rendering cycle.

## Recent Feature Updates

### Phase A: Speed Test Subsystem
- **Execution Loop:** Functional `POST /api/speed-test/run` execution loop utilizing Cloudflare 5MB payload sampling.
- **History:** Upgraded `GET /api/speed-test/history` endpoint returning the latest 10 database records.

### Phase B: Alerts Acknowledgment System
- **Optimized Backend Architecture:** Transitioned from client-side array filtering to a dedicated `GET /api/alerts` endpoint running an efficient SQLite `JOIN` targeting `d.acknowledged = 0`.

## Deployment
Refer to `docker-compose.yml` for the standard deployment configuration. Remember to ensure `network_mode: "host"` is active when deploying to your NAS or physical host.
