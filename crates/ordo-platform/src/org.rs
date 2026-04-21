//! Organization CRUD handlers

use crate::{
    error::{ApiResult, PlatformError},
    models::{
        AssignRoleRequest, Claims, CreateRoleRequest, OrgRole, Organization, UpdateRoleRequest,
        UserRoleAssignment,
    },
    project::{sync_tenant_delete, sync_tenant_upsert},
    rbac::{require_permission, PERM_ROLE_MANAGE, PERM_ROLE_VIEW},
    AppState,
};
use axum::{
    extract::{Path, State},
    Extension, Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Request / Response types ─────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateOrgRequest {
    pub name: String,
    pub description: Option<String>,
    /// If provided, creates a sub-org under this parent. Caller must be Admin+
    /// in the parent org. The parent must be a root org (depth 0).
    pub parent_org_id: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateOrgRequest {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Serialize)]
pub struct OrgResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: chrono::DateTime<Utc>,
    pub created_by: String,
    pub member_count: usize,
    pub parent_org_id: Option<String>,
    pub depth: i32,
    /// Number of direct child (sub) organizations. Always 0 for depth-1 orgs.
    pub child_count: usize,
}

impl OrgResponse {
    fn from_org(org: &Organization, child_count: usize) -> Self {
        Self {
            id: org.id.clone(),
            name: org.name.clone(),
            description: org.description.clone(),
            created_at: org.created_at,
            created_by: org.created_by.clone(),
            member_count: org.members.len(),
            parent_org_id: org.parent_org_id.clone(),
            depth: org.depth,
            child_count,
        }
    }
}

impl From<&Organization> for OrgResponse {
    fn from(org: &Organization) -> Self {
        Self::from_org(org, 0)
    }
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// POST /api/v1/orgs — create organization (caller becomes owner)
///
/// - Root org (`parent_org_id` absent): requires `allow_org_creation` config flag.
/// - Sub-org (`parent_org_id` present): requires Admin+ role in the parent org;
///   the parent must itself be a root org (depth 0) to enforce the two-level limit.
pub async fn create_org(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<CreateOrgRequest>,
) -> ApiResult<Json<OrgResponse>> {
    if req.name.trim().is_empty() {
        return Err(PlatformError::bad_request("Organization name is required"));
    }

    let (parent_org_id, depth) = if let Some(ref pid) = req.parent_org_id {
        // Sub-org: caller must be Admin+ in the parent org.
        let parent = load_org_and_check_role(&state, pid, &claims.sub, crate::models::Role::Admin).await?;
        if parent.depth >= 1 {
            return Err(PlatformError::bad_request(
                "Organization hierarchy is limited to two levels. Cannot create a sub-org under another sub-org.",
            ));
        }
        (Some(pid.clone()), 1_i32)
    } else {
        // Root org: check platform config flag.
        if !state.config.allow_org_creation {
            return Err(PlatformError::forbidden(
                "Creating new organizations is disabled on this platform. Contact your platform administrator.",
            ));
        }
        (None, 0_i32)
    };

    // Load user to get email + display_name for member record
    let user = state
        .store
        .get_user(&claims.sub)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("User not found"))?;

    let org_id = Uuid::new_v4().to_string();
    let org = Organization {
        id: org_id.clone(),
        name: req.name.trim().to_string(),
        description: req.description,
        created_at: Utc::now(),
        created_by: claims.sub.clone(),
        parent_org_id,
        depth,
        members: vec![crate::models::Member {
            user_id: claims.sub.clone(),
            email: user.email.clone(),
            display_name: user.display_name.clone(),
            role: crate::models::Role::Owner,
            invited_at: Utc::now(),
            invited_by: claims.sub.clone(),
        }],
    };

    state
        .store
        .save_org(&org)
        .await
        .map_err(PlatformError::Internal)?;

    state
        .store
        .seed_system_roles(&org_id)
        .await
        .map_err(PlatformError::Internal)?;
    state
        .store
        .sync_member_system_role(
            &org_id,
            &claims.sub,
            &crate::models::Role::Owner,
            &claims.sub,
        )
        .await
        .map_err(PlatformError::Internal)?;

    Ok(Json(OrgResponse::from_org(&org, 0)))
}

