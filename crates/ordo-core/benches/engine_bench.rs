//! Ordo engine benchmarks

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ordo_core::prelude::*;
use std::hint::black_box;

/// Create a simple rule set for benchmarking
fn create_simple_ruleset() -> RuleSet {
    let mut ruleset = RuleSet::new("bench_simple", "start");

    ruleset.add_step(
        Step::decision("start", "Start")
            .branch(Condition::from_string("value > 50"), "high")
            .default("low")
            .build(),
    );

    ruleset.add_step(Step::terminal(
        "high",
        "High",
        TerminalResult::new("HIGH").with_output("result", Expr::literal("high")),
    ));

    ruleset.add_step(Step::terminal(
        "low",
        "Low",
        TerminalResult::new("LOW").with_output("result", Expr::literal("low")),
    ));

    ruleset
}

/// Create a complex rule set with multiple steps
fn create_complex_ruleset() -> RuleSet {
    let mut ruleset = RuleSet::new("bench_complex", "check_amount");

    // Step 1: Check amount
    ruleset.add_step(
        Step::decision("check_amount", "Check Amount")
            .branch(Condition::from_string("amount >= 10000"), "vip_check")
            .branch(Condition::from_string("amount >= 1000"), "premium_check")
            .default("standard_check")
            .build(),
    );

    // Step 2a: VIP check
    ruleset.add_step(
        Step::decision("vip_check", "VIP Check")
            .branch(
                Condition::from_string("user.level == \"gold\""),
                "gold_discount",
            )
            .branch(
                Condition::from_string("user.level == \"silver\""),
                "silver_discount",
            )
            .default("vip_discount")
            .build(),
    );

    // Step 2b: Premium check
    ruleset.add_step(
        Step::decision("premium_check", "Premium Check")
            .branch(Condition::from_string("user.age >= 60"), "senior_discount")
            .branch(
                Condition::from_string("user.is_member == true"),
                "member_discount",
            )
            .default("normal_price")
            .build(),
    );

    // Step 2c: Standard check
    ruleset.add_step(
        Step::decision("standard_check", "Standard Check")
            .branch(
                Condition::from_string("coupon_code != null"),
                "coupon_discount",
            )
            .default("normal_price")
            .build(),
    );

    // Terminal steps
    ruleset.add_step(Step::terminal(
        "gold_discount",
        "Gold Discount",
        TerminalResult::new("GOLD")
            .with_output("discount", Expr::literal(0.25f64))
            .with_output(
                "final",
                Expr::binary(BinaryOp::Mul, Expr::field("amount"), Expr::literal(0.75f64)),
            ),
    ));

    ruleset.add_step(Step::terminal(
        "silver_discount",
        "Silver Discount",
        TerminalResult::new("SILVER")
            .with_output("discount", Expr::literal(0.20f64))
            .with_output(
                "final",
                Expr::binary(BinaryOp::Mul, Expr::field("amount"), Expr::literal(0.80f64)),
            ),
    ));

    ruleset.add_step(Step::terminal(
        "vip_discount",
        "VIP Discount",
        TerminalResult::new("VIP")
            .with_output("discount", Expr::literal(0.15f64))
            .with_output(
                "final",
                Expr::binary(BinaryOp::Mul, Expr::field("amount"), Expr::literal(0.85f64)),
            ),
    ));

    ruleset.add_step(Step::terminal(
        "senior_discount",
        "Senior Discount",
        TerminalResult::new("SENIOR")
            .with_output("discount", Expr::literal(0.10f64))
            .with_output(
                "final",
                Expr::binary(BinaryOp::Mul, Expr::field("amount"), Expr::literal(0.90f64)),
            ),
    ));

    ruleset.add_step(Step::terminal(
        "member_discount",
        "Member Discount",
        TerminalResult::new("MEMBER")
            .with_output("discount", Expr::literal(0.05f64))
            .with_output(
                "final",
                Expr::binary(BinaryOp::Mul, Expr::field("amount"), Expr::literal(0.95f64)),
            ),
    ));

    ruleset.add_step(Step::terminal(
        "coupon_discount",
        "Coupon Discount",
        TerminalResult::new("COUPON")
            .with_output("discount", Expr::literal(0.08f64))
            .with_output(
                "final",
                Expr::binary(BinaryOp::Mul, Expr::field("amount"), Expr::literal(0.92f64)),
            ),
    ));

    ruleset.add_step(Step::terminal(
        "normal_price",
        "Normal Price",
        TerminalResult::new("NORMAL")
            .with_output("discount", Expr::literal(0.0f64))
            .with_output("final", Expr::field("amount")),
    ));

    ruleset
}

