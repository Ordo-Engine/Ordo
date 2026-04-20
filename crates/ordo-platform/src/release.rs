use crate::{
    error::{ApiResult, PlatformError},
    models::{
        Claims, CreateReleasePolicyRequest, CreateReleaseRequest, DeploymentStatus,
        ReleaseApprovalDecision, ReleaseContentDiffSummary, ReleaseExecution,
        ReleaseExecutionInstance, ReleaseExecutionStatus, ReleaseInstanceStatus, ReleasePolicy,
        ReleasePolicyScope, ReleaseRequest, ReleaseRequestSnapshot, ReleaseRequestStatus,
        ReleaseStepDiffItem, ReleaseVersionDiff, ReviewReleaseRequest, RulesetDeployment,
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
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use chrono::Utc;
use serde_json::Value as JsonValue;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub async fn list_release_policies(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id)): Path<(String, String)>,
) -> ApiResult<Json<Vec<ReleasePolicy>>> {
    require_project_permission(
        &state,
        &org_id,
        &project_id,
        &claims.sub,
        PERM_RELEASE_REQUEST_VIEW,
    )
    .await?;

    let items = state
        .store
        .list_release_policies(&org_id, Some(&project_id))
        .await
        .map_err(PlatformError::Internal)?;
    Ok(Json(items))
}

pub async fn create_release_policy(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id)): Path<(String, String)>,
    Json(req): Json<CreateReleasePolicyRequest>,
) -> ApiResult<Json<ReleasePolicy>> {
    require_project_permission(
        &state,
        &org_id,
        &project_id,
        &claims.sub,
        PERM_RELEASE_POLICY_MANAGE,
    )
    .await?;

    validate_release_policy_request(&state, &org_id, &project_id, &req).await?;

    let id = Uuid::new_v4().to_string();
    let target_project_id = match req.scope {
        crate::models::ReleasePolicyScope::Org => None,
        crate::models::ReleasePolicyScope::Project => Some(project_id.as_str()),
    };

    let item = state
        .store
        .create_release_policy(&id, &org_id, target_project_id, &req)
        .await
        .map_err(PlatformError::Internal)?;
    Ok(Json(item))
}

pub async fn update_release_policy(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id, policy_id)): Path<(String, String, String)>,
    Json(req): Json<UpdateReleasePolicyRequest>,
) -> ApiResult<Json<ReleasePolicy>> {
    require_project_permission(
        &state,
        &org_id,
        &project_id,
        &claims.sub,
        PERM_RELEASE_POLICY_MANAGE,
    )
    .await?;

    if let Some(min_approvals) = req.min_approvals {
        if min_approvals < 1 {
            return Err(PlatformError::bad_request(
                "min_approvals must be at least 1",
            ));
        }
    }

    let updated = state
        .store
        .update_release_policy(&org_id, &project_id, &policy_id, &req)
        .await
        .map_err(PlatformError::Internal)?;

    if !updated {
        return Err(PlatformError::not_found("Release policy not found"));
    }

    let policy = state
        .store
        .get_release_policy(&org_id, &project_id, &policy_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Release policy not found"))?;
    Ok(Json(policy))
}

pub async fn delete_release_policy(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id, policy_id)): Path<(String, String, String)>,
) -> ApiResult<StatusCode> {
    require_project_permission(
        &state,
        &org_id,
        &project_id,
        &claims.sub,
        PERM_RELEASE_POLICY_MANAGE,
    )
    .await?;

    let deleted = state
        .store
        .delete_release_policy(&org_id, &project_id, &policy_id)
        .await
        .map_err(PlatformError::Internal)?;
    if !deleted {
        return Err(PlatformError::not_found("Release policy not found"));
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_release_requests(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id)): Path<(String, String)>,
) -> ApiResult<Json<Vec<ReleaseRequest>>> {
    require_project_permission(
        &state,
        &org_id,
        &project_id,
        &claims.sub,
        PERM_RELEASE_REQUEST_VIEW,
    )
    .await?;

    let items = state
        .store
        .list_release_requests(&org_id, &project_id)
        .await
        .map_err(PlatformError::Internal)?;
    Ok(Json(items))
}

