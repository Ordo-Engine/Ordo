//! Organization member management handlers

use crate::{
    error::{ApiResult, PlatformError},
    models::{Claims, Member, Role},
    org::load_org_and_check_role,
    AppState,
};
use axum::{
    extract::{Path, State},
    Extension, Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};

// ── Request / Response types ─────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct InviteMemberRequest {
    pub email: String,
    pub role: Role,
}

#[derive(Deserialize)]
pub struct UpdateRoleRequest {
    pub role: Role,
}

#[derive(Serialize)]
pub struct MemberResponse {
    pub user_id: String,
    pub email: String,
    pub display_name: String,
    pub role: Role,
    pub invited_at: chrono::DateTime<Utc>,
}

impl From<&Member> for MemberResponse {
    fn from(m: &Member) -> Self {
        Self {
            user_id: m.user_id.clone(),
            email: m.email.clone(),
            display_name: m.display_name.clone(),
            role: m.role,
            invited_at: m.invited_at,
        }
    }
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// GET /api/v1/orgs/:id/members
pub async fn list_members(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
) -> ApiResult<Json<Vec<MemberResponse>>> {
    let org = crate::org::load_org_and_check_member(&state, &org_id, &claims.sub).await?;
    Ok(Json(org.members.iter().map(MemberResponse::from).collect()))
}

/// POST /api/v1/orgs/:id/members — invite by email (admin+)
pub async fn invite_member(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
    Json(req): Json<InviteMemberRequest>,
) -> ApiResult<Json<MemberResponse>> {
    // Must be admin+
    let org = load_org_and_check_role(&state, &org_id, &claims.sub, Role::Admin).await?;

    // Can't assign higher role than own role
    let caller_role = org
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

    // Look up user by email
    let user = state
        .store
        .find_user_by_email(&req.email)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("No user with that email address"))?;

    // Check not already a member
    if org.members.iter().any(|m| m.user_id == user.id) {
        return Err(PlatformError::conflict("User is already a member"));
    }

    let member = Member {
        user_id: user.id.clone(),
        email: user.email.clone(),
        display_name: user.display_name.clone(),
        role: req.role,
        invited_at: Utc::now(),
        invited_by: claims.sub.clone(),
    };

    state
        .store
        .add_member(&org_id, member.clone())
        .await
        .map_err(PlatformError::Internal)?;
    state
        .store
        .sync_member_system_role(&org_id, &member.user_id, &member.role, &claims.sub)
        .await
        .map_err(PlatformError::Internal)?;

    Ok(Json(MemberResponse::from(&member)))
}

/// PUT /api/v1/orgs/:id/members/:uid — update role (admin+)
pub async fn update_member_role(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, user_id)): Path<(String, String)>,
    Json(req): Json<UpdateRoleRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let org = load_org_and_check_role(&state, &org_id, &claims.sub, Role::Admin).await?;

    // Prevent self-demotion for owners
    if user_id == claims.sub {
        return Err(PlatformError::bad_request("Cannot change your own role"));
    }

    // Can't assign higher role than own role
    let caller_role = org
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

    let updated = state
        .store
        .update_member_role(&org_id, &user_id, req.role)
        .await
        .map_err(PlatformError::Internal)?;

    if !updated {
        return Err(PlatformError::not_found("Member not found"));
    }

    state
        .store
        .sync_member_system_role(&org_id, &user_id, &req.role, &claims.sub)
        .await
        .map_err(PlatformError::Internal)?;

    Ok(Json(serde_json::json!({"success": true})))
}

/// DELETE /api/v1/orgs/:id/members/:uid — remove member (admin+, or self-leave)
pub async fn remove_member(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, user_id)): Path<(String, String)>,
) -> ApiResult<axum::http::StatusCode> {
    // Self-leave is always allowed; removing others requires admin+
    if user_id != claims.sub {
        load_org_and_check_role(&state, &org_id, &claims.sub, Role::Admin).await?;
    } else {
        crate::org::load_org_and_check_member(&state, &org_id, &claims.sub).await?;
    }

    let removed = state
        .store
        .remove_member(&org_id, &user_id)
        .await
        .map_err(PlatformError::Internal)?;

    if !removed {
        return Err(PlatformError::not_found("Member not found"));
    }

    state
        .store
        .clear_user_role_assignments(&org_id, &user_id)
        .await
        .map_err(PlatformError::Internal)?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}
