//! Unified Ordo Performance Benchmark Suite
//!
//! This benchmark suite provides comprehensive performance measurements for the Ordo rule engine.
//! It covers all critical paths and generates comparable metrics for optimization tracking.
//!
//! ## Benchmark Categories
//!
//! 1. **Expression Parsing** - Measures expression string → AST conversion speed
//! 2. **Expression Evaluation** - Measures AST evaluation speed with different complexity
//! 3. **Rule Execution** - Measures end-to-end rule execution performance
//! 4. **Function Calls** - Measures built-in function performance (fast-path optimized)
//! 5. **Initialization** - Measures component initialization overhead
//! 6. **Memory** - Measures memory allocation patterns
//!
//! ## Running Benchmarks
//!
//! ```bash
//! # Run all benchmarks and save baseline
//! cargo bench --bench unified_bench -- --save-baseline current
//!
//! # Compare with previous baseline
//! cargo bench --bench unified_bench -- --baseline previous
//!
//! # Run specific benchmark group
//! cargo bench --bench unified_bench -- "rule_execution"
//! ```

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ordo_core::prelude::*;
use std::hint::black_box;
use std::time::Duration;

// =============================================================================
// Test Data Factories
// =============================================================================

/// Create a minimal ruleset (1 decision, 2 terminals)
fn create_minimal_ruleset() -> RuleSet {
    let mut ruleset = RuleSet::new("bench_minimal", "start");
    ruleset.add_step(
        Step::decision("start", "Start")
            .branch(Condition::from_string("value > 50"), "high")
            .default("low")
            .build(),
    );
    ruleset.add_step(Step::terminal("high", "High", TerminalResult::new("HIGH")));
    ruleset.add_step(Step::terminal("low", "Low", TerminalResult::new("LOW")));
    ruleset
}

/// Create a compiled minimal ruleset (pre-compiled expressions)
fn create_minimal_ruleset_compiled() -> RuleSet {
    let mut ruleset = create_minimal_ruleset();
    ruleset.compile().unwrap();
    ruleset
}

/// Create a medium complexity ruleset (3 decisions, 6 terminals)
fn create_medium_ruleset() -> RuleSet {
    let mut ruleset = RuleSet::new("bench_medium", "check_amount");

    ruleset.add_step(
        Step::decision("check_amount", "Check Amount")
            .branch(Condition::from_string("amount >= 10000"), "vip_check")
            .branch(Condition::from_string("amount >= 1000"), "standard_check")
            .default("reject")
            .build(),
    );

    ruleset.add_step(
        Step::decision("vip_check", "VIP Check")
            .branch(Condition::from_string("user.level == \"gold\""), "gold")
            .branch(Condition::from_string("user.level == \"silver\""), "silver")
            .default("vip")
            .build(),
    );

    ruleset.add_step(
        Step::decision("standard_check", "Standard Check")
            .branch(Condition::from_string("user.is_member == true"), "member")
            .default("normal")
            .build(),
    );

    // Terminal steps
    for (id, code, discount) in [
        ("gold", "GOLD", 0.25),
        ("silver", "SILVER", 0.20),
        ("vip", "VIP", 0.15),
        ("member", "MEMBER", 0.05),
        ("normal", "NORMAL", 0.0),
        ("reject", "REJECT", 0.0),
    ] {
        ruleset.add_step(Step::terminal(
            id,
            id,
            TerminalResult::new(code).with_output("discount", Expr::literal(discount)),
        ));
    }

    ruleset
}

/// Create a compiled medium ruleset
fn create_medium_ruleset_compiled() -> RuleSet {
    let mut ruleset = create_medium_ruleset();
    ruleset.compile().unwrap();
    ruleset
}

