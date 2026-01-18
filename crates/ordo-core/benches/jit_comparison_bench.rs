//! JIT Performance Comparison Benchmark
//!
//! This benchmark compares three evaluation approaches across real business scenarios:
//! - Tree-walk Evaluator (baseline)
//! - BytecodeVM (optimized interpreter)
//! - Schema JIT (native code with direct memory access)
//!
//! Run with: cargo bench --bench jit_comparison_bench
//! View HTML report at: target/criterion/report/index.html

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box;
use std::sync::OnceLock;

use ordo_core::context::{Context, FieldType, MessageSchema, Value};
use ordo_core::expr::jit::TypedContext;
use ordo_core::expr::{BinaryOp, BytecodeVM, Evaluator, Expr, ExprCompiler, SchemaJITCompiler};

// ============================================================================
// Business Context Definitions
// ============================================================================

/// Loan Application Context - Financial Services
#[repr(C)]
struct LoanContext {
    // Applicant info
    credit_score: i32,     // 300-850
    annual_income: f64,    // USD
    employment_years: i32, // Years at current job
    age: i32,              // Years

    // Loan details
    loan_amount: f64,      // Requested amount
    loan_term_months: i32, // Duration
    interest_rate: f64,    // Annual rate

    // Financial ratios
    debt_to_income: f64, // 0.0 - 1.0
    loan_to_value: f64,  // For secured loans

    // Flags
    has_collateral: bool,
    is_first_time_buyer: bool,
}

impl TypedContext for LoanContext {
    fn schema() -> &'static MessageSchema {
        static SCHEMA: OnceLock<MessageSchema> = OnceLock::new();
        SCHEMA.get_or_init(|| {
            MessageSchema::builder("LoanContext")
                .field_at("credit_score", FieldType::Int32, 0)
                .field_at("annual_income", FieldType::Float64, 8)
                .field_at("employment_years", FieldType::Int32, 16)
                .field_at("age", FieldType::Int32, 20)
                .field_at("loan_amount", FieldType::Float64, 24)
                .field_at("loan_term_months", FieldType::Int32, 32)
                .field_at("interest_rate", FieldType::Float64, 40)
                .field_at("debt_to_income", FieldType::Float64, 48)
                .field_at("loan_to_value", FieldType::Float64, 56)
                .field_at("has_collateral", FieldType::Bool, 64)
                .field_at("is_first_time_buyer", FieldType::Bool, 65)
                .build()
        })
    }

    unsafe fn field_ptr(&self, field_name: &str) -> Option<(*const u8, FieldType)> {
        match field_name {
            "credit_score" => Some((
                std::ptr::addr_of!(self.credit_score) as *const u8,
                FieldType::Int32,
            )),
            "annual_income" => Some((
                std::ptr::addr_of!(self.annual_income) as *const u8,
                FieldType::Float64,
            )),
            "employment_years" => Some((
                std::ptr::addr_of!(self.employment_years) as *const u8,
                FieldType::Int32,
            )),
            "age" => Some((std::ptr::addr_of!(self.age) as *const u8, FieldType::Int32)),
            "loan_amount" => Some((
                std::ptr::addr_of!(self.loan_amount) as *const u8,
                FieldType::Float64,
            )),
            "loan_term_months" => Some((
                std::ptr::addr_of!(self.loan_term_months) as *const u8,
                FieldType::Int32,
            )),
            "interest_rate" => Some((
                std::ptr::addr_of!(self.interest_rate) as *const u8,
                FieldType::Float64,
            )),
            "debt_to_income" => Some((
                std::ptr::addr_of!(self.debt_to_income) as *const u8,
                FieldType::Float64,
            )),
            "loan_to_value" => Some((
                std::ptr::addr_of!(self.loan_to_value) as *const u8,
                FieldType::Float64,
            )),
            "has_collateral" => Some((
                std::ptr::addr_of!(self.has_collateral) as *const u8,
                FieldType::Bool,
            )),
            "is_first_time_buyer" => Some((
                std::ptr::addr_of!(self.is_first_time_buyer) as *const u8,
                FieldType::Bool,
            )),
            _ => None,
        }
    }
}

