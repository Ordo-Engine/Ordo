use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DeploymentStatus {
    Queued,
    Success,
    Failed,
}

impl std::fmt::Display for DeploymentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeploymentStatus::Queued => write!(f, "queued"),
            DeploymentStatus::Success => write!(f, "success"),
            DeploymentStatus::Failed => write!(f, "failed"),
        }
    }
}

impl std::str::FromStr for DeploymentStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "queued" => Ok(DeploymentStatus::Queued),
            "success" => Ok(DeploymentStatus::Success),
            "failed" => Ok(DeploymentStatus::Failed),
            other => Err(format!("invalid deployment status: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesetDeployment {
    pub id: String,
    pub project_id: String,
    pub environment_id: String,
    pub environment_name: Option<String>,
    pub ruleset_name: String,
    pub version: String,
    pub release_note: Option<String>,
    pub snapshot: JsonValue,
    pub deployed_at: DateTime<Utc>,
    pub deployed_by: Option<String>,
    pub status: DeploymentStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishRequest {
    pub environment_id: String,
    pub release_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedeployRequest {
    pub environment_id: String,
    pub release_note: Option<String>,
}
