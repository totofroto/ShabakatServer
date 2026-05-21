#!/bin/sh
# Asustor post-install script for Shabakat
# Registers the server as a native system daemon

APKG_PATH="/usr/local/AppCentral/shabakat"
BIN_PATH="$APKG_PATH/bin/shabakat-server"
INIT_SCRIPT="/usr/local/etc/init.d/S99shabakat"

echo "[Post-install] Registering Shabakat as a native system daemon service..."

# Ensure the binary is executable
if [ -f "$BIN_PATH" ]; then
    chmod +x "$BIN_PATH"
fi

# Create the system init wrapper script
cat <<EOF > "$INIT_SCRIPT"
#!/bin/sh

case "\$1" in
    start)
        echo "Starting Shabakat Server..."
        # Running with nohup to ensure it persists after the script exits
        nohup $BIN_PATH > /var/log/shabakat.log 2>&1 &
        ;;
    stop)
        echo "Stopping Shabakat Server..."
        killall shabakat-server
        ;;
    restart)
        \$0 stop
        \$0 start
        ;;
    *)
        echo "Usage: \$0 {start|stop|restart}"
        exit 1
        ;;
esac
EOF

chmod +x "$INIT_SCRIPT"

# Start the service immediately after installation
"$INIT_SCRIPT" start

echo "[Post-install] Service registered and started."
exit 0
