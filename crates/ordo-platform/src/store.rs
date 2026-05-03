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
    ServerStatus, TestCase, TestExpectation, UpdateEnvironmentRequest, UpdateReleasePolicyRequest,
    UpdateRoleRequest, User, UserRoleAssignment,
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
mod users;

use self::codec::*;
use self::rows::*;

#[derive(Clone)]
pub struct PlatformStore {
    pool: PgPool,
}

impl PlatformStore {
    pub async fn new(pool: PgPool) -> Result<Self> {
        Ok(Self { pool })
    }
}
