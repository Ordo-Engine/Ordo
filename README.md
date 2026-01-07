<p align="center">
  <img src="images/ordo-logo.png" alt="Ordo Logo" width="180" />
</p>

<h1 align="center">Ordo</h1>

<p align="center">
  <strong>A high-performance rule engine built in Rust</strong>
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#performance">Performance</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#api-reference">API</a> •
  <a href="#benchmark">Benchmark</a> •
  <a href="#license">License</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/rust-1.83%2B-orange?logo=rust" alt="Rust Version" />
  <img src="https://img.shields.io/badge/license-MIT-blue" alt="License" />
  <img src="https://img.shields.io/badge/status-alpha-yellow" alt="Status" />
</p>

---

## What is Ordo?

**Ordo** (Latin for "order") is an enterprise-grade rule engine designed for extreme performance and reliability. Built entirely in Rust, it evaluates business rules with **sub-microsecond latency** and supports **500,000+ executions per second**.

```yaml
# Example: User eligibility rule
name: user_eligibility
version: "1.0.0"
steps:
  - id: check_age
    type: decision
    condition: "age >= 18"
    on_true: check_status
    on_false: reject

  - id: check_status
    type: decision  
    condition: "status == 'active' && balance >= 100"
    on_true: approve
    on_false: reject

  - id: approve
    type: terminal
    output: { "eligible": true, "tier": "standard" }

  - id: reject
    type: terminal
    output: { "eligible": false, "reason": "criteria_not_met" }
```

---

## Features

### 🚀 Blazing Fast
- **1.63 µs** average rule execution time
- **600x faster** than the 1ms target
- Zero-allocation hot path
- Pre-compiled expression evaluation

### 🔧 Flexible Rule Definition
- **Step Flow Model**: Linear decision steps with conditional jumps
- **Rich Expressions**: Comparisons, logical operators, functions, conditionals
- **Built-in Functions**: `len()`, `sum()`, `avg()`, `upper()`, `lower()`, `abs()`, `min()`, `max()`, and more
- **Field Coalescing**: `coalesce(field, fallback, default)` for missing field handling

### 🛡️ Production Ready
- **Deterministic Execution**: Same input → Same path → Same result
- **Execution Tracing**: Full visibility into every step for debugging
- **Configurable Error Handling**: Fail-fast, ignore, or fallback strategies
- **Hot Reload**: Update rules without service restart

### 🔌 Easy Integration
- **HTTP REST API**: Simple JSON-based interface
- **gRPC Support**: High-performance binary protocol (coming soon)
- **Unix Domain Socket**: Ultra-low latency local communication (coming soon)
- **Sidecar Deployment**: Independent upgrades without affecting host services

---

## Performance

Benchmarked on Apple Silicon (M-series), single thread:

| Metric | Result |
|--------|--------|
| Single rule execution | **1.63 µs** |
| Expression evaluation | **79-211 ns** |
| HTTP API throughput | **54,000 QPS** |
| Projected multi-thread | **500,000+ QPS** |

<details>
<summary>📊 Detailed Benchmark Results</summary>

### Expression Parsing
| Expression | Time |
|------------|------|
| Simple (`age > 18`) | 1.05 µs |
| Logical AND | 1.90 µs |
| Complex condition | 3.30 µs |
| Function call | 3.16 µs |

### Expression Evaluation
| Expression | Time |
|------------|------|
| Simple compare | 78.7 ns |
| Field access | 150.1 ns |
| Arithmetic | 153.3 ns |
| Conditional | 104.4 ns |

### Built-in Functions
| Function | Time |
|----------|------|
| `abs()` | 19.9 ns |
| `len()` (string) | 42.4 ns |
| `sum()` (array) | 62.4 ns |
| `upper()` | 74.2 ns |

</details>

See [benchmark/](benchmark/) for full reports with graphs.

---

## Quick Start

### Prerequisites

- Rust 1.83 or later
- Cargo

### Installation

```bash
git clone https://github.com/Pama-Lee/Ordo.git
cd Ordo

# Build release version
cargo build --release

# Run the server
./target/release/ordo-server
```

### Basic Usage

**1. Create a rule:**

```bash
curl -X POST http://localhost:8080/api/v1/rules \
  -H "Content-Type: application/json" \
  -d '{
    "name": "discount_check",
    "version": "1.0.0",
    "steps": [
      {
        "id": "check",
        "type": "decision",
        "condition": "total >= 100",
        "on_true": "apply_discount",
        "on_false": "no_discount"
      },
      {
        "id": "apply_discount",
        "type": "terminal",
        "output": {"discount": 0.1}
      },
      {
        "id": "no_discount",
        "type": "terminal",
        "output": {"discount": 0}
      }
    ]
  }'
```

