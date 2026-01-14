//! Ordo Rule Engine Core
//!
//! A high-performance rule engine for enterprise applications.
//!
//! # Features
//!
//! - **Step Flow Model**: Rules are organized as step flows with decision,
//!   action, and terminal steps
//! - **Expression Language**: Rich expression syntax with operators and functions
//! - **Execution Tracing**: Full traceability of rule execution for debugging
//! - **Configurable Behavior**: Flexible handling of missing fields and errors
//!
//! # Example
//!
//! ```rust,no_run
//! use ordo_core::prelude::*;
//!
//! // Create a rule set
//! let mut ruleset = RuleSet::new("discount_rules", "check_user");
//!
//! // Add decision step
//! ruleset.add_step(
//!     Step::decision("check_user", "Check User Type")
//!         .branch(Condition::from_string("user.vip == true"), "vip_discount")
//!         .default("normal_discount")
//!         .build()
//! );
//!
//! // Add terminal steps
//! ruleset.add_step(Step::terminal(
//!     "vip_discount",
//!     "VIP Discount",
//!     TerminalResult::new("VIP").with_output("discount", Expr::literal(0.2))
//! ));
//!
//! ruleset.add_step(Step::terminal(
//!     "normal_discount",
//!     "Normal Discount",
//!     TerminalResult::new("NORMAL").with_output("discount", Expr::literal(0.05))
//! ));
//!
//! // Execute
//! let executor = RuleExecutor::new();
//! let input = serde_json::from_str(r#"{"user": {"vip": true}}"#).unwrap();
//! let result = executor.execute(&ruleset, input).unwrap();
//! println!("Result: {} - {}", result.code, result.message);
//! ```

// Documentation requirements - allow missing docs for struct fields and enum variants
#![allow(missing_docs)]
#![warn(clippy::all)]

pub mod context;
pub mod error;
pub mod expr;
pub mod rule;
pub mod trace;

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::context::{Context, Value};
    pub use crate::error::{OrdoError, Result};
    pub use crate::expr::{BinaryOp, Evaluator, Expr, ExprParser, FunctionRegistry, UnaryOp};
    pub use crate::rule::{
        Action, ActionKind, BatchExecutionResult, Branch, Condition, ExecutionResult,
        LoggingMetricSink, MetricSink, MetricType, NoOpMetricSink, RuleExecutor, RuleSet,
        RuleSetConfig, SingleExecutionResult, Step, StepKind, TerminalResult,
    };
    pub use crate::trace::{ExecutionTrace, StepTrace, TraceConfig};
}

/// Engine version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Engine name
pub const NAME: &str = "ordo-core";

#[cfg(test)]
mod tests {
    use super::prelude::*;

    #[test]
    fn test_full_workflow() {
        // Create a rule set for user verification
        let mut ruleset = RuleSet::new("user_verification", "check_balance");

        // Step 1: Check balance
        ruleset.add_step(
            Step::decision("check_balance", "Check Balance")
                .branch(Condition::from_string("balance >= 1000"), "check_status")
                .default("reject_low_balance")
                .build(),
        );

        // Step 2: Check status
        ruleset.add_step(
            Step::decision("check_status", "Check Status")
                .branch(Condition::from_string("status == \"active\""), "approve")
                .branch(
                    Condition::from_string("status == \"pending\""),
                    "pending_review",
                )
                .default("reject_inactive")
                .build(),
        );

        // Terminal: Approve
        ruleset.add_step(Step::terminal(
            "approve",
            "Approve",
            TerminalResult::new("APPROVED")
                .with_message("User approved")
                .with_output("allowed", Expr::literal(true)),
        ));

        // Terminal: Pending
        ruleset.add_step(Step::terminal(
            "pending_review",
            "Pending Review",
            TerminalResult::new("PENDING")
                .with_message("User pending review")
                .with_output("allowed", Expr::literal(false)),
        ));

        // Terminal: Reject (low balance)
        ruleset.add_step(Step::terminal(
            "reject_low_balance",
            "Reject Low Balance",
            TerminalResult::new("REJECTED")
                .with_message("Insufficient balance")
                .with_output("allowed", Expr::literal(false)),
        ));

        // Terminal: Reject (inactive)
        ruleset.add_step(Step::terminal(
            "reject_inactive",
            "Reject Inactive",
            TerminalResult::new("REJECTED")
                .with_message("Account not active")
                .with_output("allowed", Expr::literal(false)),
        ));

        // Validate
        assert!(ruleset.validate().is_ok());

        // Test case 1: Active user with sufficient balance
        let executor = RuleExecutor::new();
        let input: Value =
            serde_json::from_str(r#"{"balance": 1500, "status": "active"}"#).unwrap();
        let result = executor.execute(&ruleset, input).unwrap();
        assert_eq!(result.code, "APPROVED");

        // Test case 2: Active user with insufficient balance
        let input: Value = serde_json::from_str(r#"{"balance": 500, "status": "active"}"#).unwrap();
        let result = executor.execute(&ruleset, input).unwrap();
        assert_eq!(result.code, "REJECTED");
        assert!(result.message.contains("balance"));

        // Test case 3: Pending user with sufficient balance
        let input: Value =
            serde_json::from_str(r#"{"balance": 2000, "status": "pending"}"#).unwrap();
        let result = executor.execute(&ruleset, input).unwrap();
        assert_eq!(result.code, "PENDING");
    }
}
