//! Studio ruleset types (mirrors the TypeScript RuleSet / RuleSetConfig model)

use hashbrown::HashMap;
use serde::{Deserialize, Serialize};

use super::step::StudioStep;

/// Top-level studio ruleset — what the frontend sends and stores.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioRuleSet {
    pub config: StudioConfig,
    pub start_step_id: String,
    pub steps: Vec<StudioStep>,
    /// Named inline sub-rule graphs
    #[serde(default)]
    pub sub_rules: HashMap<String, StudioSubRuleGraph>,
    /// Visual groups — stored as-is, not used during execution
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub groups: Option<serde_json::Value>,
    /// Ruleset-level metadata
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Ruleset configuration block.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioConfig {
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub enable_trace: Option<bool>,
    /// Timeout in milliseconds
    #[serde(default)]
    pub timeout: Option<u64>,
    #[serde(default)]
    pub input_schema: Option<serde_json::Value>,
    #[serde(default)]
    pub output_schema: Option<serde_json::Value>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// An inline sub-rule graph embedded in a ruleset.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioSubRuleGraph {
    pub entry_step: String,
    pub steps: Vec<StudioStep>,
}
