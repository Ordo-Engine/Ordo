#!/usr/bin/env bash
set -euo pipefail

WORKSPACE=${WORKSPACE:-/workspace}
DATA_DIR=${DATA_DIR:-/data}
STUDIO_PORT=${STUDIO_PORT:-3002}
PLATFORM_PORT=${PLATFORM_PORT:-3001}
ENGINE_PORT=${ENGINE_PORT:-8080}
CARGO_HOME=${CARGO_HOME:-$DATA_DIR/cargo-home}
CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$DATA_DIR/cargo-target}
PNPM_STORE_DIR=${PNPM_STORE_DIR:-$DATA_DIR/pnpm-store}
SETUP_DIR=${SETUP_DIR:-$DATA_DIR/devcontainer-state}
LOG_DIR=${LOG_DIR:-$DATA_DIR/logs}
DATABASE_HOST=${DATABASE_HOST:-127.0.0.1}
DATABASE_PORT=${DATABASE_PORT:-5432}
VITE_CACHE_DIR=${VITE_CACHE_DIR:-.vite-devcontainer}

: "${ORDO_JWT_SECRET:?ORDO_JWT_SECRET must be provided by the Nomad job env}"
: "${ORDO_DATABASE_URL:?ORDO_DATABASE_URL must be provided by the Nomad job env}"
: "${ORDO_PLATFORM_TEMPLATES_DIR:?ORDO_PLATFORM_TEMPLATES_DIR must be provided by the Nomad job env}"
: "${ORDO_ENGINE_URL:?ORDO_ENGINE_URL must be provided by the Nomad job env}"
ORDO_LOCAL_ENGINE_ENABLED=${ORDO_LOCAL_ENGINE_ENABLED:-true}

cd "$WORKSPACE"
mkdir -p \
  "$DATA_DIR/platform" \
  "$DATA_DIR/rules" \
  "$CARGO_HOME" \
  "$CARGO_TARGET_DIR" \
  "$PNPM_STORE_DIR" \
  "$SETUP_DIR" \
  "$LOG_DIR"

: > "$LOG_DIR/ordo-server.log"
: > "$LOG_DIR/ordo-platform.log"
: > "$LOG_DIR/ordo-studio.log"

export DATA_DIR
export ORDO_JWT_SECRET
export ORDO_DATABASE_URL
export ORDO_PLATFORM_TEMPLATES_DIR
export ORDO_ENGINE_URL
export CARGO_HOME
export CARGO_TARGET_DIR
export PNPM_STORE_DIR
export VITE_CACHE_DIR
export CI=true

hash_file() {
  local target=$1
  shift
  sha256sum "$@" | sha256sum | awk '{print $1}' > "$target"
}

hash_matches() {
  local current=$1
  local recorded=$2
  [ -f "$current" ] && [ -f "$recorded" ] && cmp -s "$current" "$recorded"
}

wait_for_http() {
  local url=$1
  local name=$2
  for _ in $(seq 1 240); do
    if curl -sf "$url" >/dev/null 2>&1; then
      echo "==> $name is ready"
      return 0
    fi
    sleep 1
  done
  echo "==> $name failed to become ready in time" >&2
  return 1
}

wait_for_tcp() {
  local host=$1
  local port=$2
  local name=$3
  for _ in $(seq 1 240); do
    if nc -z "$host" "$port" >/dev/null 2>&1; then
      echo "==> $name is ready"
      return 0
    fi
    sleep 1
  done
  echo "==> $name failed to become ready in time" >&2
  return 1
}

ensure_frontend_deps() {
  local current="$SETUP_DIR/pnpm-lock.current"
  local recorded="$SETUP_DIR/pnpm-lock.applied"

  hash_file "$current" "$WORKSPACE/ordo-editor/pnpm-lock.yaml"
  if hash_matches "$current" "$recorded" && [ -d "$WORKSPACE/ordo-editor/node_modules" ]; then
    echo "==> Frontend dependencies unchanged, skipping pnpm install"
    return
  fi

  echo "==> Installing frontend dependencies"
  cd "$WORKSPACE/ordo-editor"
  pnpm install --frozen-lockfile=false --config.confirmModulesPurge=false
  cp "$current" "$recorded"
}

