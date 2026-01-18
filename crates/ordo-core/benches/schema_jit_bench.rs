//! Benchmark comparing Schema-Aware JIT vs Legacy JIT vs BytecodeVM
//!
//! This benchmark demonstrates the performance improvement from using
//! Schema-Aware JIT compilation with direct field access.
//!
//! Expected results:
//! - Schema JIT: ~3-10ns per field access
//! - Legacy JIT (trampoline): ~60-100ns per field access
//! - BytecodeVM: ~15-40ns per field access

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::OnceLock;

use ordo_core::context::{Context, FieldType, MessageSchema, Value};
use ordo_core::expr::jit::TypedContext;
use ordo_core::expr::{
    BinaryOp, BytecodeVM, CompiledExpr, Evaluator, Expr, ExprCompiler, ExprParser,
    SchemaJITCompiler, SchemaJITEvaluator, SchemaJITEvaluatorConfig,
};

// ============================================================================
// Test Context Definitions
// ============================================================================

/// Test context for Schema JIT benchmarks
#[repr(C)]
struct LoanContext {
    amount: f64,
    credit_score: i32,
    debt_ratio: f64,
    years_employed: i32,
    approved: bool,
}

impl TypedContext for LoanContext {
    fn schema() -> &'static MessageSchema {
        static SCHEMA: OnceLock<MessageSchema> = OnceLock::new();
        SCHEMA.get_or_init(|| {
            MessageSchema::builder("LoanContext")
                .field_at("amount", FieldType::Float64, 0)
                .field_at("credit_score", FieldType::Int32, 8)
                .field_at("debt_ratio", FieldType::Float64, 16)
                .field_at("years_employed", FieldType::Int32, 24)
                .field_at("approved", FieldType::Bool, 28)
                .build()
        })
    }

    unsafe fn field_ptr(&self, field_name: &str) -> Option<(*const u8, FieldType)> {
        match field_name {
            "amount" => Some((
                std::ptr::addr_of!(self.amount) as *const u8,
                FieldType::Float64,
            )),
            "credit_score" => Some((
                std::ptr::addr_of!(self.credit_score) as *const u8,
                FieldType::Int32,
            )),
            "debt_ratio" => Some((
                std::ptr::addr_of!(self.debt_ratio) as *const u8,
                FieldType::Float64,
            )),
            "years_employed" => Some((
                std::ptr::addr_of!(self.years_employed) as *const u8,
                FieldType::Int32,
            )),
            "approved" => Some((
                std::ptr::addr_of!(self.approved) as *const u8,
                FieldType::Bool,
            )),
            _ => None,
        }
    }
}

// ============================================================================
// Benchmark Expressions
// ============================================================================

fn create_expressions() -> Vec<(&'static str, Expr)> {
    vec![
        // Simple literal (baseline)
        ("literal", Expr::Literal(Value::Float(42.0))),
        // Single field access
        ("single_field", Expr::Field("amount".to_string())),
        // Simple comparison
        (
            "comparison",
            Expr::Binary {
                left: Box::new(Expr::Field("credit_score".to_string())),
                op: BinaryOp::Ge,
                right: Box::new(Expr::Literal(Value::Int(700))),
            },
        ),
        // Arithmetic
        (
            "arithmetic",
            Expr::Binary {
                left: Box::new(Expr::Field("amount".to_string())),
                op: BinaryOp::Mul,
                right: Box::new(Expr::Literal(Value::Float(1.05))),
            },
        ),
        // Multiple fields
        (
            "multi_field",
            Expr::Binary {
                left: Box::new(Expr::Binary {
                    left: Box::new(Expr::Field("amount".to_string())),
                    op: BinaryOp::Mul,
                    right: Box::new(Expr::Field("debt_ratio".to_string())),
                }),
                op: BinaryOp::Lt,
                right: Box::new(Expr::Literal(Value::Float(50000.0))),
            },
        ),
        // Complex rule expression
        (
            "complex_rule",
            Expr::Binary {
                left: Box::new(Expr::Binary {
                    left: Box::new(Expr::Binary {
                        left: Box::new(Expr::Field("credit_score".to_string())),
                        op: BinaryOp::Ge,
                        right: Box::new(Expr::Literal(Value::Int(700))),
                    }),
                    op: BinaryOp::And,
                    right: Box::new(Expr::Binary {
                        left: Box::new(Expr::Field("debt_ratio".to_string())),
                        op: BinaryOp::Lt,
                        right: Box::new(Expr::Literal(Value::Float(0.4))),
                    }),
                }),
                op: BinaryOp::And,
                right: Box::new(Expr::Binary {
                    left: Box::new(Expr::Field("years_employed".to_string())),
                    op: BinaryOp::Ge,
                    right: Box::new(Expr::Literal(Value::Int(2))),
                }),
            },
        ),
    ]
}

