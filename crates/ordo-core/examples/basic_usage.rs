//! Basic usage example for Ordo rule engine
//!
//! This example demonstrates how to create and execute a simple discount rule.

use ordo_core::prelude::*;

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Create a rule set for discount calculation
    let mut ruleset = RuleSet::new("discount_rules", "check_membership");

    // Step 1: Check membership level
    ruleset.add_step(
        Step::decision("check_membership", "Check Membership Level")
            .branch(
                Condition::from_str("membership == \"gold\""),
                "gold_discount",
            )
            .branch(
                Condition::from_str("membership == \"silver\""),
                "silver_discount",
            )
            .default("check_amount")
            .build(),
    );

    // Step 2: Check purchase amount (for non-members)
    ruleset.add_step(
        Step::decision("check_amount", "Check Purchase Amount")
            .branch(Condition::from_str("amount >= 1000"), "bulk_discount")
            .default("no_discount")
            .build(),
    );

    // Terminal: Gold member discount
    ruleset.add_step(Step::terminal(
        "gold_discount",
        "Gold Member Discount",
        TerminalResult::new("GOLD_DISCOUNT")
            .with_message("20% discount for gold members")
            .with_output("discount_rate", Expr::literal(0.20f64))
            .with_output(
                "final_price",
                Expr::binary(BinaryOp::Mul, Expr::field("amount"), Expr::literal(0.80f64)),
            ),
    ));

    // Terminal: Silver member discount
    ruleset.add_step(Step::terminal(
        "silver_discount",
        "Silver Member Discount",
        TerminalResult::new("SILVER_DISCOUNT")
            .with_message("10% discount for silver members")
            .with_output("discount_rate", Expr::literal(0.10f64))
            .with_output(
                "final_price",
                Expr::binary(BinaryOp::Mul, Expr::field("amount"), Expr::literal(0.90f64)),
            ),
    ));

    // Terminal: Bulk purchase discount
    ruleset.add_step(Step::terminal(
        "bulk_discount",
        "Bulk Purchase Discount",
        TerminalResult::new("BULK_DISCOUNT")
            .with_message("5% discount for purchases over 1000")
            .with_output("discount_rate", Expr::literal(0.05f64))
            .with_output(
                "final_price",
                Expr::binary(BinaryOp::Mul, Expr::field("amount"), Expr::literal(0.95f64)),
            ),
    ));

    // Terminal: No discount
    ruleset.add_step(Step::terminal(
        "no_discount",
        "No Discount",
        TerminalResult::new("NO_DISCOUNT")
            .with_message("No discount applicable")
            .with_output("discount_rate", Expr::literal(0.0f64))
            .with_output("final_price", Expr::field("amount")),
    ));

    // Validate the rule set
    if let Err(errors) = ruleset.validate() {
        eprintln!("Validation errors: {:?}", errors);
        return Err("RuleSet validation failed".into());
    }

    println!("RuleSet validated successfully!\n");

    // Create executor with tracing enabled
    let executor = RuleExecutor::with_trace(TraceConfig::minimal());

    // Test cases
    let test_cases = vec![
        r#"{"membership": "gold", "amount": 500}"#,
        r#"{"membership": "silver", "amount": 300}"#,
        r#"{"membership": "bronze", "amount": 1500}"#,
        r#"{"membership": "none", "amount": 200}"#,
    ];

    for (i, test_json) in test_cases.iter().enumerate() {
        println!("=== Test Case {} ===", i + 1);
        println!("Input: {}", test_json);

        let input: Value = serde_json::from_str(test_json)?;
        let result = executor.execute(&ruleset, input)?;

        println!("Result Code: {}", result.code);
        println!("Message: {}", result.message);
        println!("Output: {}", result.output);
        println!("Duration: {}µs", result.duration_us);

        if let Some(trace) = &result.trace {
            println!("Execution Path: {}", trace.path_string());
        }

        println!();
    }

    // Demonstrate YAML serialization
    println!("=== RuleSet as YAML ===");
    println!("{}", ruleset.to_yaml().unwrap());

    Ok(())
}
