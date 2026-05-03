use crate::{
    error::{ApiResult, PlatformError},
    models::{
        Claims, CreateReleasePolicyRequest, CreateReleaseRequest, DeploymentStatus,
        ReleaseApprovalDecision, ReleaseContentDiffSummary, ReleaseExecution,
        ReleaseExecutionInstance, ReleaseExecutionStatus, ReleaseHistoryActor,
        ReleaseHistoryActorType, ReleaseHistoryScope, ReleaseInstanceStatus, ReleasePolicy,
        ReleasePolicyScope, ReleaseRequest, ReleaseRequestHistoryEntry, ReleaseRequestSnapshot,
        ReleaseRequestStatus, ReleaseStepDiffItem, ReleaseTargetPreview,
        ReleaseTargetServerPreview, ReleaseVersionDiff, ReviewReleaseRequest, RolloutStrategy,
        RolloutStrategyKind, RulesetDeployment, RulesetHistoryEntry, RulesetHistorySource,
        UpdateReleasePolicyRequest, User,
    },
    rbac::{
        require_project_permission, PERM_RELEASE_EXECUTE, PERM_RELEASE_INSTANCE_VIEW,
        PERM_RELEASE_PAUSE, PERM_RELEASE_POLICY_MANAGE, PERM_RELEASE_REQUEST_APPROVE,
        PERM_RELEASE_REQUEST_CREATE, PERM_RELEASE_REQUEST_REJECT, PERM_RELEASE_REQUEST_VIEW,
        PERM_RELEASE_RESUME, PERM_RELEASE_ROLLBACK,
    },
    ruleset_draft::inline_sub_rules_with_manifest,
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

pub(crate) use diff::hash_json_value;
use diff::*;

pub(crate) fn user_history_actor(claims: &Claims, user: Option<&User>) -> ReleaseHistoryActor {
    ReleaseHistoryActor {
        actor_type: ReleaseHistoryActorType::User,
        actor_id: Some(claims.sub.clone()),
        actor_name: user.map(|item| item.display_name.clone()),
        actor_email: user.map(|item| item.email.clone()),
    }
}

pub(crate) fn system_history_actor(name: &str) -> ReleaseHistoryActor {
    ReleaseHistoryActor {
        actor_type: ReleaseHistoryActorType::System,
        actor_id: Some(name.to_string()),
        actor_name: Some(name.to_string()),
        actor_email: None,
    }
}

pub(crate) fn server_history_actor(
    server_id: &str,
    server_name: Option<&str>,
) -> ReleaseHistoryActor {
    ReleaseHistoryActor {
        actor_type: ReleaseHistoryActorType::Server,
        actor_id: Some(server_id.to_string()),
        actor_name: server_name
            .map(str::to_string)
            .or_else(|| Some(server_id.to_string())),
        actor_email: None,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn append_release_history(
    state: &AppState,
    release_request_id: &str,
    release_execution_id: Option<&str>,
    instance_id: Option<&str>,
    scope: ReleaseHistoryScope,
    action: &str,
    actor: &ReleaseHistoryActor,
    from_status: Option<String>,
    to_status: Option<String>,
    detail: JsonValue,
) -> anyhow::Result<()> {
    state
        .store
        .create_release_request_history(
            &Uuid::new_v4().to_string(),
            release_request_id,
            release_execution_id,
            instance_id,
            scope,
            action,
            actor,
            from_status.as_deref(),
            to_status.as_deref(),
            detail,
        )
        .await
}

pub(crate) fn merge_history_detail(mut base: JsonValue, extra: JsonValue) -> JsonValue {
    match (&mut base, extra) {
        (JsonValue::Object(base_map), JsonValue::Object(extra_map)) => {
            for (key, value) in extra_map {
                base_map.insert(key, value);
            }
            base
        }
        (_, other) => other,
    }
}

pub(crate) const MAX_RELEASE_EXECUTION_ATTEMPTS: usize = 3;

pub(crate) fn can_transition_release_request_status(
    from: &ReleaseRequestStatus,
    to: &ReleaseRequestStatus,
) -> bool {
    if from == to {
        return true;
    }

    matches!(
        (from, to),
        (
            ReleaseRequestStatus::Draft,
            ReleaseRequestStatus::PendingApproval
        ) | (
            ReleaseRequestStatus::PendingApproval,
            ReleaseRequestStatus::Approved
        ) | (
            ReleaseRequestStatus::PendingApproval,
            ReleaseRequestStatus::Rejected
        ) | (
            ReleaseRequestStatus::PendingApproval,
            ReleaseRequestStatus::Cancelled
        ) | (
            ReleaseRequestStatus::Rejected,
            ReleaseRequestStatus::PendingApproval
        ) | (
            ReleaseRequestStatus::Rejected,
            ReleaseRequestStatus::Cancelled
        ) | (
            ReleaseRequestStatus::Approved,
            ReleaseRequestStatus::Executing
        ) | (
            ReleaseRequestStatus::Approved,
            ReleaseRequestStatus::Cancelled
        ) | (
            ReleaseRequestStatus::Executing,
            ReleaseRequestStatus::Completed
        ) | (
            ReleaseRequestStatus::Executing,
            ReleaseRequestStatus::Failed
        ) | (
            ReleaseRequestStatus::Executing,
            ReleaseRequestStatus::RollbackFailed
        ) | (
            ReleaseRequestStatus::Executing,
            ReleaseRequestStatus::RolledBack
        ) | (
            ReleaseRequestStatus::Completed,
            ReleaseRequestStatus::Executing
        ) | (
            ReleaseRequestStatus::Failed,
            ReleaseRequestStatus::Executing
        ) | (
            ReleaseRequestStatus::Failed,
            ReleaseRequestStatus::RolledBack
        ) | (
            ReleaseRequestStatus::RollbackFailed,
            ReleaseRequestStatus::RolledBack
        ) | (
            ReleaseRequestStatus::Completed,
            ReleaseRequestStatus::RolledBack
        )
    )
}

pub(crate) fn can_transition_release_execution_status(
    from: &ReleaseExecutionStatus,
    to: &ReleaseExecutionStatus,
) -> bool {
    if from == to {
        return true;
    }

    matches!(
        (from, to),
        (
            ReleaseExecutionStatus::Preparing,
            ReleaseExecutionStatus::RollingOut
        ) | (
            ReleaseExecutionStatus::Preparing,
            ReleaseExecutionStatus::Paused
        ) | (
            ReleaseExecutionStatus::Preparing,
            ReleaseExecutionStatus::Failed
        ) | (
            ReleaseExecutionStatus::RollingOut,
            ReleaseExecutionStatus::WaitingStart
        ) | (
            ReleaseExecutionStatus::RollingOut,
            ReleaseExecutionStatus::Paused
        ) | (
            ReleaseExecutionStatus::RollingOut,
            ReleaseExecutionStatus::Completed
        ) | (
            ReleaseExecutionStatus::RollingOut,
            ReleaseExecutionStatus::Failed
        ) | (
            ReleaseExecutionStatus::RollingOut,
            ReleaseExecutionStatus::RollbackInProgress
        ) | (
            ReleaseExecutionStatus::WaitingStart,
            ReleaseExecutionStatus::RollingOut
        ) | (
            ReleaseExecutionStatus::WaitingStart,
            ReleaseExecutionStatus::Paused
        ) | (
            ReleaseExecutionStatus::WaitingStart,
            ReleaseExecutionStatus::Failed
        ) | (
            ReleaseExecutionStatus::Paused,
            ReleaseExecutionStatus::RollingOut
        ) | (
            ReleaseExecutionStatus::Paused,
            ReleaseExecutionStatus::RollbackInProgress
        ) | (
            ReleaseExecutionStatus::Paused,
            ReleaseExecutionStatus::Failed
        ) | (
            ReleaseExecutionStatus::Completed,
            ReleaseExecutionStatus::RollbackInProgress
        ) | (
            ReleaseExecutionStatus::Failed,
            ReleaseExecutionStatus::RollbackInProgress
        ) | (
            ReleaseExecutionStatus::Verifying,
            ReleaseExecutionStatus::Completed
        ) | (
            ReleaseExecutionStatus::Verifying,
            ReleaseExecutionStatus::Failed
        ) | (
            ReleaseExecutionStatus::Verifying,
            ReleaseExecutionStatus::RollbackInProgress
        ) | (
            ReleaseExecutionStatus::RollbackInProgress,
            ReleaseExecutionStatus::RollbackFailed
        ) | (
            ReleaseExecutionStatus::RollbackInProgress,
            ReleaseExecutionStatus::Completed
        ) | (
            ReleaseExecutionStatus::RollbackInProgress,
            ReleaseExecutionStatus::Failed
        )
    )
}

pub(crate) fn validate_release_request_transition(
    from: &ReleaseRequestStatus,
    to: &ReleaseRequestStatus,
) -> anyhow::Result<()> {
    if can_transition_release_request_status(from, to) {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "invalid release request status transition: {} -> {}",
            from,
            to
        ))
    }
}

pub(crate) fn validate_release_execution_transition(
    from: &ReleaseExecutionStatus,
    to: &ReleaseExecutionStatus,
) -> anyhow::Result<()> {
    if can_transition_release_execution_status(from, to) {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "invalid release execution status transition: {} -> {}",
            from,
            to
        ))
    }
}

