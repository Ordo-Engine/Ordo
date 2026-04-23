use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use super::ServerStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEnvironment {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub server_ids: Vec<String>,
    pub nats_subject_prefix: Option<String>,
    pub is_default: bool,
    pub canary_target_env_id: Option<String>,
    pub canary_percentage: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEnvironmentRequest {
    pub name: String,
    pub server_ids: Vec<String>,
    pub nats_subject_prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateEnvironmentRequest {
    pub name: Option<String>,
    pub server_ids: Option<Vec<String>>,
    pub nats_subject_prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetCanaryRequest {
    pub canary_target_env_id: Option<String>,
    pub canary_percentage: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRulesetMeta {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub draft_seq: i64,
    pub draft_updated_at: DateTime<Utc>,
    pub draft_updated_by: Option<String>,
    pub draft_version: Option<String>,
    pub published_version: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRuleset {
    #[serde(flatten)]
    pub meta: ProjectRulesetMeta,
    pub draft: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveDraftRequest {
    pub ruleset: JsonValue,
    pub expected_seq: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftConflictResponse {
    pub conflict: bool,
    pub server_draft: JsonValue,
    pub server_seq: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseTargetServerPreview {
    pub id: String,
    pub name: String,
    pub url: String,
    pub status: ServerStatus,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseTargetPreview {
    pub environment_id: String,
    pub environment_name: String,
    pub affected_instance_count: i32,
    pub bound_servers: Vec<ReleaseTargetServerPreview>,
    pub message: Option<String>,
}