pub async fn create_release_request(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id)): Path<(String, String)>,
    Json(req): Json<CreateReleaseRequest>,
) -> ApiResult<Json<ReleaseRequest>> {
    require_project_permission(
        &state,
        &org_id,
        &project_id,
        &claims.sub,
        PERM_RELEASE_REQUEST_CREATE,
    )
    .await?;

    if req.ruleset_name.trim().is_empty()
        || req.version.trim().is_empty()
        || req.environment_id.trim().is_empty()
        || req.title.trim().is_empty()
        || req.change_summary.trim().is_empty()
    {
        return Err(PlatformError::bad_request(
            "ruleset_name, version, environment_id, title, and change_summary are required",
        ));
    }

    let environment = state
        .store
        .get_environment(&project_id, &req.environment_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Environment not found"))?;

    let policy = if let Some(policy_id) = req.policy_id.as_deref() {
        state
            .store
            .get_release_policy(&org_id, &project_id, policy_id)
            .await
            .map_err(PlatformError::Internal)?
            .ok_or_else(|| PlatformError::not_found("Release policy not found"))?
    } else {
        state
            .store
            .find_matching_release_policy(&org_id, &project_id, &req.environment_id)
            .await
            .map_err(PlatformError::Internal)?
            .ok_or_else(|| {
                PlatformError::bad_request(
                    "No release policy matched this project/environment target",
                )
            })?
    };

    if policy.approver_ids.len() < policy.min_approvals as usize {
        return Err(PlatformError::bad_request(
            "Release policy does not define enough approvers for min_approvals",
        ));
    }

    let requester = state
        .store
        .get_user(&claims.sub)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("User not found"))?;

    let draft = state
        .store
        .get_draft_ruleset(&project_id, &req.ruleset_name)
        .await
        .map_err(PlatformError::Internal)?;
    let current_version = draft
        .as_ref()
        .and_then(|item| item.meta.published_version.clone());
    let baseline_snapshot = load_release_baseline_snapshot(
        &state,
        &org_id,
        &project_id,
        &req.ruleset_name,
        current_version.as_deref(),
    )
    .await?;
    let target_snapshot = draft
        .as_ref()
        .map(|item| item.draft.clone())
        .unwrap_or(JsonValue::Null);

    let approver_users = {
        let mut items = Vec::new();
        for approver_id in &policy.approver_ids {
            if let Some(user) = state
                .store
                .get_user(approver_id)
                .await
                .map_err(PlatformError::Internal)?
            {
                items.push(user);
            }
        }
        items
    };

    let version_diff = ReleaseVersionDiff {
        from_version: current_version.clone(),
        to_version: req.version.clone(),
        rollback_version: req
            .rollback_version
            .clone()
            .or_else(|| current_version.clone()),
        changed: current_version.as_deref() != Some(req.version.as_str()),
    };
    let content_diff = build_release_content_diff(
        baseline_snapshot.as_ref(),
        &target_snapshot,
        current_version.as_deref(),
    );

    let request_snapshot = ReleaseRequestSnapshot {
        requester_id: requester.id.clone(),
        requester_name: Some(requester.display_name.clone()),
        requester_email: Some(requester.email.clone()),
        policy_name: Some(policy.name.clone()),
        policy_scope: Some(match policy.scope {
            ReleasePolicyScope::Org => ReleasePolicyScope::Org,
            ReleasePolicyScope::Project => ReleasePolicyScope::Project,
        }),
        target_type: Some(policy.target_type.clone()),
        target_id: Some(policy.target_id.clone()),
        environment_name: Some(environment.name.clone()),
        approver_ids: policy.approver_ids.clone(),
        approver_names: approver_users
            .iter()
            .map(|user| user.display_name.clone())
            .collect(),
        approver_emails: approver_users
            .iter()
            .map(|user| user.email.clone())
            .collect(),
        rollout_strategy: policy.rollout_strategy.clone(),
        rollback_policy: policy.rollback_policy.clone(),
        affected_instance_count: req.affected_instance_count.unwrap_or_default(),
    };

    let mut create_req = req;
    create_req.policy_id = Some(policy.id.clone());
    let release_id = Uuid::new_v4().to_string();
    let created = state
        .store
        .create_release_request(
            &release_id,
            &org_id,
            &project_id,
            &claims.sub,
            Some(&requester.display_name),
            Some(&requester.email),
            &create_req,
            current_version.as_deref(),
            &version_diff,
            &content_diff,
            &request_snapshot,
        )
        .await
        .map_err(PlatformError::Internal)?;

    for (idx, reviewer_id) in policy
        .approver_ids
        .iter()
        .take(policy.min_approvals as usize)
        .enumerate()
    {
        state
            .store
            .create_release_approval(
                &Uuid::new_v4().to_string(),
                &release_id,
                (idx as i32) + 1,
                reviewer_id,
                approver_users
                    .iter()
                    .find(|user| user.id == *reviewer_id)
                    .map(|user| user.display_name.as_str()),
                approver_users
                    .iter()
                    .find(|user| user.id == *reviewer_id)
                    .map(|user| user.email.as_str()),
            )
            .await
            .map_err(PlatformError::Internal)?;
    }

    let release = state
        .store
        .get_release_request(&org_id, &project_id, &created.id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Release request not found"))?;
    Ok(Json(release))
}

pub async fn get_release_request(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id, release_id)): Path<(String, String, String)>,
) -> ApiResult<Json<ReleaseRequest>> {
    require_project_permission(
        &state,
        &org_id,
        &project_id,
        &claims.sub,
        PERM_RELEASE_REQUEST_VIEW,
    )
    .await?;

    let item = state
        .store
        .get_release_request(&org_id, &project_id, &release_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Release request not found"))?;
    Ok(Json(item))
}

pub async fn get_release_execution_for_request(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id, release_id)): Path<(String, String, String)>,
) -> ApiResult<Json<Option<ReleaseExecution>>> {
    require_project_permission(
        &state,
        &org_id,
        &project_id,
        &claims.sub,
        PERM_RELEASE_INSTANCE_VIEW,
    )
    .await?;

    let item = state
        .store
        .find_release_execution_by_request_id(&release_id)
        .await
        .map_err(PlatformError::Internal)?;
    Ok(Json(item))
}

