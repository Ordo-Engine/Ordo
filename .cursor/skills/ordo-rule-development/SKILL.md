---
name: ordo-rule-development
description: Ordo rule engine Rust development guide. Helps create RuleSet, Step, conditional branches, terminal results. Use for writing rule definitions, understanding Step Flow model, creating Decision/Action/Terminal steps.
---

# Ordo Rule Engine Development

## Core Concepts

### Step Flow Model

Ordo uses Step Flow model to organize rules:

- **Decision Step**: Conditional judgment, jumps to different steps based on conditions
- **Action Step**: Execute actions (set variables, call external services)
- **Terminal Step**: Terminate execution, return result

### RuleSet Structure

```rust
use ordo_core::prelude::*;

// Create ruleset
let mut ruleset = RuleSet::new("rule_name", "entry_step_id");

// Add steps
ruleset.add_step(step);

// Validate ruleset
ruleset.validate()?;

// Execute
let executor = RuleExecutor::new();
let result = executor.execute(&ruleset, input)?;
```

## Creating Steps

### Decision Step

```rust
// Builder pattern
let step = Step::decision("check_balance", "Check Balance")
    .branch(Condition::from_string("balance >= 1000"), "approve")
    .branch(Condition::from_string("balance >= 500"), "review")
    .default("reject")
    .build();
```

### Terminal Step

```rust
let step = Step::terminal(
    "approve",
    "Approved",
    TerminalResult::new("APPROVED")
        .with_message("User has been approved")
        .with_output("allowed", Expr::literal(true))
        .with_output("discount", Expr::literal(0.2))
);
```

### Action Step

```rust
let step = Step::action("set_discount", "Set Discount")
    .set("discount_rate", Expr::literal(0.15))
    .set("applied_at", Expr::field("timestamp"))
    .next("final_check")
    .build();
```

## Condition Expressions

### Basic Comparisons

```rust
Condition::from_string("age >= 18")
Condition::from_string("status == \"active\"")
Condition::from_string("balance > 0")
```

### Logical Combinations

```rust
Condition::from_string("age >= 18 && status == \"active\"")
Condition::from_string("tier == \"gold\" || tier == \"platinum\"")
Condition::from_string("!(is_blocked == true)")
```

### Field Access

```rust
Condition::from_string("user.profile.level >= 5")
Condition::from_string("items[0].price > 100")
Condition::from_string("len(orders) > 0")
```

## JSON/YAML Format

### JSON Rule Definition

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
      "result": { "code": "VIP", "message": "20% discount" }
    }
  }
}
```

### Loading and Executing

```rust
// Load from JSON and pre-compile
let mut ruleset = RuleSet::from_json_compiled(json_str)?;

// Or step by step
let mut ruleset = RuleSet::from_json(json_str)?;
ruleset.compile()?;  // Pre-compile expressions

// Execute
let executor = RuleExecutor::new();
let input: Value = serde_json::from_str(r#"{"user": {"vip": true}}"#)?;
let result = executor.execute(&ruleset, input)?;
```

## Configuration Options

### RuleSetConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | String | Required | RuleSet name |
| `entry_step` | String | Required | Entry step ID |
| `version` | String | "1.0.0" | Version number |
| `field_missing` | enum | Lenient | Missing field handling |
| `max_depth` | usize | 100 | Max execution depth |
| `timeout_ms` | u64 | 0 | Timeout (0=unlimited) |
| `enable_trace` | bool | false | Enable execution tracing |

### FieldMissingBehavior

- `Lenient`: Treat missing field as null
- `Strict`: Error on missing field
- `Default`: Use default value

## Compiled Rules (.ordo format)

```rust
// Compile to binary format
let compiled = RuleSetCompiler::compile(&ruleset)?;
compiled.save_to_file("rules.ordo")?;

// Load and execute
let loaded = CompiledRuleSet::load_from_file("rules.ordo")?;
let executor = CompiledRuleExecutor::new();
let result = executor.execute(&loaded, input)?;
```

## Execution Tracing

```rust
// Enable tracing
ruleset.config.enable_trace = true;

let result = executor.execute(&ruleset, input)?;

// Access trace information
if let Some(trace) = result.trace {
    for step in trace.steps {
        println!("Step: {} -> {}", step.step_id, step.result);
    }
}
```

## Key Files

- `crates/ordo-core/src/rule/model.rs` - RuleSet model
- `crates/ordo-core/src/rule/step.rs` - Step type definitions
- `crates/ordo-core/src/rule/executor.rs` - Rule executor
- `crates/ordo-core/src/rule/compiler.rs` - Rule compiler
- `crates/ordo-core/examples/basic_usage.rs` - Basic example
