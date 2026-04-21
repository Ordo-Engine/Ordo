//! Platform data models — User, Organization, Member, Project, Role
//!
//! Maps to ordo-book governance framework:
//! - Organization = decision governance unit (Ch13)
//! - Project = decision domain (Ch7-10)
//! - Role = governance roles (Ch13 + Ch15)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

// ── Role ─────────────────────────────────────────────────────────────────────

/// Four-level role hierarchy mapping ordo-book governance roles:
/// - Owner   → Decision Owner (Ch15)
/// - Admin   → Rule Reviewer + Data Steward (Ch13)
/// - Editor  → Rule Author (Ch13)
/// - Viewer  → Governor / auditor (Ch13)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Viewer = 0,
    Editor = 1,
    Admin = 2,
    Owner = 3,
}

impl Role {
    pub fn can_edit_rules(&self) -> bool {
        *self >= Role::Editor
    }
    pub fn can_manage_members(&self) -> bool {
        *self >= Role::Admin
    }
    pub fn can_manage_org(&self) -> bool {
        *self >= Role::Admin
    }
    pub fn can_approve_changes(&self) -> bool {
        *self >= Role::Admin
    }
    pub fn can_publish(&self) -> bool {
        *self >= Role::Admin
    }
    pub fn can_rollback(&self) -> bool {
        *self >= Role::Admin
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Owner => write!(f, "owner"),
            Role::Admin => write!(f, "admin"),
            Role::Editor => write!(f, "editor"),
            Role::Viewer => write!(f, "viewer"),
        }
    }
}

impl std::str::FromStr for Role {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "owner" => Ok(Role::Owner),
            "admin" => Ok(Role::Admin),
            "editor" => Ok(Role::Editor),
            "viewer" => Ok(Role::Viewer),
            other => Err(format!(
                "invalid role '{}', expected: owner, admin, editor, viewer",
                other
            )),
        }
    }
}

// ── User ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub password_hash: String,
    pub display_name: String,
    pub created_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
}

/// Public user info (no password hash)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub email: String,
    pub display_name: String,
    pub created_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
}

impl From<&User> for UserInfo {
    fn from(u: &User) -> Self {
        Self {
            id: u.id.clone(),
            email: u.email.clone(),
            display_name: u.display_name.clone(),
            created_at: u.created_at,
            last_login: u.last_login,
        }
    }
}

// ── Organization ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub members: Vec<Member>,
    /// `None` for root orgs; set to parent org ID for sub-orgs (depth 1).
    #[serde(default)]
    pub parent_org_id: Option<String>,
    /// 0 = root org, 1 = sub-org. Maximum allowed depth is 1.
    #[serde(default)]
    pub depth: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    pub user_id: String,
    pub email: String,
    pub display_name: String,
    pub role: Role,
    pub invited_at: DateTime<Utc>,
    pub invited_by: String,
}

// ── Project (= Decision Domain, Ch7-10) ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// Also used as the execution tenant ID.
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub org_id: String,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    /// Preferred execution node for the project.
    #[serde(default)]
    pub server_id: Option<String>,
}

// ── Ruleset Change History ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RulesetHistorySource {
    Sync,
    Edit,
    Save,
    Restore,
    Create,
    Publish,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesetHistoryEntry {
    pub id: String,
    pub ruleset_name: String,
    pub action: String,
    pub source: RulesetHistorySource,
    pub created_at: DateTime<Utc>,
    pub author_id: String,
    pub author_email: String,
    pub author_display_name: String,
    pub snapshot: JsonValue,
}

// ── Fact Catalog (ordo-book Ch7 事实五元组) ──────────────────────────────────

/// Data type for facts and concepts
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FactDataType {
    String,
    Number,
    Boolean,
    Date,
    Object,
}

/// Null handling policy for a fact field
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NullPolicy {
    /// Treat null as an error
    Error,
    /// Use a default value
    Default,
    /// Skip the rule if null
    Skip,
}

