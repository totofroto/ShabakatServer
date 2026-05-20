# ── Stage 1: Build React frontend ─────────────────────────────────────────────
FROM node:20-slim AS web-builder
WORKDIR /web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ .
RUN npm run build

# ── Stage 2: Build Rust backend ───────────────────────────────────────────────
FROM rust:slim-bookworm AS rust-builder

# build-essential gives gcc (needed by rusqlite's bundled C compile)
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Pre-compile all external crates so rebuilds skip the slow part.
# The stub src/main.rs has no mod declarations — cargo compiles only
# the dependency graph and caches the artifacts in this layer.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs \
    && cargo build --release \
    && rm -f target/release/shabakat-server \
              target/release/deps/shabakat_server-* \
              target/release/deps/libshabakat_server*

# Now build the real application (deps already compiled above)
COPY src/       src/
COPY resources/ resources/
RUN touch src/main.rs && cargo build --release

# ── Stage 3: Runtime image ────────────────────────────────────────────────────
FROM debian:bookworm-slim

# ca-certificates: needed for Telegram / webhook HTTPS calls (reqwest + rustls)
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=rust-builder /build/target/release/shabakat-server /usr/local/bin/shabakat-server
COPY --from=web-builder  /web/dist                             /srv/web

ENV SHABAKAT_DATA_DIR=/data
ENV SHABAKAT_WEB_DIR=/srv/web
ENV SHABAKAT_PORT=8080
ENV RUST_LOG=info

VOLUME /data
EXPOSE 8080

CMD ["shabakat-server"]