/// GET /api/v1/orgs — list orgs the caller belongs to (includes sub-orgs)
pub async fn list_orgs(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> ApiResult<Json<Vec<OrgResponse>>> {
    let orgs = state
        .store
        .list_user_orgs(&claims.sub)
        .await
        .map_err(PlatformError::Internal)?;

    let mut responses = Vec::with_capacity(orgs.len());
    for org in &orgs {
        let child_count = state
            .store
            .count_child_orgs(&org.id)
            .await
            .map_err(PlatformError::Internal)? as usize;
        responses.push(OrgResponse::from_org(org, child_count));
    }
    Ok(Json(responses))
}

/// GET /api/v1/orgs/:id — org detail (must be member)
pub async fn get_org(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
) -> ApiResult<Json<OrgResponse>> {
    let org = load_org_and_check_member(&state, &org_id, &claims.sub).await?;
    let child_count = state
        .store
        .count_child_orgs(&org_id)
        .await
        .map_err(PlatformError::Internal)? as usize;
    Ok(Json(OrgResponse::from_org(&org, child_count)))
}

/// GET /api/v1/orgs/:id/sub-orgs — list direct child orgs (must be member of parent)
pub async fn list_sub_orgs(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
) -> ApiResult<Json<Vec<OrgResponse>>> {
    load_org_and_check_member(&state, &org_id, &claims.sub).await?;
    let children = state
        .store
        .list_child_orgs(&org_id)
        .await
        .map_err(PlatformError::Internal)?;
    // Sub-orgs are depth=1 and cannot have children; child_count is always 0
    Ok(Json(children.iter().map(OrgResponse::from).collect()))
}

/// PUT /api/v1/orgs/:id — update org (admin+)
pub async fn update_org(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
    Json(req): Json<UpdateOrgRequest>,
) -> ApiResult<Json<OrgResponse>> {
    let mut org =
        load_org_and_check_role(&state, &org_id, &claims.sub, crate::models::Role::Admin).await?;

    if let Some(name) = req.name {
        if name.trim().is_empty() {
            return Err(PlatformError::bad_request("Name cannot be empty"));
        }
        org.name = name.trim().to_string();
    }
    if let Some(desc) = req.description {
        org.description = if desc.trim().is_empty() {
            None
        } else {
            Some(desc)
        };
    }

    state
        .store
        .save_org(&org)
        .await
        .map_err(PlatformError::Internal)?;

    let child_count = state
        .store
        .count_child_orgs(&org.id)
        .await
        .map_err(PlatformError::Internal)? as usize;
    Ok(Json(OrgResponse::from_org(&org, child_count)))
}

/// DELETE /api/v1/orgs/:id — delete org (owner only)
///
/// Cascades NATS sync deletions for projects in this org and all child sub-orgs.
/// DB FK ON DELETE CASCADE handles the actual row cleanup.
pub async fn delete_org(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
) -> ApiResult<axum::http::StatusCode> {
    load_org_and_check_role(&state, &org_id, &claims.sub, crate::models::Role::Owner).await?;

    // Collect all projects: own org + all sub-org projects
    let mut all_projects: Vec<crate::models::Project> = state
        .store
        .list_projects(&org_id)
        .await
        .map_err(PlatformError::Internal)?;

    let sub_orgs = state
        .store
        .list_child_orgs(&org_id)
        .await
        .map_err(PlatformError::Internal)?;

    for sub_org in &sub_orgs {
        let sub_projects = state
            .store
            .list_projects(&sub_org.id)
            .await
            .map_err(PlatformError::Internal)?;
        all_projects.extend(sub_projects);
    }

    // NATS sync deletions with full compensation on failure
    let mut deleted_tenants: Vec<crate::models::Project> = Vec::new();
    for project in &all_projects {
        if let Err(err) = sync_tenant_delete(&state, &project.id).await {
            for deleted in &deleted_tenants {
                let _ = sync_tenant_upsert(&state, &deleted.id, &deleted.name, true).await;
            }
            return Err(err);
        }
        deleted_tenants.push(project.clone());
    }

    // DB delete — FK CASCADE removes sub-orgs, members, projects, roles automatically
    if let Err(err) = state.store.delete_org(&org_id).await {
        for project in &deleted_tenants {
            let _ = sync_tenant_upsert(&state, &project.id, &project.name, true).await;
        }
        return Err(PlatformError::Internal(err));
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

pub async fn load_org_and_check_member(
    state: &AppState,
    org_id: &str,
    user_id: &str,
) -> ApiResult<Organization> {
    let org = state
        .store
        .get_org(org_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Organization not found"))?;

    if !org.members.iter().any(|m| m.user_id == user_id) {
        return Err(PlatformError::forbidden(
            "Not a member of this organization",
        ));
    }

    Ok(org)
}

// ── RBAC: Role CRUD ───────────────────────────────────────────────────────────

/// GET /api/v1/orgs/:oid/roles
pub async fn list_roles(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
) -> ApiResult<Json<Vec<OrgRole>>> {
    require_permission(&state, &org_id, &claims.sub, PERM_ROLE_VIEW).await?;
    let roles = state
        .store
        .list_org_roles(&org_id)
        .await
        .map_err(PlatformError::Internal)?;
    Ok(Json(roles))
}

/// POST /api/v1/orgs/:oid/roles
pub async fn create_role(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
    Json(req): Json<CreateRoleRequest>,
) -> ApiResult<Json<OrgRole>> {
    require_permission(&state, &org_id, &claims.sub, PERM_ROLE_MANAGE).await?;
    if req.name.trim().is_empty() {
        return Err(PlatformError::bad_request("Role name is required"));
    }
    let id = Uuid::new_v4().to_string();
    let role = state
        .store
        .create_org_role(&id, &org_id, &req, false)
        .await
        .map_err(PlatformError::Internal)?;
    Ok(Json(role))
}

/// PUT /api/v1/orgs/:oid/roles/:rid
pub async fn update_role(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, role_id)): Path<(String, String)>,
    Json(req): Json<UpdateRoleRequest>,
) -> ApiResult<Json<OrgRole>> {
    require_permission(&state, &org_id, &claims.sub, PERM_ROLE_MANAGE).await?;
    let updated = state
        .store
        .update_org_role(&org_id, &role_id, &req)
        .await
        .map_err(PlatformError::Internal)?;
    if !updated {
        return Err(PlatformError::not_found(
            "Role not found or is a system role (cannot be modified)",
        ));
    }
    let role = state
        .store
        .get_org_role(&org_id, &role_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Role not found"))?;
    Ok(Json(role))
}

/// DELETE /api/v1/orgs/:oid/roles/:rid
pub async fn delete_role(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, role_id)): Path<(String, String)>,
) -> ApiResult<axum::http::StatusCode> {
    require_permission(&state, &org_id, &claims.sub, PERM_ROLE_MANAGE).await?;
    let deleted = state
        .store
        .delete_org_role(&org_id, &role_id)
        .await
        .map_err(PlatformError::Internal)?;
    if !deleted {
        return Err(PlatformError::bad_request(
            "Role not found or is a built-in system role (cannot be deleted)",
        ));
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ── RBAC: Member Role Assignments ─────────────────────────────────────────────

/// GET /api/v1/orgs/:oid/members/:uid/roles
pub async fn list_member_roles(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, user_id)): Path<(String, String)>,
) -> ApiResult<Json<Vec<UserRoleAssignment>>> {
    require_permission(&state, &org_id, &claims.sub, PERM_ROLE_VIEW).await?;
    let assignments = state
        .store
        .list_user_roles(&org_id, &user_id)
        .await
        .map_err(PlatformError::Internal)?;
    Ok(Json(assignments))
}

/// POST /api/v1/orgs/:oid/members/:uid/roles
pub async fn assign_member_role(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, user_id)): Path<(String, String)>,
    Json(req): Json<AssignRoleRequest>,
) -> ApiResult<axum::http::StatusCode> {
    require_permission(&state, &org_id, &claims.sub, PERM_ROLE_MANAGE).await?;
    // Verify role belongs to this org
    state
        .store
        .get_org_role(&org_id, &req.role_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Role not found in this org"))?;
    state
        .store
        .assign_role(&org_id, &user_id, &req.role_id, &claims.sub)
        .await
        .map_err(PlatformError::Internal)?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// DELETE /api/v1/orgs/:oid/members/:uid/roles/:rid
pub async fn revoke_member_role(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, user_id, role_id)): Path<(String, String, String)>,
) -> ApiResult<axum::http::StatusCode> {
    require_permission(&state, &org_id, &claims.sub, PERM_ROLE_MANAGE).await?;
    state
        .store
        .revoke_role(&org_id, &user_id, &role_id)
        .await
        .map_err(PlatformError::Internal)?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn load_org_and_check_role(
    state: &AppState,
    org_id: &str,
    user_id: &str,
    required: crate::models::Role,
) -> ApiResult<Organization> {
    let org = state
        .store
        .get_org(org_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Organization not found"))?;

    let member = org
        .members
        .iter()
        .find(|m| m.user_id == user_id)
        .ok_or_else(|| PlatformError::forbidden("Not a member of this organization"))?;

    if member.role < required {
        return Err(PlatformError::requires_role_or_higher(required.to_string()));
    }

    Ok(org)
}
