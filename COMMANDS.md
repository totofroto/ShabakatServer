# SHABAKAT SERVER - RUNBOOK (`COMMANDS.md`)

[FLIGHT_RECORDER] Headless system deployment engine targeting the "Waddan" NAS hardware (Intel Celeron J4125 x86_64 architecture).

================================================================================
1. VERIFY REMOTE NETWORK VISIBILITY
================================================================================
Short Name: Test Connection
What to do: Copy and paste this line to check if your MacBook can see the NAS and verify Docker is running.

ssh "totofroto@192.168.254.18" "uname -a && docker --version && docker-compose --version"

================================================================================
2. PLATFORM CROSS-COMPILATION
================================================================================
Short Name: Cross-Compile
What to do: Run this line inside your Mac terminal to build the server engine specifically for the NAS architecture.

docker buildx build --platform linux/amd64 -t shabakat-server:latest --load .

================================================================================
3. IMAGE ARCHIVING
================================================================================
Short Name: Compress Image
What to do: Run this line right after compilation to compress the build into a transport file.

docker save shabakat-server:latest | gzip > shabakat-server.tar.gz

================================================================================
4. PRODUCTION DEPLOYMENT PIPELINE
================================================================================
Short Name: Deploy to NAS
What to do: Copy and paste this entire block together. It creates the folder, uploads the configuration, injects the container image, restarts the server on your NAS, and cleans up your Mac.

ssh "totofroto@192.168.254.18" "mkdir -p ~/Documents/ShabakatServer"
rsync -avz --exclude='.git' --exclude='target' ./docker-compose.yml "totofroto@192.168.254.18:~/Documents/ShabakatServer/"
cat shabakat-server.tar.gz | ssh "totofroto@192.168.254.18" "gunzip | docker load"
ssh "totofroto@192.168.254.18" "cd ~/Documents/ShabakatServer && docker-compose up -d --force-recreate"
rm shabakat-server.tar.gz

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
What to do: Run this line to verify that your background discovery tools (mDNS and SSDP) are actively listening.

ssh "totofroto@192.168.254.18" "docker exec -it shabakat-server netstat -tlpn"

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
What to do: Run this line to confirm your database parameters are running optimized in WAL mode.

ssh "totofroto@192.168.254.18" "docker exec -it shabakat-server sqlite3 /app/data/shabakat.db 'PRAGMA journal_mode; PRAGMA synchronous; PRAGMA busy_timeout;'"

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