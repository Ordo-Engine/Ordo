//! Assignments, outputs and other expression slots accept expression strings,
//! the same syntax conditions already use, instead of only the JSON AST.

use ordo_core::prelude::*;

const PRICING: &str = r#"{
  "config": { "name": "pricing", "version": "1.0.0", "entry_step": "calc" },
  "steps": {
    "calc": { "id": "calc", "name": "calc", "type": "action",
      "actions": [
        { "action": "set_variable", "name": "subtotal", "value": "price * qty" },
        { "action": "set_variable", "name": "discount",
          "value": "if vip then 0.15 else 0" }
      ],
      "next_step": "check" },
    "check": { "id": "check", "name": "check", "type": "decision",
      "branches": [ { "condition": "$subtotal >= 100", "next_step": "big",
        "actions": [ { "action": "set_variable", "name": "tier", "value": "\"big\"" } ] } ],
      "default_next": "small" },
    "big": { "id": "big", "name": "big", "type": "terminal", "result": {
      "code": "BIG",
      "output": [
        ["total", "round($subtotal * (1 - $discount), 2)"],
        ["tier", "$tier"],
        ["mixed", {"Binary": {"op": "Add", "left": "$subtotal", "right": {"Literal": 1}}}]
      ] } },
    "small": { "id": "small", "name": "small", "type": "terminal", "result": {
      "code": "SMALL", "output": [["total", "$subtotal"]] } }
  }
}"#;

fn run(ruleset: &RuleSet, input: &str) -> ExecutionResult {
    let input: Value = serde_json::from_str(input).unwrap();
    RuleExecutor::new().execute(ruleset, input).unwrap()
}

#[test]
fn json_ruleset_with_expression_strings() {
    let ruleset = RuleSet::from_json_compiled(PRICING).unwrap();

    let big = run(&ruleset, r#"{"price": 40, "qty": 3, "vip": true}"#);
    assert_eq!(big.code, "BIG");
    assert_eq!(big.output.get_path("total"), Some(&Value::float(102.0)));
    assert_eq!(big.output.get_path("tier"), Some(&Value::string("big")));
    assert_eq!(big.output.get_path("mixed"), Some(&Value::int(121)));

    let small = run(&ruleset, r#"{"price": 10, "qty": 3, "vip": false}"#);
    assert_eq!(small.code, "SMALL");
    assert_eq!(small.output.get_path("total"), Some(&Value::int(30)));
}

#[test]
fn yaml_ruleset_with_expression_strings() {
    let yaml = r#"
config:
  name: pricing
  version: 1.0.0
  entry_step: calc
steps:
  calc:
    id: calc
    name: calc
    type: action
    actions:
      - action: set_variable
        name: subtotal
        value: price * qty
    next_step: done
  done:
    id: done
    name: done
    type: terminal
    result:
      code: OK
      output:
        - [total, $subtotal]
        - [ast, {Field: price}]
"#;
    let ruleset = RuleSet::from_yaml_compiled(yaml).unwrap();
    let result = run(&ruleset, r#"{"price": 2.5, "qty": 4}"#);
    assert_eq!(result.output.get_path("total"), Some(&Value::float(10.0)));
    assert_eq!(result.output.get_path("ast"), Some(&Value::float(2.5)));
}

#[test]
fn expression_strings_serialize_as_ast() {
    let ruleset = RuleSet::from_json_compiled(PRICING).unwrap();
    let json = serde_json::to_string(&ruleset).unwrap();
    assert!(json.contains(r#""value":{"Binary":{"op":"Mul""#), "{json}");

    // and the AST form loads back to the same behavior
    let reloaded = RuleSet::from_json_compiled(&json).unwrap();
    let result = run(&reloaded, r#"{"price": 40, "qty": 3, "vip": true}"#);
    assert_eq!(result.output.get_path("total"), Some(&Value::float(102.0)));
}

#[test]
fn invalid_expression_string_names_the_expression() {
    let bad = PRICING.replace(r#""price * qty""#, r#""price * * qty""#);
    let err = RuleSet::from_json_compiled(&bad).unwrap_err().to_string();
    assert!(err.contains("invalid expression `price * * qty`"), "{err}");
}
