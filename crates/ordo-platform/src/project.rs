//! Project (= Decision Domain) CRUD handlers

use crate::{
    error::{ApiResult, PlatformError},
    models::{Claims, Project, Role},
    org::load_org_and_check_role,
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
pub struct CreateProjectRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateProjectRequest {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Serialize)]
pub struct ProjectResponse {
    /// Same as ordo-server tenant_id
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub org_id: String,
    pub created_at: chrono::DateTime<Utc>,
    pub created_by: String,
}

impl From<&Project> for ProjectResponse {
    fn from(p: &Project) -> Self {
        Self {
            id: p.id.clone(),
            name: p.name.clone(),
            description: p.description.clone(),
            org_id: p.org_id.clone(),
            created_at: p.created_at,
            created_by: p.created_by.clone(),
        }
    }
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// POST /api/v1/orgs/:oid/projects — create project (admin+)
pub async fn create_project(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
    Json(req): Json<CreateProjectRequest>,
) -> ApiResult<Json<ProjectResponse>> {
    load_org_and_check_role(&state, &org_id, &claims.sub, Role::Admin).await?;

    if req.name.trim().is_empty() {
        return Err(PlatformError::bad_request("Project name is required"));
    }

    let project = Project {
        id: Uuid::new_v4().to_string(),
        name: req.name.trim().to_string(),
        description: req.description,
        org_id: org_id.clone(),
        created_at: Utc::now(),
        created_by: claims.sub.clone(),
    };

    state
        .store
        .save_project(&project)
        .await
        .map_err(PlatformError::Internal)?;

    // Register the project's ID as a tenant in ordo-server
    register_tenant_in_engine(&state, &project.id, &project.name).await?;

    Ok(Json(ProjectResponse::from(&project)))
}

/// GET /api/v1/orgs/:oid/projects — list projects (member)
pub async fn list_projects(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
) -> ApiResult<Json<Vec<ProjectResponse>>> {
    crate::org::load_org_and_check_member(&state, &org_id, &claims.sub).await?;

    let projects = state
        .store
        .list_projects(&org_id)
        .await
        .map_err(PlatformError::Internal)?;

    Ok(Json(projects.iter().map(ProjectResponse::from).collect()))
}

/// GET /api/v1/orgs/:oid/projects/:pid — project detail (member)
pub async fn get_project(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id)): Path<(String, String)>,
) -> ApiResult<Json<ProjectResponse>> {
    crate::org::load_org_and_check_member(&state, &org_id, &claims.sub).await?;

    let project = state
        .store
        .get_project(&org_id, &project_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Project not found"))?;

    Ok(Json(ProjectResponse::from(&project)))
}

/// PUT /api/v1/orgs/:oid/projects/:pid — update project (admin+)
pub async fn update_project(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id)): Path<(String, String)>,
    Json(req): Json<UpdateProjectRequest>,
) -> ApiResult<Json<ProjectResponse>> {
    load_org_and_check_role(&state, &org_id, &claims.sub, Role::Admin).await?;

    let mut project = state
        .store
        .get_project(&org_id, &project_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Project not found"))?;

    if let Some(name) = req.name {
        if name.trim().is_empty() {
            return Err(PlatformError::bad_request("Name cannot be empty"));
        }
        project.name = name.trim().to_string();
    }
    if let Some(desc) = req.description {
        project.description = if desc.trim().is_empty() {
            None
        } else {
            Some(desc)
        };
    }

    state
        .store
        .save_project(&project)
        .await
        .map_err(PlatformError::Internal)?;

    Ok(Json(ProjectResponse::from(&project)))
}

/// DELETE /api/v1/orgs/:oid/projects/:pid — delete project (admin+)
pub async fn delete_project(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id)): Path<(String, String)>,
) -> ApiResult<axum::http::StatusCode> {
    load_org_and_check_role(&state, &org_id, &claims.sub, Role::Admin).await?;

    let deleted = state
        .store
        .delete_project(&org_id, &project_id)
        .await
        .map_err(PlatformError::Internal)?;

    if !deleted {
        return Err(PlatformError::not_found("Project not found"));
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ── Engine integration ────────────────────────────────────────────────────────

/// Register a new tenant in ordo-server when a project is created.
/// This is a best-effort call — if ordo-server is unreachable, the project
/// is still created and the tenant can be registered later.
async fn register_tenant_in_engine(state: &AppState, tenant_id: &str, name: &str) -> ApiResult<()> {
    let url = format!("{}/api/v1/tenants", state.config.engine_url);
    let body = serde_json::json!({
        "id": tenant_id,
        "name": name,
        "enabled": true
    });

    match state.http_client.post(&url).json(&body).send().await {
        Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 409 => {
            // 409 means tenant already exists — that's fine
            Ok(())
        }
        Ok(resp) => {
            tracing::warn!(
                "Failed to register tenant '{}' in engine: HTTP {}",
                tenant_id,
                resp.status()
            );
            Ok(()) // Non-fatal
        }
        Err(e) => {
            tracing::warn!(
                "Could not reach engine to register tenant '{}': {}",
                tenant_id,
                e
            );
            Ok(()) // Non-fatal — engine may not be running yet
        }
    }
}