/// Create a complex ruleset with many branches (stress test)
fn create_complex_ruleset(branch_count: usize) -> RuleSet {
    let mut ruleset = RuleSet::new("bench_complex", "start");

    let mut step = Step::decision("start", "Start");
    for i in 0..branch_count {
        step = step.branch(
            Condition::from_string(format!("value == {}", i)),
            format!("result_{}", i),
        );
    }
    ruleset.add_step(step.default("default").build());

    for i in 0..branch_count {
        ruleset.add_step(Step::terminal(
            format!("result_{}", i),
            format!("Result {}", i),
            TerminalResult::new(format!("R{}", i)),
        ));
    }
    ruleset.add_step(Step::terminal(
        "default",
        "Default",
        TerminalResult::new("DEFAULT"),
    ));

    ruleset
}

/// Standard test context with various data types
fn create_test_context() -> Context {
    Context::from_json(
        r#"{
        "value": 75,
        "amount": 15000,
        "user": {
            "level": "gold",
            "age": 35,
            "is_member": true,
            "name": "Alice"
        },
        "items": [10, 20, 30, 40, 50],
        "tags": ["premium", "verified", "active"],
        "metadata": {
            "source": "api",
            "version": "2.0"
        }
    }"#,
    )
    .unwrap()
}

// =============================================================================
// Benchmark: Expression Parsing
// =============================================================================

fn bench_expression_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("01_expression_parsing");
    group.measurement_time(Duration::from_secs(5));

    let expressions = [
        ("simple_field", "age > 18"),
        ("field_eq_string", "status == \"active\""),
        ("logical_and", "age > 18 && status == \"active\""),
        ("logical_or", "age < 13 || age > 65"),
        ("nested_field", "user.profile.level == \"gold\""),
        ("in_operator", "status in [\"active\", \"pending\"]"),
        ("function_len", "len(items) > 0"),
        ("function_sum", "sum(values) >= 100"),
        ("arithmetic", "price * quantity * (1 - discount)"),
        ("conditional", "if premium then price * 0.9 else price"),
        ("coalesce", "coalesce(appid, in_appid, \"default\")"),
        (
            "complex",
            "amount >= 1000 && user.level in [\"gold\", \"silver\"] && !blocked",
        ),
    ];

    for (name, expr) in expressions {
        group.bench_with_input(BenchmarkId::from_parameter(name), expr, |b, expr| {
            b.iter(|| ExprParser::parse(black_box(expr)))
        });
    }

    group.finish();
}

// =============================================================================
// Benchmark: Expression Evaluation
// =============================================================================

fn bench_expression_evaluation(c: &mut Criterion) {
    let mut group = c.benchmark_group("02_expression_evaluation");
    group.measurement_time(Duration::from_secs(5));

    let evaluator = Evaluator::new();
    let ctx = create_test_context();

    let expressions = [
        ("field_compare", ExprParser::parse("value > 50").unwrap()),
        (
            "nested_field",
            ExprParser::parse("user.level == \"gold\"").unwrap(),
        ),
        (
            "logical_and",
            ExprParser::parse("value > 50 && user.is_member == true").unwrap(),
        ),
        (
            "logical_or",
            ExprParser::parse("value < 10 || value > 100").unwrap(),
        ),
        ("arithmetic", ExprParser::parse("amount * 0.1").unwrap()),
        ("function_len", ExprParser::parse("len(items)").unwrap()),
        ("function_sum", ExprParser::parse("sum(items)").unwrap()),
        (
            "conditional",
            ExprParser::parse("if user.is_member then 0.1 else 0.0").unwrap(),
        ),
        (
            "array_in",
            ExprParser::parse("\"premium\" in tags").unwrap(),
        ),
    ];

    for (name, expr) in expressions {
        group.bench_with_input(BenchmarkId::from_parameter(name), &expr, |b, expr| {
            b.iter(|| evaluator.eval(black_box(expr), black_box(&ctx)))
        });
    }

    group.finish();
}

// =============================================================================
// Benchmark: Rule Execution (Interpreted)
// =============================================================================