/// E-commerce Order Context - Retail
#[repr(C)]
struct OrderContext {
    // Order info
    order_total: f64,
    item_count: i32,
    shipping_cost: f64,
    discount_percent: f64,

    // Customer info
    customer_tier: i32,   // 1=Bronze, 2=Silver, 3=Gold, 4=Platinum
    total_purchases: f64, // Lifetime value
    account_age_days: i32,

    // Flags
    is_prime_member: bool,
    has_coupon: bool,
    is_gift: bool,
}

impl TypedContext for OrderContext {
    fn schema() -> &'static MessageSchema {
        static SCHEMA: OnceLock<MessageSchema> = OnceLock::new();
        SCHEMA.get_or_init(|| {
            MessageSchema::builder("OrderContext")
                .field_at("order_total", FieldType::Float64, 0)
                .field_at("item_count", FieldType::Int32, 8)
                .field_at("shipping_cost", FieldType::Float64, 16)
                .field_at("discount_percent", FieldType::Float64, 24)
                .field_at("customer_tier", FieldType::Int32, 32)
                .field_at("total_purchases", FieldType::Float64, 40)
                .field_at("account_age_days", FieldType::Int32, 48)
                .field_at("is_prime_member", FieldType::Bool, 52)
                .field_at("has_coupon", FieldType::Bool, 53)
                .field_at("is_gift", FieldType::Bool, 54)
                .build()
        })
    }

    unsafe fn field_ptr(&self, field_name: &str) -> Option<(*const u8, FieldType)> {
        match field_name {
            "order_total" => Some((
                std::ptr::addr_of!(self.order_total) as *const u8,
                FieldType::Float64,
            )),
            "item_count" => Some((
                std::ptr::addr_of!(self.item_count) as *const u8,
                FieldType::Int32,
            )),
            "shipping_cost" => Some((
                std::ptr::addr_of!(self.shipping_cost) as *const u8,
                FieldType::Float64,
            )),
            "discount_percent" => Some((
                std::ptr::addr_of!(self.discount_percent) as *const u8,
                FieldType::Float64,
            )),
            "customer_tier" => Some((
                std::ptr::addr_of!(self.customer_tier) as *const u8,
                FieldType::Int32,
            )),
            "total_purchases" => Some((
                std::ptr::addr_of!(self.total_purchases) as *const u8,
                FieldType::Float64,
            )),
            "account_age_days" => Some((
                std::ptr::addr_of!(self.account_age_days) as *const u8,
                FieldType::Int32,
            )),
            "is_prime_member" => Some((
                std::ptr::addr_of!(self.is_prime_member) as *const u8,
                FieldType::Bool,
            )),
            "has_coupon" => Some((
                std::ptr::addr_of!(self.has_coupon) as *const u8,
                FieldType::Bool,
            )),
            "is_gift" => Some((
                std::ptr::addr_of!(self.is_gift) as *const u8,
                FieldType::Bool,
            )),
            _ => None,
        }
    }
}

// ============================================================================
// Business Rules
// ============================================================================

