---
name: Ordo project current state
description: What's built, what's planned, what to avoid duplicating with open PRs
type: project
---

Ordo is a high-performance Rust rule engine with JIT (Cranelift), 92 builtin functions, HTTP/gRPC/UDS transports, multi-tenancy, NATS JetStream sync, webhooks, visual editor (Vue/React), and SDKs (Go/Python/Java).

**Open PRs (do not duplicate):**
- #61: runtime config hot-reload (`/api/v1/admin/config`)
- #49: CLI tool (`ordo eval/exec/test`)
- #47: integer overflow fixes (rate limiter + compiler)
- #39: cargo-deny CI dependency auditing

**Already implemented (roadmap was wrong):**
- ED25519 rule signatures (`ordo-core/src/signature/`)
- Execution timeout + max_depth in executor
- 92 builtin functions (roadmap said "20+")
- Rule testing framework (`ordo-core/src/testing.rs`)
- Go SDK 1946 lines (retry, connection pool, HTTP+gRPC)
- Python SDK 1001 lines, Java SDK 1468 lines
- Webhook exponential backoff retry already in `webhook.rs`

**NOT introducing SQLite** — storage stays file-system based.

**Next priority areas (Phase 1 roadmap):**
1. WAL + crash recovery (file-based, no external DB)
2. Offline operation mode (state machine: ONLINE ↔ OFFLINE)
3. Enhanced health check (expose mode/sync status)
4. Loop iteration limits in BytecodeVM (Phase 2)

**Why:** WAL/offline is needed: current file-store has no atomicity guarantee on crash mid-write.
**How to apply:** When suggesting next work, lean toward WAL/recovery or offline mode. Avoid suggesting SQLite.
