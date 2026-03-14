# Contributing to Ordo

Thank you for your interest in contributing. This document covers how to set up the project, run tests, and submit changes.

## Development setup

**Requirements:**
- Rust 1.83+ (`rustup update stable`)
- Node.js 20+ and pnpm 9+ (for the frontend)
- `cargo-nextest` (optional, faster test runner): `cargo install cargo-nextest`

**Clone and build:**

```bash
git clone https://github.com/Pama-Lee/Ordo.git
cd Ordo

# Install git hooks (auto-runs cargo fmt on commit)
./scripts/setup-hooks.sh

# Build all crates
cargo build

# Build the frontend
cd ordo-editor && pnpm install && pnpm build
```

## Running tests

```bash
# All crates
cargo test

# Specific crate
cargo test -p ordo-core
cargo test -p ordo-server

# Single test
cargo test -p ordo-core test_full_workflow

# Frontend
cd ordo-editor && pnpm test
```

## Lint and format

```bash
cargo fmt              # format (enforced by pre-commit hook)
cargo clippy -- -D warnings   # must pass with no errors
```

## Project structure

```
crates/
  ordo-core/      # Rule engine library — most feature work goes here
  ordo-derive/    # Proc-macro: #[derive(TypedContext)]
  ordo-server/    # HTTP + gRPC server binary
  ordo-wasm/      # WebAssembly bindings
  ordo-proto/     # gRPC protobuf definitions
ordo-editor/      # Visual editor (pnpm workspace)
  packages/core   # Framework-agnostic editor logic
  packages/vue    # Vue 3 components
  packages/react  # React components
sdk/go/           # Go client SDK
scripts/bench/    # HTTP benchmark runner (Docker + hey)
benchmark/        # Published benchmark reports
```

## Submitting a pull request

1. **Create a branch from `main`**: one feature or fix per branch.
2. **Write tests** for any logic change in `ordo-core` or `ordo-server`.
3. **Run `cargo clippy -- -D warnings`** and fix all warnings before opening a PR.
4. **Use conventional commit messages**: `feat(core): ...`, `fix(server): ...`, `docs: ...`, `perf(core): ...`.
5. Open the PR against `main`. Keep the description short and focused on *why*, not *what*.

## Areas where contributions are especially welcome

- Additional built-in functions in `crates/ordo-core/src/expr/`
- Test coverage for `ordo-derive` and `ordo-wasm` (currently zero tests)
- Go SDK improvements in `sdk/go/`
- VitePress documentation in `ordo-editor/apps/docs/`
- Frontend editor features in `ordo-editor/packages/`

## Design principles

Before adding a feature, read the **Design Philosophy** section in [CLAUDE.md](CLAUDE.md). In short:

- Speed and correctness over convenience
- Lean — no overhead unless the gain is measurable
- Crash-safety over feature completeness

## Questions

Open a [GitHub Discussion](https://github.com/Pama-Lee/Ordo/discussions) or join [Discord](https://discord.gg/Y529FkArhh).
