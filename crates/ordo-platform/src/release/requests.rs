use super::*;
use uuid::Uuid as LocalUuid;

#[derive(Debug, Deserialize)]
pub struct ReleaseTargetPreviewQuery {
    pub environment_id: String,
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

pub async fn preview_release_target(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id)): Path<(String, String)>,
    Query(query): Query<ReleaseTargetPreviewQuery>,
) -> ApiResult<Json<ReleaseTargetPreview>> {
    require_project_permission(
        &state,
        &org_id,
        &project_id,
        &claims.sub,
        PERM_RELEASE_REQUEST_CREATE,
    )
    .await?;

    if query.environment_id.trim().is_empty() {
        return Err(PlatformError::bad_request("environment_id is required"));
    }

    let environment = state
        .store
        .get_environment(&project_id, &query.environment_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Environment not found"))?;

    let mut bound_servers = Vec::new();
    for server_id in &environment.server_ids {
        if let Some(server) = state
            .store
            .get_server(server_id)
            .await
            .map_err(PlatformError::Internal)?
        {
            bound_servers.push(ReleaseTargetServerPreview {
                id: server.id,
                name: server.name,
                url: server.url,
                status: server.status,
                version: server.version,
            });
        }
    }

    let preview = if bound_servers.is_empty() {
        ReleaseTargetPreview {
            environment_id: environment.id,
            environment_name: environment.name,
            affected_instance_count: 0,
            bound_servers: Vec::new(),
            message: Some("Environment has no bound server".to_string()),
        }
    } else {
        ReleaseTargetPreview {
            environment_id: environment.id,
            environment_name: environment.name,
            affected_instance_count: bound_servers.len() as i32,
            bound_servers,
            message: None,
        }
    };

    Ok(Json(preview))
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
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Draft ruleset not found"))?;
    let draft_version = draft
        .draft
        .get("config")
        .and_then(|config| config.get("version"))
        .and_then(|version| version.as_str())
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .ok_or_else(|| PlatformError::bad_request("ruleset.config.version is required"))?
        .to_string();
    if req.version.trim() != draft_version {
        return Err(PlatformError::bad_request(
            "Release request version must match the draft ruleset version",
        ));
    }
    let environment_baseline = load_release_environment_baseline(
        &state,
        &project_id,
        &req.ruleset_name,
        &req.environment_id,
    )
    .await?;
    let current_version = environment_baseline
        .as_ref()
        .map(|deployment| deployment.version.clone())
        .or_else(|| draft.meta.published_version.clone());
    let baseline_snapshot = if let Some(deployment) = environment_baseline.as_ref() {
        Some(deployment.snapshot.clone())
    } else {
        load_release_baseline_snapshot(
            &state,
            &org_id,
            &project_id,
            &req.ruleset_name,
            current_version.as_deref(),
        )
        .await?
    };
    // Inline sub-rule assets before storing the snapshot so the snapshot is self-contained.
    let target_snapshot =
        inline_sub_rules_into_draft(&state, &org_id, &project_id, draft.draft.clone()).await?;

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
        to_version: draft_version.clone(),
        rollback_version: current_version.clone(),
        changed: current_version.as_deref() != Some(draft_version.as_str()),
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
        target_ruleset_snapshot: Some(target_snapshot.clone()),
    };

    let mut create_req = req;
    create_req.policy_id = Some(policy.id.clone());
    create_req.version = draft_version;
    create_req.rollback_version = current_version.clone();
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

    let actor = user_history_actor(&claims, Some(&requester));
    append_release_history(
        &state,
        &release_id,
        None,
        None,
        ReleaseHistoryScope::Request,
        "request_created",
        &actor,
        None,
        None,
        serde_json::json!({
            "title": create_req.title,
            "ruleset_name": create_req.ruleset_name,
            "version": create_req.version,
            "environment_id": create_req.environment_id,
            "environment_name": environment.name,
            "policy_id": create_req.policy_id,
            "policy_name": policy.name,
            "rollback_version": create_req.rollback_version,
            "affected_instance_count": request_snapshot.affected_instance_count,
            "version_diff": version_diff,
            "content_diff": content_diff,
        }),
    )
    .await
    .map_err(PlatformError::Internal)?;

    append_release_history(
        &state,
        &release_id,
        None,
        None,
        ReleaseHistoryScope::Request,
        "request_status_changed",
        &actor,
        None,
        Some(ReleaseRequestStatus::PendingApproval.to_string()),
        serde_json::json!({
            "reason": "request_created",
        }),
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

        append_release_history(
            &state,
            &release_id,
            None,
            None,
            ReleaseHistoryScope::Approval,
            "approval_assigned",
            &actor,
            None,
            Some(ReleaseApprovalDecision::Pending.to_string()),
            serde_json::json!({
                "stage": (idx as i32) + 1,
                "reviewer_id": reviewer_id,
                "reviewer_name": approver_users
                    .iter()
                    .find(|user| user.id == *reviewer_id)
                    .map(|user| user.display_name.clone()),
                "reviewer_email": approver_users
                    .iter()
                    .find(|user| user.id == *reviewer_id)
                    .map(|user| user.email.clone()),
            }),
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

    // Notify each reviewer that their approval is requested
    for reviewer_id in policy
        .approver_ids
        .iter()
        .take(policy.min_approvals as usize)
    {
        if *reviewer_id == claims.sub {
            continue;
        }
        let _ = state
            .store
            .create_notification(
                &LocalUuid::new_v4().to_string(),
                &org_id,
                reviewer_id,
                "release_review_requested",
                Some(&release.id),
                Some("release_request"),
                serde_json::json!({
                    "title": release.title,
                    "project_id": project_id,
                    "requester_id": claims.sub,
                }),
            )
            .await;
    }

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

pub async fn list_release_request_history(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id, release_id)): Path<(String, String, String)>,
) -> ApiResult<Json<Vec<ReleaseRequestHistoryEntry>>> {
    require_project_permission(
        &state,
        &org_id,
        &project_id,
        &claims.sub,
        PERM_RELEASE_REQUEST_VIEW,
    )
    .await?;

    state
        .store
        .get_release_request(&org_id, &project_id, &release_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Release request not found"))?;

    let history = state
        .store
        .list_release_request_history(&release_id)
        .await
        .map_err(PlatformError::Internal)?;

    Ok(Json(history))
}

pub(super) async fn load_release_baseline_snapshot(
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

async fn load_release_environment_baseline(
    state: &AppState,
    project_id: &str,
    ruleset_name: &str,
    environment_id: &str,
) -> ApiResult<Option<RulesetDeployment>> {
    let deployments = state
        .store
        .list_deployments(project_id, Some(ruleset_name), 100)
        .await
        .map_err(PlatformError::Internal)?;

    Ok(deployments.into_iter().find(|deployment| {
        deployment.environment_id == environment_id
            && deployment.status == DeploymentStatus::Success
    }))
}
