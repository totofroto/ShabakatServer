#!/bin/bash

# Shabakat Debug Toolkit for Asustor NAS
# This script is designed to run directly on the NAS over SSH.

API_PORT=7779
DB_PATH="/volume1/Docker/shabakat-server/shabakat_server.db"
CONTAINER_NAME="shabakat-server"

usage() {
    echo "Shabakat Server Debug Toolkit"
    echo "Usage: $0 {logs|state|db-check}"
    echo ""
    echo "Commands:"
    echo "  logs      - Tails container logs filtering for ERROR, WARN, SCAN_TRACE, and PANIC"
    echo "  state     - Fetches real-time server state from the diagnostic API"
    echo "  db-check  - Checks SQLite database file health, size, and WAL status"
}

case "$1" in
    logs)
        echo "--- Tailing logs for $CONTAINER_NAME (Filtered) ---"
        sudo docker logs -f $CONTAINER_NAME | grep -E "ERROR|WARN|SCAN_TRACE|PANIC"
        ;;
    state)
        echo "--- Querying Diagnostic API (http://127.0.0.1:$API_PORT/api/debug/state) ---"
        # Try to use python3 for pretty printing, fallback to raw curl if not available
        if command -v python3 > /dev/null 2>&1; then
            curl -s http://127.0.0.1:$API_PORT/api/debug/state | python3 -m json.tool
        else
            curl -s http://127.0.0.1:$API_PORT/api/debug/state
            echo ""
        fi
        ;;
    db-check)
        echo "--- Database Health Check: $DB_PATH ---"
        if [ -f "$DB_PATH" ]; then
            ls -lh "$DB_PATH"
            echo "Last modified: $(stat -c %y "$DB_PATH")"
            
            # Check for WAL files which might indicate active transactions or locks
            if [ -f "${DB_PATH}-wal" ]; then
                echo "WAL file active: $(ls -lh "${DB_PATH}-wal")"
            else
                echo "WAL file not present (database idle or in DELETE mode)"
            fi

            if [ -f "${DB_PATH}-shm" ]; then
                echo "Shared memory file active: $(ls -lh "${DB_PATH}-shm")"
            fi
        else
            echo "ERROR: Database file not found at $DB_PATH"
            echo "Ensure the volume is correctly mounted and path is accurate."
        fi
        ;;
    *)
        usage
        exit 1
        ;;
esac
