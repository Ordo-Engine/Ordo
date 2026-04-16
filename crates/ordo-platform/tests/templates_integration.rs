//! Integration test: load the on-disk `templates/` directory and validate
//! that every shipped template parses cleanly through ordo-core.
//!
//! This catches drift between the platform's template JSON and the engine's
//! RuleSet schema — a class of bug that otherwise only surfaces at runtime
//! when a user clicks "Create from template".

use ordo_core::rule::RuleSet;

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
