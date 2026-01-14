//! Expression optimization benchmarks
//!
//! Compares performance of different expression evaluation strategies:
//! 1. Baseline: Tree-walking interpreter (Evaluator)
//! 2. Optimized AST: Constant folding + algebraic simplification
//! 3. Bytecode VM: Compiled bytecode execution
//! 4. Vectorized: Batch execution with bytecode VM
//!
//! This benchmark generates data for the academic research report.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ordo_core::prelude::*;
use std::hint::black_box;

// ============================================================================
// Test Expressions
// ============================================================================

/// Simple comparison: age > 18
fn expr_simple_compare() -> Expr {
    Expr::binary(BinaryOp::Gt, Expr::field("age"), Expr::literal(18))
}

/// Constant-heavy expression: price * (1 - 0.2) + 10
fn expr_constant_heavy() -> Expr {
    let discount = Expr::binary(BinaryOp::Sub, Expr::literal(1.0f64), Expr::literal(0.2f64));
    let discounted = Expr::binary(BinaryOp::Mul, Expr::field("price"), discount);
    Expr::binary(BinaryOp::Add, discounted, Expr::literal(10))
}

/// Logical expression: (age > 18 && status == "active") || vip == true
fn expr_logical() -> Expr {
    Expr::binary(
        BinaryOp::Or,
        Expr::binary(
            BinaryOp::And,
            Expr::binary(BinaryOp::Gt, Expr::field("age"), Expr::literal(18)),
            Expr::binary(BinaryOp::Eq, Expr::field("status"), Expr::literal("active")),
        ),
        Expr::binary(BinaryOp::Eq, Expr::field("vip"), Expr::literal(true)),
    )
}

/// Function call expression: len(items) > 0 && sum(items) > 100
fn expr_function_call() -> Expr {
    Expr::binary(
        BinaryOp::And,
        Expr::binary(
            BinaryOp::Gt,
            Expr::call("len", vec![Expr::field("items")]),
            Expr::literal(0),
        ),
        Expr::binary(
            BinaryOp::Gt,
            Expr::call("sum", vec![Expr::field("items")]),
            Expr::literal(100),
        ),
    )
}

/// Conditional expression: if premium then price * 0.9 else price
fn expr_conditional() -> Expr {
    Expr::conditional(
        Expr::field("premium"),
        Expr::binary(BinaryOp::Mul, Expr::field("price"), Expr::literal(0.9f64)),
        Expr::field("price"),
    )
}

/// Nested field access: user.profile.level == "gold"
fn expr_nested_field() -> Expr {
    Expr::binary(
        BinaryOp::Eq,
        Expr::field("user.profile.level"),
        Expr::literal("gold"),
    )
}

// ============================================================================
// Test Contexts
// ============================================================================

fn make_context() -> Context {
    Context::from_json(
        r#"{
        "age": 25,
        "status": "active",
        "vip": false,
        "price": 100.0,
        "premium": true,
        "items": [10, 20, 30, 40, 50],
        "user": {
            "profile": {
                "level": "gold"
            }
        }
    }"#,
    )
    .unwrap()
}

fn make_contexts(count: usize) -> Vec<Context> {
    (0..count)
        .map(|i| {
            Context::from_json(&format!(
                r#"{{
                "age": {},
                "status": "{}",
                "vip": {},
                "price": {},
                "premium": {},
                "items": [10, 20, 30, 40, 50],
                "user": {{"profile": {{"level": "{}"}}}}
            }}"#,
                18 + (i % 50),
                if i % 3 == 0 { "active" } else { "inactive" },
                i % 5 == 0,
                100.0 + (i as f64),
                i % 2 == 0,
                if i % 4 == 0 { "gold" } else { "silver" }
            ))
            .unwrap()
        })
        .collect()
}

// ============================================================================
// Benchmark: Baseline vs Optimized AST (Constant Folding)
// ============================================================================