ensure_wasm() {
  local current="$SETUP_DIR/wasm.current"
  local recorded="$SETUP_DIR/wasm.applied"

  hash_file "$current" \
    "$WORKSPACE/Cargo.lock" \
    "$WORKSPACE/crates/ordo-wasm/Cargo.toml" \
    "$WORKSPACE/crates/ordo-wasm/src/lib.rs" \
    "$WORKSPACE/crates/ordo-wasm/build.sh"

  if hash_matches "$current" "$recorded" && [ -d "$WORKSPACE/crates/ordo-wasm/pkg" ]; then
    echo "==> WASM artifacts unchanged, skipping wasm build"
    return
  fi

  echo "==> Building WASM artifacts"
  cd "$WORKSPACE"
  make build-wasm
  cp "$current" "$recorded"
}

start_server_watch() {
  cd "$WORKSPACE"
  (
    cargo watch \
      -w crates \
      -w Cargo.toml \
      -w Cargo.lock \
      -x "run -p ordo-server --bin ordo-server -- --http-addr 0.0.0.0:${ENGINE_PORT} --rules-dir ${DATA_DIR}/rules --multi-tenancy-enabled --cors-allowed-origins '*'"
  ) 2>&1 | tee "$LOG_DIR/ordo-server.log" &
  SERVER_PID=$!
}

start_platform_watch() {
  cd "$WORKSPACE"
  (
    cargo watch \
      -w crates \
      -w Cargo.toml \
      -w Cargo.lock \
      -x "run -p ordo-platform -- --addr 0.0.0.0:${PLATFORM_PORT} --database-url ${ORDO_DATABASE_URL} --engine-url ${ORDO_ENGINE_URL} --jwt-secret ${ORDO_JWT_SECRET} --templates-dir ${ORDO_PLATFORM_TEMPLATES_DIR}"
  ) 2>&1 | tee "$LOG_DIR/ordo-platform.log" &
  PLATFORM_PID=$!
}

start_studio() {
  cd "$WORKSPACE/ordo-editor/apps/studio"
  # Pre-warm Vite dep cache so the first proxied request doesn't hit Traefik's timeout.
  # Only clear the cache when the lockfile has changed (same pattern as other deps).
  local vite_stamp="$SETUP_DIR/vite-deps.applied"
  local pnpm_stamp="$SETUP_DIR/pnpm-lock.applied"
  if [ ! -f "$vite_stamp" ] || ! cmp -s "$pnpm_stamp" "$vite_stamp" 2>/dev/null || [ ! -f "$VITE_CACHE_DIR/deps/tdesign-vue-next.js" ]; then
    echo "==> Pre-bundling Vite dependencies"
    pnpm exec vite optimize 2>&1 | tee -a "$LOG_DIR/ordo-studio.log" || true
    cp "$pnpm_stamp" "$vite_stamp" 2>/dev/null || true
  fi
  (
    pnpm exec vite --host 0.0.0.0 --port "$STUDIO_PORT"
  ) 2>&1 | tee "$LOG_DIR/ordo-studio.log" &
  STUDIO_PID=$!
}

shutdown() {
  for pid in ${SERVER_PID:-} ${PLATFORM_PID:-} ${STUDIO_PID:-}; do
    if [ -n "${pid:-}" ] && kill -0 "$pid" 2>/dev/null; then
      kill "$pid" 2>/dev/null || true
    fi
  done
}

trap shutdown EXIT INT TERM

ensure_frontend_deps
ensure_wasm

echo "==> Starting long-lived dev services"
PIDS=()
if [ "$ORDO_LOCAL_ENGINE_ENABLED" = "true" ]; then
  start_server_watch
  PIDS+=("$SERVER_PID")
  wait_for_http "http://127.0.0.1:${ENGINE_PORT}/health" "ordo-server"
else
  echo "==> Skipping local ordo-server startup; using external engine ${ORDO_ENGINE_URL}"
fi
wait_for_tcp "$DATABASE_HOST" "$DATABASE_PORT" "postgres"
start_platform_watch
PIDS+=("$PLATFORM_PID")
wait_for_http "http://127.0.0.1:${PLATFORM_PORT}/health" "ordo-platform"
start_studio
PIDS+=("$STUDIO_PID")

echo "==> Devcontainer services started"
echo "    studio log:   $LOG_DIR/ordo-studio.log"
echo "    server log:   $LOG_DIR/ordo-server.log"
echo "    platform log: $LOG_DIR/ordo-platform.log"

wait -n "${PIDS[@]}"
exit 1
