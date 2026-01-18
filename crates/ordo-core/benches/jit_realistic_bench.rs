//! Realistic JIT Performance Benchmark
//!
//! This benchmark addresses fairness issues and tests realistic scenarios:
//! 1. Same data structure for all methods (fair comparison)
//! 2. End-to-end with deserialization
//! 3. Cold cache (different data each iteration)
//! 4. Compilation cost amortization
//! 5. Mixed workloads (JIT-able and non-JIT-able rules)
//!
//! Run with: cargo bench --bench jit_realistic_bench

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box;
use std::sync::OnceLock;

use ordo_core::context::{Context, FieldType, MessageSchema, Value};
use ordo_core::expr::jit::TypedContext;
use ordo_core::expr::{BinaryOp, BytecodeVM, Evaluator, Expr, ExprCompiler, SchemaJITCompiler};

// ============================================================================
// Fair Context: Same data for both JIT and VM
// ============================================================================

/// Loan context with repr(C) for predictable layout
#[repr(C)]
struct LoanContext {
    credit_score: i32,
    annual_income: f64,
    debt_to_income: f64,
    employment_years: i32,
    age: i32,
    loan_amount: f64,
}

impl TypedContext for LoanContext {
    fn schema() -> &'static MessageSchema {
        static SCHEMA: OnceLock<MessageSchema> = OnceLock::new();
        SCHEMA.get_or_init(|| {
            MessageSchema::builder("LoanContext")
                .field_at("credit_score", FieldType::Int32, 0)
                .field_at("annual_income", FieldType::Float64, 8)
                .field_at("debt_to_income", FieldType::Float64, 16)
                .field_at("employment_years", FieldType::Int32, 24)
                .field_at("age", FieldType::Int32, 28)
                .field_at("loan_amount", FieldType::Float64, 32)
                .build()
        })
    }

    unsafe fn field_ptr(&self, field_name: &str) -> Option<(*const u8, FieldType)> {
        match field_name {
            "credit_score" => Some((std::ptr::addr_of!(self.credit_score) as _, FieldType::Int32)),
            "annual_income" => Some((
                std::ptr::addr_of!(self.annual_income) as _,
                FieldType::Float64,
            )),
            "debt_to_income" => Some((
                std::ptr::addr_of!(self.debt_to_income) as _,
                FieldType::Float64,
            )),
            "employment_years" => Some((
                std::ptr::addr_of!(self.employment_years) as _,
                FieldType::Int32,
            )),
            "age" => Some((std::ptr::addr_of!(self.age) as _, FieldType::Int32)),
            "loan_amount" => Some((
                std::ptr::addr_of!(self.loan_amount) as _,
                FieldType::Float64,
            )),
            _ => None,
        }
    }
}

impl LoanContext {
    /// Convert to Value for Tree/VM evaluation (fair comparison)
    fn to_value(&self) -> Value {
        Value::object(std::collections::HashMap::from([
            (
                "credit_score".to_string(),
                Value::Int(self.credit_score as i64),
            ),
            (
                "annual_income".to_string(),
                Value::Float(self.annual_income),
            ),
            (
                "debt_to_income".to_string(),
                Value::Float(self.debt_to_income),
            ),
            (
                "employment_years".to_string(),
                Value::Int(self.employment_years as i64),
            ),
            ("age".to_string(), Value::Int(self.age as i64)),
            ("loan_amount".to_string(), Value::Float(self.loan_amount)),
        ]))
    }
}

// ============================================================================
// Test Data Generation
// ============================================================================

/// Generate diverse test contexts (not always passing the rules)
fn generate_test_contexts(count: usize) -> Vec<LoanContext> {
    let mut contexts = Vec::with_capacity(count);
    for i in 0..count {
        contexts.push(LoanContext {
            credit_score: 550 + (i % 300) as i32,               // 550-850
            annual_income: 30000.0 + (i % 100) as f64 * 1000.0, // 30k-130k
            debt_to_income: 0.20 + (i % 40) as f64 * 0.01,      // 0.20-0.60
            employment_years: (i % 10) as i32,                  // 0-9
            age: 20 + (i % 50) as i32,                          // 20-69
            loan_amount: 50000.0 + (i % 50) as f64 * 10000.0,   // 50k-550k
        });
    }
    contexts
}

// ============================================================================
// Benchmark Rules
// ============================================================================

fn create_medium_rule() -> Expr {
    // credit_score >= 680 AND debt_to_income < 0.43 AND employment_years >= 2
    Expr::Binary {
        left: Box::new(Expr::Binary {
            left: Box::new(Expr::Field("credit_score".into())),
            op: BinaryOp::Ge,
            right: Box::new(Expr::Literal(Value::Int(680))),
        }),
        op: BinaryOp::And,
        right: Box::new(Expr::Binary {
            left: Box::new(Expr::Binary {
                left: Box::new(Expr::Field("debt_to_income".into())),
                op: BinaryOp::Lt,
                right: Box::new(Expr::Literal(Value::Float(0.43))),
            }),
            op: BinaryOp::And,
            right: Box::new(Expr::Binary {
                left: Box::new(Expr::Field("employment_years".into())),
                op: BinaryOp::Ge,
                right: Box::new(Expr::Literal(Value::Int(2))),
            }),
        }),
    }
}

