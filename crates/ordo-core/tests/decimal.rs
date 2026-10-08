//! Exact decimal arithmetic for money.

use ordo_core::prelude::*;

/// Evaluate on both the interpreter and the bytecode VM and require agreement.
fn eval(src: &str, input: &str) -> Value {
    let expr = ExprParser::parse(src).unwrap();
    let ctx = Context::from_json(input).unwrap();
    let interp = Evaluator::new().eval(&expr, &ctx).unwrap();
    let compiled = ExprCompiler::new().compile(&expr).unwrap();
    let vm = BytecodeVM::new().execute(&compiled, &ctx).unwrap();
    assert_eq!(interp, vm, "interpreter and VM disagree on `{src}`");
    assert_eq!(interp.type_name(), vm.type_name(), "`{src}`");
    interp
}

fn dec(s: &str) -> Value {
    Value::decimal(s.parse().unwrap())
}

#[test]
fn decimal_arithmetic_is_exact() {
    assert_eq!(eval("decimal(0.1) + decimal(0.2)", "{}"), dec("0.3"));
    assert_eq!(eval("decimal(0.1) + 0.2", "{}"), dec("0.3"));
    assert_eq!(eval("decimal(\"19.99\") * 3", "{}"), dec("59.97"));
    assert_eq!(eval("decimal(100) / 3 * 3", "{}"), dec("100"));
    assert_eq!(eval("decimal(10) - 0.01", "{}"), dec("9.99"));
    assert_eq!(eval("decimal(10) % 3", "{}"), dec("1"));
    assert_eq!(eval("-decimal(1.5)", "{}"), dec("-1.5"));
    assert_eq!(eval("decimal(0.1) + 0.2 == 0.3", "{}"), Value::bool(true));
    assert_eq!(eval("0.1 + 0.2 == 0.3", "{}"), Value::bool(false));
}

#[test]
fn decimal_compares_with_other_numbers() {
    assert_eq!(eval("decimal(\"10.00\") == 10", "{}"), Value::bool(true));
    assert_eq!(eval("decimal(\"10.5\") > 10", "{}"), Value::bool(true));
    assert_eq!(
        eval("decimal(\"0.3\") in [0.3, 1]", "{}"),
        Value::bool(true)
    );
    assert_eq!(eval("max(decimal(1.5), 2, 0.5)", "{}"), Value::int(2));
}

#[test]
fn decimal_functions() {
    assert_eq!(eval("round(decimal(\"2.675\"), 2)", "{}"), dec("2.68"));
    assert_eq!(eval("round(decimal(\"-2.5\"))", "{}"), Value::int(-3));
    assert_eq!(eval("floor(decimal(\"2.9\"))", "{}"), Value::int(2));
    assert_eq!(eval("ceil(decimal(\"2.1\"))", "{}"), Value::int(3));
    assert_eq!(eval("abs(decimal(\"-2.1\"))", "{}"), dec("2.1"));
    assert_eq!(eval("sum([decimal(19.99), 0.01, 5])", "{}"), dec("25"));
    assert_eq!(eval("avg([decimal(1), 2])", "{}"), dec("1.5"));
    assert_eq!(eval("type(decimal(1))", "{}"), Value::string("decimal"));
    assert_eq!(
        eval("to_string(decimal(\"1.50\"))", "{}"),
        Value::string("1.5")
    );
    assert_eq!(eval("decimal(\"1e3\")", "{}"), dec("1000"));
}

#[test]
fn decimal_errors() {
    let ctx = Context::from_json("{}").unwrap();
    for src in ["decimal(\"abc\")", "decimal(1) / 0", "decimal(1) + \"x\""] {
        let expr = ExprParser::parse(src).unwrap();
        assert!(Evaluator::new().eval(&expr, &ctx).is_err(), "{src}");
    }
}

#[test]
fn decimal_serializes_as_json_number() {
    let v = dec("16.99");
    assert_eq!(serde_json::to_string(&v).unwrap(), "16.99");
}