/// Create business rule expressions with varying complexity
fn create_loan_rules() -> Vec<(&'static str, Expr)> {
    vec![
        // Rule 1: Simple credit check (1 comparison)
        (
            "1_simple_credit_check",
            Expr::Binary {
                left: Box::new(Expr::Field("credit_score".into())),
                op: BinaryOp::Ge,
                right: Box::new(Expr::Literal(Value::Int(700))),
            },
        ),
        // Rule 2: Basic eligibility (2 conditions)
        (
            "2_basic_eligibility",
            Expr::Binary {
                left: Box::new(Expr::Binary {
                    left: Box::new(Expr::Field("credit_score".into())),
                    op: BinaryOp::Ge,
                    right: Box::new(Expr::Literal(Value::Int(650))),
                }),
                op: BinaryOp::And,
                right: Box::new(Expr::Binary {
                    left: Box::new(Expr::Field("annual_income".into())),
                    op: BinaryOp::Ge,
                    right: Box::new(Expr::Literal(Value::Float(30000.0))),
                }),
            },
        ),
        // Rule 3: DTI check (3 conditions)
        (
            "3_dti_check",
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
            },
        ),
        // Rule 4: Full underwriting (5 conditions)
        (
            "4_full_underwriting",
            Expr::Binary {
                left: Box::new(Expr::Binary {
                    left: Box::new(Expr::Binary {
                        left: Box::new(Expr::Field("credit_score".into())),
                        op: BinaryOp::Ge,
                        right: Box::new(Expr::Literal(Value::Int(700))),
                    }),
                    op: BinaryOp::And,
                    right: Box::new(Expr::Binary {
                        left: Box::new(Expr::Field("debt_to_income".into())),
                        op: BinaryOp::Lt,
                        right: Box::new(Expr::Literal(Value::Float(0.36))),
                    }),
                }),
                op: BinaryOp::And,
                right: Box::new(Expr::Binary {
                    left: Box::new(Expr::Binary {
                        left: Box::new(Expr::Binary {
                            left: Box::new(Expr::Field("loan_to_value".into())),
                            op: BinaryOp::Lt,
                            right: Box::new(Expr::Literal(Value::Float(0.80))),
                        }),
                        op: BinaryOp::And,
                        right: Box::new(Expr::Binary {
                            left: Box::new(Expr::Field("employment_years".into())),
                            op: BinaryOp::Ge,
                            right: Box::new(Expr::Literal(Value::Int(2))),
                        }),
                    }),
                    op: BinaryOp::And,
                    right: Box::new(Expr::Binary {
                        left: Box::new(Expr::Field("age".into())),
                        op: BinaryOp::Ge,
                        right: Box::new(Expr::Literal(Value::Int(21))),
                    }),
                }),
            },
        ),
        // Rule 5: Premium tier calculation (arithmetic + comparison)
        (
            "5_loan_affordability",
            Expr::Binary {
                left: Box::new(Expr::Binary {
                    // Monthly payment estimate: loan_amount * (interest_rate / 12) / (1 - (1 + interest_rate/12)^-term)
                    // Simplified: loan_amount / loan_term_months
                    left: Box::new(Expr::Binary {
                        left: Box::new(Expr::Field("loan_amount".into())),
                        op: BinaryOp::Div,
                        right: Box::new(Expr::Field("loan_term_months".into())),
                    }),
                    op: BinaryOp::Lt,
                    // Max monthly payment: annual_income / 12 * 0.28
                    right: Box::new(Expr::Binary {
                        left: Box::new(Expr::Binary {
                            left: Box::new(Expr::Field("annual_income".into())),
                            op: BinaryOp::Div,
                            right: Box::new(Expr::Literal(Value::Float(12.0))),
                        }),
                        op: BinaryOp::Mul,
                        right: Box::new(Expr::Literal(Value::Float(0.28))),
                    }),
                }),
                op: BinaryOp::And,
                right: Box::new(Expr::Binary {
                    left: Box::new(Expr::Field("credit_score".into())),
                    op: BinaryOp::Ge,
                    right: Box::new(Expr::Literal(Value::Int(680))),
                }),
            },
        ),
        // Rule 6: Complex risk assessment (7 conditions)
        (
            "6_complex_risk",
            Expr::Binary {
                left: Box::new(Expr::Binary {
                    left: Box::new(Expr::Binary {
                        left: Box::new(Expr::Binary {
                            left: Box::new(Expr::Field("credit_score".into())),
                            op: BinaryOp::Ge,
                            right: Box::new(Expr::Literal(Value::Int(720))),
                        }),
                        op: BinaryOp::And,
                        right: Box::new(Expr::Binary {
                            left: Box::new(Expr::Field("debt_to_income".into())),
                            op: BinaryOp::Lt,
                            right: Box::new(Expr::Literal(Value::Float(0.33))),
                        }),
                    }),
                    op: BinaryOp::And,
                    right: Box::new(Expr::Binary {
                        left: Box::new(Expr::Binary {
                            left: Box::new(Expr::Field("loan_to_value".into())),
                            op: BinaryOp::Lt,
                            right: Box::new(Expr::Literal(Value::Float(0.75))),
                        }),
                        op: BinaryOp::And,
                        right: Box::new(Expr::Binary {
                            left: Box::new(Expr::Field("employment_years".into())),
                            op: BinaryOp::Ge,
                            right: Box::new(Expr::Literal(Value::Int(3))),
                        }),
                    }),
                }),
                op: BinaryOp::And,
                right: Box::new(Expr::Binary {
                    left: Box::new(Expr::Binary {
                        left: Box::new(Expr::Binary {
                            left: Box::new(Expr::Field("annual_income".into())),
                            op: BinaryOp::Ge,
                            right: Box::new(Expr::Literal(Value::Float(50000.0))),
                        }),
                        op: BinaryOp::And,
                        right: Box::new(Expr::Binary {
                            left: Box::new(Expr::Field("age".into())),
                            op: BinaryOp::Ge,
                            right: Box::new(Expr::Literal(Value::Int(25))),
                        }),
                    }),
                    op: BinaryOp::And,
                    right: Box::new(Expr::Binary {
                        left: Box::new(Expr::Field("interest_rate".into())),
                        op: BinaryOp::Lt,
                        right: Box::new(Expr::Literal(Value::Float(0.08))),
                    }),
                }),
            },
        ),
    ]
}