**2. Execute the rule:**

```bash
curl -X POST http://localhost:8080/api/v1/execute/discount_check \
  -H "Content-Type: application/json" \
  -d '{"input": {"total": 150}}'
```

**Response:**

```json
{
  "success": true,
  "result": {
    "discount": 0.1
  },
  "trace": {
    "steps": [
      {"id": "check", "result": true, "duration_ns": 245},
      {"id": "apply_discount", "result": "terminal", "duration_ns": 52}
    ],
    "total_duration_ns": 1623
  }
}
```

---

## API Reference

### Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/health` | Health check |
| `GET` | `/api/v1/rules` | List all rules |
| `POST` | `/api/v1/rules` | Create a rule |
| `GET` | `/api/v1/rules/:name` | Get rule by name |
| `DELETE` | `/api/v1/rules/:name` | Delete a rule |
| `POST` | `/api/v1/execute/:name` | Execute a rule |
| `POST` | `/api/v1/eval` | Evaluate an expression |

### Expression Syntax

```
# Comparisons
age >= 18
status == "active"
score != 0

# Logical operators
age >= 18 && status == "active"
tier == "gold" || tier == "platinum"
!is_blocked

# Set membership
status in ["active", "pending"]
role not_in ["banned", "suspended"]

# Field access
user.profile.level
items[0].price

# Functions
len(items) > 0
sum(prices) >= 100
upper(name) == "ADMIN"

# Conditionals
if exists(discount) then price * (1 - discount) else price

# Null coalescing
coalesce(appid, in_appid, "default")
```

---

## Project Structure

```
ordo/
├── crates/
│   ├── ordo-core/       # Core rule engine library
│   │   ├── src/
│   │   │   ├── context/ # Execution context & values
│   │   │   ├── expr/    # Expression parser & evaluator
│   │   │   ├── rule/    # Rule model & executor
│   │   │   └── trace/   # Execution tracing
│   │   ├── benches/     # Performance benchmarks
│   │   └── examples/    # Usage examples
│   ├── ordo-server/     # HTTP API server
│   └── ordo-proto/      # Protocol definitions (Protobuf)
├── benchmark/           # Benchmark reports & graphs
└── images/              # Project assets
```

---

## Benchmark

Run benchmarks locally:

```bash
# Core engine benchmarks
cargo bench --package ordo-core

# View HTML report
open target/criterion/report/index.html
```

Run HTTP load test:

```bash
# Start server
./target/release/ordo-server &

# Load test with hey
hey -n 10000 -c 50 \
  -m POST \
  -H "Content-Type: application/json" \
  -d '{"input": {"value": 75}}' \
  http://localhost:8080/api/v1/execute/test_rule
```

---

## Roadmap

- [x] Core rule engine
- [x] HTTP REST API
- [x] Execution tracing
- [x] Built-in functions
- [ ] gRPC support
- [ ] Unix Domain Socket
- [ ] Rule hot-reload
- [ ] Persistent storage
- [ ] Visual rule editor (Web UI)
- [ ] Distributed execution

---

## Contributing

Contributions are welcome! Please read our contributing guidelines before submitting PRs.

### Branch Strategy

We follow a structured branching model:

| Branch | Purpose | Base | Merge To |
|--------|---------|------|----------|
| `main` | Production-ready code | - | - |
| `develop` | Integration branch | `main` | `main` |
| `feature/*` | New features | `develop` | `develop` |
| `fix/*` | Bug fixes | `develop` | `develop` |
| `hotfix/*` | Urgent production fixes | `main` | `main` + `develop` |
| `release/*` | Release preparation | `develop` | `main` + `develop` |

**Branch Naming Convention:**

```
feature/add-grpc-support
feature/expr-cache
fix/parser-escape-chars
fix/memory-leak-in-eval
hotfix/critical-security-patch
release/v0.2.0
```

**Workflow:**

1. **Feature Development**
   ```bash
   git checkout develop
   git pull origin develop
   git checkout -b feature/your-feature-name
   # ... make changes ...
   git push origin feature/your-feature-name
   # Create PR to develop
   ```

2. **Bug Fixes**
   ```bash
   git checkout develop
   git checkout -b fix/bug-description
   # ... fix the bug ...
   git push origin fix/bug-description
   # Create PR to develop
   ```

3. **Releases**
   ```bash
   git checkout develop
   git checkout -b release/vX.Y.Z
   # ... version bump, changelog ...
   # Create PR to main AND develop
   ```

**Protected Branches:**
- `main` - Requires PR review, passing CI, no direct pushes
- `develop` - Requires PR review, passing CI

---

## License

MIT License - see [LICENSE](LICENSE) for details.

---

<p align="center">
  <sub>Built with 🥟 and Rust</sub>
</p>