// ============================================================================
// Schema JIT Benchmark
// ============================================================================

fn bench_schema_jit(c: &mut Criterion) {
    let mut group = c.benchmark_group("schema_jit");

    let ctx = LoanContext {
        amount: 50000.0,
        credit_score: 720,
        debt_ratio: 0.35,
        years_employed: 5,
        approved: false,
    };

    let evaluator = SchemaJITEvaluator::simple().unwrap();
    let expressions = create_expressions();

    for (name, expr) in &expressions {
        // Warm up: compile the expression
        let _ = evaluator.eval_typed(expr, &ctx);

        group.bench_with_input(BenchmarkId::new("eval", name), expr, |b, expr| {
            b.iter(|| {
                let result = evaluator.eval_typed(black_box(expr), black_box(&ctx));
                black_box(result)
            });
        });
    }

    group.finish();
}

// ============================================================================
// BytecodeVM Benchmark (for comparison)
// ============================================================================

fn bench_bytecode_vm(c: &mut Criterion) {
    let mut group = c.benchmark_group("bytecode_vm");

    // Create Value-based context for VM
    let ctx_value = Value::object(std::collections::HashMap::from([
        ("amount".to_string(), Value::Float(50000.0)),
        ("credit_score".to_string(), Value::Int(720)),
        ("debt_ratio".to_string(), Value::Float(0.35)),
        ("years_employed".to_string(), Value::Int(5)),
        ("approved".to_string(), Value::Bool(false)),
    ]));
    let ctx = Context::new(ctx_value);

    let vm = BytecodeVM::new();
    let expressions = create_expressions();

    // Pre-compile all expressions
    let compiled_exprs: Vec<_> = expressions
        .iter()
        .map(|(name, expr)| {
            let compiler = ExprCompiler::new();
            (*name, compiler.compile(expr))
        })
        .collect();

    for (name, compiled) in &compiled_exprs {
        group.bench_with_input(BenchmarkId::new("eval", name), compiled, |b, compiled| {
            b.iter(|| {
                let result = vm.execute(black_box(compiled), black_box(&ctx));
                black_box(result)
            });
        });
    }

    group.finish();
}

// ============================================================================
// Tree-walk Evaluator Benchmark (baseline)
// ============================================================================

fn bench_tree_walk(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_walk");

    // Create Value-based context
    let ctx_value = Value::object(std::collections::HashMap::from([
        ("amount".to_string(), Value::Float(50000.0)),
        ("credit_score".to_string(), Value::Int(720)),
        ("debt_ratio".to_string(), Value::Float(0.35)),
        ("years_employed".to_string(), Value::Int(5)),
        ("approved".to_string(), Value::Bool(false)),
    ]));
    let ctx = Context::new(ctx_value);

    let evaluator = Evaluator::new();
    let expressions = create_expressions();

    for (name, expr) in &expressions {
        group.bench_with_input(BenchmarkId::new("eval", name), expr, |b, expr| {
            b.iter(|| {
                let result = evaluator.eval(black_box(expr), black_box(&ctx));
                black_box(result)
            });
        });
    }

    group.finish();
}

// ============================================================================
// Direct Comparison: Schema JIT vs VM vs Tree-walk
// ============================================================================

fn bench_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("comparison");

    // Test expression: credit_score >= 700 && debt_ratio < 0.4
    let expr = Expr::Binary {
        left: Box::new(Expr::Binary {
            left: Box::new(Expr::Field("credit_score".to_string())),
            op: BinaryOp::Ge,
            right: Box::new(Expr::Literal(Value::Int(700))),
        }),
        op: BinaryOp::And,
        right: Box::new(Expr::Binary {
            left: Box::new(Expr::Field("debt_ratio".to_string())),
            op: BinaryOp::Lt,
            right: Box::new(Expr::Literal(Value::Float(0.4))),
        }),
    };

    // Schema JIT context
    let typed_ctx = LoanContext {
        amount: 50000.0,
        credit_score: 720,
        debt_ratio: 0.35,
        years_employed: 5,
        approved: false,
    };

    // Value-based context for VM and tree-walk
    let ctx_value = Value::object(std::collections::HashMap::from([
        ("amount".to_string(), Value::Float(50000.0)),
        ("credit_score".to_string(), Value::Int(720)),
        ("debt_ratio".to_string(), Value::Float(0.35)),
        ("years_employed".to_string(), Value::Int(5)),
        ("approved".to_string(), Value::Bool(false)),
    ]));
    let ctx = Context::new(ctx_value);

    // Setup
    let schema_evaluator = SchemaJITEvaluator::simple().unwrap();
    let vm = BytecodeVM::new();
    let compiler = ExprCompiler::new();
    let compiled = compiler.compile(&expr);
    let tree_evaluator = Evaluator::new();

    // Warm up Schema JIT
    let _ = schema_evaluator.eval_typed(&expr, &typed_ctx);

    // Benchmark Schema JIT
    group.bench_function("schema_jit", |b| {
        b.iter(|| {
            let result = schema_evaluator.eval_typed(black_box(&expr), black_box(&typed_ctx));
            black_box(result)
        });
    });

    // Benchmark BytecodeVM
    group.bench_function("bytecode_vm", |b| {
        b.iter(|| {
            let result = vm.execute(black_box(&compiled), black_box(&ctx));
            black_box(result)
        });
    });

    // Benchmark Tree-walk
    group.bench_function("tree_walk", |b| {
        b.iter(|| {
            let result = tree_evaluator.eval(black_box(&expr), black_box(&ctx));
            black_box(result)
        });
    });

    group.finish();
}