fn create_order_rules() -> Vec<(&'static str, Expr)> {
    vec![
        // Rule 1: Free shipping check
        (
            "1_free_shipping",
            Expr::Binary {
                left: Box::new(Expr::Field("order_total".into())),
                op: BinaryOp::Ge,
                right: Box::new(Expr::Literal(Value::Float(50.0))),
            },
        ),
        // Rule 2: Prime discount eligibility
        (
            "2_prime_discount",
            Expr::Binary {
                left: Box::new(Expr::Binary {
                    left: Box::new(Expr::Field("is_prime_member".into())),
                    op: BinaryOp::Eq,
                    right: Box::new(Expr::Literal(Value::Bool(true))),
                }),
                op: BinaryOp::And,
                right: Box::new(Expr::Binary {
                    left: Box::new(Expr::Field("order_total".into())),
                    op: BinaryOp::Ge,
                    right: Box::new(Expr::Literal(Value::Float(25.0))),
                }),
            },
        ),
        // Rule 3: VIP treatment (tier + history)
        (
            "3_vip_treatment",
            Expr::Binary {
                left: Box::new(Expr::Binary {
                    left: Box::new(Expr::Field("customer_tier".into())),
                    op: BinaryOp::Ge,
                    right: Box::new(Expr::Literal(Value::Int(3))),
                }),
                op: BinaryOp::And,
                right: Box::new(Expr::Binary {
                    left: Box::new(Expr::Field("total_purchases".into())),
                    op: BinaryOp::Ge,
                    right: Box::new(Expr::Literal(Value::Float(1000.0))),
                }),
            },
        ),
        // Rule 4: Fraud detection (complex)
        (
            "4_fraud_check",
            Expr::Binary {
                left: Box::new(Expr::Binary {
                    left: Box::new(Expr::Binary {
                        left: Box::new(Expr::Field("order_total".into())),
                        op: BinaryOp::Gt,
                        right: Box::new(Expr::Literal(Value::Float(500.0))),
                    }),
                    op: BinaryOp::And,
                    right: Box::new(Expr::Binary {
                        left: Box::new(Expr::Field("account_age_days".into())),
                        op: BinaryOp::Lt,
                        right: Box::new(Expr::Literal(Value::Int(30))),
                    }),
                }),
                op: BinaryOp::And,
                right: Box::new(Expr::Binary {
                    left: Box::new(Expr::Binary {
                        left: Box::new(Expr::Field("item_count".into())),
                        op: BinaryOp::Gt,
                        right: Box::new(Expr::Literal(Value::Int(10))),
                    }),
                    op: BinaryOp::And,
                    right: Box::new(Expr::Binary {
                        left: Box::new(Expr::Field("is_gift".into())),
                        op: BinaryOp::Eq,
                        right: Box::new(Expr::Literal(Value::Bool(true))),
                    }),
                }),
            },
        ),
    ]
}

