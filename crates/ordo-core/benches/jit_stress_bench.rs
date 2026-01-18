//! JIT Stress Benchmark - Maximum Performance Test
//!
//! This benchmark pushes the limits to show JIT's maximum advantage:
//! 1. Complex rules with many conditions (10-20 conditions)
//! 2. High volume processing (100K-1M evaluations)
//! 3. Real-world simulation (risk scoring pipeline)
//!
//! Run with: cargo bench --bench jit_stress_bench

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box;
use std::sync::OnceLock;

use ordo_core::context::{Context, FieldType, MessageSchema, Value};
use ordo_core::expr::jit::TypedContext;
use ordo_core::expr::{BinaryOp, BytecodeVM, Evaluator, Expr, ExprCompiler, SchemaJITCompiler};

// ============================================================================
// Complex Business Context (15 fields)
// ============================================================================

#[repr(C)]
struct RiskContext {
    // Credit factors (5 fields)
    credit_score: i32,
    credit_utilization: f64,
    credit_history_months: i32,
    num_credit_inquiries: i32,
    num_delinquencies: i32,

    // Income factors (4 fields)
    annual_income: f64,
    monthly_debt: f64,
    employment_months: i32,
    income_stability: f64,

    // Loan factors (4 fields)
    loan_amount: f64,
    loan_term_months: i32,
    interest_rate: f64,
    collateral_value: f64,

    // Risk indicators (2 fields)
    risk_score: f64,
    fraud_probability: f64,
}

impl TypedContext for RiskContext {
    fn schema() -> &'static MessageSchema {
        static SCHEMA: OnceLock<MessageSchema> = OnceLock::new();
        SCHEMA.get_or_init(|| {
            MessageSchema::builder("RiskContext")
                .field_at("credit_score", FieldType::Int32, 0)
                .field_at("credit_utilization", FieldType::Float64, 8)
                .field_at("credit_history_months", FieldType::Int32, 16)
                .field_at("num_credit_inquiries", FieldType::Int32, 20)
                .field_at("num_delinquencies", FieldType::Int32, 24)
                .field_at("annual_income", FieldType::Float64, 32)
                .field_at("monthly_debt", FieldType::Float64, 40)
                .field_at("employment_months", FieldType::Int32, 48)
                .field_at("income_stability", FieldType::Float64, 56)
                .field_at("loan_amount", FieldType::Float64, 64)
                .field_at("loan_term_months", FieldType::Int32, 72)
                .field_at("interest_rate", FieldType::Float64, 80)
                .field_at("collateral_value", FieldType::Float64, 88)
                .field_at("risk_score", FieldType::Float64, 96)
                .field_at("fraud_probability", FieldType::Float64, 104)
                .build()
        })
    }

    unsafe fn field_ptr(&self, field_name: &str) -> Option<(*const u8, FieldType)> {
        match field_name {
            "credit_score" => Some((std::ptr::addr_of!(self.credit_score) as _, FieldType::Int32)),
            "credit_utilization" => Some((
                std::ptr::addr_of!(self.credit_utilization) as _,
                FieldType::Float64,
            )),
            "credit_history_months" => Some((
                std::ptr::addr_of!(self.credit_history_months) as _,
                FieldType::Int32,
            )),
            "num_credit_inquiries" => Some((
                std::ptr::addr_of!(self.num_credit_inquiries) as _,
                FieldType::Int32,
            )),
            "num_delinquencies" => Some((
                std::ptr::addr_of!(self.num_delinquencies) as _,
                FieldType::Int32,
            )),
            "annual_income" => Some((
                std::ptr::addr_of!(self.annual_income) as _,
                FieldType::Float64,
            )),
            "monthly_debt" => Some((
                std::ptr::addr_of!(self.monthly_debt) as _,
                FieldType::Float64,
            )),
            "employment_months" => Some((
                std::ptr::addr_of!(self.employment_months) as _,
                FieldType::Int32,
            )),
            "income_stability" => Some((
                std::ptr::addr_of!(self.income_stability) as _,
                FieldType::Float64,
            )),
            "loan_amount" => Some((
                std::ptr::addr_of!(self.loan_amount) as _,
                FieldType::Float64,
            )),
            "loan_term_months" => Some((
                std::ptr::addr_of!(self.loan_term_months) as _,
                FieldType::Int32,
            )),
            "interest_rate" => Some((
                std::ptr::addr_of!(self.interest_rate) as _,
                FieldType::Float64,
            )),
            "collateral_value" => Some((
                std::ptr::addr_of!(self.collateral_value) as _,
                FieldType::Float64,
            )),
            "risk_score" => Some((std::ptr::addr_of!(self.risk_score) as _, FieldType::Float64)),
            "fraud_probability" => Some((
                std::ptr::addr_of!(self.fraud_probability) as _,
                FieldType::Float64,
            )),
            _ => None,
        }
    }
}