fn bench_rule_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("03_rule_execution");
    group.measurement_time(Duration::from_secs(5));

    let executor = RuleExecutor::new();

    // Minimal ruleset - uncompiled
    let minimal = create_minimal_ruleset();
    let input_minimal: Value = serde_json::from_str(r#"{"value": 75}"#).unwrap();

    group.bench_function("minimal_uncompiled", |b| {
        b.iter(|| executor.execute(black_box(&minimal), black_box(input_minimal.clone())))
    });

    // Minimal ruleset - compiled
    let minimal_compiled = create_minimal_ruleset_compiled();

    group.bench_function("minimal_compiled", |b| {
        b.iter(|| {
            executor.execute(
                black_box(&minimal_compiled),
                black_box(input_minimal.clone()),
            )
        })
    });

    // Medium ruleset - uncompiled
    let medium = create_medium_ruleset();
    let input_medium: Value =
        serde_json::from_str(r#"{"amount": 15000, "user": {"level": "gold", "is_member": true}}"#)
            .unwrap();

    group.bench_function("medium_uncompiled", |b| {
        b.iter(|| executor.execute(black_box(&medium), black_box(input_medium.clone())))
    });

    // Medium ruleset - compiled
    let medium_compiled = create_medium_ruleset_compiled();

    group.bench_function("medium_compiled", |b| {
        b.iter(|| executor.execute(black_box(&medium_compiled), black_box(input_medium.clone())))
    });

    group.finish();
}

// =============================================================================
// Benchmark: Compiled Rule Execution
// =============================================================================

fn bench_compiled_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("04_compiled_execution");
    group.measurement_time(Duration::from_secs(5));

    // Compile ruleset to binary format
    let ruleset = create_medium_ruleset();
    let compiled = RuleSetCompiler::compile(&ruleset).unwrap();
    let executor = CompiledRuleExecutor::new();

    let input: Value =
        serde_json::from_str(r#"{"amount": 15000, "user": {"level": "gold", "is_member": true}}"#)
            .unwrap();

    group.bench_function("compiled_binary", |b| {
        b.iter(|| executor.execute(black_box(&compiled), black_box(input.clone())))
    });

    group.finish();
}

// =============================================================================
// Benchmark: Built-in Functions (Fast Path)
// =============================================================================

fn bench_builtin_functions(c: &mut Criterion) {
    let mut group = c.benchmark_group("05_builtin_functions");
    group.measurement_time(Duration::from_secs(3));

    let registry = FunctionRegistry::new();

    // String operations
    let string_val = Value::string("Hello, World! This is a test string.");
    group.bench_function("len_string", |b| {
        b.iter(|| registry.call("len", black_box(std::slice::from_ref(&string_val))))
    });

    group.bench_function("upper", |b| {
        b.iter(|| registry.call("upper", black_box(std::slice::from_ref(&string_val))))
    });

    group.bench_function("lower", |b| {
        b.iter(|| registry.call("lower", black_box(std::slice::from_ref(&string_val))))
    });

    // Array operations
    let array_val = Value::array(vec![
        Value::int(10),
        Value::int(20),
        Value::int(30),
        Value::int(40),
        Value::int(50),
    ]);

    group.bench_function("len_array", |b| {
        b.iter(|| registry.call("len", black_box(std::slice::from_ref(&array_val))))
    });

    group.bench_function("sum", |b| {
        b.iter(|| registry.call("sum", black_box(std::slice::from_ref(&array_val))))
    });

    group.bench_function("avg", |b| {
        b.iter(|| registry.call("avg", black_box(std::slice::from_ref(&array_val))))
    });

    group.bench_function("first", |b| {
        b.iter(|| registry.call("first", black_box(std::slice::from_ref(&array_val))))
    });

    group.bench_function("last", |b| {
        b.iter(|| registry.call("last", black_box(std::slice::from_ref(&array_val))))
    });

    // Math operations (fast path optimized)
    group.bench_function("abs", |b| {
        b.iter(|| registry.call("abs", black_box(&[Value::int(-42)])))
    });

    group.bench_function("min", |b| {
        b.iter(|| {
            registry.call(
                "min",
                black_box(&[Value::int(10), Value::int(5), Value::int(20)]),
            )
        })
    });

    group.bench_function("max", |b| {
        b.iter(|| {
            registry.call(
                "max",
                black_box(&[Value::int(10), Value::int(5), Value::int(20)]),
            )
        })
    });

    // Type checking
    group.bench_function("is_null", |b| {
        b.iter(|| registry.call("is_null", black_box(&[Value::Null])))
    });

    group.bench_function("type", |b| {
        b.iter(|| registry.call("type", black_box(&[Value::int(42)])))
    });

    group.finish();
}

// =============================================================================
// Benchmark: Initialization Overhead
// =============================================================================

fn bench_initialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("06_initialization");
    group.measurement_time(Duration::from_secs(3));

    // FunctionRegistry creation (should be fast with global singleton)
    group.bench_function("function_registry_new", |b| {
        b.iter(|| black_box(FunctionRegistry::new()))
    });

    // Evaluator creation
    group.bench_function("evaluator_new", |b| b.iter(|| black_box(Evaluator::new())));

    // RuleExecutor creation
    group.bench_function("rule_executor_new", |b| {
        b.iter(|| black_box(RuleExecutor::new()))
    });

    // Context creation from JSON
    let json = r#"{"value": 42, "user": {"name": "test"}}"#;
    group.bench_function("context_from_json", |b| {
        b.iter(|| Context::from_json(black_box(json)))
    });

    // RuleSet creation and compilation
    group.bench_function("ruleset_compile", |b| {
        b.iter(|| {
            let mut rs = create_minimal_ruleset();
            rs.compile().unwrap();
            black_box(rs)
        })
    });

    group.finish();
}