// ============================================================================
// Helper Functions
// ============================================================================

fn create_loan_context() -> LoanContext {
    LoanContext {
        credit_score: 720,
        annual_income: 85000.0,
        employment_years: 5,
        age: 35,
        loan_amount: 250000.0,
        loan_term_months: 360,
        interest_rate: 0.065,
        debt_to_income: 0.32,
        loan_to_value: 0.75,
        has_collateral: true,
        is_first_time_buyer: false,
    }
}

fn create_loan_value_context() -> Context {
    let value = Value::object(std::collections::HashMap::from([
        ("credit_score".to_string(), Value::Int(720)),
        ("annual_income".to_string(), Value::Float(85000.0)),
        ("employment_years".to_string(), Value::Int(5)),
        ("age".to_string(), Value::Int(35)),
        ("loan_amount".to_string(), Value::Float(250000.0)),
        ("loan_term_months".to_string(), Value::Int(360)),
        ("interest_rate".to_string(), Value::Float(0.065)),
        ("debt_to_income".to_string(), Value::Float(0.32)),
        ("loan_to_value".to_string(), Value::Float(0.75)),
        ("has_collateral".to_string(), Value::Bool(true)),
        ("is_first_time_buyer".to_string(), Value::Bool(false)),
    ]));
    Context::new(value)
}

fn create_order_context() -> OrderContext {
    OrderContext {
        order_total: 156.99,
        item_count: 5,
        shipping_cost: 9.99,
        discount_percent: 10.0,
        customer_tier: 3,
        total_purchases: 2500.0,
        account_age_days: 730,
        is_prime_member: true,
        has_coupon: false,
        is_gift: false,
    }
}

fn create_order_value_context() -> Context {
    let value = Value::object(std::collections::HashMap::from([
        ("order_total".to_string(), Value::Float(156.99)),
        ("item_count".to_string(), Value::Int(5)),
        ("shipping_cost".to_string(), Value::Float(9.99)),
        ("discount_percent".to_string(), Value::Float(10.0)),
        ("customer_tier".to_string(), Value::Int(3)),
        ("total_purchases".to_string(), Value::Float(2500.0)),
        ("account_age_days".to_string(), Value::Int(730)),
        ("is_prime_member".to_string(), Value::Bool(true)),
        ("has_coupon".to_string(), Value::Bool(false)),
        ("is_gift".to_string(), Value::Bool(false)),
    ]));
    Context::new(value)
}

// ============================================================================
// Benchmarks
// ============================================================================

/// Benchmark: Loan Rules - Tree vs VM vs JIT
fn bench_loan_rules(c: &mut Criterion) {
    let mut group = c.benchmark_group("loan_rules");
    group.sample_size(100);

    // Contexts
    let typed_ctx = create_loan_context();
    let value_ctx = create_loan_value_context();

    // Evaluators
    let tree_eval = Evaluator::new();
    let vm = BytecodeVM::new();
    let mut jit_compiler = SchemaJITCompiler::new().unwrap();
    let schema = LoanContext::schema();

    let rules = create_loan_rules();

    for (name, expr) in &rules {
        // Pre-compile for VM
        let bc_compiler = ExprCompiler::new();
        let compiled_bc = bc_compiler.compile(expr);

        // Pre-compile for JIT
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        format!("{:?}", expr).hash(&mut hasher);
        let hash = hasher.finish();
        let compiled_jit = jit_compiler
            .compile_with_schema(expr, hash, schema)
            .unwrap();

        // Count conditions for throughput
        let condition_count = count_conditions(expr);
        group.throughput(Throughput::Elements(condition_count as u64));

        // Tree-walk
        group.bench_with_input(
            BenchmarkId::new("tree", *name),
            &(&tree_eval, &value_ctx, expr),
            |b, (eval, ctx, expr)| {
                b.iter(|| black_box(eval.eval(black_box(*expr), black_box(*ctx))));
            },
        );

        // BytecodeVM
        group.bench_with_input(
            BenchmarkId::new("vm", *name),
            &(&vm, &value_ctx, &compiled_bc),
            |b, (vm, ctx, compiled)| {
                b.iter(|| black_box(vm.execute(black_box(*compiled), black_box(*ctx))));
            },
        );

        // Schema JIT
        group.bench_with_input(
            BenchmarkId::new("jit", *name),
            &(compiled_jit, &typed_ctx),
            |b, (compiled, ctx)| {
                b.iter(|| black_box(unsafe { compiled.call_typed(black_box(*ctx)) }));
            },
        );
    }

    group.finish();
}