/// A registered fact — raw input field that rules consume (Ch7)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactDefinition {
    /// Dotted field path, e.g. "user.age"
    pub name: String,
    pub data_type: FactDataType,
    /// Where this fact comes from (e.g. "user-service API")
    pub source: String,
    /// Typical fetch latency in ms (acquisition cost)
    pub latency_ms: Option<u32>,
    pub null_policy: NullPolicy,
    pub description: Option<String>,
    /// Responsible team or person
    pub owner: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── Concept Registry (ordo-book Ch7 派生字段 DAG) ─────────────────────────────

/// A derived concept — computed from facts/other concepts via an expression (Ch7)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptDefinition {
    /// Concept name, e.g. "risk_score"
    pub name: String,
    pub data_type: FactDataType,
    /// Expression string referencing facts and other concepts
    pub expression: String,
    /// Names of facts/concepts this concept depends on (for DAG + cycle detection)
    pub dependencies: Vec<String>,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── Decision Contract (ordo-book Ch13 契约维度) ───────────────────────────────

/// A field in a decision contract (input or output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractField {
    pub name: String,
    pub data_type: FactDataType,
    pub required: bool,
    pub description: Option<String>,
}

/// Formal input/output contract for a ruleset (Ch13)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionContract {
    /// Name of the ruleset this contract covers
    pub ruleset_name: String,
    /// Version pattern, e.g. "1.x"
    pub version_pattern: String,
    /// Responsible team
    pub owner: String,
    /// P99 latency SLA in milliseconds
    pub sla_p99_ms: Option<u32>,
    pub input_fields: Vec<ContractField>,
    pub output_fields: Vec<ContractField>,
    pub notes: Option<String>,
    pub updated_at: DateTime<Utc>,
}

// ── Templates (M1.1: rule templates) ─────────────────────────────────────────

/// Metadata for a single template (listed in manifest.json).
///
/// String fields may contain `i18n:<key>` references; the template API layer
/// resolves them against the locale files before returning to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateMetadata {
    /// Stable identifier (directory name), e.g. `ecommerce-coupon`
    pub id: String,
    /// Display name (i18n key)
    pub name: String,
    /// One-line description (i18n key)
    pub description: String,
    /// Tag labels (i18n keys) — e.g. e-commerce, decision-table
    #[serde(default)]
    pub tags: Vec<String>,
    /// Lucide icon name (not i18n)
    #[serde(default)]
    pub icon: Option<String>,
    /// "beginner" | "intermediate" | "advanced"
    pub difficulty: String,
    /// Short feature labels (i18n keys) highlighted on the template card
    #[serde(default)]
    pub features: Vec<String>,
}

/// One example input that can be run against the template's ruleset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateSample {
    /// Human-readable label (i18n key)
    pub label: String,
    /// Input payload passed to the ruleset's execute endpoint
    pub input: JsonValue,
    /// Expected outcome description (i18n key), rendered in the UI
    #[serde(default)]
    pub expected_result: Option<String>,
}

/// Full detail of a template — returned by `GET /api/v1/templates/:id`
/// and consumed by `POST /api/v1/orgs/:oid/projects/from-template`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateDetail {
    #[serde(flatten)]
    pub metadata: TemplateMetadata,
    pub facts: Vec<FactDefinition>,
    pub concepts: Vec<ConceptDefinition>,
    /// Raw RuleSet JSON — handed to the engine unmodified (after tenant_id rewrite).
    pub ruleset: JsonValue,
    pub samples: Vec<TemplateSample>,
    /// Pre-built decision contract for this ruleset (optional — not all templates include one).
    #[serde(default)]
    pub contract: Option<DecisionContract>,
    /// Bundled test cases (optional — applied to the new project on from-template clone).
    #[serde(default)]
    pub tests: Vec<TestCase>,
}

// ── Test Cases (M1.2: rule testing system) ───────────────────────────────────

/// The assertion side of a test case.
/// All fields are optional — only supplied fields are checked.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestExpectation {
    /// Expected result code (exact match)
    #[serde(default)]
    pub code: Option<String>,
    /// Expected result message (exact match, optional)
    #[serde(default)]
    pub message: Option<String>,
    /// Expected output fields (per-field comparison — only listed keys are checked)
    #[serde(default)]
    pub output: Option<JsonValue>,
}

