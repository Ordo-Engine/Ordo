use super::*;
use std::time::Duration;
use tracing::{error, info, warn};

const RELEASE_ACK_TIMEOUT_SECS: u64 = 30;
const ROLLBACK_BATCH_INTERVAL_SECS: u64 = 0;

fn execution_is_active(execution: &ReleaseExecution) -> bool {
    matches!(
        execution.status,
        ReleaseExecutionStatus::Preparing
            | ReleaseExecutionStatus::WaitingStart
            | ReleaseExecutionStatus::RollingOut
            | ReleaseExecutionStatus::Paused
            | ReleaseExecutionStatus::Verifying
            | ReleaseExecutionStatus::RollbackInProgress
    )
}

fn instance_is_terminal(status: &ReleaseInstanceStatus) -> bool {
    matches!(
        status,
        ReleaseInstanceStatus::Success
            | ReleaseInstanceStatus::Failed
            | ReleaseInstanceStatus::RolledBack
            | ReleaseInstanceStatus::Skipped
    )
}

async fn set_release_request_status_with_history(
    state: &AppState,
    release_request_id: &str,
    from_status: ReleaseRequestStatus,
    to_status: ReleaseRequestStatus,
    actor: &ReleaseHistoryActor,
    detail: JsonValue,
) -> anyhow::Result<()> {
    validate_release_request_transition(&from_status, &to_status)?;
    state
        .store
        .set_release_request_status(release_request_id, to_status.clone())
        .await?;

    if from_status == to_status {
        return Ok(());
    }

    append_release_history(
        state,
        release_request_id,
        None,
        None,
        ReleaseHistoryScope::Request,
        "request_status_changed",
        actor,
        Some(from_status.to_string()),
        Some(to_status.to_string()),
        detail,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn update_release_execution_status_with_history(
    state: &AppState,
    release_request_id: &str,
    execution_id: &str,
    from_status: Option<ReleaseExecutionStatus>,
    to_status: ReleaseExecutionStatus,
    current_batch: Option<i32>,
    actor: &ReleaseHistoryActor,
    detail: JsonValue,
) -> anyhow::Result<()> {
    let previous = match from_status {
        Some(status) => status,
        None => {
            state
                .store
                .get_release_execution(execution_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Release execution not found"))?
                .status
        }
    };

    validate_release_execution_transition(&previous, &to_status)?;

    state
        .store
        .update_release_execution_status(execution_id, to_status.clone(), current_batch)
        .await?;

    append_release_history(
        state,
        release_request_id,
        Some(execution_id),
        None,
        ReleaseHistoryScope::Execution,
        "execution_status_changed",
        actor,
        Some(previous.to_string()),
        Some(to_status.to_string()),
        detail,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn update_instance_status_with_history(
    state: &AppState,
    release_request_id: &str,
    execution_id: &str,
    instance_id: &str,
    to_status: ReleaseInstanceStatus,
    message: Option<&str>,
    metric_summary: Option<&str>,
    actor: &ReleaseHistoryActor,
    action: &str,
    detail: JsonValue,
) -> anyhow::Result<()> {
    let previous = state
        .store
        .get_release_execution_instance(instance_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Release execution instance not found"))?;

    validate_release_instance_transition(&previous.status, &to_status)?;

    state
        .store
        .update_release_execution_instance(instance_id, to_status.clone(), message, metric_summary)
        .await?;

    append_release_history(
        state,
        release_request_id,
        Some(execution_id),
        Some(instance_id),
        ReleaseHistoryScope::Instance,
        action,
        actor,
        Some(previous.status.to_string()),
        Some(to_status.to_string()),
        merge_history_detail(
            serde_json::json!({
                "instance_name": previous.instance_name,
                "target_instance_id": previous.instance_id,
                "batch_index": previous.batch_index,
                "zone": previous.zone,
                "current_version": previous.current_version,
                "target_version": previous.target_version,
                "message": message,
            }),
            detail,
        ),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn update_instance_schedule_with_history(
    state: &AppState,
    release_request_id: &str,
    execution_id: &str,
    instance_id: &str,
    to_status: ReleaseInstanceStatus,
    scheduled_at: Option<chrono::DateTime<chrono::Utc>>,
    message: Option<&str>,
    actor: &ReleaseHistoryActor,
    action: &str,
    detail: JsonValue,
) -> anyhow::Result<()> {
    let previous = state
        .store
        .get_release_execution_instance(instance_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Release execution instance not found"))?;

    validate_release_instance_transition(&previous.status, &to_status)?;

    state
        .store
        .update_release_execution_instance_schedule(
            instance_id,
            to_status.clone(),
            scheduled_at,
            message,
        )
        .await?;

    append_release_history(
        state,
        release_request_id,
        Some(execution_id),
        Some(instance_id),
        ReleaseHistoryScope::Instance,
        action,
        actor,
        Some(previous.status.to_string()),
        Some(to_status.to_string()),
        merge_history_detail(
            serde_json::json!({
                "instance_name": previous.instance_name,
                "target_instance_id": previous.instance_id,
                "batch_index": previous.batch_index,
                "zone": previous.zone,
                "scheduled_at": scheduled_at.map(|value| value.to_rfc3339()),
                "message": message,
            }),
            detail,
        ),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn update_batch_schedule_with_history(
    state: &AppState,
    release_request_id: &str,
    execution_id: &str,
    batch_index: i32,
    to_status: ReleaseInstanceStatus,
    scheduled_at: Option<chrono::DateTime<chrono::Utc>>,
    message: Option<&str>,
    actor: &ReleaseHistoryActor,
    action: &str,
    detail: JsonValue,
) -> anyhow::Result<()> {
    let before = state
        .store
        .list_release_execution_instances(execution_id)
        .await?;
    let affected: Vec<_> = before
        .into_iter()
        .filter(|instance| {
            instance.batch_index == batch_index && !instance_is_terminal(&instance.status)
        })
        .collect();

    for instance in &affected {
        validate_release_instance_transition(&instance.status, &to_status)?;
    }

    state
        .store
        .update_release_execution_batch_schedule(
            execution_id,
            batch_index,
            to_status.clone(),
            scheduled_at,
            message,
        )
        .await?;

    for instance in affected {
        append_release_history(
            state,
            release_request_id,
            Some(execution_id),
            Some(&instance.id),
            ReleaseHistoryScope::Instance,
            action,
            actor,
            Some(instance.status.to_string()),
            Some(to_status.to_string()),
            merge_history_detail(
                serde_json::json!({
                    "instance_name": instance.instance_name,
                    "target_instance_id": instance.instance_id,
                    "batch_index": batch_index,
                    "zone": instance.zone,
                    "scheduled_at": scheduled_at.map(|value| value.to_rfc3339()),
                    "message": message,
                }),
                detail.clone(),
            ),
        )
        .await?;
    }

    Ok(())
}

pub async fn list_release_execution_events(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id, _release_id, execution_id)): Path<(String, String, String, String)>,
) -> ApiResult<Json<Vec<crate::models::ReleaseExecutionEvent>>> {
    require_project_permission(
        &state,
        &org_id,
        &project_id,
        &claims.sub,
        PERM_RELEASE_INSTANCE_VIEW,
    )
    .await?;

    let events = state
        .store
        .list_release_execution_events(&execution_id)
        .await
        .map_err(PlatformError::Internal)?;
    Ok(Json(events))
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
    let latest_execution = state
        .store
        .find_release_execution_by_request_id(&release.id)
        .await
        .map_err(PlatformError::Internal)?;
    let execution_attempts = state
        .store
        .count_release_executions_by_request(&release.id)
        .await
        .map_err(PlatformError::Internal)? as usize;

    if latest_execution.as_ref().is_some_and(execution_is_active) {
        return Err(PlatformError::conflict(
            "Release execution is already in progress",
        ));
    }
    release_request_can_execute(&release.status, execution_attempts)
        .map_err(|err| PlatformError::conflict(err.to_string()))?;

    let env = state
        .store
        .get_environment(&project_id, &release.environment_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Environment not found"))?;
    if env.server_ids.is_empty() {
        return Err(PlatformError::bad_request(
            "Environment has no bound server",
        ));
    }
    let mut bound_servers = Vec::new();
    for server_id in &env.server_ids {
        let server = state
            .store
            .get_server(server_id)
            .await
            .map_err(PlatformError::Internal)?
            .ok_or_else(|| PlatformError::not_found("Bound server not found"))?;
        bound_servers.push(server);
    }
    let strategy = release.request_snapshot.rollout_strategy.clone();
    let total_instances = bound_servers.len();
    let batch_size = compute_batch_size(&strategy, total_instances);
    let total_batches = total_instances.div_ceil(batch_size);
    let executor = state
        .store
        .get_user(&claims.sub)
        .await
        .map_err(PlatformError::Internal)?;
    let actor = user_history_actor(&claims, executor.as_ref());

    let execution_id = Uuid::new_v4().to_string();

    set_release_request_status_with_history(
        &state,
        &release.id,
        release.status.clone(),
        ReleaseRequestStatus::Executing,
        &actor,
        serde_json::json!({
            "reason": "execution_requested",
            "triggered_by": claims.sub,
        }),
    )
    .await
    .map_err(PlatformError::Internal)?;

    state
        .store
        .create_release_execution(
            &execution_id,
            &release.id,
            ReleaseExecutionStatus::RollingOut,
            0,
            total_batches as i32,
            &strategy,
            Some(&claims.sub),
        )
        .await
        .map_err(PlatformError::Internal)?;

    append_release_history(
        &state,
        &release.id,
        Some(&execution_id),
        None,
        ReleaseHistoryScope::Execution,
        "execution_created",
        &actor,
        None,
        Some(ReleaseExecutionStatus::RollingOut.to_string()),
        serde_json::json!({
            "ruleset_name": release.ruleset_name,
            "version": release.version,
            "environment_id": release.environment_id,
            "environment_name": env.name,
            "batch_size": batch_size,
            "total_batches": total_batches,
            "total_instances": total_instances,
            "rollout_strategy": strategy,
        }),
    )
    .await
    .map_err(PlatformError::Internal)?;

    append_release_history(
        &state,
        &release.id,
        Some(&execution_id),
        None,
        ReleaseHistoryScope::Execution,
        "execution_queued",
        &actor,
        None,
        None,
        serde_json::json!({
            "ruleset_name": release.ruleset_name,
            "version": release.version,
            "environment_name": env.name,
            "worker": "ordo-platform-worker",
        }),
    )
    .await
    .map_err(PlatformError::Internal)?;

    let current_version = release
        .version_diff
        .from_version
        .clone()
        .unwrap_or_else(|| "unreleased".to_string());
    let mut instances = Vec::new();
    for (idx, server) in bound_servers.iter().enumerate() {
        let batch_index = (idx / batch_size) as i32 + 1;
        let instance = ReleaseExecutionInstance {
            id: Uuid::new_v4().to_string(),
            release_execution_id: execution_id.clone(),
            instance_id: server.id.clone(),
            instance_name: server.name.clone(),
            zone: server
                .labels
                .get("zone")
                .and_then(|value| value.as_str())
                .map(str::to_string),
            batch_index,
            current_version: current_version.clone(),
            target_version: release.version.clone(),
            status: if batch_index == 1 {
                ReleaseInstanceStatus::Pending
            } else {
                ReleaseInstanceStatus::WaitingBatch
            },
            scheduled_at: None,
            updated_at: Utc::now(),
            message: if batch_index == 1 {
                Some("Queued for immediate rollout".to_string())
            } else {
                Some(format!("Waiting for batch {}", batch_index))
            },
            metric_summary: None,
        };
        state
            .store
            .create_release_execution_instance(&instance)
            .await
            .map_err(PlatformError::Internal)?;
        append_release_history(
            &state,
            &release.id,
            Some(&execution_id),
            Some(&instance.id),
            ReleaseHistoryScope::Instance,
            "instance_initialized",
            &actor,
            None,
            Some(instance.status.to_string()),
            serde_json::json!({
                "instance_name": instance.instance_name,
                "target_instance_id": instance.instance_id,
                "batch_index": batch_index,
                "zone": instance.zone,
                "current_version": instance.current_version,
                "target_version": instance.target_version,
                "message": instance.message,
            }),
        )
        .await
        .map_err(PlatformError::Internal)?;
        instances.push(instance);
    }

    let execution = state
        .store
        .get_release_execution(&execution_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Release execution not found"))?;
    Ok(Json(execution))
}

// ---------------------------------------------------------------------------
// Rolling deployment background task
// ---------------------------------------------------------------------------

struct RollingDeploymentContext {
    state: AppState,
    execution_id: String,
    org_id: String,
    project_id: String,
    release_id: String,
    ruleset_name: String,
    version: String,
    env: crate::models::ProjectEnvironment,
    instances: Vec<ReleaseExecutionInstance>,
    bound_servers: Vec<crate::models::ServerNode>,
    draft: JsonValue,
    strategy: RolloutStrategy,
    deployed_by: String,
    deployer_email: Option<String>,
    deployer_display_name: Option<String>, // None if user record not found
    auto_rollback: bool,
    rollback_version: Option<String>,
    release_note: Option<String>,
}

pub async fn run_release_worker_loop(
    state: AppState,
    poll_interval: Duration,
) -> anyhow::Result<()> {
    loop {
        if let Err(err) = run_release_worker_once(state.clone()).await {
            warn!("release worker poll failed: {err}");
        }
        tokio::time::sleep(poll_interval).await;
    }
}

pub async fn run_release_worker_once(state: AppState) -> anyhow::Result<usize> {
    let executions = state
        .store
        .list_worker_claimable_release_executions(32)
        .await?;
    let mut claimed = 0;

    for execution in executions {
        let Some(lock) = state
            .store
            .try_lock_release_execution(&execution.id)
            .await?
        else {
            continue;
        };

        claimed += 1;
        let worker_state = state.clone();
        let execution_id = execution.id.clone();
        tokio::spawn(async move {
            let _lock = lock;
            if let Err(err) = run_claimed_release_execution(worker_state, &execution_id).await {
                error!(execution_id, "release worker execution failed: {err}");
            }
        });
    }

    Ok(claimed)
}

async fn run_claimed_release_execution(state: AppState, execution_id: &str) -> anyhow::Result<()> {
    let execution = match state.store.get_release_execution(execution_id).await? {
        Some(execution) => execution,
        None => return Ok(()),
    };

    match execution.status {
        ReleaseExecutionStatus::RollbackInProgress => {
            let ctx = build_rollback_context(state, execution).await?;
            run_rollback_deployment(ctx).await;
        }
        ReleaseExecutionStatus::Preparing
        | ReleaseExecutionStatus::WaitingStart
        | ReleaseExecutionStatus::RollingOut => {
            if execution.status == ReleaseExecutionStatus::WaitingStart
                && !wait_until_next_batch_due(&state, execution_id, execution.next_batch_at).await?
            {
                return Ok(());
            }
            let ctx = build_rolling_context(state, execution).await?;
            run_rolling_deployment(ctx).await;
        }
        _ => {}
    }

    Ok(())
}

async fn wait_until_next_batch_due(
    state: &AppState,
    execution_id: &str,
    next_batch_at: Option<chrono::DateTime<Utc>>,
) -> anyhow::Result<bool> {
    let Some(next_batch_at) = next_batch_at else {
        return Ok(true);
    };

    loop {
        match state.store.get_release_execution(execution_id).await? {
            Some(execution)
                if matches!(
                    execution.status,
                    ReleaseExecutionStatus::Failed
                        | ReleaseExecutionStatus::Completed
                        | ReleaseExecutionStatus::RollbackFailed
                ) =>
            {
                return Ok(false);
            }
            Some(execution) if execution.status == ReleaseExecutionStatus::Paused => {
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
            Some(_) => {}
            None => return Ok(false),
        }

        let now = Utc::now();
        if now >= next_batch_at {
            return Ok(true);
        }
        let sleep_for = (next_batch_at - now)
            .to_std()
            .unwrap_or_else(|_| Duration::from_secs(0))
            .min(Duration::from_secs(1));
        tokio::time::sleep(sleep_for).await;
    }
}

async fn build_rolling_context(
    state: AppState,
    mut execution: ReleaseExecution,
) -> anyhow::Result<RollingDeploymentContext> {
    let release = state
        .store
        .get_release_request_by_id(&execution.request_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Release request not found"))?;
    let env = state
        .store
        .get_environment(&release.project_id, &release.environment_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Environment not found"))?;

    execution.instances.sort_by(|left, right| {
        left.batch_index
            .cmp(&right.batch_index)
            .then_with(|| left.instance_name.cmp(&right.instance_name))
    });

    let mut bound_servers = Vec::with_capacity(execution.instances.len());
    for instance in &execution.instances {
        let server = state
            .store
            .get_server(&instance.instance_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Bound server {} not found", instance.instance_id))?;
        bound_servers.push(server);
    }

    let draft = if let Some(snapshot) = release.request_snapshot.target_ruleset_snapshot.clone() {
        snapshot
    } else {
        state
            .store
            .get_draft_ruleset(&release.project_id, &release.ruleset_name)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Draft ruleset not found"))?
            .draft
    };

    let deployer = state.store.get_user(&release.created_by).await?;

    Ok(RollingDeploymentContext {
        state,
        execution_id: execution.id,
        org_id: release.org_id,
        project_id: release.project_id,
        release_id: release.id,
        ruleset_name: release.ruleset_name,
        version: release.version,
        env,
        instances: execution.instances,
        bound_servers,
        draft,
        strategy: execution.strategy,
        deployed_by: release.created_by,
        deployer_email: deployer.as_ref().map(|user| user.email.clone()),
        deployer_display_name: deployer.as_ref().map(|user| user.display_name.clone()),
        auto_rollback: release.request_snapshot.rollback_policy.auto_rollback,
        rollback_version: release
            .version_diff
            .rollback_version
            .clone()
            .or_else(|| release.rollback_version.clone()),
        release_note: release.release_note,
    })
}

async fn build_rollback_context(
    state: AppState,
    mut execution: ReleaseExecution,
) -> anyhow::Result<RollbackDeploymentContext> {
    let release = state
        .store
        .get_release_request_by_id(&execution.request_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Release request not found"))?;
    let env = state
        .store
        .get_environment(&release.project_id, &release.environment_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Environment not found"))?;
    let rollback_version = execution
        .instances
        .first()
        .map(|instance| instance.target_version.clone())
        .or_else(|| release.version_diff.rollback_version.clone())
        .or_else(|| release.rollback_version.clone())
        .ok_or_else(|| anyhow::anyhow!("Release request has no rollback version"))?;
    let rollback_deployment = state
        .store
        .list_deployments(&release.project_id, Some(&release.ruleset_name), 50)
        .await?
        .into_iter()
        .find(|deployment| {
            deployment.environment_id == release.environment_id
                && deployment.version == rollback_version
                && deployment.status == DeploymentStatus::Success
        })
        .ok_or_else(|| anyhow::anyhow!("Rollback deployment snapshot not found"))?;

    execution.instances.sort_by(|left, right| {
        left.batch_index
            .cmp(&right.batch_index)
            .then_with(|| left.instance_name.cmp(&right.instance_name))
    });
    let release_request_id = execution.request_id.clone();

    Ok(RollbackDeploymentContext {
        state,
        execution_id: execution.id,
        org_id: release.org_id,
        project_id: release.project_id,
        release_id: release.id,
        release_status_before: ReleaseRequestStatus::Executing,
        ruleset_name: release.ruleset_name,
        rollback_version,
        env,
        instances: execution.instances,
        snapshot: rollback_deployment.snapshot,
        strategy: execution.strategy,
        actor: system_history_actor("release_worker"),
        release_note: Some(format!("Rollback for release {}", release_request_id)),
    })
}

/// Compute per-batch instance count from strategy.
fn compute_batch_size(strategy: &RolloutStrategy, total: usize) -> usize {
    if total == 0 {
        return 1;
    }
    match strategy.kind {
        Some(RolloutStrategyKind::FixedBatch) => {
            let sz = strategy.batch_size.unwrap_or(1).max(1) as usize;
            sz.min(total)
        }
        Some(RolloutStrategyKind::TimeIntervalBatch) => {
            let sz = strategy.batch_size.unwrap_or(1).max(1) as usize;
            sz.min(total)
        }
        Some(RolloutStrategyKind::PercentageBatch) => {
            let pct = strategy.batch_percentage.unwrap_or(25).clamp(1, 100) as usize;
            let sz = ((total * pct) as f64 / 100.0).ceil() as usize;
            sz.max(1).min(total)
        }
        // AllAtOnce or unset: single batch
        _ => total,
    }
}

/// Interval between batches in seconds (0 = no wait).
fn batch_interval_secs(strategy: &RolloutStrategy) -> u64 {
    match strategy.kind {
        Some(RolloutStrategyKind::TimeIntervalBatch)
        | Some(RolloutStrategyKind::FixedBatch)
        | Some(RolloutStrategyKind::PercentageBatch) => {
            strategy.batch_interval_seconds.unwrap_or(0).max(0) as u64
        }
        _ => 0,
    }
}

async fn run_rolling_deployment(ctx: RollingDeploymentContext) {
    let RollingDeploymentContext {
        state,
        execution_id,
        org_id,
        project_id,
        release_id,
        ruleset_name,
        version,
        env,
        instances,
        bound_servers,
        draft,
        strategy,
        deployed_by,
        deployer_email,
        deployer_display_name,
        auto_rollback,
        rollback_version,
        release_note,
    } = ctx;

    let batch_size = compute_batch_size(&strategy, instances.len());
    let interval_secs = batch_interval_secs(&strategy);
    // Pair each instance with its server
    let pairs: Vec<_> = instances.into_iter().zip(bound_servers).collect();
    let total_batches = pairs.len().div_ceil(batch_size);
    let system_actor = system_history_actor("release_rollout_worker");

    let mut failed = false;
    let mut terminal_batch = 0;

    let _ = append_release_history(
        &state,
        &release_id,
        Some(&execution_id),
        None,
        ReleaseHistoryScope::Execution,
        "execution_started",
        &system_actor,
        None,
        None,
        serde_json::json!({
            "ruleset_name": ruleset_name,
            "version": version,
            "environment_name": env.name,
            "total_batches": total_batches,
            "total_instances": pairs.len(),
        }),
    )
    .await;

    'batches: for (batch_idx, batch) in pairs.chunks(batch_size).enumerate() {
        let batch_num = batch_idx + 1;
        terminal_batch = batch_num as i32;
        let batch_start = std::time::Instant::now();
        let active_batch = batch
            .iter()
            .filter(|(inst, _)| !instance_is_terminal(&inst.status))
            .cloned()
            .collect::<Vec<_>>();

        if active_batch.is_empty() {
            continue;
        }

        if let Err(e) = append_release_history(
            &state,
            &release_id,
            Some(&execution_id),
            None,
            ReleaseHistoryScope::Batch,
            "batch_dispatch_started",
            &system_actor,
            None,
            None,
            serde_json::json!({
                "batch_index": batch_num,
                "total_batches": total_batches,
                "target_server_ids": batch
                    .iter()
                    .filter(|(inst, _)| !instance_is_terminal(&inst.status))
                    .map(|(_, server)| server.id.clone())
                    .collect::<Vec<_>>(),
            }),
        )
        .await
        {
            error!(
                execution_id,
                batch = batch_num,
                "Failed to append batch history: {e}"
            );
        }

        // Honour pause: spin-wait until resumed or terminal
        #[allow(clippy::while_let_loop)]
        loop {
            match state.store.get_release_execution(&execution_id).await {
                Ok(Some(exec)) => match exec.status {
                    ReleaseExecutionStatus::Paused => {
                        let _ = state
                            .store
                            .set_release_execution_next_batch_at(&execution_id, None)
                            .await;
                        tokio::time::sleep(Duration::from_secs(3)).await;
                        continue;
                    }
                    ReleaseExecutionStatus::Failed => return,
                    _ => break,
                },
                _ => break,
            }
        }

        // Mark batch instances as being dispatched now.
        for (inst, _) in &active_batch {
            if let Err(e) = update_instance_schedule_with_history(
                &state,
                &release_id,
                &execution_id,
                &inst.id,
                ReleaseInstanceStatus::Dispatching,
                None,
                Some("Dispatching ruleset to server"),
                &system_actor,
                "instance_status_changed",
                serde_json::json!({
                    "reason": "batch_dispatch_started",
                    "batch_index": batch_num,
                }),
            )
            .await
            {
                error!(execution_id, instance_id = %inst.id, "Failed to update instance to Dispatching: {e}");
            }
            if let Err(e) = update_instance_status_with_history(
                &state,
                &release_id,
                &execution_id,
                &inst.id,
                ReleaseInstanceStatus::Updating,
                Some("Dispatching ruleset to server"),
                None,
                &system_actor,
                "instance_status_changed",
                serde_json::json!({
                    "reason": "batch_dispatch_started",
                    "batch_index": batch_num,
                }),
            )
            .await
            {
                error!(execution_id, instance_id = %inst.id, "Failed to update instance to Updating: {e}");
            }
        }

        if let Err(e) = update_release_execution_status_with_history(
            &state,
            &release_id,
            &execution_id,
            None,
            ReleaseExecutionStatus::RollingOut,
            Some(batch_num as i32),
            &system_actor,
            serde_json::json!({
                "batch_index": batch_num,
                "total_batches": total_batches,
            }),
        )
        .await
        {
            error!(
                execution_id,
                "Failed to update execution status to RollingOut: {e}"
            );
            failed = true;
            break 'batches;
        }
        let _ = state
            .store
            .set_release_execution_next_batch_at(&execution_id, None)
            .await;

        let target_server_ids = active_batch
            .iter()
            .map(|(_, server)| server.id.clone())
            .collect::<Vec<_>>();
        let push_result = publish_release_via_nats(
            &state,
            &env,
            &project_id,
            &ruleset_name,
            &draft,
            &version,
            Some(&execution_id),
            Some(target_server_ids.as_slice()),
        )
        .await;

        match push_result {
            Ok(()) => {
                for (inst, server) in &active_batch {
                    let _ = update_instance_status_with_history(
                        &state,
                        &release_id,
                        &execution_id,
                        &inst.id,
                        ReleaseInstanceStatus::Updating,
                        Some("Ruleset published to NATS; waiting for server ack"),
                        Some("publish_sent"),
                        &system_actor,
                        "instance_status_changed",
                        serde_json::json!({
                            "reason": "publish_sent",
                            "server_name": server.name,
                            "batch_index": batch_num,
                        }),
                    )
                    .await;
                    info!(
                        execution_id,
                        server = %server.name,
                        batch = batch_num,
                        "Ruleset published to NATS batch"
                    );
                }

                match wait_for_batch_feedback(
                    &state,
                    &execution_id,
                    &target_server_ids,
                    Duration::from_secs(RELEASE_ACK_TIMEOUT_SECS),
                )
                .await
                {
                    Ok(()) => {
                        let duration_ms = batch_start.elapsed().as_millis() as u64;
                        let _ = append_release_history(
                            &state,
                            &release_id,
                            Some(&execution_id),
                            None,
                            ReleaseHistoryScope::Batch,
                            "batch_feedback_succeeded",
                            &system_actor,
                            None,
                            None,
                            serde_json::json!({
                                "batch_index": batch_num,
                                "total_batches": total_batches,
                                "duration_ms": duration_ms,
                                "target_server_ids": target_server_ids,
                            }),
                        )
                        .await;
                        for (inst, _) in &active_batch {
                            let summary = serde_json::json!({
                                "batch_index": batch_num,
                                "total_batches": total_batches,
                                "duration_ms": duration_ms,
                                "applied_at": chrono::Utc::now().to_rfc3339(),
                            });
                            let _ = state
                                .store
                                .update_execution_instance_metric_summary(&inst.id, summary)
                                .await;
                        }
                    }
                    Err(err) => {
                        let msg = err.to_string();
                        error!(execution_id, "Batch feedback failed: {msg}");
                        let _ = append_release_history(
                            &state,
                            &release_id,
                            Some(&execution_id),
                            None,
                            ReleaseHistoryScope::Batch,
                            "batch_feedback_failed",
                            &system_actor,
                            None,
                            None,
                            serde_json::json!({
                                "batch_index": batch_num,
                                "error": msg,
                                "target_server_ids": target_server_ids,
                            }),
                        )
                        .await;
                        for server_id in &target_server_ids {
                            if let Ok(Some(instance)) = state
                                .store
                                .find_release_execution_instance_by_target(&execution_id, server_id)
                                .await
                            {
                                if instance.status == ReleaseInstanceStatus::Dispatching
                                    || instance.status == ReleaseInstanceStatus::Updating
                                    || instance.status == ReleaseInstanceStatus::Pending
                                {
                                    let _ = update_instance_status_with_history(
                                        &state,
                                        &release_id,
                                        &execution_id,
                                        &instance.id,
                                        ReleaseInstanceStatus::Failed,
                                        Some(&msg),
                                        Some("release_ack_timeout"),
                                        &system_actor,
                                        "instance_status_changed",
                                        serde_json::json!({
                                            "reason": "release_ack_timeout",
                                            "batch_index": batch_num,
                                        }),
                                    )
                                    .await;
                                }
                            }
                        }
                        failed = true;
                        break 'batches;
                    }
                }
            }
            Err(err) => {
                let msg = err.to_string();
                error!(execution_id, "NATS publish failed: {msg}");
                let _ = append_release_history(
                    &state,
                    &release_id,
                    Some(&execution_id),
                    None,
                    ReleaseHistoryScope::Batch,
                    "batch_publish_failed",
                    &system_actor,
                    None,
                    None,
                    serde_json::json!({
                        "batch_index": batch_num,
                        "error": msg,
                    }),
                )
                .await;
                for (inst, _) in &active_batch {
                    let _ = update_instance_status_with_history(
                        &state,
                        &release_id,
                        &execution_id,
                        &inst.id,
                        ReleaseInstanceStatus::Failed,
                        Some(&msg),
                        Some("publish_failed"),
                        &system_actor,
                        "instance_status_changed",
                        serde_json::json!({
                            "reason": "publish_failed",
                            "batch_index": batch_num,
                        }),
                    )
                    .await;
                }
                failed = true;
                break 'batches;
            }
        }

        // Wait between batches (skip after last batch)
        if batch_num < total_batches && interval_secs > 0 {
            let next_start = (batch_idx + 1) * batch_size;
            let next_end = ((batch_idx + 2) * batch_size).min(pairs.len());
            let next_batch = &pairs[next_start..next_end];
            info!(
                execution_id,
                "Batch {}/{} complete — waiting {}s before next batch",
                batch_num,
                total_batches,
                interval_secs
            );
            if !wait_for_next_batch_window(
                &state,
                &release_id,
                &execution_id,
                batch_num as i32,
                next_batch,
                interval_secs,
            )
            .await
            {
                failed = true;
                break 'batches;
            }
        }
    }

    if failed {
        let _ = state
            .store
            .set_release_execution_next_batch_at(&execution_id, None)
            .await;
        if let Err(e) = update_release_execution_status_with_history(
            &state,
            &release_id,
            &execution_id,
            None,
            ReleaseExecutionStatus::Failed,
            Some(terminal_batch),
            &system_actor,
            serde_json::json!({
                "terminal_batch": terminal_batch,
            }),
        )
        .await
        {
            error!(execution_id, "Failed to mark execution Failed: {e}");
        }
        if let Err(e) = set_release_request_status_with_history(
            &state,
            &release_id,
            ReleaseRequestStatus::Executing,
            ReleaseRequestStatus::Failed,
            &system_actor,
            serde_json::json!({
                "reason": "execution_failed",
                "terminal_batch": terminal_batch,
            }),
        )
        .await
        {
            error!(execution_id, "Failed to mark release request Failed: {e}");
        }

        // Auto-rollback to previous version if configured
        if auto_rollback {
            if let Some(rb_version) = &rollback_version {
                info!(execution_id, rollback_version = %rb_version, "Starting auto-rollback");
                let _ = trigger_auto_rollback(
                    &state,
                    &execution_id,
                    &env,
                    &project_id,
                    &release_id,
                    &ruleset_name,
                    rb_version,
                    &deployed_by,
                )
                .await;
            }
        }
        return;
    }

    // All batches succeeded
    if let Err(e) = update_release_execution_status_with_history(
        &state,
        &release_id,
        &execution_id,
        None,
        ReleaseExecutionStatus::Completed,
        Some(total_batches as i32),
        &system_actor,
        serde_json::json!({
            "total_batches": total_batches,
        }),
    )
    .await
    {
        error!(execution_id, "Failed to mark execution Completed: {e}");
    }
    if let Err(e) = set_release_request_status_with_history(
        &state,
        &release_id,
        ReleaseRequestStatus::Executing,
        ReleaseRequestStatus::Completed,
        &system_actor,
        serde_json::json!({
            "reason": "execution_completed",
            "total_batches": total_batches,
        }),
    )
    .await
    {
        error!(
            execution_id,
            "Failed to mark release request Completed: {e}"
        );
    }
    if let Err(e) = state
        .store
        .mark_ruleset_published(&project_id, &ruleset_name, &version)
        .await
    {
        error!(execution_id, "Failed to mark ruleset published: {e}");
    }

    let deployment = RulesetDeployment {
        id: Uuid::new_v4().to_string(),
        project_id: project_id.clone(),
        environment_id: env.id.clone(),
        environment_name: Some(env.name.clone()),
        ruleset_name: ruleset_name.clone(),
        version: version.clone(),
        release_note: release_note.clone(),
        snapshot: draft.clone(),
        deployed_at: Utc::now(),
        deployed_by: Some(deployed_by.clone()),
        status: DeploymentStatus::Success,
    };
    let _ = state.store.create_deployment(&deployment).await;

    let entry = RulesetHistoryEntry {
        id: Uuid::new_v4().to_string(),
        ruleset_name: ruleset_name.clone(),
        action: format!("released to {}", env.name),
        source: RulesetHistorySource::Publish,
        created_at: Utc::now(),
        author_id: deployed_by.clone(),
        author_email: deployer_email.unwrap_or_default(),
        author_display_name: deployer_display_name.unwrap_or_default(),
        snapshot: draft,
    };
    let _ = state
        .store
        .append_ruleset_history(&org_id, &project_id, &ruleset_name, &[entry])
        .await;

    info!(
        execution_id,
        "Rolling deployment completed ({} batch(es))", total_batches
    );
}

async fn wait_for_next_batch_window(
    state: &AppState,
    release_id: &str,
    execution_id: &str,
    current_batch: i32,
    next_batch: &[(ReleaseExecutionInstance, crate::models::ServerNode)],
    interval_secs: u64,
) -> bool {
    if next_batch.is_empty() || interval_secs == 0 {
        return true;
    }

    let batch_index = next_batch[0].0.batch_index;
    let mut remaining = Duration::from_secs(interval_secs);
    let mut next_batch_at = Utc::now() + chrono::Duration::seconds(interval_secs as i64);
    let system_actor = system_history_actor("release_rollout_worker");

    if update_release_execution_status_with_history(
        state,
        release_id,
        execution_id,
        None,
        ReleaseExecutionStatus::WaitingStart,
        Some(current_batch),
        &system_actor,
        serde_json::json!({
            "current_batch": current_batch,
            "next_batch_index": batch_index,
            "wait_seconds": interval_secs,
        }),
    )
    .await
    .is_err()
    {
        return false;
    }
    let _ = state
        .store
        .set_release_execution_next_batch_at(execution_id, Some(next_batch_at))
        .await;
    let _ = append_release_history(
        state,
        release_id,
        Some(execution_id),
        None,
        ReleaseHistoryScope::Batch,
        "batch_wait_started",
        &system_actor,
        None,
        None,
        serde_json::json!({
            "current_batch": current_batch,
            "next_batch_index": batch_index,
            "wait_seconds": interval_secs,
            "next_batch_at": next_batch_at.to_rfc3339(),
        }),
    )
    .await;
    let _ = update_batch_schedule_with_history(
        state,
        release_id,
        execution_id,
        batch_index,
        ReleaseInstanceStatus::Scheduled,
        Some(next_batch_at),
        Some("Scheduled for next rollout window"),
        &system_actor,
        "instance_status_changed",
        serde_json::json!({
            "reason": "batch_wait_started",
        }),
    )
    .await;

    loop {
        match state.store.get_release_execution(execution_id).await {
            Ok(Some(exec)) => match exec.status {
                ReleaseExecutionStatus::Paused => {
                    let _ = state
                        .store
                        .set_release_execution_next_batch_at(execution_id, None)
                        .await;
                    let _ = update_batch_schedule_with_history(
                        state,
                        release_id,
                        execution_id,
                        batch_index,
                        ReleaseInstanceStatus::WaitingBatch,
                        None,
                        Some(&format!("Paused before batch {}", batch_index)),
                        &system_actor,
                        "instance_status_changed",
                        serde_json::json!({
                            "reason": "execution_paused",
                        }),
                    )
                    .await;
                    loop {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        match state.store.get_release_execution(execution_id).await {
                            Ok(Some(paused_exec))
                                if paused_exec.status == ReleaseExecutionStatus::Paused =>
                            {
                                continue;
                            }
                            Ok(Some(paused_exec))
                                if paused_exec.status == ReleaseExecutionStatus::Failed =>
                            {
                                return false;
                            }
                            Ok(Some(_)) => break,
                            _ => return false,
                        }
                    }

                    next_batch_at = Utc::now()
                        + chrono::Duration::from_std(remaining)
                            .unwrap_or_else(|_| chrono::Duration::seconds(0));
                    let _ = update_release_execution_status_with_history(
                        state,
                        release_id,
                        execution_id,
                        None,
                        ReleaseExecutionStatus::WaitingStart,
                        Some(current_batch),
                        &system_actor,
                        serde_json::json!({
                            "current_batch": current_batch,
                            "next_batch_index": batch_index,
                            "remaining_wait_seconds": remaining.as_secs(),
                        }),
                    )
                    .await;
                    let _ = state
                        .store
                        .set_release_execution_next_batch_at(execution_id, Some(next_batch_at))
                        .await;
                    let _ = append_release_history(
                        state,
                        release_id,
                        Some(execution_id),
                        None,
                        ReleaseHistoryScope::Batch,
                        "batch_wait_resumed",
                        &system_actor,
                        None,
                        None,
                        serde_json::json!({
                            "current_batch": current_batch,
                            "next_batch_index": batch_index,
                            "next_batch_at": next_batch_at.to_rfc3339(),
                            "remaining_wait_seconds": remaining.as_secs(),
                        }),
                    )
                    .await;
                    let _ = update_batch_schedule_with_history(
                        state,
                        release_id,
                        execution_id,
                        batch_index,
                        ReleaseInstanceStatus::Scheduled,
                        Some(next_batch_at),
                        Some("Scheduled for next rollout window"),
                        &system_actor,
                        "instance_status_changed",
                        serde_json::json!({
                            "reason": "batch_wait_resumed",
                        }),
                    )
                    .await;
                }
                ReleaseExecutionStatus::Failed => return false,
                _ => {}
            },
            _ => return false,
        }

        if remaining.is_zero() {
            break;
        }

        let tick = remaining.min(Duration::from_secs(1));
        tokio::time::sleep(tick).await;
        remaining = remaining.saturating_sub(tick);
    }

    let _ = state
        .store
        .set_release_execution_next_batch_at(execution_id, None)
        .await;
    true
}

struct RollbackDeploymentContext {
    state: AppState,
    execution_id: String,
    org_id: String,
    project_id: String,
    release_id: String,
    release_status_before: ReleaseRequestStatus,
    ruleset_name: String,
    rollback_version: String,
    env: crate::models::ProjectEnvironment,
    instances: Vec<ReleaseExecutionInstance>,
    snapshot: JsonValue,
    strategy: RolloutStrategy,
    actor: ReleaseHistoryActor,
    release_note: Option<String>,
}

async fn wait_for_rollback_batch_window(
    state: &AppState,
    release_id: &str,
    execution_id: &str,
    next_batch: &[ReleaseExecutionInstance],
    interval_secs: u64,
) -> bool {
    if next_batch.is_empty() || interval_secs == 0 {
        return true;
    }

    let batch_index = next_batch[0].batch_index;
    let next_batch_at = Utc::now() + chrono::Duration::seconds(interval_secs as i64);
    let system_actor = system_history_actor("release_rollback_worker");

    let _ = state
        .store
        .set_release_execution_next_batch_at(execution_id, Some(next_batch_at))
        .await;
    let _ = append_release_history(
        state,
        release_id,
        Some(execution_id),
        None,
        ReleaseHistoryScope::Batch,
        "rollback_batch_wait_started",
        &system_actor,
        None,
        None,
        serde_json::json!({
            "next_batch_index": batch_index,
            "wait_seconds": interval_secs,
            "next_batch_at": next_batch_at.to_rfc3339(),
        }),
    )
    .await;
    let _ = update_batch_schedule_with_history(
        state,
        release_id,
        execution_id,
        batch_index,
        ReleaseInstanceStatus::Scheduled,
        Some(next_batch_at),
        Some("Scheduled for next rollback window"),
        &system_actor,
        "instance_status_changed",
        serde_json::json!({
            "reason": "rollback_batch_wait_started",
        }),
    )
    .await;

    tokio::time::sleep(Duration::from_secs(interval_secs)).await;

    let _ = state
        .store
        .set_release_execution_next_batch_at(execution_id, None)
        .await;
    let _ = update_batch_schedule_with_history(
        state,
        release_id,
        execution_id,
        batch_index,
        ReleaseInstanceStatus::Pending,
        None,
        Some("Ready for rollback dispatch"),
        &system_actor,
        "instance_status_changed",
        serde_json::json!({
            "reason": "rollback_batch_wait_finished",
        }),
    )
    .await;
    let _ = append_release_history(
        state,
        release_id,
        Some(execution_id),
        None,
        ReleaseHistoryScope::Batch,
        "rollback_batch_wait_finished",
        &system_actor,
        None,
        None,
        serde_json::json!({
            "next_batch_index": batch_index,
        }),
    )
    .await;

    true
}

async fn run_rollback_deployment(ctx: RollbackDeploymentContext) {
    let RollbackDeploymentContext {
        state,
        execution_id,
        org_id,
        project_id,
        release_id,
        release_status_before,
        ruleset_name,
        rollback_version,
        env,
        mut instances,
        snapshot,
        strategy: _strategy,
        actor,
        release_note,
    } = ctx;

    instances.sort_by(|left, right| {
        left.batch_index
            .cmp(&right.batch_index)
            .then_with(|| left.instance_name.cmp(&right.instance_name))
    });
    let interval_secs = ROLLBACK_BATCH_INTERVAL_SECS;
    let total_batches = instances
        .iter()
        .map(|item| item.batch_index)
        .max()
        .unwrap_or(1);
    let system_actor = system_history_actor("release_rollback_worker");
    let mut failed = false;
    let mut terminal_batch = 0;

    for batch_index in 1..=total_batches {
        let batch_instances = instances
            .iter()
            .filter(|instance| {
                instance.batch_index == batch_index && !instance_is_terminal(&instance.status)
            })
            .cloned()
            .collect::<Vec<_>>();

        if batch_instances.is_empty() {
            continue;
        }

        terminal_batch = batch_index;
        let batch_start = std::time::Instant::now();
        let target_server_ids = batch_instances
            .iter()
            .map(|instance| instance.instance_id.clone())
            .collect::<Vec<_>>();

        let _ = append_release_history(
            &state,
            &release_id,
            Some(&execution_id),
            None,
            ReleaseHistoryScope::Batch,
            "rollback_batch_dispatch_started",
            &system_actor,
            None,
            None,
            serde_json::json!({
                "batch_index": batch_index,
                "total_batches": total_batches,
                "target_server_ids": target_server_ids,
            }),
        )
        .await;

        for instance in &batch_instances {
            let _ = update_instance_schedule_with_history(
                &state,
                &release_id,
                &execution_id,
                &instance.id,
                ReleaseInstanceStatus::Dispatching,
                None,
                Some("Dispatching rollback snapshot to server"),
                &system_actor,
                "instance_status_changed",
                serde_json::json!({
                    "reason": "rollback_batch_dispatch_started",
                    "batch_index": batch_index,
                }),
            )
            .await;
            let _ = update_instance_status_with_history(
                &state,
                &release_id,
                &execution_id,
                &instance.id,
                ReleaseInstanceStatus::Updating,
                Some("Dispatching rollback snapshot to server"),
                None,
                &system_actor,
                "instance_status_changed",
                serde_json::json!({
                    "reason": "rollback_batch_dispatch_started",
                    "batch_index": batch_index,
                }),
            )
            .await;
        }

        let _ = state
            .store
            .set_release_execution_next_batch_at(&execution_id, None)
            .await;
        if let Err(err) = update_release_execution_status_with_history(
            &state,
            &release_id,
            &execution_id,
            None,
            ReleaseExecutionStatus::RollbackInProgress,
            Some(batch_index),
            &system_actor,
            serde_json::json!({
                "reason": "rollback_batch_dispatch_started",
                "batch_index": batch_index,
                "total_batches": total_batches,
                "rollback_version": rollback_version,
            }),
        )
        .await
        {
            error!(
                execution_id,
                batch = batch_index,
                "Failed to update rollback status: {err}"
            );
            failed = true;
            break;
        }

        match publish_release_via_nats(
            &state,
            &env,
            &project_id,
            &ruleset_name,
            &snapshot,
            &rollback_version,
            Some(&execution_id),
            Some(target_server_ids.as_slice()),
        )
        .await
        {
            Ok(()) => {
                for instance in &batch_instances {
                    let _ = update_instance_status_with_history(
                        &state,
                        &release_id,
                        &execution_id,
                        &instance.id,
                        ReleaseInstanceStatus::Updating,
                        Some("Rollback snapshot published to NATS; waiting for server ack"),
                        Some("rollback_publish_sent"),
                        &system_actor,
                        "instance_status_changed",
                        serde_json::json!({
                            "reason": "rollback_publish_sent",
                            "batch_index": batch_index,
                        }),
                    )
                    .await;
                }

                match wait_for_batch_feedback(
                    &state,
                    &execution_id,
                    &target_server_ids,
                    Duration::from_secs(RELEASE_ACK_TIMEOUT_SECS),
                )
                .await
                {
                    Ok(()) => {
                        let duration_ms = batch_start.elapsed().as_millis() as u64;
                        let _ = append_release_history(
                            &state,
                            &release_id,
                            Some(&execution_id),
                            None,
                            ReleaseHistoryScope::Batch,
                            "rollback_batch_succeeded",
                            &system_actor,
                            None,
                            None,
                            serde_json::json!({
                                "batch_index": batch_index,
                                "total_batches": total_batches,
                                "duration_ms": duration_ms,
                                "target_server_ids": target_server_ids,
                            }),
                        )
                        .await;
                    }
                    Err(err) => {
                        let msg = err.to_string();
                        let _ = append_release_history(
                            &state,
                            &release_id,
                            Some(&execution_id),
                            None,
                            ReleaseHistoryScope::Batch,
                            "rollback_batch_failed",
                            &system_actor,
                            None,
                            None,
                            serde_json::json!({
                                "batch_index": batch_index,
                                "error": msg,
                                "target_server_ids": target_server_ids,
                            }),
                        )
                        .await;
                        for server_id in &target_server_ids {
                            if let Ok(Some(instance)) = state
                                .store
                                .find_release_execution_instance_by_target(&execution_id, server_id)
                                .await
                            {
                                if matches!(
                                    instance.status,
                                    ReleaseInstanceStatus::Dispatching
                                        | ReleaseInstanceStatus::Updating
                                        | ReleaseInstanceStatus::Pending
                                ) {
                                    let _ = update_instance_status_with_history(
                                        &state,
                                        &release_id,
                                        &execution_id,
                                        &instance.id,
                                        ReleaseInstanceStatus::Failed,
                                        Some(&msg),
                                        Some("rollback_ack_timeout"),
                                        &system_actor,
                                        "instance_status_changed",
                                        serde_json::json!({
                                            "reason": "rollback_ack_timeout",
                                            "batch_index": batch_index,
                                        }),
                                    )
                                    .await;
                                }
                            }
                        }
                        failed = true;
                        break;
                    }
                }
            }
            Err(err) => {
                let msg = err.to_string();
                let _ = append_release_history(
                    &state,
                    &release_id,
                    Some(&execution_id),
                    None,
                    ReleaseHistoryScope::Batch,
                    "rollback_batch_failed",
                    &system_actor,
                    None,
                    None,
                    serde_json::json!({
                        "batch_index": batch_index,
                        "error": msg,
                        "target_server_ids": target_server_ids,
                    }),
                )
                .await;
                for instance in &batch_instances {
                    let _ = update_instance_status_with_history(
                        &state,
                        &release_id,
                        &execution_id,
                        &instance.id,
                        ReleaseInstanceStatus::Failed,
                        Some(&msg),
                        Some("rollback_publish_failed"),
                        &system_actor,
                        "instance_status_changed",
                        serde_json::json!({
                            "reason": "rollback_publish_failed",
                            "batch_index": batch_index,
                        }),
                    )
                    .await;
                }
                failed = true;
                break;
            }
        }

        if batch_index < total_batches
            && interval_secs > 0
            && !wait_for_rollback_batch_window(
                &state,
                &release_id,
                &execution_id,
                &instances
                    .iter()
                    .filter(|instance| instance.batch_index == batch_index + 1)
                    .cloned()
                    .collect::<Vec<_>>(),
                interval_secs,
            )
            .await
        {
            failed = true;
            break;
        }
    }

    let _ = state
        .store
        .set_release_execution_next_batch_at(&execution_id, None)
        .await;

    if failed {
        if let Ok(remaining_instances) = state
            .store
            .list_release_execution_instances(&execution_id)
            .await
        {
            for instance in remaining_instances
                .into_iter()
                .filter(|instance| !instance_is_terminal(&instance.status))
            {
                let _ = update_instance_status_with_history(
                    &state,
                    &release_id,
                    &execution_id,
                    &instance.id,
                    ReleaseInstanceStatus::Skipped,
                    Some("Rollback aborted before this instance could be dispatched"),
                    Some("rollback_aborted"),
                    &system_actor,
                    "instance_status_changed",
                    serde_json::json!({
                        "reason": "rollback_aborted",
                        "terminal_batch": terminal_batch,
                        "rollback_version": rollback_version,
                    }),
                )
                .await;
            }
        }
        let _ = update_release_execution_status_with_history(
            &state,
            &release_id,
            &execution_id,
            None,
            ReleaseExecutionStatus::RollbackFailed,
            Some(terminal_batch),
            &system_actor,
            serde_json::json!({
                "reason": "rollback_failed",
                "terminal_batch": terminal_batch,
                "rollback_version": rollback_version,
            }),
        )
        .await;
        let _ = set_release_request_status_with_history(
            &state,
            &release_id,
            ReleaseRequestStatus::Executing,
            ReleaseRequestStatus::RollbackFailed,
            &actor,
            serde_json::json!({
                "reason": "rollback_failed",
                "terminal_batch": terminal_batch,
                "rollback_version": rollback_version,
            }),
        )
        .await;
        let _ = append_release_history(
            &state,
            &release_id,
            Some(&execution_id),
            None,
            ReleaseHistoryScope::Rollback,
            "rollback_failed",
            &actor,
            None,
            None,
            serde_json::json!({
                "rollback_version": rollback_version,
                "terminal_batch": terminal_batch,
            }),
        )
        .await;
        return;
    }

    let _ = update_release_execution_status_with_history(
        &state,
        &release_id,
        &execution_id,
        None,
        ReleaseExecutionStatus::Completed,
        Some(total_batches),
        &actor,
        serde_json::json!({
            "reason": "rollback_completed",
            "rollback_version": rollback_version,
            "total_batches": total_batches,
        }),
    )
    .await;
    let _ = set_release_request_status_with_history(
        &state,
        &release_id,
        ReleaseRequestStatus::Executing,
        ReleaseRequestStatus::RolledBack,
        &actor,
        serde_json::json!({
            "reason": "rollback_completed",
            "rollback_version": rollback_version,
            "previous_request_status": release_status_before.to_string(),
        }),
    )
    .await;
    let _ = state
        .store
        .mark_ruleset_published(&project_id, &ruleset_name, &rollback_version)
        .await;

    let deployment = RulesetDeployment {
        id: Uuid::new_v4().to_string(),
        project_id: project_id.clone(),
        environment_id: env.id.clone(),
        environment_name: Some(env.name.clone()),
        ruleset_name: ruleset_name.clone(),
        version: rollback_version.clone(),
        release_note,
        snapshot: snapshot.clone(),
        deployed_at: Utc::now(),
        deployed_by: actor.actor_id.clone(),
        status: DeploymentStatus::Success,
    };
    let _ = state.store.create_deployment(&deployment).await;

    let entry = RulesetHistoryEntry {
        id: Uuid::new_v4().to_string(),
        ruleset_name: ruleset_name.clone(),
        action: format!("rolled back in {}", env.name),
        source: RulesetHistorySource::Publish,
        created_at: Utc::now(),
        author_id: actor.actor_id.clone().unwrap_or_default(),
        author_email: actor.actor_email.clone().unwrap_or_default(),
        author_display_name: actor.actor_name.clone().unwrap_or_default(),
        snapshot,
    };
    let _ = state
        .store
        .append_ruleset_history(&org_id, &project_id, &ruleset_name, &[entry])
        .await;

    let _ = append_release_history(
        &state,
        &release_id,
        Some(&execution_id),
        None,
        ReleaseHistoryScope::Rollback,
        "rollback_completed",
        &actor,
        None,
        None,
        serde_json::json!({
            "rollback_version": rollback_version,
            "total_batches": total_batches,
        }),
    )
    .await;
}

#[allow(clippy::too_many_arguments)]
async fn trigger_auto_rollback(
    state: &AppState,
    execution_id: &str,
    env: &crate::models::ProjectEnvironment,
    project_id: &str,
    release_id: &str,
    ruleset_name: &str,
    rollback_version: &str,
    _deployed_by: &str,
) -> anyhow::Result<()> {
    let actor = system_history_actor("release_auto_rollback");
    let execution = state
        .store
        .get_release_execution(execution_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Release execution not found"))?;
    let release = state
        .store
        .get_release_request_by_id(release_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Release request not found"))?;
    let rollback_deployment = state
        .store
        .list_deployments(project_id, Some(ruleset_name), 50)
        .await?
        .into_iter()
        .find(|d| {
            d.environment_id == env.id
                && d.version == rollback_version
                && d.status == DeploymentStatus::Success
        });

    let Some(_rollback_deployment) = rollback_deployment else {
        warn!(
            execution_id,
            "Auto-rollback: snapshot not found for version {}", rollback_version
        );
        return Err(anyhow::anyhow!("Rollback snapshot not found"));
    };

    let _ = append_release_history(
        state,
        release_id,
        Some(execution_id),
        None,
        ReleaseHistoryScope::Rollback,
        "auto_rollback_started",
        &actor,
        None,
        None,
        serde_json::json!({
            "rollback_version": rollback_version,
        }),
    )
    .await;
    let _ = set_release_request_status_with_history(
        state,
        release_id,
        ReleaseRequestStatus::Failed,
        ReleaseRequestStatus::Executing,
        &actor,
        serde_json::json!({
            "reason": "auto_rollback_started",
            "rollback_version": rollback_version,
        }),
    )
    .await;
    let _ = update_release_execution_status_with_history(
        state,
        release_id,
        execution_id,
        None,
        ReleaseExecutionStatus::RollbackInProgress,
        Some(0),
        &actor,
        serde_json::json!({
            "reason": "auto_rollback_started",
            "rollback_version": rollback_version,
        }),
    )
    .await;

    let mut instances = execution.instances.clone();
    instances.sort_by(|left, right| {
        left.batch_index
            .cmp(&right.batch_index)
            .then_with(|| left.instance_name.cmp(&right.instance_name))
    });
    for instance in &instances {
        let planned_status = if instance.batch_index == 1 {
            ReleaseInstanceStatus::Pending
        } else {
            ReleaseInstanceStatus::WaitingBatch
        };
        validate_release_instance_transition(&instance.status, &planned_status)?;
        state
            .store
            .update_release_execution_instance_plan(
                &instance.id,
                rollback_version,
                planned_status,
                None,
                Some(if instance.batch_index == 1 {
                    "Queued for immediate rollback"
                } else {
                    "Waiting for rollback batch"
                }),
            )
            .await?;
    }

    let _ = state
        .store
        .create_release_execution_event(
            &Uuid::new_v4().to_string(),
            execution_id,
            None,
            "rollback_started",
            serde_json::json!({
                "release_id": release.id,
                "rollback_version": rollback_version,
                "requested_by": actor.actor_id.clone(),
                "mode": "auto",
            }),
        )
        .await;

    let _ = append_release_history(
        state,
        release_id,
        Some(execution_id),
        None,
        ReleaseHistoryScope::Rollback,
        "auto_rollback_scheduled",
        &actor,
        None,
        None,
        serde_json::json!({
            "rollback_version": rollback_version,
            "total_batches": execution.total_batches,
        }),
    )
    .await;

    info!(
        execution_id,
        "Auto-rollback to {} scheduled", rollback_version
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Pause / Resume / Rollback
// ---------------------------------------------------------------------------

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

    if release.status == ReleaseRequestStatus::RolledBack {
        return Err(PlatformError::conflict(
            "Rolled back release requests are already closed",
        ));
    }
    if execution.status == ReleaseExecutionStatus::RollbackInProgress {
        return Err(PlatformError::conflict(
            "Release execution is already rolling back",
        ));
    }
    if execution.status != ReleaseExecutionStatus::Completed
        && execution.status != ReleaseExecutionStatus::Failed
        && execution.status != ReleaseExecutionStatus::RollbackFailed
        && execution.status != ReleaseExecutionStatus::Paused
    {
        return Err(PlatformError::conflict(
            "Release execution cannot be rolled back from its current status",
        ));
    }
    validate_release_execution_transition(
        &execution.status,
        &ReleaseExecutionStatus::RollbackInProgress,
    )
    .map_err(|err| PlatformError::conflict(err.to_string()))?;

    let rollback_version = release
        .version_diff
        .rollback_version
        .clone()
        .or_else(|| release.rollback_version.clone())
        .ok_or_else(|| PlatformError::bad_request("Release request has no rollback version"))?;
    if release.version_diff.from_version.as_deref() != Some(rollback_version.as_str()) {
        return Err(PlatformError::conflict(
            "Cross-version rollback requires an approved release request",
        ));
    }

    let _env = state
        .store
        .get_environment(&project_id, &release.environment_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Environment not found"))?;

    let _rollback_deployment = state
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
    let executor = state
        .store
        .get_user(&claims.sub)
        .await
        .map_err(PlatformError::Internal)?;
    let actor = user_history_actor(&claims, executor.as_ref());

    append_release_history(
        &state,
        &release.id,
        Some(&execution.id),
        None,
        ReleaseHistoryScope::Rollback,
        "rollback_requested",
        &actor,
        None,
        None,
        serde_json::json!({
            "rollback_version": rollback_version,
            "requested_by": claims.sub,
        }),
    )
    .await
    .map_err(PlatformError::Internal)?;
    set_release_request_status_with_history(
        &state,
        &release.id,
        release.status.clone(),
        ReleaseRequestStatus::Executing,
        &actor,
        serde_json::json!({
            "reason": "rollback_requested",
            "rollback_version": rollback_version,
            "requested_by": claims.sub,
        }),
    )
    .await
    .map_err(PlatformError::Internal)?;
    update_release_execution_status_with_history(
        &state,
        &release.id,
        &execution.id,
        Some(execution.status.clone()),
        ReleaseExecutionStatus::RollbackInProgress,
        Some(0),
        &actor,
        serde_json::json!({
            "reason": "rollback_requested",
            "rollback_version": rollback_version,
        }),
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
    let _ = state
        .store
        .set_release_execution_next_batch_at(&execution.id, None)
        .await;

    let mut instances = execution.instances.clone();
    instances.sort_by(|left, right| {
        left.batch_index
            .cmp(&right.batch_index)
            .then_with(|| left.instance_name.cmp(&right.instance_name))
    });
    for instance in &instances {
        let planned_status = if instance.batch_index == 1 {
            ReleaseInstanceStatus::Pending
        } else {
            ReleaseInstanceStatus::WaitingBatch
        };
        validate_release_instance_transition(&instance.status, &planned_status)
            .map_err(PlatformError::Internal)?;
        state
            .store
            .update_release_execution_instance_plan(
                &instance.id,
                &rollback_version,
                planned_status.clone(),
                None,
                Some(if instance.batch_index == 1 {
                    "Queued for immediate rollback"
                } else {
                    "Waiting for rollback batch"
                }),
            )
            .await
            .map_err(PlatformError::Internal)?;
        append_release_history(
            &state,
            &release.id,
            Some(&execution.id),
            Some(&instance.id),
            ReleaseHistoryScope::Instance,
            "instance_rollback_queued",
            &actor,
            Some(instance.status.to_string()),
            Some(planned_status.to_string()),
            serde_json::json!({
                "instance_name": instance.instance_name,
                "target_instance_id": instance.instance_id,
                "batch_index": instance.batch_index,
                "rollback_version": rollback_version,
            }),
        )
        .await
        .map_err(PlatformError::Internal)?;
    }

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
                "mode": "manual",
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

    validate_release_execution_transition(&execution.status, &target_status)
        .map_err(|_| PlatformError::conflict(invalid_state_message))?;
    let actor = user_history_actor(
        &claims,
        state
            .store
            .get_user(&claims.sub)
            .await
            .map_err(PlatformError::Internal)?
            .as_ref(),
    );

    update_release_execution_status_with_history(
        &state,
        &release.id,
        &execution.id,
        Some(execution.status.clone()),
        target_status.clone(),
        Some(execution.current_batch),
        &actor,
        serde_json::json!({
            "event_type": event_type,
            "changed_by": claims.sub,
            "current_batch": execution.current_batch,
        }),
    )
    .await
    .map_err(PlatformError::Internal)?;
    if target_status == ReleaseExecutionStatus::Paused {
        let _ = state
            .store
            .set_release_execution_next_batch_at(&execution.id, None)
            .await;
        let _ = update_batch_schedule_with_history(
            &state,
            &release.id,
            &execution.id,
            execution.current_batch + 1,
            ReleaseInstanceStatus::WaitingBatch,
            None,
            Some(&format!(
                "Paused before batch {}",
                execution.current_batch + 1
            )),
            &actor,
            "instance_status_changed",
            serde_json::json!({
                "reason": "execution_paused",
            }),
        )
        .await;
    }
    if let Some(next_request_status) = request_status {
        set_release_request_status_with_history(
            &state,
            &release.id,
            release.status.clone(),
            next_request_status,
            &actor,
            serde_json::json!({
                "reason": event_type,
                "changed_by": claims.sub,
            }),
        )
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
    append_release_history(
        &state,
        &release.id,
        Some(&execution.id),
        None,
        ReleaseHistoryScope::Execution,
        event_type,
        &actor,
        None,
        None,
        serde_json::json!({
            "changed_by": claims.sub,
            "status": target_status.to_string(),
        }),
    )
    .await
    .map_err(PlatformError::Internal)?;

    let updated = state
        .store
        .get_release_execution(&execution.id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Release execution not found"))?;
    Ok(Json(updated))
}

#[allow(clippy::too_many_arguments)]
async fn publish_release_via_nats(
    state: &AppState,
    env: &crate::models::ProjectEnvironment,
    project_id: &str,
    ruleset_name: &str,
    ruleset_json: &JsonValue,
    version: &str,
    release_execution_id: Option<&str>,
    target_server_ids: Option<&[String]>,
) -> anyhow::Result<()> {
    let publisher = state
        .sync_publisher
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("NATS publisher is not configured"))?;

    let concepts = state.store.get_concepts("", project_id).await?;
    let normalized_ruleset = normalize_release_ruleset_json_with_concepts(ruleset_json, &concepts)?;
    let json_str = serde_json::to_string(&normalized_ruleset)?;
    let event = SyncEvent::RulePut {
        tenant_id: project_id.to_string(),
        name: ruleset_name.to_string(),
        ruleset_json: json_str,
        version: version.to_string(),
        release_execution_id: release_execution_id.map(str::to_string),
        target_server_ids: target_server_ids.map(|ids| ids.to_vec()),
    };

    let prefix = env
        .nats_subject_prefix
        .as_deref()
        .unwrap_or(&state.config.nats_subject_prefix);

    publisher.publish_to(prefix, event).await
}

fn looks_like_engine_ruleset(ruleset: &JsonValue) -> bool {
    ruleset
        .get("config")
        .and_then(|config| config.get("entry_step"))
        .and_then(JsonValue::as_str)
        .is_some()
        && ruleset.get("steps").is_some_and(JsonValue::is_object)
}

fn looks_like_studio_ruleset(ruleset: &JsonValue) -> bool {
    ruleset
        .get("startStepId")
        .and_then(JsonValue::as_str)
        .is_some()
        && ruleset.get("steps").is_some_and(JsonValue::is_array)
}

#[cfg(test)]
fn normalize_release_ruleset_json(ruleset: &JsonValue) -> anyhow::Result<JsonValue> {
    normalize_release_ruleset_json_with_concepts(ruleset, &[])
}

fn normalize_release_ruleset_json_with_concepts(
    ruleset: &JsonValue,
    concepts: &[crate::models::ConceptDefinition],
) -> anyhow::Result<JsonValue> {
    if looks_like_engine_ruleset(ruleset) {
        return Ok(ruleset.clone());
    }

    if looks_like_studio_ruleset(ruleset) {
        return crate::ruleset_draft::studio_draft_to_engine_json_with_concepts(ruleset, concepts)
            .map_err(anyhow::Error::from);
    }

    Err(anyhow::anyhow!(
        "Release payload must be either studio format or engine format"
    ))
}

#[allow(clippy::items_after_test_module)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_release_ruleset_json_converts_studio_snapshot() {
        let studio_ruleset = serde_json::json!({
            "config": {
                "name": "coupon",
                "version": "1.2.3",
                "description": "demo",
                "timeout": 0,
                "enableTrace": false,
                "metadata": {}
            },
            "startStepId": "done",
            "steps": [
                {
                    "id": "done",
                    "name": "Done",
                    "type": "terminal",
                    "code": "OK",
                    "message": {
                        "type": "literal",
                        "value": "done",
                        "valueType": "string"
                    },
                    "output": []
                }
            ],
            "groups": []
        });

        let normalized = normalize_release_ruleset_json(&studio_ruleset)
            .expect("studio snapshot should convert");

        assert_eq!(normalized["config"]["entry_step"].as_str(), Some("done"));
        assert_eq!(normalized["config"]["version"].as_str(), Some("1.2.3"));
        assert!(normalized["steps"].is_object());
        assert!(normalized["steps"].get("done").is_some());
    }
}

async fn wait_for_batch_feedback(
    state: &AppState,
    execution_id: &str,
    target_server_ids: &[String],
    timeout: Duration,
) -> anyhow::Result<()> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let execution = state
            .store
            .get_release_execution(execution_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Release execution not found"))?;

        let mut pending = Vec::new();
        for server_id in target_server_ids {
            let Some(instance) = execution
                .instances
                .iter()
                .find(|instance| instance.instance_id == *server_id)
            else {
                return Err(anyhow::anyhow!(
                    "Release execution instance missing for target server {}",
                    server_id
                ));
            };
            match instance.status {
                ReleaseInstanceStatus::Success | ReleaseInstanceStatus::RolledBack => {}
                ReleaseInstanceStatus::Failed => {
                    return Err(anyhow::anyhow!(
                        "Server {} reported release failure: {}",
                        server_id,
                        instance
                            .message
                            .clone()
                            .unwrap_or_else(|| "unknown error".to_string())
                    ));
                }
                _ => pending.push(server_id.clone()),
            }
        }

        if pending.is_empty() {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(anyhow::anyhow!(
                "Timed out waiting for release ack from {}",
                pending.join(", ")
            ));
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