/// Benchmark: Order Rules - Tree vs VM vs JIT
fn bench_order_rules(c: &mut Criterion) {
    let mut group = c.benchmark_group("order_rules");
    group.sample_size(100);

    // Contexts
    let typed_ctx = create_order_context();
    let value_ctx = create_order_value_context();

    // Evaluators
    let tree_eval = Evaluator::new();
    let vm = BytecodeVM::new();
    let mut jit_compiler = SchemaJITCompiler::new().unwrap();
    let schema = OrderContext::schema();

    let rules = create_order_rules();

    for (name, expr) in &rules {
        // Pre-compile for VM
        let bc_compiler = ExprCompiler::new();
        let compiled_bc = bc_compiler.compile(expr);

        // Pre-compile for JIT
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        format!("{:?}", expr).hash(&mut hasher);
        let hash = hasher.finish();
        let compiled_jit = jit_compiler
            .compile_with_schema(expr, hash, schema)
            .unwrap();

        // Tree-walk
        group.bench_with_input(
            BenchmarkId::new("tree", *name),
            &(&tree_eval, &value_ctx, expr),
            |b, (eval, ctx, expr)| {
                b.iter(|| black_box(eval.eval(black_box(*expr), black_box(*ctx))));
            },
        );

        // BytecodeVM
        group.bench_with_input(
            BenchmarkId::new("vm", *name),
            &(&vm, &value_ctx, &compiled_bc),
            |b, (vm, ctx, compiled)| {
                b.iter(|| black_box(vm.execute(black_box(*compiled), black_box(*ctx))));
            },
        );

        // Schema JIT
        group.bench_with_input(
            BenchmarkId::new("jit", *name),
            &(compiled_jit, &typed_ctx),
            |b, (compiled, ctx)| {
                b.iter(|| black_box(unsafe { compiled.call_typed(black_box(*ctx)) }));
            },
        );
    }

    group.finish();
}

/// Benchmark: Scaling with complexity
fn bench_complexity_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("complexity_scaling");
    group.sample_size(100);

    let typed_ctx = create_loan_context();
    let value_ctx = create_loan_value_context();

    let tree_eval = Evaluator::new();
    let vm = BytecodeVM::new();
    let mut jit_compiler = SchemaJITCompiler::new().unwrap();
    let schema = LoanContext::schema();

    // Create expressions with increasing AND chain length
    for num_conditions in [1, 2, 4, 6, 8, 10] {
        let expr = create_and_chain(num_conditions);

        let bc_compiler = ExprCompiler::new();
        let compiled_bc = bc_compiler.compile(&expr);

        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        format!("{:?}", expr).hash(&mut hasher);
        let compiled_jit = jit_compiler
            .compile_with_schema(&expr, hasher.finish(), schema)
            .unwrap();

        group.throughput(Throughput::Elements(num_conditions as u64));

        group.bench_with_input(
            BenchmarkId::new("tree", num_conditions),
            &num_conditions,
            |b, _| b.iter(|| black_box(tree_eval.eval(black_box(&expr), black_box(&value_ctx)))),
        );

        group.bench_with_input(
            BenchmarkId::new("vm", num_conditions),
            &num_conditions,
            |b, _| b.iter(|| black_box(vm.execute(black_box(&compiled_bc), black_box(&value_ctx)))),
        );

        group.bench_with_input(
            BenchmarkId::new("jit", num_conditions),
            &num_conditions,
            |b, _| b.iter(|| black_box(unsafe { compiled_jit.call_typed(black_box(&typed_ctx)) })),
        );
    }

    group.finish();
}

