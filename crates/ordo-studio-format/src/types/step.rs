//! Studio step types (mirrors the TypeScript Step model)

use serde::{Deserialize, Serialize};

use super::condition::StudioCondition;
use super::expr::StudioExpr;

/// A step in the studio format.
///
/// The `type` field is flattened from `StudioStepKind` and discriminates the step kind.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudioStep {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    // position is ignored during conversion (visual-only)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_generated: Option<String>,
    #[serde(flatten)]
    pub kind: StudioStepKind,
}

/// Discriminated step kind — `type` field in JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StudioStepKind {
    Decision {
        #[serde(default)]
        branches: Vec<StudioBranch>,
        #[serde(rename = "defaultNextStepId", default)]
        default_next_step_id: Option<String>,
    },
    Action {
        #[serde(default)]
        assignments: Vec<StudioAssignment>,
        #[serde(rename = "externalCalls", default)]
        external_calls: Vec<StudioExternalCall>,
        #[serde(default)]
        logging: Option<StudioLogging>,
        #[serde(rename = "nextStepId")]
        next_step_id: String,
    },
    Terminal {
        code: String,
        #[serde(default)]
        message: Option<StudioTerminalMessage>,
        #[serde(default)]
        output: Vec<StudioOutputField>,
    },
    #[serde(rename = "sub_rule")]
    SubRule {
        #[serde(rename = "refName")]
        ref_name: String,
        #[serde(default)]
        bindings: Vec<StudioSubRuleBinding>,
        #[serde(default)]
        outputs: Vec<StudioSubRuleOutput>,
        #[serde(rename = "returnPolicy", default)]
        return_policy: Option<String>,
        #[serde(rename = "nextStepId")]
        next_step_id: String,
    },
}

/// Terminal message accepts both the modern expression object shape and the
/// legacy plain string shape for backward compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StudioTerminalMessage {
    Expr(StudioExpr),
    String(String),
}

/// A branch in a decision step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudioBranch {
    pub id: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(
        deserialize_with = "crate::types::condition::deserialize_condition",
        serialize_with = "crate::types::condition::serialize_condition"
    )]
    pub condition: StudioCondition,
    #[serde(rename = "nextStepId")]
    pub next_step_id: String,
}

/// A variable assignment in an action step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudioAssignment {
    pub name: String,
    pub value: StudioExpr,
}

/// An external service call in an action step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioExternalCall {
    #[serde(rename = "type")]
    pub call_type: String,
    pub target: String,
    #[serde(default)]
    pub params: std::collections::HashMap<String, StudioExpr>,
    #[serde(default)]
    pub result_variable: Option<String>,
    #[serde(default)]
    pub timeout: Option<u64>,
}

/// Logging configuration in an action step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudioLogging {
    pub message: StudioExpr,
    #[serde(default)]
    pub level: Option<String>,
}

/// An output field in a terminal step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudioOutputField {
    pub name: String,
    pub value: StudioExpr,
}

/// Input binding for a sub-rule step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudioSubRuleBinding {
    pub field: String,
    pub expr: StudioExpr,
}

/// Output mapping for a sub-rule step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioSubRuleOutput {
    pub parent_var: String,
    pub child_var: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::condition::StudioCondition;

    #[test]
    fn branch_condition_accepts_bare_expression_string_and_round_trips() {
        let json = r#"{"id":"b0","condition":"amount <= 10000 && is_vip","nextStepId":"approve"}"#;
        let branch: StudioBranch = serde_json::from_str(json).unwrap();
        match &branch.condition {
            StudioCondition::Expression { expression } => {
                assert_eq!(expression, "amount <= 10000 && is_vip");
            }
            other => panic!("expected Expression, got {other:?}"),
        }
        // Round-trips back to a bare string, not the tagged object.
        let out = serde_json::to_string(&branch).unwrap();
        assert!(
            out.contains(r#""condition":"amount <= 10000 && is_vip""#),
            "condition should re-serialize as a bare string, got: {out}"
        );
    }

    #[test]
    fn branch_condition_still_accepts_structured_object() {
        let json = r#"{"id":"b0","condition":{"type":"simple","left":{"type":"variable","path":"amount"},"operator":"lte","right":{"type":"literal","value":10000,"valueType":"number"}},"nextStepId":"n"}"#;
        let branch: StudioBranch = serde_json::from_str(json).unwrap();
        assert!(matches!(branch.condition, StudioCondition::Simple { .. }));
    }
}
