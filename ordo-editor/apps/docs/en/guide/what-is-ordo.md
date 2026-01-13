# What is Ordo?

**Ordo** (Latin for "order") is an enterprise-grade rule engine designed for extreme performance and reliability. Built entirely in Rust, it evaluates business rules with **sub-microsecond latency** and supports **500,000+ executions per second**.

## Key Features

### Visual Rule Editor

Design complex business rules with an intuitive drag-and-drop flow editor:

- **Flow View**: Visualize rule logic as connected decision trees
- **Form View**: Edit conditions and actions with a structured form interface
- **Real-time Execution**: Test rules instantly with WASM-powered execution
- **Execution Trace**: Debug step-by-step with visual path highlighting

### Blazing Fast Performance

- **1.63 µs** average rule execution time
- **600x faster** than the 1ms target
- Zero-allocation hot path
- Pre-compiled expression evaluation

### Flexible Rule Definition

- **Step Flow Model**: Linear decision steps with conditional jumps
- **Rich Expressions**: Comparisons, logical operators, functions, conditionals
- **Built-in Functions**: `len()`, `sum()`, `avg()`, `upper()`, `lower()`, `abs()`, `min()`, `max()`
- **Field Coalescing**: `coalesce(field, fallback, default)` for missing field handling

### Production Ready

- **Deterministic Execution**: Same input → Same path → Same result
- **Execution Tracing**: Full visibility into every step for debugging
- **Hot Reload**: Update rules without service restart
- **Audit Logging**: Track all rule changes and executions

### Easy Integration

- **HTTP REST API**: Simple JSON-based interface
- **WebAssembly**: Run rules directly in browser
- **gRPC Support**: High-performance binary protocol

## Architecture

Ordo consists of several components:

```
┌─────────────────────────────────────────────────────────────┐
│                     Visual Editor (Vue 3)                    │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │  Flow View  │  │  Form View  │  │  Execution Trace    │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      ordo-wasm (WASM)                        │
│                Browser-side rule execution                   │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                     ordo-server (Rust)                       │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌───────────────┐   │
│  │  HTTP   │  │  gRPC   │  │   UDS   │  │  Rule Store   │   │
│  └─────────┘  └─────────┘  └─────────┘  └───────────────┘   │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      ordo-core (Rust)                        │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │  Executor   │  │  Evaluator  │  │  Expression Parser  │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Use Cases

Ordo is ideal for:

- **Risk Assessment**: Credit scoring, fraud detection, KYC checks
- **Pricing Rules**: Dynamic pricing, discount calculations, promotions
- **Eligibility Checks**: Loan approval, insurance underwriting
- **Routing Logic**: Order routing, load balancing decisions
- **Compliance Rules**: Regulatory checks, policy enforcement
- **Feature Flags**: A/B testing, gradual rollouts
