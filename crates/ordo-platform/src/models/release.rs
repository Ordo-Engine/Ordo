use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
    RollbackFailed,
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
            Self::RollbackFailed => write!(f, "rollback_failed"),
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
            "rollback_failed" => Ok(Self::RollbackFailed),
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_ruleset_snapshot: Option<serde_json::Value>,
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
    pub execution_attempts: i32,
    pub max_execution_attempts: i32,
    pub is_closed: bool,
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
    RollbackFailed,
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
            Self::RollbackFailed => write!(f, "rollback_failed"),
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
            "rollback_failed" => Ok(Self::RollbackFailed),
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
    WaitingBatch,
    Scheduled,
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
            Self::WaitingBatch => write!(f, "waiting_batch"),
            Self::Scheduled => write!(f, "scheduled"),
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
            "waiting_batch" => Ok(Self::WaitingBatch),
            "scheduled" => Ok(Self::Scheduled),
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
    pub batch_index: i32,
    pub current_version: String,
    pub target_version: String,
    pub status: ReleaseInstanceStatus,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
    pub message: Option<String>,
    pub metric_summary: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseExecution {
    pub id: String,
    pub request_id: String,
    pub status: ReleaseExecutionStatus,
    pub started_at: DateTime<Utc>,
    pub current_batch: i32,
    pub total_batches: i32,
    pub next_batch_at: Option<DateTime<Utc>>,
    pub strategy: RolloutStrategy,
    pub summary: ReleaseExecutionSummary,
    pub instances: Vec<ReleaseExecutionInstance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseExecutionEvent {
    pub id: String,
    pub release_execution_id: String,
    pub instance_id: Option<String>,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseHistoryScope {
    Request,
    Approval,
    Execution,
    Batch,
    Instance,
    Rollback,
}

impl std::fmt::Display for ReleaseHistoryScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Request => write!(f, "request"),
            Self::Approval => write!(f, "approval"),
            Self::Execution => write!(f, "execution"),
            Self::Batch => write!(f, "batch"),
            Self::Instance => write!(f, "instance"),
            Self::Rollback => write!(f, "rollback"),
        }
    }
}

impl std::str::FromStr for ReleaseHistoryScope {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "request" => Ok(Self::Request),
            "approval" => Ok(Self::Approval),
            "execution" => Ok(Self::Execution),
            "batch" => Ok(Self::Batch),
            "instance" => Ok(Self::Instance),
            "rollback" => Ok(Self::Rollback),
            other => Err(format!("invalid release history scope: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseHistoryActorType {
    User,
    #[default]
    System,
    Server,
}

impl std::fmt::Display for ReleaseHistoryActorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::User => write!(f, "user"),
            Self::System => write!(f, "system"),
            Self::Server => write!(f, "server"),
        }
    }
}

impl std::str::FromStr for ReleaseHistoryActorType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "user" => Ok(Self::User),
            "system" => Ok(Self::System),
            "server" => Ok(Self::Server),
            other => Err(format!("invalid release history actor type: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReleaseHistoryActor {
    pub actor_type: ReleaseHistoryActorType,
    pub actor_id: Option<String>,
    pub actor_name: Option<String>,
    pub actor_email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseRequestHistoryEntry {
    pub id: String,
    pub release_request_id: String,
    pub release_execution_id: Option<String>,
    pub instance_id: Option<String>,
    pub scope: ReleaseHistoryScope,
    pub action: String,
    pub actor_type: ReleaseHistoryActorType,
    pub actor_id: Option<String>,
    pub actor_name: Option<String>,
    pub actor_email: Option<String>,
    pub from_status: Option<String>,
    pub to_status: Option<String>,
    pub detail: serde_json::Value,
    pub created_at: DateTime<Utc>,
}
