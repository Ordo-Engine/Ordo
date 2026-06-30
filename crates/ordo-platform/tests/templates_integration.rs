//! Integration test: load the on-disk `templates/` directory and validate
//! that every shipped template parses cleanly through ordo-core.
//!
//! This catches drift between the platform's template JSON and the engine's
//! RuleSet schema — a class of bug that otherwise only surfaces at runtime
//! when a user clicks "Create from template".

use ordo_core::rule::{RuleExecutor, RuleSet};
use ordo_platform::template::TemplateStore;

/// Every shipped template's engine-format ruleset.json must convert to a valid
/// *studio* draft (steps as an array, startStepId set) and round-trip back to a
/// compilable engine ruleset. This mirrors what `install_template_detail` does
/// when seeding a draft — drafts are canonical studio format (PR-D).
#[test]
fn templates_convert_to_studio_drafts_and_round_trip() {
    for tpl in ["ecommerce-coupon", "loan-approval"] {
        let path = format!("templates/{tpl}/ruleset.json");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
        let engine = RuleSet::from_json(&text).expect("engine ruleset parses");

        // engine → studio (what the draft is stored as)
        let studio = ordo_studio_format::engine_to_studio(&engine);
        let studio_json = serde_json::to_value(&studio).expect("studio serializes");
        assert!(
            studio_json["steps"].is_array(),
            "{tpl}: studio draft steps must be an array (got {:?})",
            studio_json["steps"]
        );
        assert!(
            studio_json["startStepId"]
                .as_str()
                .is_some_and(|s| !s.is_empty()),
            "{tpl}: studio draft must have a non-empty startStepId"
        );
        assert!(
            studio_json["config"]["entry_step"].is_null(),
            "{tpl}: studio draft must not carry the engine 'entry_step' key"
        );

        // studio → engine round-trips and compiles (every condition stays valid).
        let mut back: RuleSet = studio.try_into().expect("studio converts back to engine");
        assert_eq!(back.config.entry_step, engine.config.entry_step);
        assert_eq!(back.steps.len(), engine.steps.len());
        for step in back.steps.values_mut() {
            step.compile()
                .expect("every round-tripped condition compiles");
        }
    }
}

#[test]
fn ecommerce_coupon_ruleset_parses_with_ordo_core() {
    // The tests binary runs from the crate root, so this is a relative path.
    let path = std::path::Path::new("templates/ecommerce-coupon/ruleset.json");
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {:?}: {}", path, e));

    let ruleset = RuleSet::from_json(&text).expect("ruleset should parse");
    ruleset
        .validate()
        .expect("ruleset.validate() should pass (all referenced steps must exist)");

    assert_eq!(ruleset.config.entry_step, "check_eligibility");
    assert!(ruleset.steps.contains_key("terminal_vip_grant"));
    assert!(ruleset.steps.contains_key("terminal_normal_grant"));
    assert!(ruleset.steps.contains_key("terminal_no_coupon"));
    assert!(ruleset.steps.contains_key("terminal_not_eligible"));
}

#[test]
fn ecommerce_coupon_ruleset_compiles() {
    // `compile()` pre-parses every expression string — this is the same
    // call the engine's executor makes, so a passing test here proves
    // every branch condition is syntactically valid.
    let path = std::path::Path::new("templates/ecommerce-coupon/ruleset.json");
    let text = std::fs::read_to_string(path).unwrap();
    let mut ruleset = RuleSet::from_json(&text).unwrap();

    for step in ruleset.steps.values_mut() {
        step.compile().expect("every branch condition must compile");
    }
}

#[test]
fn loan_approval_ruleset_parses_with_ordo_core() {
    let path = std::path::Path::new("templates/loan-approval/ruleset.json");
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {:?}: {}", path, e));

    let ruleset = RuleSet::from_json(&text).expect("ruleset should parse");
    ruleset
        .validate()
        .expect("ruleset.validate() should pass (all referenced steps must exist)");

    assert_eq!(ruleset.config.entry_step, "screen_application");
    for terminal in [
        "terminal_approved",
        "terminal_manual_review",
        "terminal_rejected_bankruptcy",
        "terminal_rejected_credit",
        "terminal_rejected_affordability",
    ] {
        assert!(
            ruleset.steps.contains_key(terminal),
            "missing terminal step {terminal}"
        );
    }
}

