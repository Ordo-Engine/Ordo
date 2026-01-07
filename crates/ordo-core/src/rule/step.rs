//! Step definitions
//!
//! Defines the step flow model

use crate::context::Value;
use crate::expr::Expr;
use serde::{Deserialize, Serialize};

/// A step in the rule flow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    /// Step ID (unique within RuleSet)
    pub id: String,

    /// Step name (for display)
    pub name: String,

    /// Step kind
    #[serde(flatten)]
    pub kind: StepKind,
}

impl Step {
    /// Create a decision step
    pub fn decision(id: impl Into<String>, name: impl Into<String>) -> StepBuilder {
        StepBuilder {
            id: id.into(),
            name: name.into(),
            branches: vec![],
            default_next: None,
        }
    }

    /// Create an action step
    pub fn action(
        id: impl Into<String>,
        name: impl Into<String>,
        actions: Vec<Action>,
        next_step: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind: StepKind::Action {
                actions,
                next_step: next_step.into(),
            },
        }
    }

    /// Create a terminal step
    pub fn terminal(
        id: impl Into<String>,
        name: impl Into<String>,
        result: TerminalResult,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind: StepKind::Terminal { result },
        }
    }

    /// Get all steps referenced by this step
    pub fn referenced_steps(&self) -> Vec<String> {
        match &self.kind {
            StepKind::Decision {
                branches,
                default_next,
            } => {
                let mut refs: Vec<String> = branches.iter().map(|b| b.next_step.clone()).collect();
                if let Some(default) = default_next {
                    refs.push(default.clone());
                }
                refs
            }
            StepKind::Action { next_step, .. } => vec![next_step.clone()],
            StepKind::Terminal { .. } => vec![],
        }
    }
}

/// Step builder for decision steps
pub struct StepBuilder {
    id: String,
    name: String,
    branches: Vec<Branch>,
    default_next: Option<String>,
}

impl StepBuilder {
    /// Add a branch
    pub fn branch(mut self, condition: Condition, next_step: impl Into<String>) -> Self {
        self.branches.push(Branch {
            condition,
            next_step: next_step.into(),
            actions: vec![],
        });
        self
    }

    /// Add a branch with actions
    pub fn branch_with_actions(
        mut self,
        condition: Condition,
        next_step: impl Into<String>,
        actions: Vec<Action>,
    ) -> Self {
        self.branches.push(Branch {
            condition,
            next_step: next_step.into(),
            actions,
        });
        self
    }

    /// Set default next step
    pub fn default(mut self, next_step: impl Into<String>) -> Self {
        self.default_next = Some(next_step.into());
        self
    }

    /// Build the step
    pub fn build(self) -> Step {
        Step {
            id: self.id,
            name: self.name,
            kind: StepKind::Decision {
                branches: self.branches,
                default_next: self.default_next,
            },
        }
    }
}

/// Step kind enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StepKind {
    /// Decision step - evaluates conditions and branches
    Decision {
        /// Branches to evaluate (in order)
        branches: Vec<Branch>,
        /// Default next step if no branch matches
        default_next: Option<String>,
    },

    /// Action step - performs actions and continues
    Action {
        /// Actions to perform
        actions: Vec<Action>,
        /// Next step
        next_step: String,
    },

    /// Terminal step - ends execution with a result
    Terminal {
        /// Result to return
        result: TerminalResult,
    },
}

/// A branch in a decision step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    /// Condition to evaluate
    pub condition: Condition,

    /// Next step if condition is true
    pub next_step: String,

    /// Actions to perform before branching
    #[serde(default)]
    pub actions: Vec<Action>,
}

/// Condition for branching
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Condition {
    /// Always true
    Always,

    /// Expression condition
    Expression(Expr),

    /// Expression string (will be parsed)
    ExpressionString(String),
}

impl Condition {
    /// Create an expression condition
    pub fn expr(expr: Expr) -> Self {
        Self::Expression(expr)
    }

    /// Create an expression condition from string
    pub fn from_str(s: impl Into<String>) -> Self {
        Self::ExpressionString(s.into())
    }
}

/// Action to perform
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    /// Action kind
    #[serde(flatten)]
    pub kind: ActionKind,

    /// Optional description
    #[serde(default)]
    pub description: String,
}

impl Action {
    /// Create a set variable action
    pub fn set_var(name: impl Into<String>, value: Expr) -> Self {
        Self {
            kind: ActionKind::SetVariable {
                name: name.into(),
                value,
            },
            description: String::new(),
        }
    }

    /// Create a log action
    pub fn log(message: impl Into<String>, level: LogLevel) -> Self {
        Self {
            kind: ActionKind::Log {
                message: message.into(),
                level,
            },
            description: String::new(),
        }
    }
}

/// Action kind enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ActionKind {
    /// Set a variable
    SetVariable { name: String, value: Expr },

    /// Log a message
    Log {
        message: String,
        #[serde(default)]
        level: LogLevel,
    },

    /// Record a metric
    Metric {
        name: String,
        value: Expr,
        #[serde(default)]
        tags: Vec<(String, String)>,
    },

    /// External call (future)
    #[serde(skip)]
    ExternalCall {
        service: String,
        method: String,
        params: Vec<(String, Expr)>,
        timeout_ms: u64,
    },
}

/// Log level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

/// Terminal result
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TerminalResult {
    /// Result code
    #[serde(default)]
    pub code: String,

    /// Result message
    #[serde(default)]
    pub message: String,

    /// Output values (expressions to evaluate)
    #[serde(default)]
    pub output: Vec<(String, Expr)>,

    /// Static data to include
    #[serde(default)]
    pub data: Value,
}

impl TerminalResult {
    /// Create a new terminal result
    pub fn new(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: String::new(),
            output: vec![],
            data: Value::Null,
        }
    }

    /// Set message
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    /// Add output field
    pub fn with_output(mut self, name: impl Into<String>, expr: Expr) -> Self {
        self.output.push((name.into(), expr));
        self
    }

    /// Set static data
    pub fn with_data(mut self, data: Value) -> Self {
        self.data = data;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_builder() {
        let step = Step::decision("check_age", "Check Age")
            .branch(Condition::from_str("age >= 18"), "adult")
            .branch(Condition::from_str("age >= 13"), "teen")
            .default("child")
            .build();

        assert_eq!(step.id, "check_age");
        assert_eq!(step.name, "Check Age");

        match step.kind {
            StepKind::Decision {
                branches,
                default_next,
            } => {
                assert_eq!(branches.len(), 2);
                assert_eq!(default_next, Some("child".to_string()));
            }
            _ => panic!("Expected Decision step"),
        }
    }

    #[test]
    fn test_terminal_result() {
        let result = TerminalResult::new("SUCCESS")
            .with_message("Operation completed")
            .with_output("discount", Expr::field("$calculated_discount"));

        assert_eq!(result.code, "SUCCESS");
        assert_eq!(result.message, "Operation completed");
        assert_eq!(result.output.len(), 1);
    }
}
