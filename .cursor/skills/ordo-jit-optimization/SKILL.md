---
name: ordo-jit-optimization
description: Ordo JIT compilation and performance optimization guide. Includes Schema-aware JIT, TypedContext derive macro, Cranelift compilation, performance tuning. Use for optimizing rule execution performance, reducing latency, increasing throughput.
---

# Ordo JIT Compilation and Performance Optimization

## Schema-Aware JIT

Ordo's JIT compiler is based on Cranelift, supporting Schema-aware direct memory access with 20-30x performance improvement.

### Core Architecture

```
                    ┌─────────────────┐
                    │   Expr AST      │
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
                    │ SchemaJITCompiler│
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
     ┌────────▼────────┐    │    ┌────────▼────────┐
     │  Field Offset   │    │    │  Native Code    │
     │   Resolution    │    │    │   Generation    │
     └─────────────────┘    │    └─────────────────┘
                    ┌───────▼────────┐
                    │ Machine Code   │
                    │ ldr d0, [ptr+N]│
                    └────────────────┘
```

### TypedContext Derive Macro

```rust
use ordo_derive::TypedContext;

#[derive(TypedContext)]
struct UserContext {
    age: i64,
    balance: f64,
    vip_level: i64,
    #[typed_context(skip)]  // Skip non-numeric fields
    name: String,
}
```

The generated Schema contains field offsets, JIT compiler directly generates memory load instructions.

### Using JIT Evaluator

```rust
use ordo_core::expr::jit::{SchemaJITCompiler, SchemaJITEvaluator};

// Create compiler
let mut compiler = SchemaJITCompiler::new()?;

// Compile expression (with Schema)
let schema = UserContext::schema();
let compiled = compiler.compile_with_schema(&expr, &schema)?;

// Execute
let ctx = UserContext { age: 25, balance: 1000.0, vip_level: 3 };
let result = unsafe { compiled.call_typed(&ctx)? };
```

## Performance Comparison

| Method | Latency | Use Case |
|--------|---------|----------|
| Interpreter | ~1.63 µs | Dynamic rules, development/debugging |
| Bytecode VM | ~200 ns | General purpose |
| Schema JIT | ~50-80 ns | High-frequency execution, fixed Schema |

## Optimization Strategies

### 1. Expression Pre-compilation

```rust
// Pre-compile expressions when loading rules
let mut ruleset = RuleSet::from_json(json)?;
ruleset.compile()?;  // Pre-compile all expressions to bytecode

// Or use one-step loading
let ruleset = RuleSet::from_json_compiled(json)?;
```

### 2. Batch Execution

```rust
use ordo_core::prelude::*;

let executor = RuleExecutor::new();

// Batch execution (reduces lock contention)
let inputs: Vec<Value> = load_batch();
let results = executor.execute_batch(&ruleset, inputs)?;
```

### 3. Vectorized Evaluation

```rust
use ordo_core::expr::VectorizedEvaluator;

let evaluator = VectorizedEvaluator::new();
let contexts: Vec<Context> = prepare_contexts();
let results = evaluator.eval_batch(&expr, &contexts)?;
```

### 4. Function Fast Path

Common functions (`len`, `sum`, `max`, `min`, `abs`, `count`, `is_null`) have inline fast paths, avoiding HashMap lookups.

## Compiler Configuration

### JIT Compiler Options

```rust
let mut compiler = SchemaJITCompiler::new()?;

// View compilation statistics
let stats = compiler.stats();
println!("Successful compiles: {}", stats.successful_compiles);
println!("Total code size: {} bytes", stats.total_code_size);
```

### Feature Flags

Configure in `Cargo.toml`:

```toml
[dependencies]
ordo-core = { version = "0.2", features = ["jit"] }

# Full features
ordo-core = { version = "0.2", features = ["default"] }  # jit + signature + derive
```

**Note**: JIT is not available on WASM targets (Cranelift doesn't support wasm32).

## Performance Tuning Checklist

### Compile-time Optimization

- [ ] Build with `--release` mode
- [ ] Enable LTO: `lto = true`
- [ ] Set `codegen-units = 1`

### Runtime Optimization

- [ ] Pre-compile rule expressions
- [ ] Use JIT for fixed Schema
- [ ] Batch execution to reduce overhead
- [ ] Set reasonable `max_depth`

### Server Optimization

- [ ] Set `RUST_LOG=warn` or `info`
- [ ] Disable unnecessary tracing
- [ ] Use connection pooling
- [ ] Configure appropriate worker count

## Benchmarking

Run built-in benchmarks:

```bash
# Basic benchmarks
cargo bench --package ordo-core

# JIT comparison tests
cargo bench --package ordo-core --bench jit_comparison_bench

# Schema JIT tests
cargo bench --package ordo-core --bench schema_jit_bench
```

### Typical Results (Apple Silicon)

```
expression/eval/simple_compare    time: [79.234 ns]
expression/eval/function_call     time: [211.45 ns]
rule/simple_ruleset              time: [1.6312 µs]
jit/schema_aware/numeric         time: [52.341 ns]
```

## Key Files

- `crates/ordo-core/src/expr/jit/schema_compiler.rs` - Schema JIT compiler
- `crates/ordo-core/src/expr/jit/schema_evaluator.rs` - JIT evaluator
- `crates/ordo-core/src/expr/jit/typed_context.rs` - Typed context
- `crates/ordo-derive/src/lib.rs` - TypedContext derive macro
- `crates/ordo-core/benches/` - Benchmarks
