<p align="center">
  <img src="images/ordo-logo.png" alt="Ordo Logo" width="180" />
</p>

<h1 align="center">Ordo</h1>

<p align="center">
  <strong>A high-performance rule engine with visual editor</strong>
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#visual-editor">Visual Editor</a> •
  <a href="#performance">Performance</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#license">License</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/rust-1.83%2B-orange?logo=rust" alt="Rust Version" />
  <img src="https://img.shields.io/badge/license-MIT-blue" alt="License" />
  <a href="https://pama-lee.github.io/Ordo/"><img src="https://img.shields.io/badge/demo-playground-brightgreen" alt="Playground" /></a>
</p>

<p align="center">
  <img src="images/main.png" alt="Ordo Visual Editor" width="100%" />
</p>

---

## What is Ordo?

**Ordo** (Latin for "order") is an enterprise-grade rule engine designed for extreme performance and reliability. Built entirely in Rust, it evaluates business rules with **sub-microsecond latency** and supports **500,000+ executions per second**.

### ✨ Try it now: [Live Playground](https://pama-lee.github.io/Ordo/)

---

## Features

### 🎨 Visual Rule Editor

Design complex business rules with an intuitive drag-and-drop flow editor:

<p align="center">
  <img src="images/flow.png" alt="Flow Editor" width="100%" />
</p>

- **Flow View**: Visualize rule logic as connected decision trees
- **Form View**: Edit conditions and actions with a structured form interface
- **Real-time Execution**: Test rules instantly with WASM-powered execution
- **Execution Trace**: Debug step-by-step with visual path highlighting

<p align="center">
  <img src="images/flow2.png" alt="Form Editor" width="100%" />
</p>

### 🚀 Blazing Fast

- **1.63 µs** average rule execution time
- **600x faster** than the 1ms target
- Zero-allocation hot path
- Pre-compiled expression evaluation

### 🔧 Flexible Rule Definition

- **Step Flow Model**: Linear decision steps with conditional jumps
- **Rich Expressions**: Comparisons, logical operators, functions, conditionals
- **Built-in Functions**: `len()`, `sum()`, `avg()`, `upper()`, `lower()`, `abs()`, `min()`, `max()`
- **Field Coalescing**: `coalesce(field, fallback, default)` for missing field handling

### 🛡️ Production Ready

- **Deterministic Execution**: Same input → Same path → Same result
- **Execution Tracing**: Full visibility into every step for debugging
- **Hot Reload**: Update rules without service restart

### 🔌 Easy Integration

- **HTTP REST API**: Simple JSON-based interface
- **WebAssembly**: Run rules directly in browser
- **gRPC Support**: High-performance binary protocol

---

## Performance

Benchmarked on Apple Silicon (M-series), single thread:

| Metric | Result |
|--------|--------|
| Single rule execution | **1.63 µs** |
| Expression evaluation | **79-211 ns** |
| HTTP API throughput | **54,000 QPS** |
| Projected multi-thread | **500,000+ QPS** |

See [benchmark/](benchmark/) for detailed reports with graphs.

---

## Quick Start

### Run the Server

```bash
git clone https://github.com/Pama-Lee/Ordo.git
cd Ordo

# Build and run
cargo build --release
./target/release/ordo-server
```

### Enable Rule Persistence

By default, rules are stored in memory and lost on restart. To enable file-based persistence:

```bash
# Create a rules directory and enable persistence
./target/release/ordo-server --rules-dir ./rules

# Rules are automatically:
# - Loaded from ./rules on startup (supports .json, .yaml, .yml)
# - Saved to ./rules when created/updated via API
# - Deleted from ./rules when removed via API
```

**Example rule file** (`./rules/discount-check.json`):
```json
{
  "config": {
    "name": "discount-check",
    "version": "1.0.0",
    "entry_step": "check_vip"
  },
  "steps": {
    "check_vip": {
      "id": "check_vip",
      "name": "Check VIP Status",
      "type": "decision",
      "branches": [
        { "condition": "user.vip == true", "next_step": "vip_discount" }
      ],
      "default_next": "normal_discount"
    },
    "vip_discount": {
      "id": "vip_discount",
      "name": "VIP Discount",
      "type": "terminal",
      "result": { "code": "VIP", "message": "20% discount applied" }
    },
    "normal_discount": {
      "id": "normal_discount",
      "name": "Normal Discount",
      "type": "terminal",
      "result": { "code": "NORMAL", "message": "5% discount applied" }
    }
  }
}
```

