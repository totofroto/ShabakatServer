#!/bin/bash
set -e

# Configuration
NAS_IP="192.168.254.18"
NAS_USER="totofroto"
NAS_PATH="/volume1/Docker/ShabakatServer"

echo "--- 🚀 Deploying Shabakat Unified Binary ---"

# 1. Use rsync to transfer the project files
echo "🔄 Syncing to NAS via rsync..."
ssh $NAS_USER@$NAS_IP "mkdir -p $NAS_PATH"
rsync -avz --progress \
  --exclude 'target' \
  --exclude 'node_modules' \
  --exclude '.git' \
  --exclude '.env' \
  --exclude 'shabakat.tar.gz' \
  --exclude 'web/node_modules' \
  --exclude 'web/dist' \
  . $NAS_USER@$NAS_IP:$NAS_PATH/

# 2. SSH into NAS, force a clean, multi-stage production build, and spin it up
echo "🏗️  Remote build and restart..."
ssh -t $NAS_USER@$NAS_IP "
  cd $NAS_PATH && \
  sudo docker compose down && \
  sudo docker compose build --no-cache && \
  sudo docker compose up -d --force-recreate
"

echo "--- ✅ Deployment Complete ---"