pub async fn get_current_release_execution(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id)): Path<(String, String)>,
) -> ApiResult<Json<Option<ReleaseExecution>>> {
    require_project_permission(
        &state,
        &org_id,
        &project_id,
        &claims.sub,
        PERM_RELEASE_INSTANCE_VIEW,
    )
    .await?;

    let item = state
        .store
        .find_latest_project_release_execution(&org_id, &project_id)
        .await
        .map_err(PlatformError::Internal)?;
    Ok(Json(item))
}

pub async fn execute_release_request(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id, release_id)): Path<(String, String, String)>,
) -> ApiResult<Json<ReleaseExecution>> {
    require_project_permission(
        &state,
        &org_id,
        &project_id,
        &claims.sub,
        PERM_RELEASE_EXECUTE,
    )
    .await?;

    let release = state
        .store
        .get_release_request(&org_id, &project_id, &release_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Release request not found"))?;

    if release.status != ReleaseRequestStatus::Approved
        && release.status != ReleaseRequestStatus::Executing
    {
        return Err(PlatformError::conflict(
            "Release request must be approved before execution",
        ));
    }

    let env = state
        .store
        .get_environment(&project_id, &release.environment_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Environment not found"))?;
    let server_id = env
        .server_id
        .clone()
        .ok_or_else(|| PlatformError::bad_request("Environment has no bound server"))?;
    let server = state
        .store
        .get_server(&server_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Bound server not found"))?;
    let draft = state
        .store
        .get_draft_ruleset(&project_id, &release.ruleset_name)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Draft ruleset not found"))?;

    let execution_id = Uuid::new_v4().to_string();
    let instance_id = Uuid::new_v4().to_string();
    let total_batches = 1;
    let strategy = release.request_snapshot.rollout_strategy.clone();

    state
        .store
        .set_release_request_status(&release.id, ReleaseRequestStatus::Executing)
        .await
        .map_err(PlatformError::Internal)?;

    state
        .store
        .create_release_execution(
            &execution_id,
            &release.id,
            ReleaseExecutionStatus::Preparing,
            0,
            total_batches,
            &strategy,
            Some(&claims.sub),
        )
        .await
        .map_err(PlatformError::Internal)?;

    let instance = ReleaseExecutionInstance {
        id: instance_id.clone(),
        release_execution_id: execution_id.clone(),
        instance_id: server.id.clone(),
        instance_name: server.name.clone(),
        zone: server
            .labels
            .get("zone")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        current_version: release
            .version_diff
            .from_version
            .clone()
            .unwrap_or_else(|| "unreleased".to_string()),
        target_version: release.version.clone(),
        status: ReleaseInstanceStatus::Pending,
        updated_at: Utc::now(),
        message: None,
        metric_summary: None,
    };
    state
        .store
        .create_release_execution_instance(&instance)
        .await
        .map_err(PlatformError::Internal)?;

    state
        .store
        .update_release_execution_status(&execution_id, ReleaseExecutionStatus::RollingOut, Some(1))
        .await
        .map_err(PlatformError::Internal)?;
    state
        .store
        .update_release_execution_instance(
            &instance_id,
            ReleaseInstanceStatus::Updating,
            Some("Dispatching ruleset to bound server"),
            None,
        )
        .await
        .map_err(PlatformError::Internal)?;

    let publish_result = publish_release_via_nats(
        &state,
        &env,
        &project_id,
        &release.ruleset_name,
        &draft.draft,
        &release.version,
    )
    .await;

    match publish_result {
        Ok(()) => {
            state
                .store
                .update_release_execution_instance(
                    &instance_id,
                    ReleaseInstanceStatus::Success,
                    Some("Ruleset pushed and acknowledged"),
                    Some("publish_ack"),
                )
                .await
                .map_err(PlatformError::Internal)?;
            state
                .store
                .update_release_execution_status(
                    &execution_id,
                    ReleaseExecutionStatus::Completed,
                    Some(1),
                )
                .await
                .map_err(PlatformError::Internal)?;
            state
                .store
                .set_release_request_status(&release.id, ReleaseRequestStatus::Completed)
                .await
                .map_err(PlatformError::Internal)?;
            state
                .store
                .mark_ruleset_published(&project_id, &release.ruleset_name, &release.version)
                .await
                .map_err(PlatformError::Internal)?;

            let deployment = RulesetDeployment {
                id: Uuid::new_v4().to_string(),
                project_id: project_id.clone(),
                environment_id: env.id.clone(),
                environment_name: Some(env.name.clone()),
                ruleset_name: release.ruleset_name.clone(),
                version: release.version.clone(),
                release_note: release.release_note.clone(),
                snapshot: draft.draft.clone(),
                deployed_at: Utc::now(),
                deployed_by: Some(claims.sub.clone()),
                status: DeploymentStatus::Success,
            };
            state
                .store
                .create_deployment(&deployment)
                .await
                .map_err(PlatformError::Internal)?;

            if let Some(user) = state
                .store
                .get_user(&claims.sub)
                .await
                .map_err(PlatformError::Internal)?
            {
                let entry = RulesetHistoryEntry {
                    id: Uuid::new_v4().to_string(),
                    ruleset_name: release.ruleset_name.clone(),
                    action: format!("released to {}", env.name),
                    source: RulesetHistorySource::Publish,
                    created_at: Utc::now(),
                    author_id: claims.sub.clone(),
                    author_email: user.email,
                    author_display_name: user.display_name,
                    snapshot: draft.draft.clone(),
                };
                let _ = state
                    .store
                    .append_ruleset_history(&org_id, &project_id, &release.ruleset_name, &[entry])
                    .await;
            }
        }
        Err(err) => {
            state
                .store
                .update_release_execution_instance(
                    &instance_id,
                    ReleaseInstanceStatus::Failed,
                    Some(&err.to_string()),
                    Some("publish_failed"),
                )
                .await
                .map_err(PlatformError::Internal)?;
            state
                .store
                .update_release_execution_status(
                    &execution_id,
                    ReleaseExecutionStatus::Failed,
                    Some(1),
                )
                .await
                .map_err(PlatformError::Internal)?;
            state
                .store
                .set_release_request_status(&release.id, ReleaseRequestStatus::Failed)
                .await
                .map_err(PlatformError::Internal)?;
        }
    }

    let execution = state
        .store
        .get_release_execution(&execution_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Release execution not found"))?;
    Ok(Json(execution))
}

struct ControlExecutionParams {
    org_id: String,
    project_id: String,
    release_id: String,
    permission: &'static str,
    target_status: ReleaseExecutionStatus,
    request_status: Option<ReleaseRequestStatus>,
    event_type: &'static str,
    invalid_state_message: &'static str,
}

pub async fn pause_release_execution(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id, release_id)): Path<(String, String, String)>,
) -> ApiResult<Json<ReleaseExecution>> {
    control_release_execution(
        state,
        claims,
        ControlExecutionParams {
            org_id,
            project_id,
            release_id,
            permission: PERM_RELEASE_PAUSE,
            target_status: ReleaseExecutionStatus::Paused,
            request_status: Some(ReleaseRequestStatus::Executing),
            event_type: "execution_paused",
            invalid_state_message: "Release execution is not active",
        },
    )
    .await
}

pub async fn resume_release_execution(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id, release_id)): Path<(String, String, String)>,
) -> ApiResult<Json<ReleaseExecution>> {
    control_release_execution(
        state,
        claims,
        ControlExecutionParams {
            org_id,
            project_id,
            release_id,
            permission: PERM_RELEASE_RESUME,
            target_status: ReleaseExecutionStatus::RollingOut,
            request_status: Some(ReleaseRequestStatus::Executing),
            event_type: "execution_resumed",
            invalid_state_message: "Release execution is not paused",
        },
    )
    .await
}

