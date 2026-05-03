//! Cross-org sub-organization member management handlers.
//! Callers authenticate against the *parent* org (Admin+), not the sub-org.

use crate::{
    error::{ApiResult, PlatformError},
    member::MemberResponse,
    models::{Claims, Member, Role},
    org::load_org_and_check_role,
    AppState,
};
use axum::{
    extract::{Path, State},
    Extension, Json,
};
use chrono::Utc;
use serde::Deserialize;

// ── Request types ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AddSubOrgMemberRequest {
    pub user_id: String,
    pub role: Role,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Validate that sub_id is a direct child of parent_id and return the sub-org.
async fn load_sub_org(
    state: &AppState,
    parent_id: &str,
    sub_id: &str,
) -> ApiResult<crate::models::Organization> {
    let sub_org = state
        .store
        .get_org(sub_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Sub-organization not found"))?;

    if sub_org.parent_org_id.as_deref() != Some(parent_id) {
        return Err(PlatformError::not_found("Sub-organization not found"));
    }
    Ok(sub_org)
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// GET /api/v1/orgs/:parent_id/sub-orgs/:sub_id/members
///
/// List sub-org members. Caller must be Admin+ in the parent org.
pub async fn list_sub_org_members(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((parent_id, sub_id)): Path<(String, String)>,
) -> ApiResult<Json<Vec<MemberResponse>>> {
    load_org_and_check_role(&state, &parent_id, &claims.sub, Role::Admin).await?;
    let sub_org = load_sub_org(&state, &parent_id, &sub_id).await?;
    Ok(Json(
        sub_org.members.iter().map(MemberResponse::from).collect(),
    ))
}

/// POST /api/v1/orgs/:parent_id/sub-orgs/:sub_id/members
///
/// Add a parent-org member to the sub-org. Caller must be Admin+ in the parent org.
/// The target user must already be a member of the parent org.
/// The assigned role may not exceed the caller's own role in the parent org.
pub async fn add_sub_org_member(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((parent_id, sub_id)): Path<(String, String)>,
    Json(req): Json<AddSubOrgMemberRequest>,
) -> ApiResult<Json<MemberResponse>> {
    let parent_org = load_org_and_check_role(&state, &parent_id, &claims.sub, Role::Admin).await?;
    let sub_org = load_sub_org(&state, &parent_id, &sub_id).await?;

    // Caller cannot grant a role higher than their own
    let caller_role = parent_org
        .members
        .iter()
        .find(|m| m.user_id == claims.sub)
        .map(|m| m.role)
        .unwrap_or(Role::Viewer);
    if req.role > caller_role {
        return Err(PlatformError::forbidden(
            "Cannot assign a role higher than your own",
        ));
    }

    // Target must be a member of the parent org
    let parent_member = parent_org
        .members
        .iter()
        .find(|m| m.user_id == req.user_id)
        .ok_or_else(|| {
            PlatformError::bad_request("User must be a member of the parent organization")
        })?;

    // Reject if already in sub-org
    if sub_org.members.iter().any(|m| m.user_id == req.user_id) {
        return Err(PlatformError::conflict(
            "User is already a member of this sub-organization",
        ));
    }

    let member = Member {
        user_id: req.user_id.clone(),
        email: parent_member.email.clone(),
        display_name: parent_member.display_name.clone(),
        role: req.role,
        invited_at: Utc::now(),
        invited_by: claims.sub.clone(),
    };

    state
        .store
        .add_member(&sub_id, member.clone())
        .await
        .map_err(PlatformError::Internal)?;

    state
        .store
        .sync_member_system_role(&sub_id, &member.user_id, &member.role, &claims.sub)
        .await
        .map_err(PlatformError::Internal)?;

    Ok(Json(MemberResponse::from(&member)))
}

/// DELETE /api/v1/orgs/:parent_id/sub-orgs/:sub_id/members/:uid
///
/// Remove a member from the sub-org. Caller must be Admin+ in the parent org.
pub async fn remove_sub_org_member(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((parent_id, sub_id, user_id)): Path<(String, String, String)>,
) -> ApiResult<axum::http::StatusCode> {
    load_org_and_check_role(&state, &parent_id, &claims.sub, Role::Admin).await?;
    load_sub_org(&state, &parent_id, &sub_id).await?;

    let removed = state
        .store
        .remove_member(&sub_id, &user_id)
        .await
        .map_err(PlatformError::Internal)?;

    if !removed {
        return Err(PlatformError::not_found(
            "Member not found in sub-organization",
        ));
    }

    state
        .store
        .clear_user_role_assignments(&sub_id, &user_id)
        .await
        .map_err(PlatformError::Internal)?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}
