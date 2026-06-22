# =============================================================================
# Stage 1 — Builder
# Compiles the Rust binary from source inside the container.
# This guarantees architecture compatibility and eliminates stale ghost binaries.
# =============================================================================
FROM rust:slim-bookworm AS builder

# Install build essentials. rusqlite uses the "bundled" feature (compiles SQLite
# from source), so no system libsqlite3 is required. All TLS crates use
# rustls (pure Rust), so no libssl-dev is needed either.
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        pkg-config \
        build-essential \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy dependency manifests first for Docker layer caching.
# A source-only change won't re-download the entire dependency graph.
COPY Cargo.toml Cargo.lock ./

# Pre-fetch & compile dependencies in a dummy build to warm the layer cache.
RUN mkdir -p src && \
    echo 'fn main() {}' > src/main.rs && \
    CARGO_BUILD_JOBS=1 cargo build --release && \
    rm -rf src

# Copy compile-time assets BEFORE building the real source.
# src/scanner/mod.rs uses include_str!("../../resources/mac-vendors.json"),
# which is resolved at compile time by rustc — so resources/ MUST exist in
# the builder stage. Placing this COPY after the dummy dep-cache build means
# the expensive dependency layer above stays cached on source-only changes.
COPY resources/ ./resources/

# Now copy the real source tree and build the final binary.
COPY src/ ./src/
# Touch main.rs so Cargo knows to recompile it (the dummy above is cached).
RUN touch src/main.rs && \
    CARGO_BUILD_JOBS=1 cargo build --release

# =============================================================================
# Stage 2 — Runtime
# A minimal Debian image with only what is needed to run the binary.
# =============================================================================
FROM debian:bookworm-slim AS runtime

# ca-certificates is required for outbound HTTPS (reqwest, lettre).
# iputils-ping and net-tools are required for the network scanner.
# libcap2-bin provides setcap, used below to grant raw-socket capabilities
# directly to the binary so the process does not need to run as root.
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates \
        iputils-ping \
        net-tools \
        libcap2-bin \
    && rm -rf /var/lib/apt/lists/*

# Non-root execution user. The container still needs `cap_add: [NET_ADMIN,
# NET_RAW]` in docker-compose.yml (capabilities are bounded by the
# container's capability set), but the *process* runs as this unprivileged
# user instead of root — an RCE in the binary no longer hands an attacker
# root on a host-networked container (Stage 3 audit, Finding 4.1).
RUN groupadd --system shabakat && \
    useradd --system --no-create-home --gid shabakat --shell /usr/sbin/nologin shabakat

WORKDIR /app

# Copy the compiled binary from the builder stage.
COPY --from=builder /build/target/release/shabakat-server ./shabakat-server

# Copy static assets.
COPY resources/ ./resources/
COPY web/dist/  ./ui_dist/

# Grant the binary raw-socket capabilities directly so ARP/ICMP scanning
# keeps working without the process running as root.
RUN setcap 'cap_net_raw,cap_net_admin+eip' ./shabakat-server

# Pre-create the data mount point with correct ownership so the named
# Docker volume (shabakat_data:/data) inherits it on first initialization,
# and make sure the app directory is readable by the non-root user.
RUN mkdir -p /data && chown -R shabakat:shabakat /data /app

ENV SHABAKAT_WEB_DIR=/app/ui_dist
ENV SHABAKAT_DATA_DIR=/data

EXPOSE 7779

USER shabakat

ENTRYPOINT ["./shabakat-server"]