/// A single test case for a ruleset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Input payload sent to the engine's execute endpoint
    pub input: JsonValue,
    pub expect: TestExpectation,
    /// Free-form tags for filtering (e.g. ["vip", "happy-path"])
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: String,
}

/// Result of running one test case against the engine.
/// Not persisted — held in memory by the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRunResult {
    pub test_id: String,
    pub test_name: String,
    pub passed: bool,
    /// Human-readable description of each assertion failure
    #[serde(default)]
    pub failures: Vec<String>,
    pub duration_us: u64,
    #[serde(default)]
    pub actual_code: Option<String>,
    #[serde(default)]
    pub actual_output: Option<JsonValue>,
}

/// Aggregated results for all rulesets in a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTestRunResult {
    pub total: u32,
    pub passed: u32,
    pub failed: u32,
    pub rulesets: Vec<RulesetTestSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesetTestSummary {
    pub ruleset_name: String,
    pub total: u32,
    pub passed: u32,
    pub failed: u32,
    pub results: Vec<TestRunResult>,
}

// ── JWT Claims ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // user_id
    pub email: String,
    pub exp: usize, // expiry timestamp
    pub iat: usize, // issued at
}

// ── Server Registry ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ServerStatus {
    Online,
    Offline,
    Degraded,
}

impl std::fmt::Display for ServerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerStatus::Online => write!(f, "online"),
            ServerStatus::Offline => write!(f, "offline"),
            ServerStatus::Degraded => write!(f, "degraded"),
        }
    }
}

impl std::str::FromStr for ServerStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "online" => Ok(ServerStatus::Online),
            "offline" => Ok(ServerStatus::Offline),
            "degraded" => Ok(ServerStatus::Degraded),
            other => Err(format!("invalid server status: {}", other)),
        }
    }
}

/// A registered ordo-server node (internal representation with token)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerNode {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(skip_serializing)]
    pub token: String,
    pub org_id: Option<String>,
    pub labels: JsonValue,
    pub version: Option<String>,
    pub status: ServerStatus,
    pub last_seen: Option<DateTime<Utc>>,
    pub registered_at: DateTime<Utc>,
}

/// Public view of a server (no token, safe to return to clients)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub id: String,
    pub name: String,
    pub url: String,
    pub org_id: Option<String>,
    pub labels: JsonValue,
    pub version: Option<String>,
    pub status: ServerStatus,
    pub last_seen: Option<DateTime<Utc>>,
    pub registered_at: DateTime<Utc>,
}

impl From<ServerNode> for ServerInfo {
    fn from(s: ServerNode) -> Self {
        Self {
            id: s.id,
            name: s.name,
            url: s.url,
            org_id: s.org_id,
            labels: s.labels,
            version: s.version,
            status: s.status,
            last_seen: s.last_seen,
            registered_at: s.registered_at,
        }
    }
}

// ── Project Environments ──────────────────────────────────────────────────────

/// A deployment environment bound to a project (e.g. dev / staging / production).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEnvironment {
    pub id: String,
    pub project_id: String,
    /// Human-readable label, e.g. "production", "dev"
    pub name: String,
    /// Bound ordo-server nodes for this environment
    pub server_ids: Vec<String>,
    /// NATS subject prefix for this environment's ordo-server; None = platform global prefix
    pub nats_subject_prefix: Option<String>,
    /// Whether this is the project's default (production) environment
    pub is_default: bool,
    /// Canary: forward X% of execute traffic to this environment (on the default env)
    pub canary_target_env_id: Option<String>,
    /// 0-100; 0 = no canary
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
    /// Target environment that receives the canary percentage of traffic
    pub canary_target_env_id: Option<String>,
    /// 0 clears canary; 1-100 sets percentage
    pub canary_percentage: i32,
}

// ── Draft Rulesets ────────────────────────────────────────────────────────────

