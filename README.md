<p align="center">
  <img src="images/ordo-logo.png" alt="Ordo Logo" width="180" />
</p>

<h1 align="center">Ordo</h1>

<p align="center">
  <strong>The open-source decision platform — author, test, and govern business rules at scale</strong>
</p>

<p align="center">
  <a href="https://pama-lee.github.io/Ordo/"><img src="https://img.shields.io/badge/demo-playground-brightgreen" alt="Playground" /></a>
  <img src="https://img.shields.io/badge/rust-1.83%2B-orange?logo=rust" alt="Rust" />
  <img src="https://img.shields.io/badge/license-MIT-blue" alt="License" />
  <a href="https://www.npmjs.com/package/@ordo-engine/editor-core"><img src="https://img.shields.io/npm/v/@ordo-engine/editor-core?label=npm&color=cb3837" alt="npm" /></a>
  <a href="https://discord.gg/Y529FkArhh"><img src="https://img.shields.io/badge/discord-join-7289da?logo=discord&logoColor=white" alt="Discord" /></a>
</p>

---

Ordo started as a rule engine. It's grown into a platform for teams who need to **own their decision logic** — not scatter it across codebases, spreadsheets, and tribal knowledge.

**Engine** — sub-microsecond rule execution, JIT-compiled, runs everywhere (HTTP · gRPC · WASM · CLI).  
**Platform** — org and project management, fact catalog, decision contracts, version history, rule templates.  
**Studio** — visual flow editor, test case management, one-click template instantiation.

<p align="center">
  <img src="images/main.png" alt="Ordo Studio" width="100%" />
</p>

---

## Why Ordo?

| | **Ordo** | OPA | Drools | json-rules-engine |
|---|---|---|---|---|
| Single-rule latency | **~1.6 µs** | ~1 ms | ~5–10 ms | ~0.5 ms |
| JIT compilation | ✅ Cranelift | ❌ | ❌ | ❌ |
| Visual editor | ✅ | ❌ | ✅ heavy | ❌ |
| Platform / governance | ✅ | ❌ | ❌ | ❌ |
| WASM / browser | ✅ | ❌ | ❌ | ✅ Node only |
| Deployment | single binary | agent + server | JVM | Node.js |

---

## Quick Start

### Try Studio (5 minutes)

```bash
docker compose up          # platform + engine + studio
# open http://localhost:5173
```

Or use the hosted **[Live Playground](https://pama-lee.github.io/Ordo/)** — no install needed.

### Engine only

```bash
docker run -d -p 8080:8080 ghcr.io/pama-lee/ordo:latest

# Create a rule
curl -X POST http://localhost:8080/api/v1/rulesets \
  -H "Content-Type: application/json" \
  -d @examples/discount.json

# Execute
curl -X POST http://localhost:8080/api/v1/rulesets/discount/execute \
  -d '{"input": {"user": {"vip": true}}}'
# → {"code": "VIP", "message": "20% discount"}
```

### Embed the editor

```bash
npm install @ordo-engine/editor-vue    # Vue 3
npm install @ordo-engine/editor-react  # React
```

---

## Platform Features

### Rule Authoring
- **Flow editor** — drag-and-drop decision trees with live WASM execution
- **Decision table** — spreadsheet-style multi-condition rules
- **Templates** — ship pre-built rule sets (e.g. `ecommerce-coupon`) that teams clone in one click

### Governance
- **Fact catalog** — register every input field with type, source, latency, and null policy
- **Decision contracts** — typed input/output schemas per ruleset; generate test skeletons automatically
- **Version history** — full snapshot log per ruleset; roll back with one click
- **Test management** — create, run, and export test cases (ordo-cli-compatible YAML)

### Engine
- **1.63 µs** interpreter · **50–80 ns** JIT (Cranelift) · **54k QPS** HTTP single-thread
- Multi-tenancy, WAL crash recovery, hot reload, NATS JetStream sync
- Data Filter API — push rule logic into SQL / MongoDB `$match` / JSON predicates
- Compiled `.ordo` binary format with ED25519 signature for rule protection

---

## Project Structure

```
ordo/
├── crates/
│   ├── ordo-core/       # Rule engine + JIT compiler
│   ├── ordo-server/     # HTTP/gRPC API server
│   ├── ordo-platform/   # Org/project/template/testing management layer
│   ├── ordo-cli/        # CLI — eval, exec, test
│   ├── ordo-wasm/       # WebAssembly bindings
│   ├── ordo-proto/      # gRPC definitions
│   └── ordo-derive/     # TypedContext derive macro
├── ordo-editor/
│   ├── packages/        # @ordo-engine/editor-{core,vue,react,wasm}
│   └── apps/
│       ├── studio/      # Platform Studio (Vue 3 + TDesign)
│       ├── playground/  # Live demo
│       └── docs/        # VitePress documentation
└── scripts/             # sync-version.sh, benchmarks
```

---

## Roadmap

| Milestone | Target | Goal |
|-----------|--------|------|
| **M1 — Studio** | v0.4 ✅ | Platform layer, Studio app, test management |
| **First Decision** | v0.5 | 5-min onboarding, guided setup wizard |
| **Deploy & Connect** | v0.6 | One-click publish from Studio to Engine, SDK codegen |
| **Observe** | v0.7 | Execution dashboard, trace explorer, alerting |
| **Govern** | v0.8 | Change requests, impact analysis, approval flows |
| **Ordo Cloud** | v1.0 | Managed platform with hosted engine |

Full roadmap → [docs/roadmap](https://pama-lee.github.io/Ordo/docs/en/roadmap)

---

## License

MIT — see [LICENSE](LICENSE).

<p align="center"><sub>Built with Rust · <a href="https://discord.gg/Y529FkArhh">Discord</a> · <a href="https://pama-lee.github.io/Ordo/">Docs</a></sub></p>