fn bench_expression_parsing(c: &mut Criterion) {
    let expressions = vec![
        ("simple_compare", "age > 18"),
        ("logical_and", "age > 18 && status == \"active\""),
        (
            "complex_condition",
            "amount >= 1000 && user.level in [\"gold\", \"silver\"] && !is_blocked",
        ),
        ("function_call", "len(items) > 0 && sum(items) > 100"),
        (
            "conditional",
            "if exists(discount) then price * discount else price",
        ),
        ("coalesce", "coalesce(appid, in_appid, default_appid)"),
    ];

    let mut group = c.benchmark_group("expression_parsing");
    for (name, expr) in expressions {
        group.bench_with_input(BenchmarkId::new("parse", name), expr, |b, expr| {
            b.iter(|| ExprParser::parse(black_box(expr)))
        });
    }
    group.finish();
}

fn bench_expression_evaluation(c: &mut Criterion) {
    let evaluator = Evaluator::new();
    let ctx = Context::from_json(
        r#"{
        "age": 25,
        "status": "active",
        "amount": 5000,
        "user": {"level": "gold", "age": 35, "is_member": true},
        "items": [10, 20, 30, 40, 50],
        "is_blocked": false,
        "price": 100,
        "discount": 0.1,
        "appid": "wx123"
    }"#,
    )
    .unwrap();

    let expressions = vec![
        ("simple_compare", ExprParser::parse("age > 18").unwrap()),
        (
            "logical_and",
            ExprParser::parse("age > 18 && status == \"active\"").unwrap(),
        ),
        (
            "field_access",
            ExprParser::parse("user.level == \"gold\"").unwrap(),
        ),
        (
            "function_call",
            ExprParser::parse("len(items) > 0").unwrap(),
        ),
        (
            "arithmetic",
            ExprParser::parse("price * (1 - discount)").unwrap(),
        ),
        (
            "conditional",
            ExprParser::parse("if age > 18 then \"adult\" else \"minor\"").unwrap(),
        ),
    ];

    let mut group = c.benchmark_group("expression_evaluation");
    for (name, expr) in expressions {
        group.bench_with_input(BenchmarkId::new("eval", name), &expr, |b, expr| {
            b.iter(|| evaluator.eval(black_box(expr), black_box(&ctx)))
        });
    }
    group.finish();
}

fn bench_rule_execution(c: &mut Criterion) {
    let simple_ruleset = create_simple_ruleset();
    let complex_ruleset = create_complex_ruleset();
    let executor = RuleExecutor::new();

    let simple_input: Value = serde_json::from_str(r#"{"value": 75}"#).unwrap();
    let complex_input: Value = serde_json::from_str(
        r#"{
        "amount": 15000,
        "user": {"level": "gold", "age": 35, "is_member": true},
        "coupon_code": null
    }"#,
    )
    .unwrap();

    let mut group = c.benchmark_group("rule_execution");

    // Simple ruleset (2 steps)
    group.throughput(Throughput::Elements(1));
    group.bench_function("simple_ruleset", |b| {
        b.iter(|| executor.execute(black_box(&simple_ruleset), black_box(simple_input.clone())))
    });

    // Complex ruleset (3+ steps)
    group.bench_function("complex_ruleset", |b| {
        b.iter(|| {
            executor.execute(
                black_box(&complex_ruleset),
                black_box(complex_input.clone()),
            )
        })
    });

    group.finish();
}

fn bench_throughput(c: &mut Criterion) {
    let ruleset = create_simple_ruleset();
    let executor = RuleExecutor::new();

    let mut group = c.benchmark_group("throughput");
    group.throughput(Throughput::Elements(1000));
    group.sample_size(50);

    group.bench_function("1000_executions", |b| {
        let inputs: Vec<Value> = (0..1000)
            .map(|i| serde_json::from_str(&format!(r#"{{"value": {}}}"#, i % 100)).unwrap())
            .collect();

        b.iter(|| {
            for input in &inputs {
                let _ = executor.execute(black_box(&ruleset), black_box(input.clone()));
            }
        })
    });

    group.finish();
}

fn bench_builtin_functions(c: &mut Criterion) {
    let registry = FunctionRegistry::new();

    let mut group = c.benchmark_group("builtin_functions");

    // String functions
    let string_val = Value::string("Hello, World!");
    group.bench_function("len_string", |b| {
        b.iter(|| registry.call("len", black_box(std::slice::from_ref(&string_val))))
    });

    group.bench_function("upper", |b| {
        b.iter(|| registry.call("upper", black_box(std::slice::from_ref(&string_val))))
    });

    // Array functions
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

    // Math functions
    group.bench_function("abs", |b| {
        b.iter(|| registry.call("abs", black_box(&[Value::int(-42)])))
    });

    group.bench_function("min", |b| {
        b.iter(|| {
            registry.call(
                "min",
                black_box(&[Value::int(10), Value::int(20), Value::int(5)]),
            )
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_expression_parsing,
    bench_expression_evaluation,
    bench_rule_execution,
    bench_throughput,
    bench_builtin_functions,
);

criterion_main!(benches);