/// Metadata summary of a draft ruleset (list view)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRulesetMeta {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub draft_seq: i64,
    pub draft_updated_at: DateTime<Utc>,
    pub draft_updated_by: Option<String>,
    pub published_version: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Full draft ruleset (detail view)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRuleset {
    #[serde(flatten)]
    pub meta: ProjectRulesetMeta,
    /// Full RuleSet JSON content
    pub draft: JsonValue,
}

/// Body for saving a draft (includes optimistic-lock seq)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveDraftRequest {
    pub ruleset: JsonValue,
    /// Client must echo back the current draft_seq; mismatch → 409
    pub expected_seq: i64,
}

/// Returned on 409 conflict
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftConflictResponse {
    pub conflict: bool,
    pub server_draft: JsonValue,
    pub server_seq: i64,
}

// ── Deployments ───────────────────────────────────────────────────────────────

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
    /// Denormalized for convenience
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

// ── Release Center ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReleasePolicyScope {
    Org,
    Project,
}

impl std::fmt::Display for ReleasePolicyScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReleasePolicyScope::Org => write!(f, "org"),
            ReleasePolicyScope::Project => write!(f, "project"),
        }
    }
}

impl std::str::FromStr for ReleasePolicyScope {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "org" => Ok(Self::Org),
            "project" => Ok(Self::Project),
            other => Err(format!("invalid release policy scope: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReleasePolicyTargetType {
    Project,
    Environment,
}

impl std::fmt::Display for ReleasePolicyTargetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReleasePolicyTargetType::Project => write!(f, "project"),
            ReleasePolicyTargetType::Environment => write!(f, "environment"),
        }
    }
}

