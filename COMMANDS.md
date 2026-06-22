# SHABAKAT SERVER - RUNBOOK (`COMMANDS.md`)

[FLIGHT_RECORDER] Headless system deployment engine targeting the "Waddan" NAS hardware (Intel Celeron J4125 x86_64 architecture).

> **IMPORTANT: Three deploy strategies existed. §4 (image-load) and deploy_to_nas.sh (binary-rsync) are STALE.**
> **The canonical deploy path is `deploy.sh` (source-build-on-NAS) — see §4 below for corrections.**

================================================================================
1. VERIFY REMOTE NETWORK VISIBILITY
================================================================================
Short Name: Test Connection
What to do: Copy and paste this line to check if your MacBook can see the NAS and verify Docker is running.

ssh "totofroto@192.168.254.18" "uname -a && docker --version && docker-compose --version"

================================================================================
2. PLATFORM CROSS-COMPILATION (local, for Docker build context)
================================================================================
Short Name: Cross-Compile
What to do: Build a Linux/amd64 Docker image locally. Requires Docker buildx. This produces a tarball for manual transfer (§3–4 image-load path — see STALE notice).

docker buildx build --platform linux/amd64 -t shabakat-server:latest --load .

================================================================================
3. IMAGE ARCHIVING (STALE — image-load path not recommended)
================================================================================
Short Name: Compress Image
What to do: Produces a transport archive. Only needed if using the image-load deploy method (§4 image-load path), which is stale. Prefer deploy.sh (source-build-on-NAS).

docker save shabakat-server:latest | gzip > shabakat-server.tar.gz

================================================================================
4. PRODUCTION DEPLOYMENT
================================================================================
Short Name: Deploy to NAS

**CANONICAL PATH — source-build-on-NAS (use deploy.sh):**

  ./deploy.sh

  This rsyncs source (excluding target/, .env, .git) to the NAS and runs
  `docker compose build && docker compose up -d` remotely.

  KNOWN CONSTRAINTS:
  - NAS sudo PATH lacks `bash` — deploy.sh uses `sh -c` syntax internally.
  - Git is not on the NAS SSH PATH — no git operations are run remotely.
  - Sudo sessions time out on long builds (Rust full build ~15 min) — the
    SSH `-t` flag in deploy.sh keeps the session alive.

---

~~STALE — image-load path (requires local buildx + gzip transfer):~~

  ~~ssh "totofroto@192.168.254.18" "mkdir -p ~/Documents/ShabakatServer"~~
  ~~rsync -avz --exclude='.git' --exclude='target' ./docker-compose.yml "totofroto@192.168.254.18:~/Documents/ShabakatServer/"~~
  ~~cat shabakat-server.tar.gz | ssh "totofroto@192.168.254.18" "gunzip | docker load"~~
  ~~ssh "totofroto@192.168.254.18" "cd ~/Documents/ShabakatServer && docker-compose up -d --force-recreate"~~
  ~~rm shabakat-server.tar.gz~~

See also: `deploy_to_nas.sh` (binary-rsync, stale — marked deprecated at top of that file).

================================================================================
5. LIVE TELEMETRY STREAM
================================================================================
Short Name: View Logs
What to do: Run this line to stream real-time logs from the server container directly to your screen.

ssh "totofroto@192.168.254.18" "docker logs -f shabakat-server" | grep "\[FLIGHT_RECORDER\]"

================================================================================
6. BACKGROUND SOCKET AUDIT
================================================================================
Short Name: Network Audit
What to do: Verify that background discovery tools (mDNS and SSDP) are actively listening.

NOTE: The final Docker image is `debian:bookworm-slim` and does NOT include `netstat`.
Use `ss` (socket statistics) instead, via the host — NOT inside the container:

  ssh "totofroto@192.168.254.18" "ss -ulnp | grep -E '5353|1900'"

Or check from the host directly (container uses host networking):

  ssh "totofroto@192.168.254.18" "docker exec shabakat-server cat /proc/net/udp | awk 'NR>1{print \$2}' | grep -E '14E5|076C'"
  # 0x14E5 = 5333 decimal (mDNS), 0x076C = 1900 decimal (SSDP) in little-endian hex

================================================================================
7. HARDWARE RESOURCE MONITORING
================================================================================
Short Name: Resource Stats
What to do: Run this line to check live processor and memory usage on your 4-core Celeron NAS.

ssh "totofroto@192.168.254.18" "docker stats shabakat-server"

================================================================================
8. ASYNC SQLITE ENGINE VALIDATION
================================================================================
Short Name: Database Audit
What to do: Confirm the database is running in WAL mode.

NOTE: The final Docker image does NOT include `sqlite3`. Two options:

OPTION A — Query via the `/api/health` endpoint (always available):
  curl http://192.168.254.18:7779/api/health
  # Returns {"status":"ok","service":"shabakat-server","devices":<N>}
  # A device count ≥ 0 proves the DB connection is live and schema is applied.

OPTION B — Use sqlite3 from the NAS host (if installed on ADM):
  ssh "totofroto@192.168.254.18" "sqlite3 /volume1/Docker/ShabakatServer/data/shabakat.db 'PRAGMA journal_mode; PRAGMA synchronous; PRAGMA busy_timeout;'"
  # Expected output: wal / 1 / 5000
  # Volume is mounted at /data inside the container, but maps to the above host path.

  WRONG (old): docker exec -it shabakat-server sqlite3 /app/data/shabakat.db ...
  CORRECT:     sqlite3 run from NAS host against the volume-mapped path above.

================================================================================
9. SECURE GIT SOURCE PROTECTION
================================================================================
Short Name: Git Safeguard
What to do: Run this block to check git tracking status and write protection filters so your secure files never leak to public GitHub repos.

git status
cat << 'EOF' >> .gitignore
*.db
*.db-wal
*.db-shm
.env
shabakat-server.tar.gz
target/
COMMANDS.md
EOF