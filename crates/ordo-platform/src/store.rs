//! PostgreSQL-backed persistence for platform data.

use crate::models::{
    ConceptDefinition, ContractField, CreateEnvironmentRequest, CreateReleasePolicyRequest,
    CreateReleaseRequest, CreateRoleRequest, DecisionContract, DeploymentStatus, FactDataType,
    FactDefinition, Member, NullPolicy, OrgRole, Organization, PlatformNotification, Project,
    ProjectEnvironment, ProjectRuleset, ProjectRulesetMeta, ReleaseApprovalDecision,
    ReleaseApprovalRecord, ReleaseContentDiffSummary, ReleaseExecution, ReleaseExecutionEvent,
    ReleaseExecutionInstance, ReleaseExecutionStatus, ReleaseExecutionSummary, ReleaseHistoryActor,
    ReleaseHistoryActorType, ReleaseHistoryScope, ReleaseInstanceStatus, ReleasePolicy,
    ReleasePolicyScope, ReleasePolicyTargetType, ReleaseRequest, ReleaseRequestHistoryEntry,
    ReleaseRequestSnapshot, ReleaseRequestStatus, ReleaseVersionDiff, Role, RollbackPolicy,
    RolloutStrategy, RulesetDeployment, RulesetHistoryEntry, RulesetHistorySource, ServerNode,
    ServerStatus, SubRuleAsset, SubRuleAssetMeta, SubRuleScope, TestCase, TestExpectation,
    UpdateEnvironmentRequest, UpdateReleasePolicyRequest, UpdateRoleRequest, User,
    UserRoleAssignment,
};
use anyhow::Result;
use serde_json::Value as JsonValue;
use sqlx::{PgPool, Row};

mod bootstrap;
mod catalog;
mod codec;
mod environments;
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
