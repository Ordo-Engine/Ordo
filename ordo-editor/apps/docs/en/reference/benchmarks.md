# Performance Benchmarks

This document summarizes the performance benchmarks of the Ordo rule engine, including **before and after optimization comparisons**.

## Executive Summary

After optimization, Ordo achieves:

- **862x faster** FunctionRegistry initialization
- **734x faster** Evaluator initialization
- **103x faster** RuleExecutor initialization
- **2.84 million** rule executions per second throughput

## Core Optimization Results (Measured Data)

### Initialization Performance - The Most Critical Optimization

| Component                   | Before  | After       | Improvement         |
| --------------------------- | ------- | ----------- | ------------------- |
| **FunctionRegistry::new()** | 3.87 µs | **4.49 ns** | **862x ↑ (99.88%)** |
| **Evaluator::new()**        | 3.59 µs | **4.89 ns** | **734x ↑ (99.86%)** |
| **RuleExecutor::new()**     | 3.76 µs | **36.5 ns** | **103x ↑ (99.01%)** |

> **Impact**: Per-request initialization overhead reduced from ~11µs to ~46ns, a **239x reduction**

### Why This Optimization Matters

In production environments, every rule execution request requires creating these components:

- Before optimization: ~11,000 ns initialization overhead per request
- After optimization: ~46 ns initialization overhead per request
- **Savings at 100K QPS**: ~1.1 seconds of CPU time saved per second

## Key Performance Indicators (KPI)

| Metric                         | Value              | Description                         |
| ------------------------------ | ------------------ | ----------------------------------- |
| **Simple Rule Execution**      | ~352 ns            | Pre-compiled minimal ruleset        |
| **Medium Rule Execution**      | ~2.03 µs           | Pre-compiled, 3-level decision tree |
| **Binary Compiled Execution**  | ~459 ns            | Cranelift JIT compiled              |
| **Batch Throughput**           | **2.84 M ops/sec** | 1K batch sequential execution       |
| **Function Registry Creation** | ~4.5 ns            | Global singleton optimized          |
| **len() Function Call**        | ~4.4 ns            | Fast-path optimized                 |

## Detailed Benchmark Results

### 1. Expression Parsing

Time to parse expression strings into AST.

| Expression Type                                  | Time    | Complexity |
| ------------------------------------------------ | ------- | ---------- |
| `age > 18` (simple comparison)                   | 1.07 µs | Low        |
| `status == "active"` (string comparison)         | 951 ns  | Low        |
| `age > 18 && status == "active"` (logical AND)   | 2.00 µs | Medium     |
| `age < 13 \|\| age > 65` (logical OR)            | 2.03 µs | Medium     |
| `user.profile.level == "gold"` (nested field)    | 1.05 µs | Low        |
| `status in ["active", "pending"]` (in operator)  | 1.81 µs | Medium     |
| `len(items) > 0` (function call)                 | 1.64 µs | Medium     |
| `price * quantity * (1 - discount)` (arithmetic) | 1.48 µs | Medium     |
| `if premium then x * 0.9 else x` (conditional)   | 2.57 µs | High       |
| `coalesce(a, b, "default")` (coalesce)           | 2.43 µs | High       |
| Complex combined expression                      | 3.25 µs | High       |

**Key Insight**: Expression parsing is a one-time overhead. By calling `RuleSet::compile()`, this overhead is completely avoided at runtime.

### 2. Expression Evaluation

Time to evaluate pre-parsed AST against context.

| Expression Type                 | Time     | Throughput |
| ------------------------------- | -------- | ---------- |
| Field comparison (`value > 50`) | 67.1 ns  | ~14.9 M/s  |
| Nested field access             | 79.7 ns  | ~12.5 M/s  |
| Logical AND                     | 147.9 ns | ~6.8 M/s   |
| Logical OR (short-circuit)      | 141.7 ns | ~7.1 M/s   |
| Arithmetic operations           | 67.8 ns  | ~14.7 M/s  |
| Function call (`len`)           | 160.5 ns | ~6.2 M/s   |
| Function call (`sum`)           | 163.8 ns | ~6.1 M/s   |
| Conditional expression          | 74.7 ns  | ~13.4 M/s  |
| Array contains check            | 149.9 ns | ~6.7 M/s   |

**Key Insight**: Expression evaluation is extremely fast, with single evaluations in the 67-164 nanosecond range.

### 3. Rule Execution Performance Comparison

End-to-end rule execution performance.

| Scenario                     | Uncompiled | Compiled    | Improvement |
| ---------------------------- | ---------- | ----------- | ----------- |
| Minimal ruleset (1 decision) | 1.54 µs    | **352 ns**  | **4.4x**    |
| Medium ruleset (3 decisions) | 6.23 µs    | **2.03 µs** | **3.1x**    |
| Binary compiled execution    | -          | **459 ns**  | -           |

**Key Insight**:

- Pre-compiling rulesets provides 3-4x performance improvement
- For production environments, **always** call `compile()` method
- Binary compilation (Cranelift JIT) provides optimal performance

### 4. Built-in Functions Performance

Fast-path optimized function call performance.

| Function       | Time    | Description    |
| -------------- | ------- | -------------- |
| `len(string)`  | 4.43 ns | String length  |
| `len(array)`   | 4.41 ns | Array length   |
| `is_null(x)`   | 4.36 ns | Null check     |
| `abs(-42)`     | 6.05 ns | Absolute value |
| `sum([...])`   | 7.25 ns | Array sum      |
| `min(a,b,c)`   | 10.1 ns | Minimum        |
| `max(a,b,c)`   | 9.79 ns | Maximum        |
| `first([...])` | 20.8 ns | First element  |
| `last([...])`  | 20.8 ns | Last element   |
| `avg([...])`   | 21.3 ns | Average        |
| `type(x)`      | 43.1 ns | Type check     |
| `upper(str)`   | 81.9 ns | To uppercase   |
| `lower(str)`   | 82.9 ns | To lowercase   |