pub async fn rollback_release_execution(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id, release_id)): Path<(String, String, String)>,
) -> ApiResult<Json<ReleaseExecution>> {
    require_project_permission(
        &state,
        &org_id,
        &project_id,
        &claims.sub,
        PERM_RELEASE_ROLLBACK,
    )
    .await?;

    let release = state
        .store
        .get_release_request(&org_id, &project_id, &release_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Release request not found"))?;

    let execution = state
        .store
        .find_release_execution_by_request_id(&release.id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Release execution not found"))?;

    if execution.status == ReleaseExecutionStatus::RollbackInProgress {
        return Err(PlatformError::conflict(
            "Release execution is already rolling back",
        ));
    }
    if execution.status != ReleaseExecutionStatus::Completed
        && execution.status != ReleaseExecutionStatus::Failed
        && execution.status != ReleaseExecutionStatus::Paused
    {
        return Err(PlatformError::conflict(
            "Release execution cannot be rolled back from its current status",
        ));
    }

    let rollback_version = release
        .version_diff
        .rollback_version
        .clone()
        .or_else(|| release.rollback_version.clone())
        .ok_or_else(|| PlatformError::bad_request("Release request has no rollback version"))?;

    let env = state
        .store
        .get_environment(&project_id, &release.environment_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Environment not found"))?;

    let rollback_deployment = state
        .store
        .list_deployments(&project_id, Some(&release.ruleset_name), 50)
        .await
        .map_err(PlatformError::Internal)?
        .into_iter()
        .find(|deployment| {
            deployment.environment_id == release.environment_id
                && deployment.version == rollback_version
                && deployment.status == DeploymentStatus::Success
        })
        .ok_or_else(|| PlatformError::not_found("Rollback deployment snapshot not found"))?;

    state
        .store
        .update_release_execution_status(
            &execution.id,
            ReleaseExecutionStatus::RollbackInProgress,
            Some(execution.current_batch),
        )
        .await
        .map_err(PlatformError::Internal)?;
    let _ = state
        .store
        .create_release_execution_event(
            &Uuid::new_v4().to_string(),
            &execution.id,
            None,
            "rollback_started",
            serde_json::json!({
                "release_id": release.id,
                "rollback_version": rollback_version,
                "requested_by": claims.sub,
            }),
        )
        .await;

    for instance in &execution.instances {
        let _ = state
            .store
            .update_release_execution_instance(
                &instance.id,
                ReleaseInstanceStatus::Updating,
                Some("Rolling back to previous deployment snapshot"),
                Some("rollback_started"),
            )
            .await;
    }

    let publish_result = publish_release_via_nats(
        &state,
        &env,
        &project_id,
        &release.ruleset_name,
        &rollback_deployment.snapshot,
        &rollback_version,
    )
    .await;

    match publish_result {
        Ok(()) => {
            for instance in &execution.instances {
                let _ = state
                    .store
                    .update_release_execution_instance(
                        &instance.id,
                        ReleaseInstanceStatus::RolledBack,
                        Some("Rollback snapshot pushed and acknowledged"),
                        Some("rollback_ack"),
                    )
                    .await;
            }
            state
                .store
                .update_release_execution_status(
                    &execution.id,
                    ReleaseExecutionStatus::Completed,
                    Some(execution.total_batches),
                )
                .await
                .map_err(PlatformError::Internal)?;
            state
                .store
                .set_release_request_status(&release.id, ReleaseRequestStatus::RolledBack)
                .await
                .map_err(PlatformError::Internal)?;
            state
                .store
                .mark_ruleset_published(&project_id, &release.ruleset_name, &rollback_version)
                .await
                .map_err(PlatformError::Internal)?;

            let deployment = RulesetDeployment {
                id: Uuid::new_v4().to_string(),
                project_id: project_id.clone(),
                environment_id: env.id.clone(),
                environment_name: Some(env.name.clone()),
                ruleset_name: release.ruleset_name.clone(),
                version: rollback_version.clone(),
                release_note: Some(format!("Rollback for release {}", release.id)),
                snapshot: rollback_deployment.snapshot.clone(),
                deployed_at: Utc::now(),
                deployed_by: Some(claims.sub.clone()),
                status: DeploymentStatus::Success,
            };
            state
                .store
                .create_deployment(&deployment)
                .await
                .map_err(PlatformError::Internal)?;
            let _ = state
                .store
                .create_release_execution_event(
                    &Uuid::new_v4().to_string(),
                    &execution.id,
                    None,
                    "rollback_completed",
                    serde_json::json!({
                        "rollback_version": rollback_version,
                        "deployed_by": claims.sub,
                    }),
                )
                .await;
        }
        Err(err) => {
            for instance in &execution.instances {
                let _ = state
                    .store
                    .update_release_execution_instance(
                        &instance.id,
                        ReleaseInstanceStatus::Failed,
                        Some(&err.to_string()),
                        Some("rollback_failed"),
                    )
                    .await;
            }
            state
                .store
                .update_release_execution_status(
                    &execution.id,
                    ReleaseExecutionStatus::Failed,
                    Some(execution.current_batch),
                )
                .await
                .map_err(PlatformError::Internal)?;
            let _ = state
                .store
                .create_release_execution_event(
                    &Uuid::new_v4().to_string(),
                    &execution.id,
                    None,
                    "rollback_failed",
                    serde_json::json!({
                        "rollback_version": rollback_version,
                        "error": err.to_string(),
                    }),
                )
                .await;
            return Err(PlatformError::bad_request("Rollback publish failed"));
        }
    }

    let updated = state
        .store
        .get_release_execution(&execution.id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Release execution not found"))?;
    Ok(Json(updated))
}

struct ReviewParams {
    org_id: String,
    project_id: String,
    release_id: String,
    decision: ReleaseApprovalDecision,
    permission: &'static str,
    comment: Option<String>,
}

pub async fn approve_release_request(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id, release_id)): Path<(String, String, String)>,
    Json(req): Json<ReviewReleaseRequest>,
) -> ApiResult<Json<ReleaseRequest>> {
    review_release(
        state,
        claims,
        ReviewParams {
            org_id,
            project_id,
            release_id,
            decision: ReleaseApprovalDecision::Approved,
            permission: PERM_RELEASE_REQUEST_APPROVE,
            comment: req.comment,
        },
    )
    .await
}

pub async fn reject_release_request(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id, release_id)): Path<(String, String, String)>,
    Json(req): Json<ReviewReleaseRequest>,
) -> ApiResult<Json<ReleaseRequest>> {
    review_release(
        state,
        claims,
        ReviewParams {
            org_id,
            project_id,
            release_id,
            decision: ReleaseApprovalDecision::Rejected,
            permission: PERM_RELEASE_REQUEST_REJECT,
            comment: req.comment,
        },
    )
    .await
}