impl RiskContext {
    fn to_value(&self) -> Value {
        Value::object(std::collections::HashMap::from([
            (
                "credit_score".to_string(),
                Value::Int(self.credit_score as i64),
            ),
            (
                "credit_utilization".to_string(),
                Value::Float(self.credit_utilization),
            ),
            (
                "credit_history_months".to_string(),
                Value::Int(self.credit_history_months as i64),
            ),
            (
                "num_credit_inquiries".to_string(),
                Value::Int(self.num_credit_inquiries as i64),
            ),
            (
                "num_delinquencies".to_string(),
                Value::Int(self.num_delinquencies as i64),
            ),
            (
                "annual_income".to_string(),
                Value::Float(self.annual_income),
            ),
            ("monthly_debt".to_string(), Value::Float(self.monthly_debt)),
            (
                "employment_months".to_string(),
                Value::Int(self.employment_months as i64),
            ),
            (
                "income_stability".to_string(),
                Value::Float(self.income_stability),
            ),
            ("loan_amount".to_string(), Value::Float(self.loan_amount)),
            (
                "loan_term_months".to_string(),
                Value::Int(self.loan_term_months as i64),
            ),
            (
                "interest_rate".to_string(),
                Value::Float(self.interest_rate),
            ),
            (
                "collateral_value".to_string(),
                Value::Float(self.collateral_value),
            ),
            ("risk_score".to_string(), Value::Float(self.risk_score)),
            (
                "fraud_probability".to_string(),
                Value::Float(self.fraud_probability),
            ),
        ]))
    }
}

// ============================================================================
// Complex Rules
// ============================================================================

/// Create a complex underwriting rule with 10 conditions
fn create_complex_rule_10() -> Expr {
    // Premium tier approval: 10 conditions
    // credit_score >= 750 AND credit_utilization < 0.30 AND credit_history_months >= 60
    // AND num_credit_inquiries <= 2 AND num_delinquencies == 0
    // AND annual_income >= 80000 AND (monthly_debt / (annual_income/12)) < 0.35
    // AND employment_months >= 24 AND income_stability >= 0.8
    // AND (loan_amount / collateral_value) < 0.80

    and_chain(vec![
        ge("credit_score", 750),
        lt_f("credit_utilization", 0.30),
        ge("credit_history_months", 60),
        le("num_credit_inquiries", 2),
        eq("num_delinquencies", 0),
        ge_f("annual_income", 80000.0),
        lt_f("income_stability", 0.35), // Simplified DTI
        ge("employment_months", 24),
        ge_f("income_stability", 0.8),
        lt_f("interest_rate", 0.08),
    ])
}

/// Create a very complex underwriting rule with 15 conditions
fn create_complex_rule_15() -> Expr {
    and_chain(vec![
        ge("credit_score", 720),
        lt_f("credit_utilization", 0.35),
        ge("credit_history_months", 48),
        le("num_credit_inquiries", 3),
        le("num_delinquencies", 1),
        ge_f("annual_income", 60000.0),
        lt_f("monthly_debt", 2500.0),
        ge("employment_months", 18),
        ge_f("income_stability", 0.7),
        lt_f("loan_amount", 500000.0),
        ge("loan_term_months", 60),
        lt_f("interest_rate", 0.10),
        gt_f("collateral_value", 100000.0),
        lt_f("risk_score", 50.0),
        lt_f("fraud_probability", 0.05),
    ])
}

