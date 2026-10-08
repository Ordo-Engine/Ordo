//! Numeric semantics that business rules depend on: the same input must give
//! the same decision whether a client serializes a number as `10` or `10.0`,
//! and the interpreter, bytecode VM and rule executor must agree.

use ordo_core::prelude::*;

/// Evaluate `src` against `input` on both the tree-walking interpreter and the
/// bytecode VM, assert they agree, and return the result.
fn eval_both(src: &str, input: &str) -> Value {
    let expr = ExprParser::parse(src).unwrap();
    let ctx = Context::from_json(input).unwrap();

    let interp = Evaluator::new().eval(&expr, &ctx).unwrap();
    let compiled = ExprCompiler::new().compile(&expr).unwrap();
    let vm = BytecodeVM::new().execute(&compiled, &ctx).unwrap();

    assert_eq!(interp, vm, "interpreter and VM disagree on `{src}`");
    assert_eq!(
        std::mem::discriminant(&interp),
        std::mem::discriminant(&vm),
        "interpreter and VM return different types for `{src}`"
    );
    interp
}

#[test]
fn int_and_float_with_same_value_are_equal() {
    for input in [r#"{"plan": 3}"#, r#"{"plan": 3.0}"#] {
        assert_eq!(eval_both("plan == 3", input), Value::bool(true), "{input}");
        assert_eq!(eval_both("plan != 3", input), Value::bool(false), "{input}");
        assert_eq!(
            eval_both("plan in [1, 2, 3]", input),
            Value::bool(true),
            "{input}"
        );
    }
    assert_eq!(
        eval_both("plan == 3", r#"{"plan": 3.5}"#),
        Value::bool(false)
    );
    assert_eq!(Value::int(10), Value::float(10.0));
    assert_ne!(Value::int(10), Value::float(10.5));
    // No precision loss for integers beyond 2^53.
    assert_ne!(
        Value::int(9_007_199_254_740_993),
        Value::float(9_007_199_254_740_992.0)
    );
    assert_ne!(Value::int(i64::MAX), Value::float(i64::MAX as f64));
}

#[test]
fn integer_division_does_not_truncate() {
    assert_eq!(
        eval_both("a / b", r#"{"a": 10, "b": 4}"#),
        Value::float(2.5)
    );
    assert_eq!(eval_both("a / b", r#"{"a": 10, "b": 5}"#), Value::int(2));
    assert_eq!(
        eval_both("a / b", r#"{"a": -7, "b": 2}"#),
        Value::float(-3.5)
    );
    assert_eq!(eval_both("10 / 4", "{}"), Value::float(2.5));
    assert_eq!(eval_both("floor(10 / 4)", "{}"), Value::int(2));
    // i64::MIN / -1 overflows i64; it must not panic.
    assert_eq!(
        eval_both("a / b", r#"{"a": -9223372036854775808, "b": -1}"#),
        Value::float(-(i64::MIN as f64))
    );
}

#[test]
fn round_to_decimal_places() {
    let cases = [
        ("round(1.005, 2)", Value::float(1.01)),
        ("round(2.675, 2)", Value::float(2.68)),
        ("round(16.9915, 2)", Value::float(16.99)),
        ("round(-1.005, 2)", Value::float(-1.01)),
        ("round(9.995, 2)", Value::float(10.0)),
        ("round(1.5, 0)", Value::float(2.0)),
        ("round(1.25, 5)", Value::float(1.25)),
        ("round(42, 2)", Value::int(42)),
        ("round(2.5)", Value::int(3)),
        ("round(-2.5)", Value::int(-3)),
    ];
    for (src, expected) in cases {
        assert_eq!(eval_both(src, "{}"), expected, "{src}");
    }
    let expr = ExprParser::parse("round(1.5, 16)").unwrap();
    assert!(Evaluator::new()
        .eval(&expr, &Context::from_json("{}").unwrap())
        .is_err());
}

/// A debt-to-income check: the decision must not depend on whether the client
/// sent integers or floats.
#[test]
fn ruleset_decision_is_independent_of_number_representation() {
    let ruleset = r#"{
      "config": { "name": "dti", "version": "1.0.0", "entry_step": "calc" },
      "steps": {
        "calc": { "id": "calc", "name": "calc", "type": "action",
          "actions": [ { "action": "set_variable", "name": "dti",
            "value": {"Binary": {"op": "Div", "left": {"Field": "debt"}, "right": {"Field": "income"}}} } ],
          "next_step": "check" },
        "check": { "id": "check", "name": "check", "type": "decision",
          "branches": [ { "condition": "$dti > 0.5", "next_step": "reject" } ],
          "default_next": "approve" },
        "approve": { "id": "approve", "name": "approve", "type": "terminal", "result": { "code": "APPROVED" } },
        "reject": { "id": "reject", "name": "reject", "type": "terminal", "result": { "code": "REJECTED" } }
      }
    }"#;
    let ruleset = RuleSet::from_json_compiled(ruleset).unwrap();
    let executor = RuleExecutor::new();

    for input in [
        r#"{"debt": 7000, "income": 10000}"#,
        r#"{"debt": 7000.0, "income": 10000}"#,
        r#"{"debt": 7000, "income": 10000.0}"#,
    ] {
        let input: Value = serde_json::from_str(input).unwrap();
        let result = executor.execute(&ruleset, input.clone()).unwrap();
        assert_eq!(result.code, "REJECTED", "input {input:?}");
    }
}

/// The bytecode VM (used for compiled `.ordo` rulesets) used to read function
/// arguments from the wrong registers when an argument was itself computed.
#[test]
fn vm_function_arguments_with_computed_values() {
    let cases = [
        ("max(1, 2 + 3)", Value::int(5)),
        ("abs(0 - 3)", Value::int(3)),
        ("floor(a / b)", Value::int(2)),
        ("min(a * 2, b + 1, 100)", Value::int(5)),
        ("max(abs(0 - a), b)", Value::int(10)),
        ("round(a / b * 1.1, 2)", Value::float(2.75)),
    ];
    for (src, expected) in cases {
        assert_eq!(eval_both(src, r#"{"a": 10, "b": 4}"#), expected, "{src}");
    }
}
