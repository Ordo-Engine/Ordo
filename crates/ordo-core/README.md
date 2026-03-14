# ordo-core

The core rule engine library for [Ordo](https://github.com/Pama-Lee/Ordo).

Evaluate business rules with **sub-microsecond latency** — 1.63 µs interpreter, 50–80 ns with JIT — using a directed step graph with a built-in expression language.

## Quick start

```toml
[dependencies]
ordo-core = "0.2"
```

```rust
use ordo_core::prelude::*;

// Load from JSON (production: use from_json_compiled for pre-parsed expressions)
let ruleset: RuleSet = serde_json::from_str(r#"{
    "config": { "name": "discount", "version": "1.0.0", "entry_step": "check" },
    "steps": {
        "check": {
            "id": "check", "name": "Check VIP", "type": "decision",
            "branches": [{ "condition": "user.vip == true", "next_step": "vip" }],
            "default_next": "normal"
        },
        "vip":    { "id": "vip",    "name": "VIP",    "type": "terminal", "result": { "code": "VIP",    "message": "20% off" } },
        "normal": { "id": "normal", "name": "Normal", "type": "terminal", "result": { "code": "NORMAL", "message": "5% off"  } }
    }
}"#)?;

let ruleset = ruleset.compile()?;   // pre-parses all expressions

let input: Value = serde_json::from_str(r#"{"user": {"vip": true}}"#)?;
let result = RuleExecutor::new().execute(&ruleset, input)?;
assert_eq!(result.code, "VIP");
```

## Features

| Feature | What it does |
|---------|-------------|
| `derive` (default) | `#[derive(TypedContext)]` macro for schema-aware JIT |
| `jit` (default) | Cranelift JIT for numeric expressions — 20–30x speedup |
| `signature` (default) | ED25519 sign/verify for compiled `.ordo` files |

Disable features you don't need:

```toml
ordo-core = { version = "0.2", default-features = false }
```

JIT is not available on `wasm32` targets regardless of feature flags.

## Execution model

A `RuleSet` is a directed graph of `Step`s with a single entry point:

- **Decision** — evaluates ordered `Branch` conditions, jumps to first match or `default_next`
- **Action** — mutates context fields, then jumps to `next_step`
- **Terminal** — returns `ExecutionResult` with `code`, `message`, and named `outputs`

Expression evaluation pipeline per condition string:

```
ExprParser → AST → ExprOptimizer (constant folding) → ExprCompiler → BytecodeVM
                                                                    ↓ (hot numeric paths)
                                                              Cranelift JIT
```

Always call `ruleset.compile()` (or `RuleSet::from_json_compiled()`) after loading. Using `from_json()` alone leaves expressions as raw strings that are re-parsed on every execution.

## JIT compilation

Schema-aware JIT via `#[derive(TypedContext)]` — emits native code for numeric and boolean expressions:

```rust
use ordo_core::prelude::*;
use ordo_derive::TypedContext;

#[derive(TypedContext)]
struct UserCtx {
    age: i64,
    balance: f64,
    vip_level: i64,
}

let jit = ordo_core::expr::jit::JitEvaluator::<UserCtx>::new()?;
let result = jit.evaluate("age >= 18 && balance > 1000.0", &ctx)?;
```

## Data Filter API

Partially evaluate a `RuleSet` against known inputs to produce a database predicate — push rule logic into SQL, MongoDB, or a JSON predicate tree with no full-table scans:

```rust
use ordo_core::filter::{FilterCompiler, FilterRequest, FilterFormat};

let request = FilterRequest {
    known_input: serde_json::from_str(r#"{"user": {"role": "member"}}"#)?,
    target_results: vec!["ALLOW".to_string()],
    format: FilterFormat::Sql,
    field_mapping: [("doc.owner_id".to_string(), "owner_id".to_string())].into(),
    max_paths: 0,  // 0 = unlimited
};

let result = FilterCompiler::new(&ruleset).compile(request)?;
// result.filter == "(owner_id = 'member-docs') OR (visibility = 'public')"
```

## Compiled rules (`.ordo` format)

Protect business logic by compiling to a binary format with CRC32 integrity and optional ED25519 signature:

```rust
use ordo_core::prelude::*;

let compiled = RuleSetCompiler::compile(&ruleset)?;
compiled.save_to_file("rules.ordo")?;

let loaded = CompiledRuleSet::load_from_file("rules.ordo")?;
let result = CompiledRuleExecutor::new().execute(&loaded, input)?;
```

## Execution tracing

```rust
let options = ExecutionOptions { trace: true, ..Default::default() };
let result = RuleExecutor::new().execute_with_options(&ruleset, input, options)?;

if let Some(trace) = result.trace {
    for step in &trace.steps {
        println!("{}: {}µs", step.step_id, step.duration_us);
    }
}
```

## Expression syntax

```
age >= 18 && status == "active"
tier == "gold" || tier == "platinum"
user.profile.level
len(items) > 0 && sum(prices) >= 100.0
if exists(discount) then price * (1 - discount) else price
value == null
```

Built-in functions: `len`, `sum`, `avg`, `min`, `max`, `abs`, `upper`, `lower`, `exists`, `coalesce`, `contains`, `starts_with`, `ends_with`, `is_null`.

## Performance

Measured on Apple Silicon M-series, single thread, warm runs:

| Mode | Latency |
|------|---------|
| Interpreter (warm) | 1.63 µs |
| JIT (numeric expressions) | 50–80 ns |
| Expression evaluation | 79–211 ns |

See [benchmark/](../../benchmark/) for full reports and methodology.

## Related crates

- [`ordo-derive`](https://crates.io/crates/ordo-derive) — `#[derive(TypedContext)]` proc-macro
- [`ordo-server`](https://github.com/Pama-Lee/Ordo) — HTTP REST + gRPC server wrapping this library

## License

MIT