/// Create an extreme rule with 20 conditions (including arithmetic)
fn create_complex_rule_20() -> Expr {
    and_chain(vec![
        // Credit score tier
        ge("credit_score", 680),
        lt_f("credit_utilization", 0.40),
        ge("credit_history_months", 36),
        le("num_credit_inquiries", 4),
        le("num_delinquencies", 2),
        // Income requirements
        ge_f("annual_income", 50000.0),
        lt_f("monthly_debt", 3000.0),
        ge("employment_months", 12),
        ge_f("income_stability", 0.6),
        // Loan parameters
        lt_f("loan_amount", 750000.0),
        ge("loan_term_months", 36),
        le("loan_term_months", 360),
        lt_f("interest_rate", 0.12),
        gt_f("collateral_value", 50000.0),
        // Risk thresholds
        lt_f("risk_score", 70.0),
        lt_f("fraud_probability", 0.10),
        // Derived conditions
        Expr::Binary {
            left: Box::new(Expr::Binary {
                left: Box::new(Expr::Field("loan_amount".into())),
                op: BinaryOp::Div,
                right: Box::new(Expr::Field("collateral_value".into())),
            }),
            op: BinaryOp::Lt,
            right: Box::new(Expr::Literal(Value::Float(0.85))),
        },
        Expr::Binary {
            left: Box::new(Expr::Binary {
                left: Box::new(Expr::Field("monthly_debt".into())),
                op: BinaryOp::Mul,
                right: Box::new(Expr::Literal(Value::Float(12.0))),
            }),
            op: BinaryOp::Lt,
            right: Box::new(Expr::Field("annual_income".into())),
        },
        ge("credit_score", 650),          // Redundant but adds complexity
        lt_f("credit_utilization", 0.50), // Redundant but adds complexity
    ])
}

// Helper functions
fn ge(field: &str, val: i64) -> Expr {
    Expr::Binary {
        left: Box::new(Expr::Field(field.into())),
        op: BinaryOp::Ge,
        right: Box::new(Expr::Literal(Value::Int(val))),
    }
}

fn le(field: &str, val: i64) -> Expr {
    Expr::Binary {
        left: Box::new(Expr::Field(field.into())),
        op: BinaryOp::Le,
        right: Box::new(Expr::Literal(Value::Int(val))),
    }
}

fn eq(field: &str, val: i64) -> Expr {
    Expr::Binary {
        left: Box::new(Expr::Field(field.into())),
        op: BinaryOp::Eq,
        right: Box::new(Expr::Literal(Value::Int(val))),
    }
}

fn lt_f(field: &str, val: f64) -> Expr {
    Expr::Binary {
        left: Box::new(Expr::Field(field.into())),
        op: BinaryOp::Lt,
        right: Box::new(Expr::Literal(Value::Float(val))),
    }
}

fn ge_f(field: &str, val: f64) -> Expr {
    Expr::Binary {
        left: Box::new(Expr::Field(field.into())),
        op: BinaryOp::Ge,
        right: Box::new(Expr::Literal(Value::Float(val))),
    }
}

fn gt_f(field: &str, val: f64) -> Expr {
    Expr::Binary {
        left: Box::new(Expr::Field(field.into())),
        op: BinaryOp::Gt,
        right: Box::new(Expr::Literal(Value::Float(val))),
    }
}

fn and_chain(exprs: Vec<Expr>) -> Expr {
    let mut iter = exprs.into_iter();
    let first = iter.next().unwrap();
    iter.fold(first, |acc, e| Expr::Binary {
        left: Box::new(acc),
        op: BinaryOp::And,
        right: Box::new(e),
    })
}

// ============================================================================
// Test Data
// ============================================================================

fn generate_risk_contexts(count: usize) -> Vec<RiskContext> {
    (0..count)
        .map(|i| RiskContext {
            credit_score: 600 + (i % 250) as i32,
            credit_utilization: 0.10 + (i % 60) as f64 * 0.01,
            credit_history_months: 12 + (i % 100) as i32,
            num_credit_inquiries: (i % 8) as i32,
            num_delinquencies: (i % 5) as i32,
            annual_income: 40000.0 + (i % 100) as f64 * 1000.0,
            monthly_debt: 500.0 + (i % 30) as f64 * 100.0,
            employment_months: (i % 120) as i32,
            income_stability: 0.5 + (i % 50) as f64 * 0.01,
            loan_amount: 100000.0 + (i % 50) as f64 * 10000.0,
            loan_term_months: 60 + (i % 300) as i32,
            interest_rate: 0.04 + (i % 10) as f64 * 0.01,
            collateral_value: 150000.0 + (i % 40) as f64 * 10000.0,
            risk_score: 20.0 + (i % 60) as f64,
            fraud_probability: (i % 15) as f64 * 0.01,
        })
        .collect()
}

