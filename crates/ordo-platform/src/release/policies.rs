use super::*;

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

pub(super) async fn validate_release_policy_request(
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