#[test]
fn loan_approval_ruleset_compiles() {
    let path = std::path::Path::new("templates/loan-approval/ruleset.json");
    let text = std::fs::read_to_string(path).unwrap();
    let mut ruleset = RuleSet::from_json(&text).unwrap();

    for step in ruleset.steps.values_mut() {
        step.compile().expect("every branch condition must compile");
    }
}

/// Load the real `templates/` directory through the platform's TemplateStore
/// and assert the loan-approval template deserializes across every model
/// (facts, concepts, contract, samples, tests) and resolves i18n in all locales.
#[test]
fn loan_approval_loads_through_template_store() {
    let store = TemplateStore::load_from_dir(std::path::Path::new("templates"))
        .expect("templates dir loads");

    // Both shipped templates should be discoverable.
    let ids: Vec<String> = store.list("en").into_iter().map(|m| m.id).collect();
    assert!(ids.iter().any(|id| id == "ecommerce-coupon"));
    assert!(ids.iter().any(|id| id == "loan-approval"));

    let detail = store
        .get("loan-approval", "en")
        .expect("loan-approval resolves");
    assert_eq!(detail.metadata.id, "loan-approval");
    assert_eq!(detail.facts.len(), 7);
    assert_eq!(detail.concepts.len(), 4);
    assert_eq!(detail.tests.len(), 6);
    assert!(detail.contract.is_some());

    // After i18n resolution no `i18n:` sentinel should survive in any locale.
    for locale in ["en", "zh-CN", "zh-TW"] {
        let d = store
            .get("loan-approval", locale)
            .unwrap_or_else(|| panic!("loan-approval resolves for {locale}"));
        let json = serde_json::to_string(&d).expect("detail serializes");
        assert!(
            !json.contains("i18n:"),
            "{locale}: unresolved i18n sentinel(s) remain in loan-approval"
        );
    }
}

/// Execute every case in the template's `tests.json` through the engine and
/// assert the resulting code and outputs match. This proves the shipped
/// decision logic actually behaves as documented — not just that it parses.
#[test]
fn loan_approval_tests_execute_to_expected_results() {
    let ruleset_text =
        std::fs::read_to_string("templates/loan-approval/ruleset.json").expect("read ruleset");
    let ruleset = RuleSet::from_json_compiled(&ruleset_text).expect("ruleset compiles");

    let tests_text =
        std::fs::read_to_string("templates/loan-approval/tests.json").expect("read tests");
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(&tests_text).expect("tests.json is a JSON array");
    assert!(!cases.is_empty(), "tests.json should not be empty");

    let executor = RuleExecutor::new();
    for case in &cases {
        let id = case["id"].as_str().unwrap_or("<no id>");
        // The engine takes its own Value type; both it and serde_json::Value go
        // through serde, so deserialize the case input straight into it.
        let input: ordo_core::context::Value =
            serde_json::from_value(case["input"].clone()).expect("case input deserializes");
        let result = executor
            .execute(&ruleset, input)
            .unwrap_or_else(|e| panic!("case {id} failed to execute: {e}"));

        let expected_code = case["expect"]["code"].as_str().expect("expect.code");
        assert_eq!(
            result.code, expected_code,
            "case {id}: expected code {expected_code}, got {}",
            result.code
        );

        // Serialize the engine output back to JSON so we can compare fields.
        let output_json = serde_json::to_value(&result.output).expect("output serializes");
        if let Some(expected_output) = case["expect"]["output"].as_object() {
            for (key, want) in expected_output {
                let got = &output_json[key];
                assert_eq!(
                    got, want,
                    "case {id}: output[{key}] expected {want}, got {got}"
                );
            }
        }
    }
}
