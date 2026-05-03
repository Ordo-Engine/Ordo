use super::*;
use uuid::Uuid;

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

    let reviewer = state
        .store
        .get_user(&claims.sub)
        .await
        .map_err(PlatformError::Internal)?;
    let actor = user_history_actor(&claims, reviewer.as_ref());

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

    let reviewed_approval = approvals.iter().find(|item| item.reviewer_id == claims.sub);
    append_release_history(
        &state,
        &release_id,
        None,
        None,
        ReleaseHistoryScope::Approval,
        "approval_reviewed",
        &actor,
        Some(ReleaseApprovalDecision::Pending.to_string()),
        Some(decision.to_string()),
        serde_json::json!({
            "stage": reviewed_approval.map(|item| item.stage),
            "decision": decision,
            "comment": comment,
        }),
    )
    .await
    .map_err(PlatformError::Internal)?;

    validate_release_request_transition(&release.status, &next_status)
        .map_err(|err| PlatformError::conflict(err.to_string()))?;

    state
        .store
        .set_release_request_status(&release_id, next_status.clone())
        .await
        .map_err(PlatformError::Internal)?;

    if next_status != release.status {
        append_release_history(
            &state,
            &release_id,
            None,
            None,
            ReleaseHistoryScope::Request,
            "request_status_changed",
            &actor,
            Some(release.status.to_string()),
            Some(next_status.to_string()),
            serde_json::json!({
                "reason": "approval_reviewed",
                "decision": decision,
                "comment": comment,
            }),
        )
        .await
        .map_err(PlatformError::Internal)?;
    }

    let item = state
        .store
        .get_release_request(&org_id, &project_id, &release_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Release request not found"))?;

    // Notify the release requester of the decision
    let notif_type = match decision {
        ReleaseApprovalDecision::Approved => "release_approved",
        ReleaseApprovalDecision::Rejected => "release_rejected",
        _ => "",
    };
    if !notif_type.is_empty() && item.created_by != claims.sub {
        let _ = state
            .store
            .create_notification(
                &Uuid::new_v4().to_string(),
                &org_id,
                &item.created_by,
                notif_type,
                Some(&release_id),
                Some("release_request"),
                serde_json::json!({
                    "title": item.title,
                    "project_id": project_id,
                    "reviewer_id": claims.sub,
                }),
            )
            .await;
    }

    Ok(Json(item))
}
