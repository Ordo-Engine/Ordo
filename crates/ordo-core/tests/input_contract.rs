//! A ruleset's declared input schema is enforced before execution, so a
//! missing field can no longer silently skip a hard rule.

use ordo_core::prelude::*;

const RULESET: &str = r#"{
  "config": {
    "name": "credit", "version": "1.0.0", "entry_step": "hard_rules",
    "inputSchema": [
      {"name": "applicant", "type": "object", "required": true, "fields": [
        {"name": "age", "type": "number", "required": true},
        {"name": "credit_score", "type": "number", "required": true}
      ]},
      {"name": "channel", "type": "string", "defaultValue": "online"}
    ]
  },
  "steps": {
    "hard_rules": { "id": "hard_rules", "name": "Hard rules", "type": "decision",
      "branches": [
        { "condition": "applicant.age < 18", "next_step": "reject" },
        { "condition": "channel == \"online\" && applicant.credit_score >= 700", "next_step": "approve" }
      ],
      "default_next": "reject" },
    "approve": { "id": "approve", "name": "Approve", "type": "terminal", "result": { "code": "APPROVED" } },
    "reject": { "id": "reject", "name": "Reject", "type": "terminal", "result": { "code": "REJECTED" } }
  }
}"#;

fn run(input: &str) -> Result<ExecutionResult> {
    let ruleset = RuleSet::from_json_compiled(RULESET).unwrap();
    let input: Value = serde_json::from_str(input).unwrap();
    RuleExecutor::new().execute(&ruleset, input)
}

#[test]
fn missing_required_field_is_rejected_instead_of_skipping_the_rule() {
    match run(r#"{"applicant": {"credit_score": 760}}"#) {
        Err(OrdoError::InvalidInput { errors }) => {
            assert_eq!(errors, vec!["applicant.age: required field is missing"]);
        }
        other => panic!("expected InvalidInput, got {other:?}"),
    }
}

#[test]
fn wrong_type_is_rejected() {
    let err = run(r#"{"applicant": {"age": 30, "credit_score": "760"}}"#).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Invalid input: applicant.credit_score: expected number, got string"
    );
}

#[test]
fn defaults_are_applied_and_valid_input_runs() {
    let result = run(r#"{"applicant": {"age": 30, "credit_score": 760}}"#).unwrap();
    assert_eq!(result.code, "APPROVED");
    let result = run(r#"{"applicant": {"age": 16, "credit_score": 760}}"#).unwrap();
    assert_eq!(result.code, "REJECTED");
}

#[test]
fn ruleset_without_schema_is_unchanged() {
    let mut ruleset = RuleSet::from_json_compiled(RULESET).unwrap();
    ruleset.config.input_schema.clear();
    let input: Value = serde_json::from_str(r#"{"applicant": {"credit_score": 760}}"#).unwrap();
    // Lenient field-missing behaviour: the age check is skipped, `channel` is
    // missing too, so the approve branch is false and we fall through.
    let result = RuleExecutor::new().execute(&ruleset, input).unwrap();
    assert_eq!(result.code, "REJECTED");
}