async fn review_release(
    state: AppState,
    claims: Claims,
    ReviewParams {
        org_id,
        project_id,
        release_id,
        decision,
        permission,
        comment,
    }: ReviewParams,
) -> ApiResult<Json<ReleaseRequest>> {
    require_project_permission(&state, &org_id, &project_id, &claims.sub, permission).await?;

    let release = state
        .store
        .get_release_request(&org_id, &project_id, &release_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Release request not found"))?;

    if release.status != ReleaseRequestStatus::PendingApproval {
        return Err(PlatformError::conflict(
            "Release request is not pending approval",
        ));
    }

    let policy = if let Some(policy_id) = release.policy_id.as_deref() {
        state
            .store
            .get_release_policy(&org_id, &project_id, policy_id)
            .await
            .map_err(PlatformError::Internal)?
    } else {
        None
    };

    if let Some(policy) = policy {
        if !policy.approver_ids.iter().any(|id| id == &claims.sub) {
            return Err(PlatformError::forbidden(
                "You are not an assigned approver for this release request",
            ));
        }
    }

    let updated = state
        .store
        .review_release_request(
            &release_id,
            &claims.sub,
            decision.clone(),
            comment.as_deref(),
        )
        .await
        .map_err(PlatformError::Internal)?;
    if !updated {
        return Err(PlatformError::conflict(
            "No pending approval found for this reviewer",
        ));
    }

    let approvals = state
        .store
        .list_release_approvals(&release_id)
        .await
        .map_err(PlatformError::Internal)?;

    let next_status = if approvals
        .iter()
        .any(|item| item.decision == ReleaseApprovalDecision::Rejected)
    {
        ReleaseRequestStatus::Rejected
    } else if approvals
        .iter()
        .all(|item| item.decision == ReleaseApprovalDecision::Approved)
    {
        ReleaseRequestStatus::Approved
    } else {
        ReleaseRequestStatus::PendingApproval
    };

    state
        .store
        .set_release_request_status(&release_id, next_status)
        .await
        .map_err(PlatformError::Internal)?;

    let item = state
        .store
        .get_release_request(&org_id, &project_id, &release_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Release request not found"))?;
    Ok(Json(item))
}

async fn control_release_execution(
    state: AppState,
    claims: Claims,
    ControlExecutionParams {
        org_id,
        project_id,
        release_id,
        permission,
        target_status,
        request_status,
        event_type,
        invalid_state_message,
    }: ControlExecutionParams,
) -> ApiResult<Json<ReleaseExecution>> {
    require_project_permission(&state, &org_id, &project_id, &claims.sub, permission).await?;

    let release = state
        .store
        .get_release_request(&org_id, &project_id, &release_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Release request not found"))?;

    let execution = state
        .store
        .find_release_execution_by_request_id(&release.id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Release execution not found"))?;

    let is_valid = match target_status {
        ReleaseExecutionStatus::Paused => matches!(
            execution.status,
            ReleaseExecutionStatus::Preparing
                | ReleaseExecutionStatus::WaitingStart
                | ReleaseExecutionStatus::RollingOut
                | ReleaseExecutionStatus::Verifying
        ),
        ReleaseExecutionStatus::RollingOut => execution.status == ReleaseExecutionStatus::Paused,
        _ => false,
    };
    if !is_valid {
        return Err(PlatformError::conflict(invalid_state_message));
    }

    state
        .store
        .update_release_execution_status(
            &execution.id,
            target_status.clone(),
            Some(execution.current_batch),
        )
        .await
        .map_err(PlatformError::Internal)?;
    if let Some(next_request_status) = request_status {
        state
            .store
            .set_release_request_status(&release.id, next_request_status)
            .await
            .map_err(PlatformError::Internal)?;
    }
    let _ = state
        .store
        .create_release_execution_event(
            &Uuid::new_v4().to_string(),
            &execution.id,
            None,
            event_type,
            serde_json::json!({
                "release_id": release.id,
                "changed_by": claims.sub,
                "status": target_status.to_string(),
            }),
        )
        .await;

    let updated = state
        .store
        .get_release_execution(&execution.id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Release execution not found"))?;
    Ok(Json(updated))
}

async fn load_release_baseline_snapshot(
    state: &AppState,
    org_id: &str,
    project_id: &str,
    ruleset_name: &str,
    current_version: Option<&str>,
) -> ApiResult<Option<JsonValue>> {
    let history = state
        .store
        .get_ruleset_history(org_id, project_id, ruleset_name)
        .await
        .map_err(PlatformError::Internal)?;

    let matching = history.iter().rev().find(|entry| {
        entry.source == RulesetHistorySource::Publish
            && extract_ruleset_version(&entry.snapshot) == current_version
    });
    let latest_publish = history
        .iter()
        .rev()
        .find(|entry| entry.source == RulesetHistorySource::Publish);

    Ok(matching
        .or(latest_publish)
        .map(|entry| entry.snapshot.clone()))
}

fn build_release_content_diff(
    baseline: Option<&JsonValue>,
    target: &JsonValue,
    baseline_version: Option<&str>,
) -> ReleaseContentDiffSummary {
    let before_steps = baseline.map(extract_steps).unwrap_or_default();
    let after_steps = extract_steps(target);
    let before_groups = baseline.map(extract_groups).unwrap_or_default();
    let after_groups = extract_groups(target);

    let before_ids: BTreeSet<_> = before_steps.keys().cloned().collect();
    let after_ids: BTreeSet<_> = after_steps.keys().cloned().collect();
    let added_steps = after_ids
        .difference(&before_ids)
        .filter_map(|id| after_steps.get(id))
        .map(|item| item.descriptor())
        .collect();
    let removed_steps = before_ids
        .difference(&after_ids)
        .filter_map(|id| before_steps.get(id))
        .map(|item| item.descriptor())
        .collect();
    let modified_steps = before_ids
        .intersection(&after_ids)
        .filter_map(|id| {
            let before = before_steps.get(id)?;
            let after = after_steps.get(id)?;
            if before.canonical != after.canonical {
                Some(after.descriptor())
            } else {
                None
            }
        })
        .collect();

    let before_group_names: BTreeSet<_> = before_groups.keys().cloned().collect();
    let after_group_names: BTreeSet<_> = after_groups.keys().cloned().collect();
    let added_groups = after_group_names
        .difference(&before_group_names)
        .cloned()
        .collect();
    let removed_groups = before_group_names
        .difference(&after_group_names)
        .cloned()
        .collect();
    let modified_groups = before_group_names
        .intersection(&after_group_names)
        .filter_map(|name| {
            let before = before_groups.get(name)?;
            let after = after_groups.get(name)?;
            if before != after {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect();

    ReleaseContentDiffSummary {
        baseline_version: baseline_version.map(str::to_string),
        step_count_before: before_steps.len() as i32,
        step_count_after: after_steps.len() as i32,
        group_count_before: before_groups.len() as i32,
        group_count_after: after_groups.len() as i32,
        added_steps,
        removed_steps,
        modified_steps,
        added_groups,
        removed_groups,
        modified_groups,
        input_schema_changed: extract_schema_len(baseline, "inputSchema")
            != extract_schema_len(Some(target), "inputSchema"),
        output_schema_changed: extract_schema_len(baseline, "outputSchema")
            != extract_schema_len(Some(target), "outputSchema"),
        tags_changed: extract_string_array(
            baseline.and_then(|value| value.get("config").and_then(|cfg| cfg.get("tags"))),
        ) != extract_string_array(
            target.get("config").and_then(|cfg| cfg.get("tags")),
        ),
        description_changed: extract_optional_string(
            baseline.and_then(|value| value.get("config").and_then(|cfg| cfg.get("description"))),
        ) != extract_optional_string(
            target.get("config").and_then(|cfg| cfg.get("description")),
        ),
    }
}

#[derive(Clone)]
struct StepSnapshot {
    id: String,
    name: String,
    step_type: Option<String>,
    canonical: String,
}

impl StepSnapshot {
    fn descriptor(&self) -> ReleaseStepDiffItem {
        ReleaseStepDiffItem {
            id: self.id.clone(),
            name: self.name.clone(),
            step_type: self.step_type.clone(),
        }
    }
}

fn extract_steps(snapshot: &JsonValue) -> BTreeMap<String, StepSnapshot> {
    let mut items = BTreeMap::new();
    match snapshot.get("steps") {
        Some(JsonValue::Array(steps)) => {
            for step in steps {
                if let Some((id, item)) = extract_step_snapshot(step) {
                    items.insert(id, item);
                }
            }
        }
        Some(JsonValue::Object(steps)) => {
            for step in steps.values() {
                if let Some((id, item)) = extract_step_snapshot(step) {
                    items.insert(id, item);
                }
            }
        }
        _ => {}
    }
    items
}

fn extract_step_snapshot(step: &JsonValue) -> Option<(String, StepSnapshot)> {
    let id = step.get("id")?.as_str()?.to_string();
    let name = step
        .get("name")
        .and_then(|value| value.as_str())
        .unwrap_or(&id)
        .to_string();
    let step_type = step
        .get("type")
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let canonical = serde_json::to_string(step).ok()?;
    Some((
        id.clone(),
        StepSnapshot {
            id,
            name,
            step_type,
            canonical,
        },
    ))
}

fn extract_groups(snapshot: &JsonValue) -> BTreeMap<String, String> {
    let mut items = BTreeMap::new();
    if let Some(JsonValue::Array(groups)) = snapshot.get("groups") {
        for group in groups {
            let name = group
                .get("name")
                .and_then(|value| value.as_str())
                .or_else(|| group.get("id").and_then(|value| value.as_str()));
            if let Some(name) = name {
                let canonical = serde_json::to_string(group).unwrap_or_default();
                items.insert(name.to_string(), canonical);
            }
        }
    }
    items
}

fn extract_schema_len(snapshot: Option<&JsonValue>, field: &str) -> usize {
    snapshot
        .and_then(|value| value.get("config"))
        .and_then(|config| config.get(field))
        .and_then(|value| value.as_array())
        .map(Vec::len)
        .unwrap_or(0)
}

fn extract_string_array(value: Option<&JsonValue>) -> Vec<String> {
    value
        .and_then(|item| item.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|entry| entry.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn extract_optional_string(value: Option<&JsonValue>) -> Option<String> {
    value.and_then(|item| item.as_str()).map(str::to_string)
}

fn extract_ruleset_version(snapshot: &JsonValue) -> Option<&str> {
    snapshot
        .get("config")
        .and_then(|config| config.get("version"))
        .and_then(|value| value.as_str())
}

async fn publish_release_via_nats(
    state: &AppState,
    env: &crate::models::ProjectEnvironment,
    project_id: &str,
    ruleset_name: &str,
    ruleset_json: &JsonValue,
    version: &str,
) -> anyhow::Result<()> {
    let publisher = state
        .sync_publisher
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("NATS publisher is not configured"))?;

    let json_str = serde_json::to_string(ruleset_json)?;
    let event = SyncEvent::RulePut {
        tenant_id: project_id.to_string(),
        name: ruleset_name.to_string(),
        ruleset_json: json_str,
        version: version.to_string(),
    };

    let prefix = env
        .nats_subject_prefix
        .as_deref()
        .unwrap_or(&state.config.nats_subject_prefix);

    publisher.publish_to(prefix, event).await
}

async fn validate_release_policy_request(
    state: &AppState,
    org_id: &str,
    project_id: &str,
    req: &CreateReleasePolicyRequest,
) -> ApiResult<()> {
    if req.name.trim().is_empty() || req.target_id.trim().is_empty() {
        return Err(PlatformError::bad_request(
            "name and target_id are required",
        ));
    }
    if req.min_approvals < 1 {
        return Err(PlatformError::bad_request(
            "min_approvals must be at least 1",
        ));
    }
    if req.approver_ids.len() < req.min_approvals as usize {
        return Err(PlatformError::bad_request(
            "approver_ids must satisfy min_approvals",
        ));
    }

    let org = state
        .store
        .get_org(org_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Organization not found"))?;
    let member_ids: std::collections::HashSet<String> =
        org.members.into_iter().map(|m| m.user_id).collect();

    for approver_id in &req.approver_ids {
        if !member_ids.contains(approver_id) {
            return Err(PlatformError::approver_not_member(approver_id));
        }
    }

    match req.target_type {
        crate::models::ReleasePolicyTargetType::Project => {
            if req.target_id != project_id {
                return Err(PlatformError::bad_request(
                    "Project-targeted policy must target the current project",
                ));
            }
        }
        crate::models::ReleasePolicyTargetType::Environment => {
            state
                .store
                .get_environment(project_id, &req.target_id)
                .await
                .map_err(PlatformError::Internal)?
                .ok_or_else(|| PlatformError::not_found("Environment not found"))?;
        }
    }

    Ok(())
}