// ============================================================================
// Benchmarks
// ============================================================================

/// Benchmark: Complex rules (10, 15, 20 conditions)
fn bench_complex_rules(c: &mut Criterion) {
    let mut group = c.benchmark_group("complex_rules");
    group.sample_size(50);

    let typed_ctx = RiskContext {
        credit_score: 780,
        credit_utilization: 0.25,
        credit_history_months: 84,
        num_credit_inquiries: 1,
        num_delinquencies: 0,
        annual_income: 120000.0,
        monthly_debt: 2000.0,
        employment_months: 60,
        income_stability: 0.9,
        loan_amount: 300000.0,
        loan_term_months: 360,
        interest_rate: 0.055,
        collateral_value: 400000.0,
        risk_score: 25.0,
        fraud_probability: 0.02,
    };
    let value_ctx = Context::new(typed_ctx.to_value());

    let rules = vec![
        ("10_conditions", create_complex_rule_10()),
        ("15_conditions", create_complex_rule_15()),
        ("20_conditions", create_complex_rule_20()),
    ];

    let tree_eval = Evaluator::new();
    let vm = BytecodeVM::new();
    let mut jit_compiler = SchemaJITCompiler::new().unwrap();
    let schema = RiskContext::schema();

    for (name, expr) in &rules {
        let bc_compiler = ExprCompiler::new();
        let compiled_bc = bc_compiler.compile(expr);

        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        name.hash(&mut hasher);
        let compiled_jit = jit_compiler
            .compile_with_schema(expr, hasher.finish(), schema)
            .unwrap();

        group.bench_with_input(BenchmarkId::new("tree", *name), &(), |b, _| {
            b.iter(|| black_box(tree_eval.eval(black_box(expr), black_box(&value_ctx))))
        });

        group.bench_with_input(BenchmarkId::new("vm", *name), &(), |b, _| {
            b.iter(|| black_box(vm.execute(black_box(&compiled_bc), black_box(&value_ctx))))
        });

        group.bench_with_input(BenchmarkId::new("jit", *name), &(), |b, _| {
            b.iter(|| black_box(unsafe { compiled_jit.call_typed(black_box(&typed_ctx)) }))
        });
    }

    group.finish();
}

/// Benchmark: High volume processing (10K, 100K, 1M)
fn bench_high_volume(c: &mut Criterion) {
    let mut group = c.benchmark_group("high_volume");
    group.sample_size(20); // Fewer samples for long-running tests

    let expr = create_complex_rule_10();

    let tree_eval = Evaluator::new();
    let vm = BytecodeVM::new();
    let bc_compiler = ExprCompiler::new();
    let compiled_bc = bc_compiler.compile(&expr);

    let mut jit_compiler = SchemaJITCompiler::new().unwrap();
    let compiled_jit = jit_compiler
        .compile_with_schema(&expr, 1, RiskContext::schema())
        .unwrap();

    for count in [10_000u64, 100_000, 500_000] {
        let contexts = generate_risk_contexts(count as usize);
        let value_contexts: Vec<Context> = contexts
            .iter()
            .map(|c| Context::new(c.to_value()))
            .collect();

        group.throughput(Throughput::Elements(count));

        group.bench_with_input(BenchmarkId::new("tree", count), &count, |b, _| {
            b.iter(|| {
                let mut sum = 0i64;
                for ctx in &value_contexts {
                    if let Ok(v) = tree_eval.eval(&expr, ctx) {
                        if v.is_truthy() {
                            sum += 1;
                        }
                    }
                }
                black_box(sum)
            })
        });

        group.bench_with_input(BenchmarkId::new("vm", count), &count, |b, _| {
            b.iter(|| {
                let mut sum = 0i64;
                for ctx in &value_contexts {
                    if let Ok(v) = vm.execute(&compiled_bc, ctx) {
                        if v.is_truthy() {
                            sum += 1;
                        }
                    }
                }
                black_box(sum)
            })
        });

        group.bench_with_input(BenchmarkId::new("jit", count), &count, |b, _| {
            b.iter(|| {
                let mut sum = 0i64;
                for ctx in &contexts {
                    if let Ok(v) = unsafe { compiled_jit.call_typed(ctx) } {
                        if v != 0.0 {
                            sum += 1;
                        }
                    }
                }
                black_box(sum)
            })
        });
    }

    group.finish();
}

