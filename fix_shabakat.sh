#!/bin/bash
echo "Diagnostic and Fix script for Shabakat Server"

# 1. Check if the backend is actually listening
echo "Checking local ports..."
netstat -tulpn | grep 8080

# 2. Check Docker logs for AUTH_DEBUG
echo "Checking Docker logs for AUTH_DEBUG..."
docker logs shabakat-server 2>&1 | grep AUTH_DEBUG | tail -n 20

# 3. Verify .env JWT_SECRET consistency
echo "Verifying .env file..."
if [ -f .env ]; then
  grep "JWT_SECRET" .env
else
  echo ".env file missing!"
fi