### Rule Version Management

When persistence is enabled, Ordo automatically keeps historical versions of your rules:

```bash
# Control version history (default: 10 versions)
./target/release/ordo-server --rules-dir ./rules --max-versions 10
```

**List versions** (`GET /api/v1/rulesets/:name/versions`):
```bash
curl http://localhost:8080/api/v1/rulesets/discount-check/versions
```

**Rollback to a previous version** (`POST /api/v1/rulesets/:name/rollback`):
```bash
curl -X POST http://localhost:8080/api/v1/rulesets/discount-check/rollback \
  -H "Content-Type: application/json" \
  -d '{"seq": 2}'
```

### Audit Logging

Enable structured audit logging to track rule changes, executions, and system events:

```bash
# Enable audit logging with 10% execution sampling
./target/release/ordo-server --rules-dir ./rules --audit-dir ./audit --audit-sample-rate 10
```

**Audit log format** (JSON Lines):
```json
{"timestamp":"2024-01-08T10:00:00.123Z","level":"INFO","event":"server_started","version":"0.1.0","rules_count":12}
{"timestamp":"2024-01-08T10:00:01.456Z","level":"INFO","event":"rule_created","rule_name":"payment-check","version":"1.0.0","source_ip":"127.0.0.1"}
{"timestamp":"2024-01-08T10:00:02.789Z","level":"INFO","event":"rule_executed","rule_name":"payment-check","duration_us":1500,"result":"success"}
```

**Event types**: `server_started`, `server_stopped`, `rule_created`, `rule_updated`, `rule_deleted`, `rule_rollback`, `rule_executed`

**Dynamic sample rate adjustment** - update at runtime without restart:
```bash
# Get current sample rate
curl http://localhost:8080/api/v1/config/audit-sample-rate
# {"sample_rate": 10}

# Update sample rate to 50%
curl -X PUT http://localhost:8080/api/v1/config/audit-sample-rate \
  -H "Content-Type: application/json" \
  -d '{"sample_rate": 50}'
# {"sample_rate": 50, "previous": 10}
```

### Monitoring & Health Check

Ordo provides built-in observability endpoints:

**Health Check** (`GET /health`):
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "uptime_seconds": 3600,
  "storage": {
    "mode": "persistent",
    "rules_dir": "./rules",
    "rules_count": 12
  }
}
```

**Prometheus Metrics** (`GET /metrics`):
```bash
curl http://localhost:8080/metrics

# Sample output:
# ordo_info{version="0.1.0"} 1
# ordo_uptime_seconds 3600
# ordo_rules_total 12
# ordo_executions_total{ruleset="payment-check",result="success"} 1000
# ordo_execution_duration_seconds_bucket{ruleset="payment-check",le="0.001"} 950
```

### Use the Visual Editor

Visit the [Live Playground](https://pama-lee.github.io/Ordo/) or run locally:

```bash
cd ordo-editor
pnpm install
pnpm dev
```

### Expression Syntax

```
# Comparisons
age >= 18
status == "active"

# Logical operators
age >= 18 && status == "active"
tier == "gold" || tier == "platinum"

# Field access
user.profile.level
items[0].price

# Functions
len(items) > 0
sum(prices) >= 100
upper(name) == "ADMIN"

# Conditionals
if exists(discount) then price * (1 - discount) else price
```

---

## Project Structure

```
ordo/
├── crates/
│   ├── ordo-core/       # Core rule engine library
│   ├── ordo-server/     # HTTP/gRPC API server
│   ├── ordo-wasm/       # WebAssembly bindings
│   └── ordo-proto/      # Protocol definitions
├── ordo-editor/         # Visual rule editor (Vue 3)
│   ├── packages/
│   │   ├── core/        # Framework-agnostic editor core
│   │   ├── vue/         # Vue components
│   │   └── wasm/        # WASM integration
│   └── apps/
│       └── playground/  # Demo application
└── benchmark/           # Performance reports
```

---

## Roadmap

- [x] Core rule engine
- [x] HTTP REST API
- [x] Execution tracing
- [x] Built-in functions
- [x] Visual rule editor
- [x] WebAssembly support
- [x] Rule versioning & history
- [x] Audit logging
- [ ] Collaborative editing
- [ ] Rule marketplace

---

## License

MIT License - see [LICENSE](LICENSE) for details.

---

<p align="center">
  <sub>Built with 🦀 and Rust</sub>
</p>
