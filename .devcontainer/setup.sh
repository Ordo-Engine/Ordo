#!/usr/bin/env bash
# Runs ONCE after the devcontainer is first created.
# Installs tools not covered by devcontainer features.
set -euo pipefail

echo "==> Setting up Ordo dev environment..."

# ── Rust tools ────────────────────────────────────────────────────────────────
echo "==> Installing Rust tools..."
cargo install cargo-watch 2>/dev/null || true

# ── Node tools ────────────────────────────────────────────────────────────────
echo "==> Installing pnpm..."
npm install -g pnpm

# ── WASM build ───────────────────────────────────────────────────────────────
echo "==> Building WASM module..."
cd /workspace
make build-wasm

# ── Frontend dependencies ─────────────────────────────────────────────────────
echo "==> Installing frontend dependencies..."
cd /workspace/ordo-editor
pnpm install

echo ""
echo "==> Setup complete!"
echo "    Backends start automatically on container launch."
echo "    Start Studio:  cd ordo-editor && pnpm dev"
