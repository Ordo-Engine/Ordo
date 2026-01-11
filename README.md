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
- [ ] Rule versioning & history
- [ ] Collaborative editing
- [ ] Rule marketplace

---

## License

MIT License - see [LICENSE](LICENSE) for details.

---

<p align="center">
  <sub>Built with 🦀 and Rust</sub>
</p>