/// Benchmark: Pipeline simulation (multiple rules per context)
fn bench_rule_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("rule_pipeline");
    group.sample_size(30);

    // Simulate a real pipeline: 5 rules applied to each context
    let rules = vec![
        create_complex_rule_10(),
        create_complex_rule_15(),
        create_complex_rule_20(),
        create_complex_rule_10(), // Duplicate for more load
        create_complex_rule_15(),
    ];

    let contexts = generate_risk_contexts(10000);
    let value_contexts: Vec<Context> = contexts
        .iter()
        .map(|c| Context::new(c.to_value()))
        .collect();

    let tree_eval = Evaluator::new();
    let vm = BytecodeVM::new();

    // Pre-compile all
    let compiled_bc: Vec<_> = rules
        .iter()
        .map(|e| ExprCompiler::new().compile(e))
        .collect();

    let mut jit_compiler = SchemaJITCompiler::new().unwrap();
    let schema = RiskContext::schema();
    let mut compiled_jit = Vec::new();
    for (i, e) in rules.iter().enumerate() {
        compiled_jit.push(
            jit_compiler
                .compile_with_schema(e, i as u64, schema)
                .unwrap()
                .clone(),
        );
    }

    // 10K contexts × 5 rules = 50K evaluations
    group.throughput(Throughput::Elements(50_000));

    group.bench_function("tree_pipeline", |b| {
        b.iter(|| {
            let mut passed = 0i64;
            for ctx in &value_contexts {
                let mut all_pass = true;
                for expr in &rules {
                    if let Ok(v) = tree_eval.eval(expr, ctx) {
                        if !v.is_truthy() {
                            all_pass = false;
                            break;
                        }
                    }
                }
                if all_pass {
                    passed += 1;
                }
            }
            black_box(passed)
        })
    });

    group.bench_function("vm_pipeline", |b| {
        b.iter(|| {
            let mut passed = 0i64;
            for ctx in &value_contexts {
                let mut all_pass = true;
                for compiled in &compiled_bc {
                    if let Ok(v) = vm.execute(compiled, ctx) {
                        if !v.is_truthy() {
                            all_pass = false;
                            break;
                        }
                    }
                }
                if all_pass {
                    passed += 1;
                }
            }
            black_box(passed)
        })
    });

    group.bench_function("jit_pipeline", |b| {
        b.iter(|| {
            let mut passed = 0i64;
            for ctx in &contexts {
                let mut all_pass = true;
                for compiled in &compiled_jit {
                    if let Ok(v) = unsafe { compiled.call_typed(ctx) } {
                        if v == 0.0 {
                            all_pass = false;
                            break;
                        }
                    }
                }
                if all_pass {
                    passed += 1;
                }
            }
            black_box(passed)
        })
    });

    group.finish();
}

/// Benchmark: Latency percentiles simulation
fn bench_latency_distribution(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency_single");
    group.sample_size(100);

    let expr = create_complex_rule_15();
    let contexts = generate_risk_contexts(100);
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
        .compile_with_schema(&expr, 1, RiskContext::schema())
        .unwrap();

    // Single evaluation latency (best case for showing JIT advantage)
    group.bench_function("tree_single", |b| {
        let ctx = &value_contexts[0];
        b.iter(|| black_box(tree_eval.eval(black_box(&expr), black_box(ctx))))
    });

    group.bench_function("vm_single", |b| {
        let ctx = &value_contexts[0];
        b.iter(|| black_box(vm.execute(black_box(&compiled_bc), black_box(ctx))))
    });

    group.bench_function("jit_single", |b| {
        let ctx = &contexts[0];
        b.iter(|| black_box(unsafe { compiled_jit.call_typed(black_box(ctx)) }))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_complex_rules,
    bench_high_volume,
    bench_rule_pipeline,
    bench_latency_distribution,
);

criterion_main!(benches);
