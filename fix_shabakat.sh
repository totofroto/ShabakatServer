#!/bin/sh
# fix_shabakat.sh — Shabakat Server diagnostic script
# Rewritten 2026-06-15 to match actual runtime: port 7779, [FLIGHT_RECORDER] logging,
# admin_token cookie auth (no JWT_SECRET / AUTH_DEBUG scheme).
# Run from your Mac (or any host with SSH access to the NAS).

NAS="totofroto@192.168.254.18"
PORT=7779

echo "=== Shabakat Server Diagnostic ==="

# 1. Check if the backend is actually reachable on the correct port
echo ""
echo "[1] Checking backend health at port $PORT..."
curl -sf "http://192.168.254.18:$PORT/api/health" && echo "" || echo "WARN: /api/health did not respond — server may be down or on wrong port"

# 2. Check Docker container status on NAS
echo ""
echo "[2] Checking Docker container status on NAS..."
ssh "$NAS" "docker ps --filter name=shabakat-server --format 'table {{.Names}}\t{{.Status}}\t{{.Ports}}'" 2>/dev/null \
    || echo "WARN: Could not SSH to NAS at $NAS"

# 3. Tail [FLIGHT_RECORDER] log entries (not AUTH_DEBUG — that scheme was removed)
echo ""
echo "[3] Last 20 [FLIGHT_RECORDER] log entries..."
ssh "$NAS" "docker logs shabakat-server 2>&1 | grep '\[FLIGHT_RECORDER\]' | tail -n 20" 2>/dev/null \
    || echo "WARN: Could not retrieve container logs"

# 4. Verify .env exists (do NOT print its contents)
echo ""
echo "[4] Checking .env presence on NAS..."
ssh "$NAS" "test -f ~/Documents/ShabakatServer/.env && echo '.env present' || echo 'WARN: .env MISSING on NAS'" 2>/dev/null \
    || echo "WARN: Could not check .env on NAS"

echo ""
echo "=== Diagnostic complete ==="
