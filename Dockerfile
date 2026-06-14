FROM debian:bookworm-slim

# Install system utilities needed by the network scanner
RUN apt-get update && \
    apt-get install -y libssl3 ca-certificates iputils-ping net-tools && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the cross-compiled release binary
COPY target/release/shabakat-server ./shabakat-server
COPY resources/ ./resources/

# Copy the pre-built React SPA static assets
COPY web/dist/ ./ui_dist/

ENV SHABAKAT_WEB_DIR=/app/ui_dist
ENV SHABAKAT_DATA_DIR=/data

EXPOSE 7779

ENTRYPOINT ["./shabakat-server"]
