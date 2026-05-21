#!/bin/sh
# Asustor pre-remove script for Shabakat
# Cleans up processes and system daemon pointers

INIT_SCRIPT="/usr/local/etc/init.d/S99shabakat"

echo "[Pre-remove] Cleaning up active process threads and system daemon pointers..."

# Stop the service and remove the init script
if [ -f "$INIT_SCRIPT" ]; then
    "$INIT_SCRIPT" stop
    rm "$INIT_SCRIPT"
fi

# Final cleanup of any orphaned processes
killall shabakat-server > /dev/null 2>&1

echo "[Pre-remove] Cleanup complete."
exit 0