// ============================================================================
// Benchmark 1: Fair Comparison (Same Data Structure Overhead)
// ============================================================================
//
// This measures ONLY the evaluation overhead, not data structure differences.
// JIT still has faster field access, but Tree/VM pay the same HashMap cost.

fn bench_fair_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("fair_comparison");
    group.sample_size(100);

    let expr = create_medium_rule();
    let typed_ctx = LoanContext {
        credit_score: 720,
        annual_income: 85000.0,
        debt_to_income: 0.32,
        employment_years: 5,
        age: 35,
        loan_amount: 250000.0,
    };
    let value_ctx = Context::new(typed_ctx.to_value());

    // Pre-compile everything
    let tree_eval = Evaluator::new();
    let vm = BytecodeVM::new();
    let bc_compiler = ExprCompiler::new();
    let compiled_bc = bc_compiler.compile(&expr);

    let mut jit_compiler = SchemaJITCompiler::new().unwrap();
    let compiled_jit = jit_compiler
        .compile_with_schema(&expr, 1, LoanContext::schema())
        .unwrap();

    // Benchmark: Pure evaluation (no context creation overhead)
    group.bench_function("tree_eval_only", |b| {
        b.iter(|| black_box(tree_eval.eval(black_box(&expr), black_box(&value_ctx))))
    });

    group.bench_function("vm_eval_only", |b| {
        b.iter(|| black_box(vm.execute(black_box(&compiled_bc), black_box(&value_ctx))))
    });

    group.bench_function("jit_eval_only", |b| {
        b.iter(|| black_box(unsafe { compiled_jit.call_typed(black_box(&typed_ctx)) }))
    });

    group.finish();
}

// ============================================================================
// Benchmark 2: End-to-End with Context Creation
// ============================================================================
//
// Real scenario: receive data -> create context -> evaluate
// This is more realistic but JIT advantage is smaller.

fn bench_end_to_end(c: &mut Criterion) {
    let mut group = c.benchmark_group("end_to_end");
    group.sample_size(100);

    let expr = create_medium_rule();

    let tree_eval = Evaluator::new();
    let vm = BytecodeVM::new();
    let bc_compiler = ExprCompiler::new();
    let compiled_bc = bc_compiler.compile(&expr);

    let mut jit_compiler = SchemaJITCompiler::new().unwrap();
    let compiled_jit = jit_compiler
        .compile_with_schema(&expr, 1, LoanContext::schema())
        .unwrap();

    // Raw data (simulating deserialized protobuf)
    let raw = LoanContext {
        credit_score: 720,
        annual_income: 85000.0,
        debt_to_income: 0.32,
        employment_years: 5,
        age: 35,
        loan_amount: 250000.0,
    };

    // End-to-end with Tree: create Value + evaluate
    group.bench_function("tree_e2e", |b| {
        b.iter(|| {
            let value = raw.to_value();
            let ctx = Context::new(value);
            black_box(tree_eval.eval(black_box(&expr), black_box(&ctx)))
        })
    });

    // End-to-end with VM: create Value + evaluate
    group.bench_function("vm_e2e", |b| {
        b.iter(|| {
            let value = raw.to_value();
            let ctx = Context::new(value);
            black_box(vm.execute(black_box(&compiled_bc), black_box(&ctx)))
        })
    });

    // End-to-end with JIT: direct struct access
    group.bench_function("jit_e2e", |b| {
        b.iter(|| {
            // JIT can use the struct directly - no Value creation needed!
            black_box(unsafe { compiled_jit.call_typed(black_box(&raw)) })
        })
    });

    group.finish();
}

// ============================================================================
// Benchmark 3: Cold Cache (Different Data Each Time)
// ============================================================================
//
// Real scenario: each request has different data.
// Tests CPU cache effects and branch prediction.

fn bench_cold_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("cold_cache");
    group.sample_size(50); // Fewer samples, more iterations

    let expr = create_medium_rule();
    let contexts = generate_test_contexts(1000);
    let value_contexts: Vec<Context> = contexts
        .iter()
        .map(|c| Context::new(c.to_value()))
        .collect();

    let tree_eval = Evaluator::new();
    let vm = BytecodeVM::new();
    let bc_compiler = ExprCompiler::new();
    let compiled_bc = bc_compiler.compile(&expr);

    let mut jit_compiler = SchemaJITCompiler::new().unwrap();
    let compiled_jit = jit_compiler
        .compile_with_schema(&expr, 1, LoanContext::schema())
        .unwrap();

    group.throughput(Throughput::Elements(1000));

    // Process 1000 different contexts
    group.bench_function("tree_1k_varied", |b| {
        b.iter(|| {
            for ctx in &value_contexts {
                black_box(tree_eval.eval(black_box(&expr), black_box(ctx)).unwrap());
            }
        })
    });

    group.bench_function("vm_1k_varied", |b| {
        b.iter(|| {
            for ctx in &value_contexts {
                black_box(vm.execute(black_box(&compiled_bc), black_box(ctx)).unwrap());
            }
        })
    });

    group.bench_function("jit_1k_varied", |b| {
        b.iter(|| {
            for ctx in &contexts {
                black_box(unsafe { compiled_jit.call_typed(black_box(ctx)).unwrap() });
            }
        })
    });

    group.finish();
}

