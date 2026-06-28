use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SubRuleScope {
    Org,
    Project,
}

impl std::fmt::Display for SubRuleScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubRuleScope::Org => write!(f, "org"),
            SubRuleScope::Project => write!(f, "project"),
        }
    }
}

impl std::str::FromStr for SubRuleScope {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "org" => Ok(SubRuleScope::Org),
            "project" => Ok(SubRuleScope::Project),
            other => Err(format!("invalid sub-rule scope: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubRuleAssetMeta {
    pub id: String,
    pub org_id: String,
    pub project_id: Option<String>,
    pub scope: SubRuleScope,
    pub name: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub draft_seq: i64,
    pub draft_updated_at: DateTime<Utc>,
    pub draft_updated_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubRuleAsset {
    #[serde(flatten)]
    pub meta: SubRuleAssetMeta,
    pub draft: JsonValue,
    #[serde(default)]
    pub input_schema: JsonValue,
    #[serde(default)]
    pub output_schema: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveSubRuleAssetRequest {
    pub name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub draft: JsonValue,
    #[serde(default)]
    pub input_schema: JsonValue,
    #[serde(default)]
    pub output_schema: JsonValue,
    #[serde(default)]
    pub expected_seq: Option<i64>,
}