pub(crate) fn can_transition_release_instance_status(
    from: &ReleaseInstanceStatus,
    to: &ReleaseInstanceStatus,
) -> bool {
    if from == to {
        return true;
    }

    matches!(
        (from, to),
        (
            ReleaseInstanceStatus::Pending,
            ReleaseInstanceStatus::Dispatching
        ) | (
            ReleaseInstanceStatus::Pending,
            ReleaseInstanceStatus::Updating
        ) | (
            ReleaseInstanceStatus::Pending,
            ReleaseInstanceStatus::Failed
        ) | (
            ReleaseInstanceStatus::Pending,
            ReleaseInstanceStatus::Skipped
        ) | (
            ReleaseInstanceStatus::WaitingBatch,
            ReleaseInstanceStatus::Scheduled
        ) | (
            ReleaseInstanceStatus::WaitingBatch,
            ReleaseInstanceStatus::Dispatching
        ) | (
            ReleaseInstanceStatus::WaitingBatch,
            ReleaseInstanceStatus::Pending
        ) | (
            ReleaseInstanceStatus::WaitingBatch,
            ReleaseInstanceStatus::Failed
        ) | (
            ReleaseInstanceStatus::WaitingBatch,
            ReleaseInstanceStatus::Skipped
        ) | (
            ReleaseInstanceStatus::Scheduled,
            ReleaseInstanceStatus::Pending
        ) | (
            ReleaseInstanceStatus::Scheduled,
            ReleaseInstanceStatus::WaitingBatch
        ) | (
            ReleaseInstanceStatus::Scheduled,
            ReleaseInstanceStatus::Dispatching
        ) | (
            ReleaseInstanceStatus::Scheduled,
            ReleaseInstanceStatus::Failed
        ) | (
            ReleaseInstanceStatus::Scheduled,
            ReleaseInstanceStatus::Skipped
        ) | (
            ReleaseInstanceStatus::Dispatching,
            ReleaseInstanceStatus::Updating
        ) | (
            ReleaseInstanceStatus::Dispatching,
            ReleaseInstanceStatus::Failed
        ) | (
            ReleaseInstanceStatus::Updating,
            ReleaseInstanceStatus::Verifying
        ) | (
            ReleaseInstanceStatus::Updating,
            ReleaseInstanceStatus::Success
        ) | (
            ReleaseInstanceStatus::Updating,
            ReleaseInstanceStatus::RolledBack
        ) | (
            ReleaseInstanceStatus::Updating,
            ReleaseInstanceStatus::Failed
        ) | (
            ReleaseInstanceStatus::Verifying,
            ReleaseInstanceStatus::Success
        ) | (
            ReleaseInstanceStatus::Verifying,
            ReleaseInstanceStatus::RolledBack
        ) | (
            ReleaseInstanceStatus::Verifying,
            ReleaseInstanceStatus::Failed
        ) | (
            ReleaseInstanceStatus::Success,
            ReleaseInstanceStatus::Pending
        ) | (
            ReleaseInstanceStatus::Success,
            ReleaseInstanceStatus::WaitingBatch
        ) | (
            ReleaseInstanceStatus::Failed,
            ReleaseInstanceStatus::Pending
        ) | (
            ReleaseInstanceStatus::Failed,
            ReleaseInstanceStatus::WaitingBatch
        )
    )
}