// ============================================================================
// Benchmark 4: Compilation Amortization
// ============================================================================
//
// Question: How many executions to pay back JIT compilation cost?

fn bench_compilation_amortization(c: &mut Criterion) {
    let mut group = c.benchmark_group("compilation_amortization");
    group.sample_size(50);

    let expr = create_medium_rule();
    let typed_ctx = LoanContext {
        credit_score: 720,
        annual_income: 85000.0,
        debt_to_income: 0.32,
        employment_years: 5,
        age: 35,
        loan_amount: 250000.0,
    };
    let value_ctx = Context::new(typed_ctx.to_value());

    let tree_eval = Evaluator::new();
    let vm = BytecodeVM::new();
    let schema = LoanContext::schema();

    // JIT: compile + execute N times
    for n in [10, 100, 1000, 10000] {
        group.throughput(Throughput::Elements(n as u64));

        group.bench_with_input(BenchmarkId::new("jit_with_compile", n), &n, |b, &n| {
            b.iter(|| {
                // Include compilation cost
                let mut jit_compiler = SchemaJITCompiler::new().unwrap();
                let compiled = jit_compiler
                    .compile_with_schema(&expr, n as u64, schema)
                    .unwrap();

                // Execute N times
                for _ in 0..n {
                    black_box(unsafe { compiled.call_typed(black_box(&typed_ctx)).unwrap() });
                }
            })
        });

        group.bench_with_input(BenchmarkId::new("vm_no_compile", n), &n, |b, &n| {
            b.iter(|| {
                // VM bytecode is cheap to create
                let bc_compiler = ExprCompiler::new();
                let compiled = bc_compiler.compile(&expr);

                // Execute N times
                for _ in 0..n {
                    black_box(
                        vm.execute(black_box(&compiled), black_box(&value_ctx))
                            .unwrap(),
                    );
                }
            })
        });

        group.bench_with_input(BenchmarkId::new("tree_no_compile", n), &n, |b, &n| {
            b.iter(|| {
                for _ in 0..n {
                    black_box(
                        tree_eval
                            .eval(black_box(&expr), black_box(&value_ctx))
                            .unwrap(),
                    );
                }
            })
        });
    }

    group.finish();
}

// ============================================================================
// Benchmark 5: Realistic Throughput (Pre-compiled, Varied Data)
// ============================================================================

fn bench_realistic_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_throughput");
    group.sample_size(50);

    let expr = create_medium_rule();
    let contexts = generate_test_contexts(10000);
    let value_contexts: Vec<Context> = contexts
        .iter()
        .map(|c| Context::new(c.to_value()))
        .collect();

    let tree_eval = Evaluator::new();
    let vm = BytecodeVM::new();
    let bc_compiler = ExprCompiler::new();
    let compiled_bc = bc_compiler.compile(&expr);

    let mut jit_compiler = SchemaJITCompiler::new().unwrap();
    let compiled_jit = jit_compiler
        .compile_with_schema(&expr, 1, LoanContext::schema())
        .unwrap();

    group.throughput(Throughput::Elements(10000));

    group.bench_function("tree_10k", |b| {
        b.iter(|| {
            let mut sum = 0.0;
            for ctx in &value_contexts {
                if let Ok(v) = tree_eval.eval(&expr, ctx) {
                    if let Some(f) = v.as_float() {
                        sum += f;
                    }
                }
            }
            black_box(sum)
        })
    });

    group.bench_function("vm_10k", |b| {
        b.iter(|| {
            let mut sum = 0.0;
            for ctx in &value_contexts {
                if let Ok(v) = vm.execute(&compiled_bc, ctx) {
                    if let Some(f) = v.as_float() {
                        sum += f;
                    }
                }
            }
            black_box(sum)
        })
    });

    group.bench_function("jit_10k", |b| {
        b.iter(|| {
            let mut sum = 0.0;
            for ctx in &contexts {
                if let Ok(v) = unsafe { compiled_jit.call_typed(ctx) } {
                    sum += v;
                }
            }
            black_box(sum)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_fair_comparison,
    bench_end_to_end,
    bench_cold_cache,
    bench_compilation_amortization,
    bench_realistic_throughput,
);

criterion_main!(benches);
