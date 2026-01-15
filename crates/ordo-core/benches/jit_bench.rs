//! JIT Compilation Benchmarks
//!
//! Compares performance of:
//! - Tree-walking interpreter
//! - Bytecode VM
//! - JIT-enabled evaluator
//! - Pure JIT (simulated)

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ordo_core::context::{Context, Value};
use ordo_core::expr::{
    BinaryOp, BytecodeVM, Evaluator, Expr, ExprCompiler, ExprOptimizer, JITEvaluator, Profiler,
};
use std::time::Duration;

/// Create a simple arithmetic expression: (a + b) * c
fn simple_arithmetic() -> Expr {
    Expr::Binary {
        left: Box::new(Expr::Binary {
            left: Box::new(Expr::Literal(Value::Float(10.0))),
            op: BinaryOp::Add,
            right: Box::new(Expr::Literal(Value::Float(20.0))),
        }),
        op: BinaryOp::Mul,
        right: Box::new(Expr::Literal(Value::Float(3.0))),
    }
}

/// Create a complex nested expression with many operations
fn complex_expression() -> Expr {
    // ((a + b) * c - d) / e + f * g - h
    Expr::Binary {
        left: Box::new(Expr::Binary {
            left: Box::new(Expr::Binary {
                left: Box::new(Expr::Binary {
                    left: Box::new(Expr::Binary {
                        left: Box::new(Expr::Literal(Value::Float(1.0))),
                        op: BinaryOp::Add,
                        right: Box::new(Expr::Literal(Value::Float(2.0))),
                    }),
                    op: BinaryOp::Mul,
                    right: Box::new(Expr::Literal(Value::Float(3.0))),
                }),
                op: BinaryOp::Sub,
                right: Box::new(Expr::Literal(Value::Float(4.0))),
            }),
            op: BinaryOp::Div,
            right: Box::new(Expr::Literal(Value::Float(5.0))),
        }),
        op: BinaryOp::Add,
        right: Box::new(Expr::Binary {
            left: Box::new(Expr::Binary {
                left: Box::new(Expr::Literal(Value::Float(6.0))),
                op: BinaryOp::Mul,
                right: Box::new(Expr::Literal(Value::Float(7.0))),
            }),
            op: BinaryOp::Sub,
            right: Box::new(Expr::Literal(Value::Float(8.0))),
        }),
    }
}

/// Create an expression with comparisons and logic
fn comparison_expression() -> Expr {
    // (a > 10 && b < 20) || c == 30
    Expr::Binary {
        left: Box::new(Expr::Binary {
            left: Box::new(Expr::Binary {
                left: Box::new(Expr::Literal(Value::Float(15.0))),
                op: BinaryOp::Gt,
                right: Box::new(Expr::Literal(Value::Float(10.0))),
            }),
            op: BinaryOp::And,
            right: Box::new(Expr::Binary {
                left: Box::new(Expr::Literal(Value::Float(15.0))),
                op: BinaryOp::Lt,
                right: Box::new(Expr::Literal(Value::Float(20.0))),
            }),
        }),
        op: BinaryOp::Or,
        right: Box::new(Expr::Binary {
            left: Box::new(Expr::Literal(Value::Float(25.0))),
            op: BinaryOp::Eq,
            right: Box::new(Expr::Literal(Value::Float(30.0))),
        }),
    }
}

/// Benchmark tree-walking evaluator
fn bench_tree_walking(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit/tree_walking");
    group.measurement_time(Duration::from_secs(3));

    let evaluator = Evaluator::new();
    let ctx = Context::new(Value::Null);

    for (name, expr) in [
        ("simple", simple_arithmetic()),
        ("complex", complex_expression()),
        ("comparison", comparison_expression()),
    ] {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::new("eval", name), &expr, |b, expr| {
            b.iter(|| evaluator.eval(expr, &ctx).unwrap());
        });
    }

    group.finish();
}