pub(crate) fn validate_release_instance_transition(
    from: &ReleaseInstanceStatus,
    to: &ReleaseInstanceStatus,
) -> anyhow::Result<()> {
    if can_transition_release_instance_status(from, to) {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "invalid release instance status transition: {} -> {}",
            from,
            to
        ))
    }
}

pub(crate) fn release_request_can_execute(
    status: &ReleaseRequestStatus,
    attempts: usize,
) -> anyhow::Result<()> {
    match status {
        ReleaseRequestStatus::Approved => Ok(()),
        ReleaseRequestStatus::Failed if attempts < MAX_RELEASE_EXECUTION_ATTEMPTS => Ok(()),
        ReleaseRequestStatus::Failed => Err(anyhow::anyhow!(
            "release request exceeded max execution attempts ({})",
            MAX_RELEASE_EXECUTION_ATTEMPTS
        )),
        ReleaseRequestStatus::RollbackFailed => Err(anyhow::anyhow!(
            "rollback failed; the release request must finish rollback before it can run again"
        )),
        ReleaseRequestStatus::RolledBack => Err(anyhow::anyhow!(
            "rolled back release requests are closed and cannot be executed again"
        )),
        ReleaseRequestStatus::Completed => Err(anyhow::anyhow!(
            "completed release requests cannot be executed again"
        )),
        ReleaseRequestStatus::Executing => {
            Err(anyhow::anyhow!("release request is already executing"))
        }
        _ => Err(anyhow::anyhow!(
            "release request must be approved before execution"
        )),
    }
}

