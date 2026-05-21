#!/bin/bash
# Shabakat Asustor APKG Build Wrapper
# Targets: x86_64-unknown-linux-gnu (Intel Celeron AS6602T)

set -e

TARGET="x86_64-unknown-linux-gnu"
APP_ID="shabakat"
BUILD_DIR="target/apkg_build"

echo "🚀 Starting native Asustor package build for $TARGET..."

# 1. Cross-compile Rust Backend
echo "🦀 Compiling Rust backend for $TARGET..."
cargo zigbuild --release --target "$TARGET"

# 2. Build React Frontend
echo "⚛️ Building React frontend assets..."
if [ -d "web" ]; then
    cd web
    npm install
    npm run build
    cd ..
else
    echo "⚠️ Warning: web directory not found, skipping frontend build."
fi

# 3. Assemble Package Structure
echo "📦 Assembling package structure in $BUILD_DIR..."
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR/bin"
mkdir -p "$BUILD_DIR/www"
mkdir -p "$BUILD_DIR/CONTROL"

# Copy binary
if [ -f "target/$TARGET/release/shabakat-server" ]; then
    cp "target/$TARGET/release/shabakat-server" "$BUILD_DIR/bin/"
else
    echo "❌ Error: Binary target/$TARGET/release/shabakat-server not found!"
    exit 1
fi

# Copy web assets
if [ -d "web/dist" ]; then
    cp -r web/dist/* "$BUILD_DIR/www/"
fi

# Copy control manifests and hooks
cp CONTROL/* "$BUILD_DIR/CONTROL/"
chmod +x "$BUILD_DIR/CONTROL/"*.sh

# 4. Packaging
echo "🗜️ Creating tarball payload..."
cd "$BUILD_DIR"
tar -czf "../../$APP_ID.tar.gz" .
cd ../..

echo "✅ Build complete! Payload: $APP_ID.tar.gz"
echo "Ready for Asustor APKG compression."