impl std::str::FromStr for ReleasePolicyTargetType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "project" => Ok(Self::Project),
            "environment" => Ok(Self::Environment),
            other => Err(format!("invalid release policy target type: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RolloutStrategyKind {
    AllAtOnce,
    FixedBatch,
    PercentageBatch,
    TimeIntervalBatch,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RolloutStrategy {
    pub kind: Option<RolloutStrategyKind>,
    pub batch_size: Option<i32>,
    pub batch_percentage: Option<i32>,
    pub batch_interval_seconds: Option<i32>,
    pub auto_rollback_on_failure: Option<bool>,
    pub pause_on_error_rate: Option<f64>,
    pub pause_on_metric_breach: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RollbackPolicy {
    pub auto_rollback: bool,
    pub max_failed_instances: i32,
    pub metric_guard: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleasePolicy {
    pub id: String,
    pub org_id: String,
    pub project_id: Option<String>,
    pub name: String,
    pub scope: ReleasePolicyScope,
    pub target_type: ReleasePolicyTargetType,
    pub target_id: String,
    pub description: Option<String>,
    pub min_approvals: i32,
    pub allow_self_approval: bool,
    pub approver_ids: Vec<String>,
    pub rollout_strategy: RolloutStrategy,
    pub rollback_policy: RollbackPolicy,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateReleasePolicyRequest {
    pub name: String,
    pub scope: ReleasePolicyScope,
    pub target_type: ReleasePolicyTargetType,
    pub target_id: String,
    pub description: Option<String>,
    pub min_approvals: i32,
    pub allow_self_approval: bool,
    pub approver_ids: Vec<String>,
    pub rollout_strategy: RolloutStrategy,
    pub rollback_policy: RollbackPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateReleasePolicyRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub min_approvals: Option<i32>,
    pub allow_self_approval: Option<bool>,
    pub approver_ids: Option<Vec<String>>,
    pub rollout_strategy: Option<RolloutStrategy>,
    pub rollback_policy: Option<RollbackPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseRequestStatus {
    Draft,
    PendingApproval,
    Approved,
    Rejected,
    Cancelled,
    Executing,
    Completed,
    Failed,
    RolledBack,
}

impl std::fmt::Display for ReleaseRequestStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Draft => write!(f, "draft"),
            Self::PendingApproval => write!(f, "pending_approval"),
            Self::Approved => write!(f, "approved"),
            Self::Rejected => write!(f, "rejected"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::Executing => write!(f, "executing"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::RolledBack => write!(f, "rolled_back"),
        }
    }
}

impl std::str::FromStr for ReleaseRequestStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "draft" => Ok(Self::Draft),
            "pending_approval" => Ok(Self::PendingApproval),
            "approved" => Ok(Self::Approved),
            "rejected" => Ok(Self::Rejected),
            "cancelled" => Ok(Self::Cancelled),
            "executing" => Ok(Self::Executing),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "rolled_back" => Ok(Self::RolledBack),
            other => Err(format!("invalid release request status: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseApprovalDecision {
    Pending,
    Approved,
    Rejected,
}

impl std::fmt::Display for ReleaseApprovalDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Approved => write!(f, "approved"),
            Self::Rejected => write!(f, "rejected"),
        }
    }
}

impl std::str::FromStr for ReleaseApprovalDecision {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "approved" => Ok(Self::Approved),
            "rejected" => Ok(Self::Rejected),
            other => Err(format!("invalid release approval decision: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseApprovalRecord {
    pub id: String,
    pub release_request_id: String,
    pub stage: i32,
    pub reviewer_id: String,
    pub reviewer_name: Option<String>,
    pub reviewer_email: Option<String>,
    pub decision: ReleaseApprovalDecision,
    pub comment: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReleaseVersionDiff {
    pub from_version: Option<String>,
    pub to_version: String,
    pub rollback_version: Option<String>,
    pub changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReleaseStepDiffItem {
    pub id: String,
    pub name: String,
    pub step_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReleaseContentDiffSummary {
    pub baseline_version: Option<String>,
    pub step_count_before: i32,
    pub step_count_after: i32,
    pub group_count_before: i32,
    pub group_count_after: i32,
    pub added_steps: Vec<ReleaseStepDiffItem>,
    pub removed_steps: Vec<ReleaseStepDiffItem>,
    pub modified_steps: Vec<ReleaseStepDiffItem>,
    pub added_groups: Vec<String>,
    pub removed_groups: Vec<String>,
    pub modified_groups: Vec<String>,
    pub input_schema_changed: bool,
    pub output_schema_changed: bool,
    pub tags_changed: bool,
    pub description_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReleaseRequestSnapshot {
    pub requester_id: String,
    pub requester_name: Option<String>,
    pub requester_email: Option<String>,
    pub policy_name: Option<String>,
    pub policy_scope: Option<ReleasePolicyScope>,
    pub target_type: Option<ReleasePolicyTargetType>,
    pub target_id: Option<String>,
    pub environment_name: Option<String>,
    pub approver_ids: Vec<String>,
    pub approver_names: Vec<String>,
    pub approver_emails: Vec<String>,
    pub rollout_strategy: RolloutStrategy,
    pub rollback_policy: RollbackPolicy,
    pub affected_instance_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseRequest {
    pub id: String,
    pub org_id: String,
    pub project_id: String,
    pub ruleset_name: String,
    pub version: String,
    pub environment_id: String,
    pub environment_name: Option<String>,
    pub policy_id: Option<String>,
    pub status: ReleaseRequestStatus,
    pub title: String,
    pub change_summary: String,
    pub release_note: Option<String>,
    pub affected_instance_count: i32,
    pub rollout_strategy: RolloutStrategy,
    pub rollback_version: Option<String>,
    pub created_by: String,
    pub created_by_name: Option<String>,
    pub created_by_email: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version_diff: ReleaseVersionDiff,
    pub content_diff: ReleaseContentDiffSummary,
    pub request_snapshot: ReleaseRequestSnapshot,
    pub approvals: Vec<ReleaseApprovalRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateReleaseRequest {
    pub ruleset_name: String,
    pub version: String,
    pub environment_id: String,
    pub policy_id: Option<String>,
    pub title: String,
    pub change_summary: String,
    pub release_note: Option<String>,
    pub rollback_version: Option<String>,
    pub affected_instance_count: Option<i32>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewReleaseRequest {
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseExecutionStatus {
    Preparing,
    WaitingStart,
    RollingOut,
    Paused,
    Verifying,
    RollbackInProgress,
    Completed,
    Failed,
}

impl std::fmt::Display for ReleaseExecutionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Preparing => write!(f, "preparing"),
            Self::WaitingStart => write!(f, "waiting_start"),
            Self::RollingOut => write!(f, "rolling_out"),
            Self::Paused => write!(f, "paused"),
            Self::Verifying => write!(f, "verifying"),
            Self::RollbackInProgress => write!(f, "rollback_in_progress"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

impl std::str::FromStr for ReleaseExecutionStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "preparing" => Ok(Self::Preparing),
            "waiting_start" => Ok(Self::WaitingStart),
            "rolling_out" => Ok(Self::RollingOut),
            "paused" => Ok(Self::Paused),
            "verifying" => Ok(Self::Verifying),
            "rollback_in_progress" => Ok(Self::RollbackInProgress),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            other => Err(format!("invalid release execution status: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseInstanceStatus {
    Pending,
    Dispatching,
    Updating,
    Verifying,
    Success,
    Failed,
    RolledBack,
    Skipped,
}

impl std::fmt::Display for ReleaseInstanceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Dispatching => write!(f, "dispatching"),
            Self::Updating => write!(f, "updating"),
            Self::Verifying => write!(f, "verifying"),
            Self::Success => write!(f, "success"),
            Self::Failed => write!(f, "failed"),
            Self::RolledBack => write!(f, "rolled_back"),
            Self::Skipped => write!(f, "skipped"),
        }
    }
}

impl std::str::FromStr for ReleaseInstanceStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "dispatching" => Ok(Self::Dispatching),
            "updating" => Ok(Self::Updating),
            "verifying" => Ok(Self::Verifying),
            "success" => Ok(Self::Success),
            "failed" => Ok(Self::Failed),
            "rolled_back" => Ok(Self::RolledBack),
            "skipped" => Ok(Self::Skipped),
            other => Err(format!("invalid release instance status: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReleaseExecutionSummary {
    pub total_instances: i32,
    pub succeeded_instances: i32,
    pub failed_instances: i32,
    pub pending_instances: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseExecutionInstance {
    pub id: String,
    pub release_execution_id: String,
    pub instance_id: String,
    pub instance_name: String,
    pub zone: Option<String>,
    pub current_version: String,
    pub target_version: String,
    pub status: ReleaseInstanceStatus,
    pub updated_at: DateTime<Utc>,
    pub message: Option<String>,
    pub metric_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseExecution {
    pub id: String,
    pub request_id: String,
    pub status: ReleaseExecutionStatus,
    pub started_at: DateTime<Utc>,
    pub current_batch: i32,
    pub total_batches: i32,
    pub strategy: RolloutStrategy,
    pub summary: ReleaseExecutionSummary,
    pub instances: Vec<ReleaseExecutionInstance>,
}

// ── RBAC ──────────────────────────────────────────────────────────────────────

/// A custom role scoped to an organization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgRole {
    pub id: String,
    pub org_id: String,
    pub name: String,
    pub description: Option<String>,
    /// Permission bit strings, e.g. "ruleset:publish"
    pub permissions: Vec<String>,
    /// System roles are built-in and cannot be deleted
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoleRequest {
    pub name: String,
    pub description: Option<String>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRoleRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub permissions: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRoleAssignment {
    pub user_id: String,
    pub org_id: String,
    pub role_id: String,
    pub role_name: Option<String>,
    pub assigned_at: DateTime<Utc>,
    pub assigned_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignRoleRequest {
    pub role_id: String,
}

/// Member enriched with their RBAC role assignments
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct MemberWithRoles {
    pub user_id: String,
    pub email: String,
    pub display_name: String,
    /// Primary role label.
    pub role: Role,
    pub roles: Vec<UserRoleAssignment>,
    pub invited_at: DateTime<Utc>,
    pub invited_by: String,
}