pub use executions::{
    execute_release_request, get_current_release_execution, get_release_execution_for_request,
    list_release_execution_events, pause_release_execution, resume_release_execution,
    rollback_release_execution, run_release_worker_loop, run_release_worker_once,
};
pub use policies::{
    create_release_policy, delete_release_policy, list_release_policies, update_release_policy,
};
pub use requests::{
    create_release_request, get_release_request, list_release_request_history,
    list_release_requests, preview_release_target,
};
pub use reviews::{approve_release_request, reject_release_request};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_state_machine_closes_after_rollback() {
        assert!(can_transition_release_request_status(
            &ReleaseRequestStatus::Completed,
            &ReleaseRequestStatus::Executing,
        ));
        assert!(can_transition_release_request_status(
            &ReleaseRequestStatus::Executing,
            &ReleaseRequestStatus::RollbackFailed,
        ));
        assert!(can_transition_release_request_status(
            &ReleaseRequestStatus::RollbackFailed,
            &ReleaseRequestStatus::RolledBack,
        ));
        assert!(!can_transition_release_request_status(
            &ReleaseRequestStatus::RolledBack,
            &ReleaseRequestStatus::Executing,
        ));
    }

    #[test]
    fn request_execute_rules_only_allow_approved_or_retryable_failed() {
        assert!(release_request_can_execute(&ReleaseRequestStatus::Approved, 0).is_ok());
        assert!(release_request_can_execute(&ReleaseRequestStatus::Failed, 2).is_ok());
        assert!(release_request_can_execute(&ReleaseRequestStatus::Failed, 3).is_err());
        assert!(release_request_can_execute(&ReleaseRequestStatus::RollbackFailed, 1).is_err());
        assert!(release_request_can_execute(&ReleaseRequestStatus::Completed, 1).is_err());
        assert!(release_request_can_execute(&ReleaseRequestStatus::RolledBack, 1).is_err());
    }

    #[test]
    fn execution_state_machine_allows_manual_rollback_from_terminal_release() {
        assert!(can_transition_release_execution_status(
            &ReleaseExecutionStatus::Completed,
            &ReleaseExecutionStatus::RollbackInProgress,
        ));
        assert!(can_transition_release_execution_status(
            &ReleaseExecutionStatus::Failed,
            &ReleaseExecutionStatus::RollbackInProgress,
        ));
        assert!(can_transition_release_execution_status(
            &ReleaseExecutionStatus::RollbackInProgress,
            &ReleaseExecutionStatus::RollbackFailed,
        ));
        assert!(!can_transition_release_execution_status(
            &ReleaseExecutionStatus::Completed,
            &ReleaseExecutionStatus::RollingOut,
        ));
    }

    #[test]
    fn instance_state_machine_allows_batch_and_rollback_replanning() {
        assert!(can_transition_release_instance_status(
            &ReleaseInstanceStatus::WaitingBatch,
            &ReleaseInstanceStatus::Scheduled,
        ));
        assert!(can_transition_release_instance_status(
            &ReleaseInstanceStatus::WaitingBatch,
            &ReleaseInstanceStatus::Dispatching,
        ));
        assert!(can_transition_release_instance_status(
            &ReleaseInstanceStatus::Scheduled,
            &ReleaseInstanceStatus::Pending,
        ));
        assert!(can_transition_release_instance_status(
            &ReleaseInstanceStatus::Success,
            &ReleaseInstanceStatus::Pending,
        ));
        assert!(can_transition_release_instance_status(
            &ReleaseInstanceStatus::Failed,
            &ReleaseInstanceStatus::WaitingBatch,
        ));
        assert!(!can_transition_release_instance_status(
            &ReleaseInstanceStatus::RolledBack,
            &ReleaseInstanceStatus::Pending,
        ));
    }
}