/// Benchmark: Throughput (operations per second)
fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");
    group.sample_size(100);

    let typed_ctx = create_loan_context();
    let value_ctx = create_loan_value_context();

    // Medium complexity rule
    let expr = Expr::Binary {
        left: Box::new(Expr::Binary {
            left: Box::new(Expr::Field("credit_score".into())),
            op: BinaryOp::Ge,
            right: Box::new(Expr::Literal(Value::Int(700))),
        }),
        op: BinaryOp::And,
        right: Box::new(Expr::Binary {
            left: Box::new(Expr::Field("debt_to_income".into())),
            op: BinaryOp::Lt,
            right: Box::new(Expr::Literal(Value::Float(0.40))),
        }),
    };

    let tree_eval = Evaluator::new();
    let vm = BytecodeVM::new();
    let bc_compiler = ExprCompiler::new();
    let compiled_bc = bc_compiler.compile(&expr);

    let mut jit_compiler = SchemaJITCompiler::new().unwrap();
    let compiled_jit = jit_compiler
        .compile_with_schema(&expr, 12345, LoanContext::schema())
        .unwrap();

    group.throughput(Throughput::Elements(1));

    group.bench_function("tree_1k", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                black_box(
                    tree_eval
                        .eval(black_box(&expr), black_box(&value_ctx))
                        .unwrap(),
                );
            }
        });
    });

    group.bench_function("vm_1k", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                black_box(
                    vm.execute(black_box(&compiled_bc), black_box(&value_ctx))
                        .unwrap(),
                );
            }
        });
    });

    group.bench_function("jit_1k", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                black_box(unsafe { compiled_jit.call_typed(black_box(&typed_ctx)).unwrap() });
            }
        });
    });

    group.finish();
}

// ============================================================================
// Helper Functions
// ============================================================================

fn count_conditions(expr: &Expr) -> usize {
    match expr {
        Expr::Binary { left, op, right } => {
            match op {
                BinaryOp::And | BinaryOp::Or => count_conditions(left) + count_conditions(right),
                _ => 1, // Comparison operators count as 1 condition
            }
        }
        _ => 0,
    }
}

fn create_and_chain(n: usize) -> Expr {
    let fields = [
        "credit_score",
        "annual_income",
        "employment_years",
        "age",
        "debt_to_income",
        "loan_to_value",
        "loan_amount",
        "interest_rate",
        "loan_term_months",
        "has_collateral",
    ];
    let thresholds = [700, 50000, 2, 25, 43, 80, 100000, 8, 120, 1];

    let mut expr = Expr::Binary {
        left: Box::new(Expr::Field(fields[0].into())),
        op: BinaryOp::Ge,
        right: Box::new(Expr::Literal(Value::Int(thresholds[0] as i64))),
    };

    for i in 1..n.min(10) {
        let next = Expr::Binary {
            left: Box::new(Expr::Field(fields[i].into())),
            op: if i % 2 == 0 {
                BinaryOp::Ge
            } else {
                BinaryOp::Lt
            },
            right: Box::new(Expr::Literal(Value::Int(thresholds[i] as i64))),
        };
        expr = Expr::Binary {
            left: Box::new(expr),
            op: BinaryOp::And,
            right: Box::new(next),
        };
    }

    expr
}

criterion_group!(
    benches,
    bench_loan_rules,
    bench_order_rules,
    bench_complexity_scaling,
    bench_throughput,
);

criterion_main!(benches);