// ============================================================================
// Compilation Time Benchmark
// ============================================================================

fn bench_compilation_time(c: &mut Criterion) {
    let mut group = c.benchmark_group("compilation");

    let expressions = create_expressions();
    let schema = LoanContext::schema();

    for (name, expr) in &expressions {
        group.bench_with_input(
            BenchmarkId::new("schema_jit_compile", name),
            expr,
            |b, expr| {
                b.iter(|| {
                    let mut compiler = SchemaJITCompiler::new().unwrap();
                    use std::hash::{Hash, Hasher};
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    format!("{:?}", expr).hash(&mut hasher);
                    let hash = hasher.finish();
                    // Note: we can't return the result directly due to lifetime issues
                    // Instead, just measure compilation time
                    let result =
                        compiler.compile_with_schema(black_box(expr), hash, black_box(schema));
                    black_box(result.is_ok())
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Field Access Microbenchmark
// ============================================================================

fn bench_field_access_only(c: &mut Criterion) {
    let mut group = c.benchmark_group("field_access");

    // Schema JIT: direct memory access
    let typed_ctx = LoanContext {
        amount: 50000.0,
        credit_score: 720,
        debt_ratio: 0.35,
        years_employed: 5,
        approved: false,
    };

    let expr = Expr::Field("amount".to_string());
    let schema = LoanContext::schema();

    // Compile directly and get the function
    let mut compiler = SchemaJITCompiler::new().unwrap();
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    format!("{:?}", expr).hash(&mut hasher);
    let hash = hasher.finish();
    let compiled = compiler.compile_with_schema(&expr, hash, schema).unwrap();

    // Benchmark raw JIT execution (no evaluator overhead)
    group.bench_function("schema_jit_raw", |b| {
        b.iter(|| {
            let result = unsafe { compiled.call_typed(black_box(&typed_ctx)) };
            black_box(result)
        });
    });

    // Benchmark through evaluator (with cache lookup overhead)
    let schema_evaluator = SchemaJITEvaluator::simple().unwrap();
    let _ = schema_evaluator.eval_typed(&expr, &typed_ctx); // Warm up

    group.bench_function("schema_jit_evaluator", |b| {
        b.iter(|| {
            let result = schema_evaluator.eval_typed(black_box(&expr), black_box(&typed_ctx));
            black_box(result)
        });
    });

    // Value-based context
    let ctx_value = Value::object(std::collections::HashMap::from([
        ("amount".to_string(), Value::Float(50000.0)),
        ("credit_score".to_string(), Value::Int(720)),
        ("debt_ratio".to_string(), Value::Float(0.35)),
        ("years_employed".to_string(), Value::Int(5)),
        ("approved".to_string(), Value::Bool(false)),
    ]));
    let ctx = Context::new(ctx_value);

    // BytecodeVM field access
    let vm = BytecodeVM::new();
    let bc_compiler = ExprCompiler::new();
    let bc_compiled = bc_compiler.compile(&expr);

    group.bench_function("vm_field", |b| {
        b.iter(|| {
            let result = vm.execute(black_box(&bc_compiled), black_box(&ctx));
            black_box(result)
        });
    });

    // Tree-walk field access
    let tree_evaluator = Evaluator::new();

    group.bench_function("tree_walk_field", |b| {
        b.iter(|| {
            let result = tree_evaluator.eval(black_box(&expr), black_box(&ctx));
            black_box(result)
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_schema_jit,
    bench_bytecode_vm,
    bench_tree_walk,
    bench_comparison,
    bench_compilation_time,
    bench_field_access_only,
);

criterion_main!(benches);
