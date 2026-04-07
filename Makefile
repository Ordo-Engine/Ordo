# ── Ordo Development Makefile ────────────────────────────────────────────────
#
# Quick reference:
#   make dev          Start everything for local development
#   make up           Start backend containers (detached)
#   make down         Stop containers
#   make logs         Follow container logs
#   make studio       Start Vite dev server (:3002)
#   make build        Build all Rust binaries (debug)
#   make build-rs     Build Rust binaries (release)
#   make reset        Wipe all data volumes and restart containers
#

.PHONY: dev stop restart up down logs studio build build-rs build-wasm reset kill-3002 help

# ── Variables ─────────────────────────────────────────────────────────────────
ORDO_JWT_SECRET ?= dev-secret-change-me-in-production-32c
DATA_DIR        ?= $(HOME)/.local/share/ordo-dev
PLATFORM_PORT   ?= 3001
ENGINE_PORT     ?= 8080
STUDIO_PORT     ?= 3002

# ── Dev (local, no Docker) ───────────────────────────────────────────────────
# Starts both backend services locally (idempotent — skips if already running).
dev: build
	@mkdir -p $(DATA_DIR)/rules $(DATA_DIR)/platform
	@echo ""
	@if curl -sf http://localhost:$(ENGINE_PORT)/health >/dev/null 2>&1; then \
		echo "ordo-server already running on :$(ENGINE_PORT) ✓"; \
	else \
		echo "Starting ordo-server on :$(ENGINE_PORT)..."; \
		./target/debug/ordo-server \
			--http-addr 0.0.0.0:$(ENGINE_PORT) \
			--rules-dir $(DATA_DIR)/rules \
			--multi-tenancy-enabled \
			--cors-allowed-origins '*' \
			> /tmp/ordo-server.log 2>&1 & \
		echo "  PID $$!  log: /tmp/ordo-server.log"; \
		for i in $$(seq 1 20); do \
			if curl -sf http://localhost:$(ENGINE_PORT)/health >/dev/null 2>&1; then break; fi; \
			sleep 0.5; \
		done; \
		echo "  ordo-server ready ✓"; \
	fi
	@if curl -sf http://localhost:$(PLATFORM_PORT)/health >/dev/null 2>&1; then \
		echo "ordo-platform already running on :$(PLATFORM_PORT) ✓"; \
	else \
		echo "Starting ordo-platform on :$(PLATFORM_PORT)..."; \
		./target/debug/ordo-platform \
			--addr 0.0.0.0:$(PLATFORM_PORT) \
			--platform-dir $(DATA_DIR)/platform \
			--engine-url http://localhost:$(ENGINE_PORT) \
			--jwt-secret $(ORDO_JWT_SECRET) \
			> /tmp/ordo-platform.log 2>&1 & \
		echo "  PID $$!  log: /tmp/ordo-platform.log"; \
		for i in $$(seq 1 10); do \
			if curl -sf http://localhost:$(PLATFORM_PORT)/health >/dev/null 2>&1; then break; fi; \
			sleep 0.5; \
		done; \
		echo "  ordo-platform ready ✓"; \
	fi
	@echo ""
	@echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
	@echo "  Backend ready:"
	@echo "    ordo-platform  http://localhost:$(PLATFORM_PORT)  ← public API"
	@echo "    ordo-server    http://localhost:$(ENGINE_PORT)    ← engine (internal)"
	@echo ""
	@echo "  Now start Studio in another terminal:"
	@echo "    cd ordo-editor && pnpm dev"
	@echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Stop locally running backends
stop:
	@pkill -f 'ordo-server.*--http-addr' 2>/dev/null && echo "Stopped ordo-server" || echo "ordo-server not running"
	@pkill -f 'ordo-platform.*--addr' 2>/dev/null && echo "Stopped ordo-platform" || echo "ordo-platform not running"

# Restart backends (stop + dev)
restart: stop
	@sleep 1
	@$(MAKE) dev

# Start everything: backends + Studio (single command for devcontainer)
dev-all: dev studio

# Watch mode: auto-rebuild and restart on file changes (requires cargo-watch)
dev-watch:
	@mkdir -p $(DATA_DIR)/rules $(DATA_DIR)/platform
	@ORDO_JWT_SECRET=$(ORDO_JWT_SECRET) cargo watch -x \
		"run -p ordo-server -- --addr 0.0.0.0:$(ENGINE_PORT) --rules-dir $(DATA_DIR)/rules" \
		-x "run -p ordo-platform -- --addr 0.0.0.0:$(PLATFORM_PORT) --platform-dir $(DATA_DIR)/platform --engine-url http://localhost:$(ENGINE_PORT) --jwt-secret $(ORDO_JWT_SECRET)"

# ── Docker Compose workflow ───────────────────────────────────────────────────
up:
	ORDO_JWT_SECRET=$(ORDO_JWT_SECRET) docker compose up -d
	@echo ""
	@echo "  ordo-platform  http://localhost:$(PLATFORM_PORT)"
	@echo "  Start Studio:  cd ordo-editor && pnpm dev"

down:
	docker compose down

logs:
	docker compose logs -f

reset:
	docker compose down -v
	ORDO_JWT_SECRET=$(ORDO_JWT_SECRET) docker compose up -d --build

# ── Studio ────────────────────────────────────────────────────────────────────
studio:
	@$(MAKE) kill-3002
	cd ordo-editor && pnpm dev

kill-3002:
	@fuser -k $(STUDIO_PORT)/tcp 2>/dev/null && echo "Killed process on :$(STUDIO_PORT)" || true

# ── Build ─────────────────────────────────────────────────────────────────────
build:
	cargo build -p ordo-server -p ordo-platform

build-rs:
	cargo build --release -p ordo-server -p ordo-platform

# Build WASM module (requires wasm-pack + wasm32-unknown-unknown target)
build-wasm:
	@if ! command -v wasm-pack >/dev/null 2>&1; then \
		echo "Installing wasm-pack..."; \
		cargo install wasm-pack; \
	fi
	@if ! rustup target list --installed | grep -q wasm32-unknown-unknown; then \
		echo "Adding wasm32-unknown-unknown target..."; \
		rustup target add wasm32-unknown-unknown; \
	fi
	cd crates/ordo-wasm && bash build.sh

# ── Help ──────────────────────────────────────────────────────────────────────
help:
	@echo "Ordo dev commands:"
	@echo "  make dev         Start backends locally (idempotent, debug build)"
	@echo "  make stop        Stop locally running backends"
	@echo "  make restart     Stop + start backends"
	@echo "  make dev-watch   Start backends with cargo-watch hot reload"
	@echo "  make up          Start backends in Docker (detached)"
	@echo "  make down        Stop Docker containers"
	@echo "  make logs        Follow Docker logs"
	@echo "  make studio      Kill :3002 and start Vite"
	@echo "  make build       cargo build (debug)"
	@echo "  make build-rs    cargo build --release"
	@echo "  make reset       Wipe Docker volumes and restart"
