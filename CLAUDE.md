# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Design Philosophy

Ordo is an **internal infrastructure service** — it serves business-side applications within trusted networks, not public internet traffic. Design decisions should reflect this:

- **Speed and efficiency first.** Sub-microsecond latency is a core value. Avoid adding overhead (middleware layers, serialization steps, runtime checks) unless the gain clearly justifies the cost. When in doubt, benchmark.
- **Keep it lean.** Don't over-engineer for hypothetical threats on the public internet. Auth, TLS, and tenant isolation should be available but not mandatory — trust the deployment environment.
- **Developer experience for business teams.** SDK ergonomics, clear error messages, and low integration friction matter more than exhaustive admin UI features.
- **Stability over features.** A crash or silent data corruption is worse than a missing feature. Prioritize graceful degradation, proper shutdown, and execution correctness.

## Commands

### Rust (Backend)

```bash
# Build
cargo build
cargo build --release

# Run tests (all crates)
cargo test

# Run tests for a specific crate
cargo test -p ordo-core
cargo test -p ordo-server

# Run a single test
cargo test -p ordo-core test_full_workflow
cargo test -p ordo-core -- test_ruleset_validation

# Lint
cargo clippy -- -D warnings

# Format (enforced by pre-commit hook)
cargo fmt

# Run the server
cargo run -p ordo-server -- --rules-dir ./rules

# Run benchmarks
cargo bench -p ordo-core
```

### Frontend (ordo-editor)

```bash
cd ordo-editor

# Install dependencies
pnpm install

# Run playground (dev server)
pnpm dev

# Build all packages
pnpm build

# Build individual packages (must build in order: core → vue/react)
pnpm build:core
pnpm build:vue
pnpm build:react

# Lint / typecheck
pnpm lint
pnpm typecheck

# Tests
pnpm test
```

### Git Hooks

Run `./scripts/setup-hooks.sh` once after cloning to enable the pre-commit hook that auto-runs `cargo fmt`.

## Architecture

Ordo is a high-performance rule engine. The Rust backend and TypeScript frontend are two separate subsystems in one monorepo.

### Rust Workspace (`crates/`)

**`ordo-core`** — The rule engine library. All other crates depend on this.
- `rule/model.rs` — `RuleSet` and `RuleSetConfig` (the top-level rule container)
- `rule/step.rs` — `Step`, `StepKind` (Decision / Action / Terminal), `Branch`, `Condition`, `TerminalResult`
- `rule/executor.rs` — `RuleExecutor`: walks the step graph against a JSON input `Value`
- `rule/compiled.rs` / `rule/compiler.rs` / `rule/compiled_executor.rs` — Binary `.ordo` format for protecting rule logic (MAGIC header `ORDO`, CRC32 integrity, optional ED25519 signature)
- `expr/` — Expression pipeline: `parser.rs` → AST (`ast.rs`) → `optimizer.rs` → bytecode `compiler.rs` + `vm.rs` → optional Cranelift JIT (`jit/`)
- `context/` — `Value` (JSON-compatible), `Context`, `Schema`
- `signature/` — ED25519 sign/verify for rule files (feature-gated)
- `trace/` — `ExecutionTrace` / `StepTrace` for step-by-step debugging

Features: `default = ["derive", "jit", "signature"]`. JIT is not available on `wasm32`.

**`ordo-derive`** — Proc-macro crate. Provides `#[derive(TypedContext)]` which generates the typed schema needed by the JIT compiler.

**`ordo-proto`** — gRPC protobuf definitions (via `tonic`/`prost`). Defines `OrdoService` with `Execute`, `BatchExecute`, `Eval`, `ListRuleSets`, `GetRuleSet`, `Health`.

**`ordo-server`** — Binary that exposes all three transports simultaneously:
- HTTP REST (Axum, default `:8080`) — full CRUD for rulesets, execute, batch execute, version management, tenant management, Prometheus metrics at `/metrics`
- gRPC (Tonic, default `:50051`) — multi-tenancy via `x-tenant-id` metadata, batch execute, max 1000 items per batch
- Unix Domain Socket (gRPC-over-UDS, Unix only, opt-in via `--uds-path`)

Key server modules:
- `config.rs` — `ServerConfig` (Clap + env vars, all prefixed `ORDO_*`)
- `store.rs` — `RuleStore`: in-memory (`DashMap`) or file-backed (`.json`/`.yaml`/`.yml`), with version history
- `tenant.rs` — `TenantManager` / `TenantStore`: per-tenant QPS limits, burst limits, timeouts
- `audit.rs` — JSON Lines audit log with configurable sampling rate
- `telemetry.rs` — Tracing subscriber init + optional OTLP export (`ORDO_OTLP_ENDPOINT`)
- `middleware/tenant.rs` — Extracts tenant from `X-Tenant-ID` HTTP header
- `debug/` — Debug API (SSE session streaming, inline execution, VM trace dump); only mounted when `--debug-mode` is active

**`ordo-wasm`** — WebAssembly bindings for running the engine in browsers (used by the visual editor playground).

### Rule Execution Model

A `RuleSet` is a directed graph of `Step`s with a single entry step:
- **Decision** step: evaluates ordered `Branch` conditions, jumps to the first matching `next_step`, falls through to `default_next`
- **Action** step: mutates context fields, then jumps to `next_step`
- **Terminal** step: produces the final `ExecutionResult` (code, message, outputs)

Expression evaluation pipeline (per condition string):
1. `ExprParser` parses to `Expr` AST
2. `ExprOptimizer` constant-folds and eliminates dead branches
3. `ExprCompiler` compiles to register-based bytecode (`CompiledExpr`)
4. `BytecodeVM` executes at runtime — or the Cranelift JIT emits native code for hot numeric expressions

**Important:** Always call `RuleSet::from_json_compiled()` / `from_yaml_compiled()` (or `ruleset.compile()`) after loading, to pre-parse all expressions. Using `from_json()` leaves expressions as raw strings that are re-parsed on every execution.

### Frontend (`ordo-editor/`)

pnpm workspace with two apps and four packages:
- `packages/core` (`@ordo-engine/editor-core`) — Framework-agnostic rule editor logic
- `packages/vue` (`@ordo-engine/editor-vue`) — Vue 3 components
- `packages/react` (`@ordo-engine/editor-react`) — React components
- `packages/wasm` (`@ordo-engine/wasm`) — WASM bindings (wraps `ordo-wasm`)
- `apps/playground` — Live demo app
- `apps/docs` — VitePress documentation site

Build order matters: `core` must be built before `vue`/`react`.

### Multi-tenancy

Enabled with `--multi-tenancy-enabled`. Tenant is identified by:
- HTTP: `X-Tenant-ID` header (extracted by `middleware/tenant.rs`)
- gRPC: `x-tenant-id` metadata key

With persistence enabled (`--rules-dir`), tenant rules are stored under `<rules-dir>/tenants/<tenant-id>/`. Tenant configs (QPS, burst, timeout) are stored separately under `--tenants-dir` (defaults to `<rules-dir>/tenants.json`).
