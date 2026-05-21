#!/bin/bash
set -e

echo "Building docker image for linux/amd64..."
docker buildx build --platform linux/amd64 -t shabakat-server:latest .

echo "Saving image to /tmp/shabakat-server.tar.gz..."
docker save shabakat-server:latest | gzip > /tmp/shabakat-server.tar.gz

echo "Transferring configuration to WADDAN..."
scp -o IdentitiesOnly=yes -o PreferredAuthentications=password docker-compose.yml totofroto@192.168.254.18:/volume1/Docker/shabakat-server/
scp -o IdentitiesOnly=yes -o PreferredAuthentications=password .env.example totofroto@192.168.254.18:/volume1/Docker/shabakat-server/

echo "Transferring image to WADDAN..."
scp -o IdentitiesOnly=yes -o PreferredAuthentications=password /tmp/shabakat-server.tar.gz totofroto@192.168.254.18:/tmp/

echo "Loading image on WADDAN..."
ssh -t -o IdentitiesOnly=yes -o PreferredAuthentications=password totofroto@192.168.254.18 'sudo docker load -i /tmp/shabakat-server.tar.gz'

echo "Restarting shabakat-server on WADDAN..."
ssh -t -o IdentitiesOnly=yes -o PreferredAuthentications=password totofroto@192.168.254.18 'sudo docker stop shabakat-server || true && sudo docker rm shabakat-server || true && cd /volume1/Docker/shabakat-server && sudo docker compose up -d'

echo "Verifying logs..."
ssh -t -o IdentitiesOnly=yes -o PreferredAuthentications=password totofroto@192.168.254.18 'sudo docker logs --tail 10 shabakat-server'
