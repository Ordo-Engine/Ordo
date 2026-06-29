use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DeploymentStatus {
    Queued,
    /// Published to NATS and confirmed applied by at least one bound server.
    Success,
    /// Published to NATS, but not (yet) confirmed applied — either no servers are
    /// registered for the environment, or none reported the new version within the
    /// confirmation window. The publish itself succeeded; this is not a failure.
    Dispatched,
    Failed,
}

impl std::fmt::Display for DeploymentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeploymentStatus::Queued => write!(f, "queued"),
            DeploymentStatus::Success => write!(f, "success"),
            DeploymentStatus::Dispatched => write!(f, "dispatched"),
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
            "dispatched" => Ok(DeploymentStatus::Dispatched),
            "failed" => Ok(DeploymentStatus::Failed),
            other => Err(format!("invalid deployment status: {}", other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatched_status_roundtrips() {
        assert_eq!(DeploymentStatus::Dispatched.to_string(), "dispatched");
        assert_eq!(
            "dispatched".parse::<DeploymentStatus>().unwrap(),
            DeploymentStatus::Dispatched
        );
        // serde uses lowercase rename, matching Display/FromStr.
        let json = serde_json::to_string(&DeploymentStatus::Dispatched).unwrap();
        assert_eq!(json, "\"dispatched\"");
        let back: DeploymentStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, DeploymentStatus::Dispatched);
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
