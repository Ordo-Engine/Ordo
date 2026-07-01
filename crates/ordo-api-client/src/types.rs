//! Lean DTOs mirroring the Ordo platform HTTP API. Only the fields the CLI
//! reads are modeled; unknown fields are ignored on deserialize.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── auth ──

#[derive(Serialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserInfo,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub email: String,
    pub display_name: String,
}

// ── orgs / projects / environments ──

#[derive(Debug, Clone, Deserialize)]
pub struct Org {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub org_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Environment {
    pub id: String,
    pub name: String,
}

// ── rulesets ──

/// List item — draft metadata without the draft body.
#[derive(Debug, Clone, Deserialize)]
pub struct RulesetMeta {
    pub name: String,
    #[serde(default)]
    pub draft_seq: i64,
    #[serde(default)]
    pub draft_version: Option<String>,
    #[serde(default)]
    pub published_version: Option<String>,
}

/// A single ruleset — metadata plus the studio-format `draft` body.
#[derive(Debug, Clone, Deserialize)]
pub struct RulesetDraft {
    pub name: String,
    #[serde(default)]
    pub draft_seq: i64,
    pub draft: Value,
}

#[derive(Serialize)]
pub struct SaveDraftRequest {
    /// Studio-format ruleset body.
    pub ruleset: Value,
    pub expected_seq: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DraftConflict {
    pub server_seq: i64,
    pub server_draft: Value,
}

// ── publish / deployments ──

#[derive(Serialize)]
pub struct PublishRequest {
    pub environment_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_note: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Deployment {
    pub id: String,
    pub ruleset_name: String,
    #[serde(default)]
    pub environment_name: Option<String>,
    pub version: String,
    /// `queued` | `dispatched` | `success` | `failed`
    pub status: String,
    #[serde(default)]
    pub deployed_at: Option<String>,
}

// ── catalog ──

/// A fact definition (external input). Kept permissive — round-tripped through
/// `facts.json` verbatim, so we model it as an opaque object plus its name.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FactDefinition {
    pub name: String,
    #[serde(flatten)]
    pub rest: Value,
}