// =============================================================================
// Benchmark: Throughput (Batch Processing)
// =============================================================================

fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("07_throughput");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(50);

    let executor = RuleExecutor::new();
    let mut ruleset = create_minimal_ruleset();
    ruleset.compile().unwrap();

    // Prepare batch inputs
    let batch_1k: Vec<Value> = (0..1000)
        .map(|i| serde_json::from_str(&format!(r#"{{"value": {}}}"#, i % 100)).unwrap())
        .collect();

    let batch_10k: Vec<Value> = (0..10000)
        .map(|i| serde_json::from_str(&format!(r#"{{"value": {}}}"#, i % 100)).unwrap())
        .collect();

    // 1K executions
    group.throughput(Throughput::Elements(1000));
    group.bench_function("batch_1k", |b| {
        b.iter(|| {
            for input in &batch_1k {
                let _ = executor.execute(black_box(&ruleset), black_box(input.clone()));
            }
        })
    });

    // 10K executions
    group.throughput(Throughput::Elements(10000));
    group.bench_function("batch_10k", |b| {
        b.iter(|| {
            for input in &batch_10k {
                let _ = executor.execute(black_box(&ruleset), black_box(input.clone()));
            }
        })
    });

    group.finish();
}

// =============================================================================
// Benchmark: Complexity Scaling
// =============================================================================

fn bench_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("08_scaling");
    group.measurement_time(Duration::from_secs(5));

    let executor = RuleExecutor::new();

    for branch_count in [5, 10, 20, 50] {
        let mut ruleset = create_complex_ruleset(branch_count);
        ruleset.compile().unwrap();
        let input: Value = serde_json::from_str(r#"{"value": 25}"#).unwrap();

        group.bench_with_input(
            BenchmarkId::new("branches", branch_count),
            &(ruleset, input),
            |b, (ruleset, input)| {
                b.iter(|| executor.execute(black_box(ruleset), black_box(input.clone())))
            },
        );
    }

    group.finish();
}

// =============================================================================
// Criterion Configuration
// =============================================================================

criterion_group! {
    name = benches;
    config = Criterion::default()
        .significance_level(0.05)
        .noise_threshold(0.02)
        .warm_up_time(Duration::from_secs(2));
    targets =
        bench_expression_parsing,
        bench_expression_evaluation,
        bench_rule_execution,
        bench_compiled_execution,
        bench_builtin_functions,
        bench_initialization,
        bench_throughput,
        bench_scaling,
}

criterion_main!(benches);
