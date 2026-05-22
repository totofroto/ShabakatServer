#!/bin/sh
case $1 in
    start)
        # Ensure any stale instance is cleared, then spin up the host-attached layout live
        docker rm -f shabakat-server 2>/dev/null
        docker run -d --name shabakat-server --network host --restart unless-stopped shabakat-server:latest
        ;;
    stop)
        docker stop shabakat-server
        ;;
    restart)
        $0 stop
        $0 start
        ;;
esac
exit 0
