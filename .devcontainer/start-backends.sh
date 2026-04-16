#!/usr/bin/env bash
# Runs every time the devcontainer starts.
# Launches ordo-server and ordo-platform as background processes.
set -euo pipefail

WORKSPACE=/workspace

# ── Check if JWT secret is set ────────────────────────────────────────────────
if [ -z "${ORDO_JWT_SECRET:-}" ]; then
  export ORDO_JWT_SECRET="dev-secret-devcontainer-not-for-production"
fi

# ── Use Makefile to start backends (idempotent) ──────────────────────────────
cd "$WORKSPACE"
export DATA_DIR=/data

make dev

echo ""
echo "    Start Studio:  cd ordo-editor && pnpm dev"
echo "    Watch logs:    tail -f /tmp/ordo-*.log"
