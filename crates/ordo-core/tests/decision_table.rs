//! Engine-native decision tables: hit policies, cell syntax, and loading.

use ordo_core::prelude::*;
use ordo_core::rule::RuleSetCompiler;

fn ruleset(table: &str) -> RuleSet {
    let json = format!(
        r#"{{
          "config": {{ "name": "t", "version": "1.0.0", "entry_step": "table" }},
          "steps": {{
            "table": {table},
            "done": {{ "id": "done", "name": "done", "type": "terminal", "result": {{
              "code": "OK",
              "output": [
                ["a", {{"Field": "$a"}}],
                ["b", {{"Coalesce": [{{"Field": "$b"}}, {{"Literal": null}}]}}]
              ] }} }}
          }}
        }}"#
    );
    RuleSet::from_json_compiled(&json).unwrap()
}

fn run(ruleset: &RuleSet, input: &str) -> Result<ExecutionResult> {
    let input: Value = serde_json::from_str(input).unwrap();
    RuleExecutor::new().execute(ruleset, input)
}

fn outputs(ruleset: &RuleSet, input: &str) -> (Value, Value) {
    let result = run(ruleset, input).unwrap();
    (
        result.output.get_path("a").cloned().unwrap_or(Value::Null),
        result.output.get_path("b").cloned().unwrap_or(Value::Null),
    )
}

const DISCOUNT: &str = r#"{
  "id": "table", "name": "Discount", "type": "decision_table",
  "inputs": ["tier", "amount"],
  "outputs": ["a", "b"],
  "rules": [
    { "when": ["gold", ">= 1000"], "then": [0.15, "gold large"] },
    { "when": [["silver", "bronze"], "100..999"], "then": [0.05, "= tier + \" mid\""] },
    { "when": ["*", "= amount < 0 || tier == \"blocked\""], "then": [0, "invalid"] },
    { "when": ["-", "> 5000"], "then": [0.02, "any tier, huge"] }
  ],
  "default": [0, "none"],
  "next_step": "done"
}"#;

#[test]
fn first_hit_policy() {
    let rs = ruleset(DISCOUNT);
    let cases = [
        (r#"{"tier": "gold", "amount": 1500}"#, 0.15, "gold large"),
        (r#"{"tier": "silver", "amount": 100}"#, 0.05, "silver mid"),
        (r#"{"tier": "bronze", "amount": 999.5}"#, 0.0, "none"),
        (
            r#"{"tier": "basic", "amount": 6000}"#,
            0.02,
            "any tier, huge",
        ),
        (r#"{"tier": "gold", "amount": 50}"#, 0.0, "none"),
    ];
    for (input, a, b) in cases {
        let (got_a, got_b) = outputs(&rs, input);
        assert_eq!(got_a, Value::float(a), "{input}");
        assert_eq!(got_b, Value::string(b), "{input}");
    }
}

#[test]
fn first_without_default_fails_closed() {
    let table = DISCOUNT.replace(r#""default": [0, "none"],"#, "");
    let rs = ruleset(&table);
    let err = run(&rs, r#"{"tier": "gold", "amount": 50}"#).unwrap_err();
    assert!(
        err.to_string()
            .contains("No matching row in decision table 'table' and no default"),
        "{err}"
    );
}

const RISK: &str = r#"{
  "id": "table", "name": "Risk", "type": "decision_table",
  "hit_policy": "collect", "aggregate": "AGG",
  "inputs": ["age", "country", "amount"],
  "outputs": ["a"],
  "rules": [
    { "when": ["< 21", "*", "*"], "then": [30] },
    { "when": ["*", "not in [\"CN\", \"US\"]", "*"], "then": [20] },
    { "when": ["*", "*", ">= 10000"], "then": [50] }
  ],
  "next_step": "done"
}"#;

#[test]
fn collect_with_aggregates() {
    let risky = r#"{"age": 19, "country": "XX", "amount": 20000}"#;
    let safe = r#"{"age": 40, "country": "CN", "amount": 100}"#;
    let expect = [
        ("sum", Value::int(100), Value::int(0)),
        ("count", Value::int(3), Value::int(0)),
        ("min", Value::int(20), Value::Null),
        ("max", Value::int(50), Value::Null),
    ];
    for (aggregate, risky_expected, safe_expected) in expect {
        let rs = ruleset(&RISK.replace("AGG", aggregate));
        assert_eq!(outputs(&rs, risky).0, risky_expected, "{aggregate}");
        assert_eq!(outputs(&rs, safe).0, safe_expected, "{aggregate}");
    }

    let rs = ruleset(&RISK.replace(r#""aggregate": "AGG","#, ""));
    assert_eq!(
        outputs(&rs, risky).0,
        Value::array(vec![Value::int(30), Value::int(20), Value::int(50)])
    );
    assert_eq!(outputs(&rs, safe).0, Value::array(vec![]));
}

#[test]
fn loads_from_yaml_and_round_trips_as_written() {
    let yaml = r#"
config:
  name: t
  version: 1.0.0
  entry_step: table
steps:
  table:
    id: table
    name: Tier
    type: decision_table
    inputs: [score]
    outputs: [a]
    rules:
      - when: [">= 700"]
        then: [A]
      - when: ["*"]
        then: [B]
    next_step: done
  done:
    id: done
    name: done
    type: terminal
    result:
      code: OK
      output:
        - [a, {Field: $a}]
"#;
    let rs = RuleSet::from_yaml_compiled(yaml).unwrap();
    assert_eq!(outputs(&rs, r#"{"score": 720}"#).0, Value::string("A"));
    assert_eq!(outputs(&rs, r#"{"score": 600}"#).0, Value::string("B"));

    let json = serde_json::to_string(&rs).unwrap();
    assert!(
        json.contains(r#""rules":[{"when":[">= 700"],"then":["A"]}"#),
        "{json}"
    );
    let reloaded = RuleSet::from_json_compiled(&json).unwrap();
    assert_eq!(
        outputs(&reloaded, r#"{"score": 720}"#).0,
        Value::string("A")
    );
}

#[test]
fn invalid_tables_are_rejected_on_load() {
    let bad = DISCOUNT.replace(r#""100..999""#, r#""> )""#);
    let json = format!(
        r#"{{"config": {{"name": "t", "version": "1", "entry_step": "table"}},
            "steps": {{"table": {bad}}}}}"#
    );
    let err = RuleSet::from_json_compiled(&json).unwrap_err().to_string();
    assert!(err.contains("row 2, input `amount`"), "{err}");
}

#[test]
fn compiled_format_reports_unsupported() {
    let rs = ruleset(DISCOUNT);
    let err = RuleSetCompiler::compile(&rs).unwrap_err().to_string();
    assert!(err.contains("decision_table"), "{err}");
}
