# PROJECT HANDOFF & ARCHITECTURE SYNC
**Date:** May 20, 2026
**System Status:** Operational Command Center (V1 Complete)

## 1. Core Architecture (The Stack)
* **Backend:** Rust (Axum, Tokio) running as a Docker container.
* **Database:** SQLite via `libSQL` crate. **CRITICAL:** WAL (Write-Ahead Logging) mode is ENABLED to handle concurrent R/W operations during scans.
* **Frontend:** React + TypeScript (Vite).
* **Network:** NAS runs Out-of-Band on an unmanaged 2.5Gbps switch.

## 2. Completed Features (Current Capability)
* **Active Discovery:** Rust engine scans subnet, resolving 20+ devices via ARP/ICMP.
* **Data Persistence & Aliasing:** Device state is stored in SQLite. Users can rename devices; aliases persist in `device_aliases` and override raw hostnames.
* **Out-of-Band Telemetry (Bandwidth):** Implemented UPnP/TR-064 API polling to the TP-Link 5G CPE to fetch live RX/TX bytes without packet sniffing.
* **Out-of-Band Telemetry (Presence):** Implemented a passive mDNS/Bonjour listener to update device "last_seen" timestamps silently.
* **Command Center UI:** Upgraded the real-time topology map to an interactive physics graph (zoom, pan, drag) with a dynamic "Device Details" floating panel on click.

## 3. Infrastructure Constraints (DO NOT VIOLATE)
* **RULE 1: NO INLINE BRIDGING / SPOOFING.** The NAS sleeps at night. We cannot use ARP Spoofing or physical inline bridging, as it will break the home internet when the NAS is offline or degrade gaming latency.
* **RULE 2: DATABASE BATCHING.** The scanner must commit data to SQLite in bulk transactions (not individually) to prevent UI desyncs and database locks.
* **RULE 3: LAZY SCORING.** Dashboard health scores must be calculated asynchronously by a background task and saved to a database table. The API should only `SELECT` the pre-calculated score.

## 4. Next Phase Objectives (For Next Session)
* V1 is stable. Next session will focus on long-term data retention (Time-Series metrics for bandwidth history) and setting up Telegram/Email alerts for rogue devices.