fn bench_constant_folding(c: &mut Criterion) {
    let mut group = c.benchmark_group("constant_folding");

    let evaluator = Evaluator::new();
    let ctx = make_context();

    // Test expressions with varying amounts of constant sub-expressions
    let expressions = vec![
        ("simple_compare", expr_simple_compare()),
        ("constant_heavy", expr_constant_heavy()),
        ("logical", expr_logical()),
        ("conditional", expr_conditional()),
    ];

    for (name, expr) in expressions {
        // Baseline: unoptimized
        group.bench_with_input(BenchmarkId::new("baseline", name), &expr, |b, expr| {
            b.iter(|| evaluator.eval(black_box(expr), black_box(&ctx)))
        });

        // Optimized: with constant folding
        let mut optimizer = ExprOptimizer::new();
        let optimized_expr = optimizer.optimize(expr.clone());

        group.bench_with_input(
            BenchmarkId::new("optimized", name),
            &optimized_expr,
            |b, expr| b.iter(|| evaluator.eval(black_box(expr), black_box(&ctx))),
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark: Tree-Walking vs Bytecode VM
// ============================================================================

fn bench_bytecode_vm(c: &mut Criterion) {
    let mut group = c.benchmark_group("bytecode_vm");

    let evaluator = Evaluator::new();
    let vm = BytecodeVM::new();
    let ctx = make_context();

    let expressions = vec![
        ("simple_compare", expr_simple_compare()),
        ("constant_heavy", expr_constant_heavy()),
        ("logical", expr_logical()),
        ("function_call", expr_function_call()),
        ("conditional", expr_conditional()),
        ("nested_field", expr_nested_field()),
    ];

    for (name, expr) in expressions {
        // Baseline: tree-walking interpreter
        group.bench_with_input(BenchmarkId::new("tree_walking", name), &expr, |b, expr| {
            b.iter(|| evaluator.eval(black_box(expr), black_box(&ctx)))
        });

        // Bytecode VM
        let compiled = ExprCompiler::new().compile(&expr);
        group.bench_with_input(
            BenchmarkId::new("bytecode_vm", name),
            &compiled,
            |b, compiled| b.iter(|| vm.execute(black_box(compiled), black_box(&ctx))),
        );

        // Optimized + Bytecode VM
        let mut optimizer = ExprOptimizer::new();
        let optimized_expr = optimizer.optimize(expr.clone());
        let compiled_optimized = ExprCompiler::new().compile(&optimized_expr);
        group.bench_with_input(
            BenchmarkId::new("optimized_bytecode", name),
            &compiled_optimized,
            |b, compiled| b.iter(|| vm.execute(black_box(compiled), black_box(&ctx))),
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark: Vectorized Batch Execution
// ============================================================================

fn bench_vectorized(c: &mut Criterion) {
    let mut group = c.benchmark_group("vectorized_batch");

    let batch_sizes = vec![10, 100, 1000];
    let expr = expr_logical();

    for batch_size in batch_sizes {
        let contexts = make_contexts(batch_size);

        group.throughput(Throughput::Elements(batch_size as u64));

        // Sequential: tree-walking interpreter
        let evaluator = Evaluator::new();
        group.bench_with_input(
            BenchmarkId::new("sequential_tree", batch_size),
            &contexts,
            |b, contexts| {
                b.iter(|| {
                    contexts
                        .iter()
                        .map(|ctx| evaluator.eval(&expr, ctx))
                        .collect::<Vec<_>>()
                })
            },
        );

        // Sequential: bytecode VM
        let vm = BytecodeVM::new();
        let compiled = ExprCompiler::new().compile(&expr);
        group.bench_with_input(
            BenchmarkId::new("sequential_bytecode", batch_size),
            &contexts,
            |b, contexts| {
                b.iter(|| {
                    contexts
                        .iter()
                        .map(|ctx| vm.execute(&compiled, ctx))
                        .collect::<Vec<_>>()
                })
            },
        );

        // Vectorized: pre-compiled batch execution
        let mut vec_eval = VectorizedEvaluator::new();
        vec_eval.compile(&expr);
        group.bench_with_input(
            BenchmarkId::new("vectorized", batch_size),
            &contexts,
            |b, contexts| b.iter(|| vec_eval.eval_batch(black_box(&expr), black_box(contexts))),
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark: Compilation Overhead
// ============================================================================

fn bench_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("compilation_overhead");

    let expressions = vec![
        ("simple_compare", expr_simple_compare()),
        ("constant_heavy", expr_constant_heavy()),
        ("logical", expr_logical()),
        ("function_call", expr_function_call()),
    ];

    for (name, expr) in expressions {
        // Parsing (already done, so we measure from string)
        let expr_str = match name {
            "simple_compare" => "age > 18",
            "constant_heavy" => "price * (1 - 0.2) + 10",
            "logical" => "(age > 18 && status == \"active\") || vip == true",
            "function_call" => "len(items) > 0 && sum(items) > 100",
            _ => "age > 18",
        };

        group.bench_with_input(BenchmarkId::new("parse", name), &expr_str, |b, s| {
            b.iter(|| ExprParser::parse(black_box(s)))
        });

        // Optimization
        group.bench_with_input(BenchmarkId::new("optimize", name), &expr, |b, expr| {
            b.iter(|| {
                let mut opt = ExprOptimizer::new();
                opt.optimize(black_box(expr.clone()))
            })
        });

        // Bytecode compilation
        group.bench_with_input(
            BenchmarkId::new("compile_bytecode", name),
            &expr,
            |b, expr| b.iter(|| ExprCompiler::new().compile(black_box(expr))),
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark: End-to-End Throughput
// ============================================================================

fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("end_to_end_throughput");
    group.throughput(Throughput::Elements(1000));
    group.sample_size(50);

    let expr = expr_logical();
    let contexts = make_contexts(1000);

    // Baseline: parse + eval each time
    group.bench_function("parse_eval_each", |b| {
        let expr_str = "(age > 18 && status == \"active\") || vip == true";
        let evaluator = Evaluator::new();
        b.iter(|| {
            let expr = ExprParser::parse(expr_str).unwrap();
            contexts
                .iter()
                .map(|ctx| evaluator.eval(&expr, ctx))
                .collect::<Vec<_>>()
        })
    });

    // Pre-parsed: eval only
    let evaluator = Evaluator::new();
    group.bench_function("pre_parsed_eval", |b| {
        b.iter(|| {
            contexts
                .iter()
                .map(|ctx| evaluator.eval(&expr, ctx))
                .collect::<Vec<_>>()
        })
    });

    // Optimized + pre-compiled bytecode
    let mut optimizer = ExprOptimizer::new();
    let optimized = optimizer.optimize(expr.clone());
    let compiled = ExprCompiler::new().compile(&optimized);
    let vm = BytecodeVM::new();
    group.bench_function("optimized_bytecode", |b| {
        b.iter(|| {
            contexts
                .iter()
                .map(|ctx| vm.execute(&compiled, ctx))
                .collect::<Vec<_>>()
        })
    });

    // Vectorized batch
    let mut vec_eval = VectorizedEvaluator::new();
    vec_eval.compile(&optimized);
    group.bench_function("vectorized_batch", |b| {
        b.iter(|| vec_eval.eval_batch(&optimized, &contexts))
    });

    group.finish();
}

// ============================================================================
// Benchmark: Memory Efficiency
// ============================================================================

fn bench_memory_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_efficiency");

    // Compare AST size vs bytecode size
    let expressions = vec![
        ("simple", expr_simple_compare()),
        ("complex", expr_logical()),
        ("function_heavy", expr_function_call()),
    ];

    for (name, expr) in expressions {
        // Measure bytecode compilation stats
        let compiled = ExprCompiler::new().compile(&expr);
        let stats = compiled.stats();

        println!(
            "{}: instructions={}, constants={}, fields={}, functions={}",
            name,
            stats.instruction_count,
            stats.constant_count,
            stats.field_count,
            stats.function_count
        );

        // Benchmark repeated execution (to show memory locality benefits)
        let ctx = make_context();
        let vm = BytecodeVM::new();

        group.bench_with_input(
            BenchmarkId::new("repeated_100", name),
            &compiled,
            |b, compiled| {
                b.iter(|| {
                    for _ in 0..100 {
                        let _ = vm.execute(compiled, &ctx);
                    }
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_constant_folding,
    bench_bytecode_vm,
    bench_vectorized,
    bench_compilation,
    bench_throughput,
    bench_memory_efficiency,
);

criterion_main!(benches);