**Key Insight**:

- Common functions (`len`, `is_null`, `abs`) are all under 10ns
- Global singleton `FunctionRegistry` avoids repeated initialization

### 5. Initialization Overhead

One-time component creation overhead.

| Component                 | Time        | Description                |
| ------------------------- | ----------- | -------------------------- |
| `FunctionRegistry::new()` | **4.49 ns** | Global singleton reference |
| `Evaluator::new()`        | 4.89 ns     | Lightweight                |
| `RuleExecutor::new()`     | 36.5 ns     | Includes Evaluator         |
| `Context::from_json()`    | 404 ns      | JSON parsing               |
| `RuleSet::compile()`      | 1.65 µs     | Pre-compile expressions    |

**Key Insight**:

- `FunctionRegistry::new()` optimized to ~4ns (previously 100+ns for initializing all built-ins)
- Compilation overhead (1.65 µs) is one-time, no repetition at execution

### 6. Batch Throughput

Large-scale batch execution performance.

| Batch Size        | Total Time | Throughput         |
| ----------------- | ---------- | ------------------ |
| 1,000 executions  | 352 µs     | **2.84 M ops/sec** |
| 10,000 executions | 3.58 ms    | **2.79 M ops/sec** |

**Key Insight**:

- Stable throughput of ~2.8 million operations per second
- Linear scaling with no significant performance degradation

### 7. Complexity Scaling

Performance changes with increasing branch count.

| Branches | Execution Time | Per-branch Overhead |
| -------- | -------------- | ------------------- |
| 5        | 642 ns         | 128 ns/branch       |
| 10       | 980 ns         | 98 ns/branch        |
| 20       | 1.65 µs        | 82 ns/branch        |
| 50       | 2.08 µs        | 42 ns/branch        |

**Key Insight**:

- Branch evaluation has sublinear scaling characteristics
- A 50-branch rule only takes ~2µs

## Optimization Summary (Measured Data)

### Initialization Optimization (Most Important)

| Component               | Before      | After       | Improvement |
| ----------------------- | ----------- | ----------- | ----------- |
| FunctionRegistry::new() | **3.87 µs** | **4.49 ns** | **862x ↑**  |
| Evaluator::new()        | **3.59 µs** | **4.89 ns** | **734x ↑**  |
| RuleExecutor::new()     | **3.76 µs** | **36.5 ns** | **103x ↑**  |

### Rule Execution Optimization

| Test Item              | Before  | After   | Change |
| ---------------------- | ------- | ------- | ------ |
| Simple rule execution  | 1.82 µs | 1.81 µs | -0.7%  |
| Complex rule execution | 3.53 µs | 3.50 µs | -1.0%  |
| Throughput (1K batch)  | 1.82 ms | 1.81 ms | -0.6%  |

### Built-in Function Optimization

| Function | Before    | After    | Change    |
| -------- | --------- | -------- | --------- |
| upper()  | 103.56 ns | 98.62 ns | **-4.8%** |
| abs()    | 7.21 ns   | 7.00 ns  | **-2.9%** |
| avg()    | 23.69 ns  | 24.16 ns | +2.0%     |
| sum()    | 7.97 ns   | 8.33 ns  | +4.5%     |

### Optimization Techniques Summary

| Technique                          | Description                                       | Impact                      |
| ---------------------------------- | ------------------------------------------------- | --------------------------- |
| Global Singleton FunctionRegistry  | Lazy initialization with `OnceLock`               | 862x initialization speedup |
| `Cow<'static, str>` Error Messages | Avoid heap allocation for static error messages   | Reduced allocations         |
| Fast-path Function Dispatch        | Common functions bypass HashMap lookup            | ~5% function call speedup   |
| ExecutionOptions                   | Runtime parameter override, avoid RuleSet cloning | Zero-copy batch execution   |

## Running Benchmarks

```bash
# Run complete benchmark suite
cargo bench --bench unified_bench

# Run specific test group
cargo bench --bench unified_bench -- "rule_execution"

# Save baseline
cargo bench --bench unified_bench -- --save-baseline my_baseline

# Compare with baseline
cargo bench --bench unified_bench -- --baseline my_baseline

# Use report generation script
./scripts/bench-report.sh all
```

## Recommended Production Configuration

```rust
// 1. Pre-compile ruleset (required)
let mut ruleset = RuleSet::from_json(&json)?;
ruleset.compile()?;  // Pre-compile all expressions

// 2. Reuse executor (recommended)
let executor = RuleExecutor::new();  // Create once, use many times

// 3. Use ExecutionOptions for runtime overrides
let options = ExecutionOptions {
    timeout_ms: Some(1000),
    enable_trace: false,
    ..Default::default()
};
let result = executor.execute_with_options(&ruleset, input, Some(&options));

// 4. For ultimate performance, use binary compilation
let compiled = RuleSetCompiler::compile(&ruleset)?;
let binary_executor = CompiledRuleExecutor::new();
let result = binary_executor.execute(&compiled, input);
```

## Test Environment

- **Platform**: macOS (Apple Silicon)
- **Rust Version**: stable
- **Benchmark Framework**: criterion

---

_Report generated by Ordo benchmark suite_
