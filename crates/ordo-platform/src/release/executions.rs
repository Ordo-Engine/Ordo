use super::*;
use std::time::Duration;
use tracing::{error, info, warn};

const RELEASE_ACK_TIMEOUT_SECS: u64 = 30;

fn execution_has_failed_outcome(execution: &ReleaseExecution) -> bool {
    execution.status == ReleaseExecutionStatus::Failed
        || (execution.status == ReleaseExecutionStatus::Completed
            && (execution.summary.failed_instances > 0 || execution.summary.pending_instances > 0))
}

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

    if latest_execution.as_ref().is_some_and(execution_is_active) {
        return Err(PlatformError::conflict(
            "Release execution is already in progress",
        ));
    }

    let retryable_execution = latest_execution
        .as_ref()
        .is_some_and(execution_has_failed_outcome);
    let can_execute = release.status == ReleaseRequestStatus::Approved
        || release.status == ReleaseRequestStatus::Failed
        || (release.status == ReleaseRequestStatus::Completed && retryable_execution)
        || (release.status == ReleaseRequestStatus::Executing && retryable_execution);

    if !can_execute {
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
    let draft = state
        .store
        .get_draft_ruleset(&project_id, &release.ruleset_name)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Draft ruleset not found"))?;

    let strategy = release.request_snapshot.rollout_strategy.clone();
    let total_instances = bound_servers.len();
    let batch_size = compute_batch_size(&strategy, total_instances);
    let total_batches = total_instances.div_ceil(batch_size);

    let execution_id = Uuid::new_v4().to_string();

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
            ReleaseExecutionStatus::RollingOut,
            0,
            total_batches as i32,
            &strategy,
            Some(&claims.sub),
        )
        .await
        .map_err(PlatformError::Internal)?;

    let current_version = release
        .version_diff
        .from_version
        .clone()
        .unwrap_or_else(|| "unreleased".to_string());
    let mut instances = Vec::new();
    for server in &bound_servers {
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
            current_version: current_version.clone(),
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
        instances.push(instance);
    }

    // Retrieve deployer info for history entry (done before spawning to avoid extra DB call)
    let deployer = state
        .store
        .get_user(&claims.sub)
        .await
        .map_err(PlatformError::Internal)?;

    let ctx = RollingDeploymentContext {
        state: state.clone(),
        execution_id: execution_id.clone(),
        org_id: org_id.clone(),
        project_id: project_id.clone(),
        release_id: release.id.clone(),
        ruleset_name: release.ruleset_name.clone(),
        version: release.version.clone(),
        env,
        instances,
        bound_servers,
        draft: draft.draft,
        strategy,
        deployed_by: claims.sub.clone(),
        deployer_email: deployer.as_ref().map(|u| u.email.clone()),
        deployer_display_name: deployer.as_ref().map(|u| u.display_name.clone()),
        auto_rollback: release.request_snapshot.rollback_policy.auto_rollback,
        rollback_version: release
            .version_diff
            .rollback_version
            .clone()
            .or_else(|| release.rollback_version.clone()),
        release_note: release.release_note.clone(),
    };

    tokio::spawn(run_rolling_deployment(ctx));

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

    let mut failed = false;
    let mut terminal_batch = 0;

    'batches: for (batch_idx, batch) in pairs.chunks(batch_size).enumerate() {
        let batch_num = batch_idx + 1;
        terminal_batch = batch_num as i32;
        let batch_start = std::time::Instant::now();

        // Honour pause: spin-wait until resumed or terminal
        #[allow(clippy::while_let_loop)]
        loop {
            match state.store.get_release_execution(&execution_id).await {
                Ok(Some(exec)) => match exec.status {
                    ReleaseExecutionStatus::Paused => {
                        tokio::time::sleep(Duration::from_secs(3)).await;
                        continue;
                    }
                    ReleaseExecutionStatus::Failed => return,
                    _ => break,
                },
                _ => break,
            }
        }

        // Mark batch instances as Updating
        for (inst, _) in batch {
            if let Err(e) = state
                .store
                .update_release_execution_instance(
                    &inst.id,
                    ReleaseInstanceStatus::Updating,
                    Some("Dispatching ruleset to server"),
                    None,
                )
                .await
            {
                error!(execution_id, instance_id = %inst.id, "Failed to update instance to Updating: {e}");
            }
        }

        if let Err(e) = state
            .store
            .update_release_execution_status(
                &execution_id,
                ReleaseExecutionStatus::RollingOut,
                Some(batch_num as i32),
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

        let target_server_ids = batch
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
                for (inst, server) in batch {
                    let _ = state
                        .store
                        .update_release_execution_instance(
                            &inst.id,
                            ReleaseInstanceStatus::Updating,
                            Some("Ruleset published to NATS; waiting for server ack"),
                            Some("publish_sent"),
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
                        for (inst, _) in batch {
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
                        for server_id in &target_server_ids {
                            if let Ok(Some(instance)) = state
                                .store
                                .find_release_execution_instance_by_target(&execution_id, server_id)
                                .await
                            {
                                if instance.status == ReleaseInstanceStatus::Updating
                                    || instance.status == ReleaseInstanceStatus::Pending
                                {
                                    let _ = state
                                        .store
                                        .update_release_execution_instance(
                                            &instance.id,
                                            ReleaseInstanceStatus::Failed,
                                            Some(&msg),
                                            Some("release_ack_timeout"),
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
                for (inst, _) in batch {
                    let _ = state
                        .store
                        .update_release_execution_instance(
                            &inst.id,
                            ReleaseInstanceStatus::Failed,
                            Some(&msg),
                            Some("publish_failed"),
                        )
                        .await;
                }
                failed = true;
                break 'batches;
            }
        }

        // Wait between batches (skip after last batch)
        if batch_num < total_batches && interval_secs > 0 {
            info!(
                execution_id,
                "Batch {}/{} complete — waiting {}s before next batch",
                batch_num,
                total_batches,
                interval_secs
            );
            tokio::time::sleep(Duration::from_secs(interval_secs)).await;
        }
    }

    if failed {
        if let Err(e) = state
            .store
            .update_release_execution_status(
                &execution_id,
                ReleaseExecutionStatus::Failed,
                Some(terminal_batch),
            )
            .await
        {
            error!(execution_id, "Failed to mark execution Failed: {e}");
        }
        if let Err(e) = state
            .store
            .set_release_request_status(&release_id, ReleaseRequestStatus::Failed)
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
    if let Err(e) = state
        .store
        .update_release_execution_status(
            &execution_id,
            ReleaseExecutionStatus::Completed,
            Some(total_batches as i32),
        )
        .await
    {
        error!(execution_id, "Failed to mark execution Completed: {e}");
    }
    if let Err(e) = state
        .store
        .set_release_request_status(&release_id, ReleaseRequestStatus::Completed)
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

    let Some(rb) = rollback_deployment else {
        warn!(
            execution_id,
            "Auto-rollback: snapshot not found for version {}", rollback_version
        );
        return Err(anyhow::anyhow!("Rollback snapshot not found"));
    };

    let _ = state
        .store
        .update_release_execution_status(
            execution_id,
            ReleaseExecutionStatus::RollbackInProgress,
            None,
        )
        .await;

    let push_result = publish_release_via_nats(
        state,
        env,
        project_id,
        ruleset_name,
        &rb.snapshot,
        rollback_version,
        Some(execution_id),
        None,
    )
    .await;

    match push_result {
        Ok(()) => {
            let _ = state
                .store
                .mark_ruleset_published(project_id, ruleset_name, rollback_version)
                .await;
            let _ = state
                .store
                .update_release_execution_status(
                    execution_id,
                    ReleaseExecutionStatus::Completed,
                    None,
                )
                .await;
            let _ = state
                .store
                .set_release_request_status(release_id, ReleaseRequestStatus::RolledBack)
                .await;
            info!(
                execution_id,
                "Auto-rollback to {} succeeded", rollback_version
            );
        }
        Err(err) => {
            warn!(execution_id, "Auto-rollback failed: {}", err);
            let _ = state
                .store
                .update_release_execution_status(execution_id, ReleaseExecutionStatus::Failed, None)
                .await;
        }
    }
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
    if release.version_diff.from_version.as_deref() != Some(rollback_version.as_str()) {
        return Err(PlatformError::conflict(
            "Cross-version rollback requires an approved release request",
        ));
    }

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
        Some(&execution.id),
        None,
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

    let json_str = serde_json::to_string(ruleset_json)?;
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
                ReleaseInstanceStatus::Success => {}
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
