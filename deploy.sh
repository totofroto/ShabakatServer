#!/bin/bash
set -e

# Configuration
NAS_IP="192.168.254.18"
NAS_USER="totofroto"
NAS_PATH="/volume1/Docker/ShabakatServer"

echo "--- 🚀 Deploying Shabakat Unified Binary ---"

# 1. Sync source tree to NAS.
#    Excludes:
#      data/        — now a named Docker volume; host directory is obsolete
#      test_data/   — dev-only fixture data
#      target/      — local Rust build artifacts (NAS builds from source in Docker)
#      node_modules — JS toolchain artifacts
#      web/dist     — built inside Docker from web/ sources; not pre-built here
#      .git/        — VCS history not needed on the NAS
#      .env         — kept on the NAS in-place; never overwritten by deploy
#      shabakat.tar.gz — old packaging artifact; not part of the new pipeline
echo "🔄 Syncing to NAS via rsync..."
ssh "$NAS_USER@$NAS_IP" "mkdir -p $NAS_PATH"
rsync -avz --progress \
  --exclude 'data/' \
  --exclude 'test_data/' \
  --exclude 'target/' \
  --exclude 'node_modules/' \
  --exclude '.git/' \
  --exclude '.env' \
  --exclude 'shabakat.tar.gz' \
  --exclude 'web/node_modules/' \
  . "$NAS_USER@$NAS_IP:$NAS_PATH/"

# 2. SSH into NAS: tear down, rebuild from source, bring back up.
#    docker compose down -v  — stops containers AND removes the old anonymous/named
#                              volumes so we start with a truly clean slate.
#    docker compose build --no-cache  — forces a full multi-stage Rust compile
#                                       inside the container; no ghost binary.
#    docker compose up -d    — starts the freshly-built image in the background.
echo "🏗️  Remote build and restart..."
ssh -t "$NAS_USER@$NAS_IP" "
  cd $NAS_PATH && \
  sudo docker compose down -v && \
  sudo docker compose build --no-cache && \
  sudo docker compose up -d
"

echo "--- ✅ Deployment Complete ---"
