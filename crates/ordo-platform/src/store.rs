//! PostgreSQL-backed persistence for platform data.

use crate::models::{
    ConceptDefinition, ConnectTokenInfo, ContractField, CreateEnvironmentRequest,
    CreateReleasePolicyRequest, CreateReleaseRequest, CreateRoleRequest, DecisionContract,
    DeploymentStatus, FactDataType, FactDefinition, Member, NullPolicy, OrgRole, Organization,
    PlatformNotification, Project, ProjectEnvironment, ProjectRuleset, ProjectRulesetMeta,
    ReleaseApprovalDecision, ReleaseApprovalRecord, ReleaseContentDiffSummary, ReleaseExecution,
    ReleaseExecutionEvent, ReleaseExecutionInstance, ReleaseExecutionStatus,
    ReleaseExecutionSummary, ReleaseHistoryActor, ReleaseHistoryActorType, ReleaseHistoryScope,
    ReleaseInstanceStatus, ReleasePolicy, ReleasePolicyScope, ReleasePolicyTargetType,
    ReleaseRequest, ReleaseRequestHistoryEntry, ReleaseRequestSnapshot, ReleaseRequestStatus,
    ReleaseVersionDiff, Role, RollbackPolicy, RolloutStrategy, RulesetDeployment,
    RulesetHistoryEntry, RulesetHistorySource, ServerNode, ServerStatus, SubRuleAsset,
    SubRuleAssetMeta, SubRuleScope, TestCase, TestExpectation, UpdateEnvironmentRequest,
    UpdateReleasePolicyRequest, UpdateRoleRequest, User, UserRoleAssignment,
};
use anyhow::Result;
use serde_json::Value as JsonValue;
use sqlx::{PgPool, Row};

/// Raised by [`PlatformStore::save_draft_ruleset`] when a draft save would
/// silently overwrite a published version in place. Shared — not
/// string-duplicated — with the `ruleset_draft.rs` handler that maps it to a
/// 409, the same pattern already used for the optimistic-lock `"conflict"`
/// case. Before this constant existed the message was a bare string literal
/// at the raise site with nothing enforcing the handler's match kept up —
/// and it hadn't: the handler only special-cased `"conflict"`, so this error
/// fell through to a generic 500 with the message discarded.
pub(crate) const RULESET_VERSION_BUMP_REQUIRED: &str =
    "Published ruleset changes require a new version number";

mod bootstrap;
mod catalog;
mod codec;
mod environments;
mod execution_metrics;
mod github;
mod members;
mod notifications;
mod organizations;
mod projects;
mod rbac;
mod releases;
mod rows;
mod rulesets;
mod servers;
mod sub_rules;
mod users;

use self::codec::*;
pub use self::execution_metrics::ExecutionSnapshot;
use self::rows::*;

pub use self::releases::NewReleaseHistory;

#[derive(Clone)]
pub struct PlatformStore {
    pool: PgPool,
}

impl PlatformStore {
    pub async fn new(pool: PgPool) -> Result<Self> {
        Ok(Self { pool })
    }

    /// Cheap connectivity probe used by the readiness endpoint.
    pub async fn ping(&self) -> Result<()> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    /// Total number of connections currently managed by the pool.
    pub fn pool_size(&self) -> u32 {
        self.pool.size()
    }

    /// Number of idle (available) connections in the pool.
    pub fn pool_idle(&self) -> usize {
        self.pool.num_idle()
    }
}
