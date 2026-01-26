---
name: ordo-testing
description: Ordo testing and benchmarking guide. Includes unit tests, integration tests, Criterion benchmarks, k6 load tests, CI configuration. Use for writing tests, performance analysis, continuous integration.
---

# Ordo Testing and Benchmarking

## Unit Tests

### Running Tests

```bash
# Run all tests
cargo test

# Run specific crate
cargo test --package ordo-core
cargo test --package ordo-server

# Run specific test
cargo test test_full_workflow

# Show output
cargo test -- --nocapture

# Parallel control
cargo test -- --test-threads=4
```

### Test Example

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ordo_core::prelude::*;

    #[test]
    fn test_rule_execution() {
        let mut ruleset = RuleSet::new("test", "start");
        
        ruleset.add_step(
            Step::decision("start", "Start")
                .branch(Condition::from_string("value > 10"), "high")
                .default("low")
                .build()
        );
        
        ruleset.add_step(Step::terminal("high", "High", 
            TerminalResult::new("HIGH")));
        ruleset.add_step(Step::terminal("low", "Low", 
            TerminalResult::new("LOW")));

        let executor = RuleExecutor::new();
        
        // Test high value
        let input: Value = serde_json::from_str(r#"{"value": 20}"#).unwrap();
        let result = executor.execute(&ruleset, input).unwrap();
        assert_eq!(result.code, "HIGH");
        
        // Test low value
        let input: Value = serde_json::from_str(r#"{"value": 5}"#).unwrap();
        let result = executor.execute(&ruleset, input).unwrap();
        assert_eq!(result.code, "LOW");
    }

    #[test]
    fn test_expression_eval() {
        let evaluator = Evaluator::new();
        let parser = ExprParser::new();
        
        let expr = parser.parse("age >= 18 && status == \"active\"").unwrap();
        let ctx = Context::from_json(r#"{"age": 25, "status": "active"}"#).unwrap();
        let result = evaluator.eval(&expr, &ctx).unwrap();
        
        assert_eq!(result, Value::Bool(true));
    }
}
```

## Integration Tests

### gRPC Tests

```bash
cargo test --package ordo-server --test grpc_test
```

### UDS Tests

```bash
cargo test --package ordo-server --test uds_test
```

## Benchmarks

### Running Criterion Benchmarks

```bash
# All benchmarks
cargo bench --package ordo-core

# Specific benchmarks
cargo bench --package ordo-core --bench engine_bench
cargo bench --package ordo-core --bench jit_comparison_bench
cargo bench --package ordo-core --bench schema_jit_bench
cargo bench --package ordo-core --bench optimization_bench
```

### Benchmark Files

| File | Description |
|------|-------------|
| `engine_bench.rs` | Overall engine performance |
| `jit_comparison_bench.rs` | JIT vs interpreter comparison |
| `jit_realistic_bench.rs` | Realistic JIT scenarios |
| `jit_stress_bench.rs` | JIT stress testing |
| `schema_jit_bench.rs` | Schema JIT performance |
| `optimization_bench.rs` | Optimization effectiveness |

### Writing Benchmarks

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ordo_core::prelude::*;

fn bench_rule_execution(c: &mut Criterion) {
    let mut ruleset = RuleSet::new("bench", "start");
    // ... setup rules
    
    let executor = RuleExecutor::new();
    let input: Value = serde_json::from_str(r#"{"value": 100}"#).unwrap();
    
    c.bench_function("rule_execution", |b| {
        b.iter(|| {
            executor.execute(&ruleset, black_box(input.clone()))
        })
    });
}

criterion_group!(benches, bench_rule_execution);
criterion_main!(benches);
```

## k6 Load Testing

### Running Load Tests

```bash
# Basic test
./scripts/benchmark.sh

# Custom parameters
./scripts/benchmark.sh \
    -h localhost \
    -p 8080 \
    -d 60s \
    -u 50 \
    -r 500 \
    --rule order_discount \
    --analyze
```

### Parameter Reference

| Parameter | Description | Default |
|-----------|-------------|---------|
| `-h, --host` | Server address | localhost |
| `-p, --port` | Port | 8080 |
| `-d, --duration` | Test duration | 30s |
| `-u, --vus` | Virtual users | 10 |
| `-r, --rps` | Target RPS | 100 |
| `--rule` | Rule name to test | order_discount |
| `--analyze` | Show CPU analysis | false |

### Custom k6 Script

```javascript
import http from 'k6/http';
import { check } from 'k6';

export const options = {
    scenarios: {
        constant_load: {
            executor: 'constant-arrival-rate',
            rate: 100,
            timeUnit: '1s',
            duration: '30s',
            preAllocatedVUs: 10,
        },
    },
    thresholds: {
        http_req_duration: ['p(95)<500', 'p(99)<1000'],
    },
};

export default function() {
    const url = 'http://localhost:8080/api/v1/execute/my-rule';
    const payload = JSON.stringify({
        input: { value: Math.random() * 1000 }
    });
    
    const response = http.post(url, payload, {
        headers: { 'Content-Type': 'application/json' },
    });
    
    check(response, {
        'status is 200': (r) => r.status === 200,
        'response time < 100ms': (r) => r.timings.duration < 100,
    });
}
```

## CI Configuration

### GitHub Actions

Key workflow file: `.github/workflows/ci.yml`

```yaml
# Test job
test:
  runs-on: ${{ matrix.os }}
  strategy:
    matrix:
      os: [ubuntu-latest, macos-latest, windows-latest]
      rust: [stable, beta]
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@master
      with:
        toolchain: ${{ matrix.rust }}
    - run: cargo test --all-features
```

### Local CI Simulation

```bash
# Format check
cargo fmt --all -- --check

# Clippy check
cargo clippy --all-targets --all-features -- -D warnings

# Test
cargo test --all-features

# Build
cargo build --release
```

## Test Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out Html --output-dir coverage/

# Or use llvm-cov
cargo install cargo-llvm-cov
cargo llvm-cov --html
```

## Frontend Testing

```bash
cd ordo-editor

# Run tests
pnpm test

# Run specific package tests
pnpm --filter @ordo-engine/editor-core test
```

## Key Files

- `crates/ordo-core/benches/` - Rust benchmarks
- `crates/ordo-server/tests/` - Server integration tests
- `scripts/benchmark.sh` - k6 load test script
- `.github/workflows/ci.yml` - CI configuration
- `ordo-editor/packages/core/src/engine/__tests__/` - Frontend tests
