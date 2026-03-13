#!/bin/bash
set -e
echo "Building Ironweave..."

# Build frontend
echo "  Building frontend..."
cd frontend && npm ci && npm run build && cd ..

# Build Rust backend
echo "  Building Rust backend..."
cargo build --release

# Install Playwright browsers system-wide
echo "  Installing Playwright browsers..."
npx playwright install chromium firefox --with-deps

echo ""
echo "Build complete!"
echo "  Binary: target/release/ironweave"
echo "  Frontend: frontend/dist/"
echo ""
echo "To deploy:"
echo "  1. Copy binary and frontend/dist/ to server"
echo "  2. Copy ironweave.toml to server"
echo "  3. Install systemd service: sudo cp deploy/ironweave.service /etc/systemd/system/"
echo "  4. sudo systemctl daemon-reload && sudo systemctl enable --now ironweave"
