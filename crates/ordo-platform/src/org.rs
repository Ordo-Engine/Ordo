//! Organization CRUD handlers

use crate::{
    error::{ApiResult, PlatformError},
    models::{Claims, Organization},
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
}

impl From<&Organization> for OrgResponse {
    fn from(org: &Organization) -> Self {
        Self {
            id: org.id.clone(),
            name: org.name.clone(),
            description: org.description.clone(),
            created_at: org.created_at,
            created_by: org.created_by.clone(),
            member_count: org.members.len(),
        }
    }
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// POST /api/v1/orgs — create organization (caller becomes owner)
pub async fn create_org(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<CreateOrgRequest>,
) -> ApiResult<Json<OrgResponse>> {
    if req.name.trim().is_empty() {
        return Err(PlatformError::bad_request("Organization name is required"));
    }

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

    Ok(Json(OrgResponse::from(&org)))
}

/// GET /api/v1/orgs — list orgs the caller belongs to
pub async fn list_orgs(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> ApiResult<Json<Vec<OrgResponse>>> {
    let orgs = state
        .store
        .list_user_orgs(&claims.sub)
        .await
        .map_err(PlatformError::Internal)?;
    Ok(Json(orgs.iter().map(OrgResponse::from).collect()))
}

/// GET /api/v1/orgs/:id — org detail (must be member)
pub async fn get_org(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
) -> ApiResult<Json<Organization>> {
    let org = load_org_and_check_member(&state, &org_id, &claims.sub).await?;
    Ok(Json(org))
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

    Ok(Json(OrgResponse::from(&org)))
}

/// DELETE /api/v1/orgs/:id — delete org (owner only)
pub async fn delete_org(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
) -> ApiResult<axum::http::StatusCode> {
    load_org_and_check_role(&state, &org_id, &claims.sub, crate::models::Role::Owner).await?;

    state
        .store
        .delete_org(&org_id)
        .await
        .map_err(PlatformError::Internal)?;

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
        return Err(PlatformError::forbidden(format!(
            "Requires {} role or higher",
            required
        )));
    }

    Ok(org)
}
