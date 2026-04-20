use crate::{
    error::{ApiResult, PlatformError},
    models::{
        Claims, CreateReleasePolicyRequest, CreateReleaseRequest, DeploymentStatus,
        ReleaseApprovalDecision, ReleaseContentDiffSummary, ReleaseExecution,
        ReleaseExecutionInstance, ReleaseExecutionStatus, ReleaseInstanceStatus, ReleasePolicy,
        ReleasePolicyScope, ReleaseRequest, ReleaseRequestSnapshot, ReleaseRequestStatus,
        ReleaseStepDiffItem, ReleaseTargetPreview, ReleaseTargetServerPreview, ReleaseVersionDiff,
        ReviewReleaseRequest, RolloutStrategy, RolloutStrategyKind, RulesetDeployment,
        RulesetHistoryEntry, RulesetHistorySource, UpdateReleasePolicyRequest,
    },
    rbac::{
        require_project_permission, PERM_RELEASE_EXECUTE, PERM_RELEASE_INSTANCE_VIEW,
        PERM_RELEASE_PAUSE, PERM_RELEASE_POLICY_MANAGE, PERM_RELEASE_REQUEST_APPROVE,
        PERM_RELEASE_REQUEST_CREATE, PERM_RELEASE_REQUEST_REJECT, PERM_RELEASE_REQUEST_VIEW,
        PERM_RELEASE_RESUME, PERM_RELEASE_ROLLBACK,
    },
    sync::SyncEvent,
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[path = "release/diff.rs"]
mod diff;
#[path = "release/executions.rs"]
mod executions;
#[path = "release/policies.rs"]
mod policies;
#[path = "release/requests.rs"]
mod requests;
#[path = "release/reviews.rs"]
mod reviews;

use diff::*;

pub use executions::{
    execute_release_request, get_current_release_execution, get_release_execution_for_request,
    list_release_execution_events, pause_release_execution, resume_release_execution,
    rollback_release_execution,
};
pub use policies::{
    create_release_policy, delete_release_policy, list_release_policies, update_release_policy,
};
pub use requests::{
    create_release_request, get_release_request, list_release_requests, preview_release_target,
};
pub use reviews::{approve_release_request, reject_release_request};