/// Benchmark bytecode VM
fn bench_bytecode_vm(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit/bytecode_vm");
    group.measurement_time(Duration::from_secs(3));

    let vm = BytecodeVM::new();
    let ctx = Context::new(Value::Null);

    for (name, expr) in [
        ("simple", simple_arithmetic()),
        ("complex", complex_expression()),
        ("comparison", comparison_expression()),
    ] {
        let compiler = ExprCompiler::new();
        let compiled = compiler.compile(&expr);

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("execute", name),
            &compiled,
            |b, compiled| {
                b.iter(|| vm.execute(compiled, &ctx).unwrap());
            },
        );
    }

    group.finish();
}

/// Benchmark optimized bytecode VM (with constant folding)
fn bench_optimized_vm(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit/optimized_vm");
    group.measurement_time(Duration::from_secs(3));

    let vm = BytecodeVM::new();
    let mut optimizer = ExprOptimizer::new();
    let ctx = Context::new(Value::Null);

    for (name, expr) in [
        ("simple", simple_arithmetic()),
        ("complex", complex_expression()),
        ("comparison", comparison_expression()),
    ] {
        let optimized = optimizer.optimize(expr);
        let compiler = ExprCompiler::new();
        let compiled = compiler.compile(&optimized);

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("execute", name),
            &compiled,
            |b, compiled| {
                b.iter(|| vm.execute(compiled, &ctx).unwrap());
            },
        );
    }

    group.finish();
}

/// Benchmark JIT-enabled evaluator
fn bench_jit_evaluator(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit/jit_evaluator");
    group.measurement_time(Duration::from_secs(3));

    let evaluator = JITEvaluator::simple();
    let ctx = Context::new(Value::Null);

    for (name, expr) in [
        ("simple", simple_arithmetic()),
        ("complex", complex_expression()),
        ("comparison", comparison_expression()),
    ] {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::new("eval", name), &expr, |b, expr| {
            b.iter(|| evaluator.eval(expr, &ctx).unwrap());
        });
    }

    group.finish();
}

/// Benchmark compilation overhead
fn bench_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit/compilation");
    group.measurement_time(Duration::from_secs(5));

    for (name, expr) in [
        ("simple", simple_arithmetic()),
        ("complex", complex_expression()),
        ("comparison", comparison_expression()),
    ] {
        // Bytecode compilation
        group.bench_with_input(
            BenchmarkId::new("bytecode_compile", name),
            &expr,
            |b, expr| {
                b.iter(|| {
                    let compiler = ExprCompiler::new();
                    compiler.compile(expr)
                });
            },
        );

        // Optimization overhead
        group.bench_with_input(BenchmarkId::new("optimize", name), &expr, |b, expr| {
            b.iter(|| {
                let mut optimizer = ExprOptimizer::new();
                optimizer.optimize(expr.clone())
            });
        });
    }

    group.finish();
}

/// Benchmark profiler overhead
fn bench_profiler(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit/profiler");
    group.measurement_time(Duration::from_secs(3));

    let profiler = Profiler::new();

    group.bench_function("record_execution", |b| {
        let hash = 12345u64;
        let duration = Duration::from_nanos(100);
        b.iter(|| {
            profiler.record_expr(hash, duration);
        });
    });

    group.bench_function("check_should_jit", |b| {
        let hash = 12345u64;
        // Pre-populate some data
        for _ in 0..100 {
            profiler.record_expr(hash, Duration::from_nanos(100));
        }
        b.iter(|| profiler.should_jit_expr(hash));
    });

    group.finish();
}

/// Benchmark batch execution throughput
fn bench_batch_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit/batch_throughput");
    group.measurement_time(Duration::from_secs(5));

    let evaluator = JITEvaluator::simple();
    let ctx = Context::new(Value::Null);
    let expr = complex_expression();

    for batch_size in [100, 1000, 10000] {
        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("execute", batch_size),
            &batch_size,
            |b, &size| {
                b.iter(|| {
                    for _ in 0..size {
                        evaluator.eval(&expr, &ctx).unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_tree_walking,
    bench_bytecode_vm,
    bench_optimized_vm,
    bench_jit_evaluator,
    bench_compilation,
    bench_profiler,
    bench_batch_throughput,
);
criterion_main!(benches);
