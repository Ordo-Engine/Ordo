//! Project (= Decision Domain) CRUD handlers

use crate::{
    error::{ApiResult, PlatformError},
    models::{Claims, Project, Role},
    org::load_org_and_check_role,
    sync::SyncEvent,
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
        server_id: None,
    };

    state
        .store
        .save_project(&project)
        .await
        .map_err(PlatformError::Internal)?;

    // Register the project's ID as a tenant in ordo-server. If the sync publish
    // fails, delete the newly-created project row so platform and server stay aligned.
    if let Err(err) = sync_tenant_upsert(&state, &project.id, &project.name, true).await {
        let _ = state.store.delete_project(&org_id, &project.id).await;
        return Err(err);
    }

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
    let previous = project.clone();
    let name_changed = req.name.is_some();

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

    if name_changed {
        if let Err(err) = sync_tenant_upsert(&state, &project.id, &project.name, true).await {
            return Err(err);
        }
    }

    if let Err(err) = state.store.save_project(&project).await {
        if name_changed {
            let _ = sync_tenant_upsert(&state, &previous.id, &previous.name, true).await;
        }
        return Err(PlatformError::Internal(err));
    }

    Ok(Json(ProjectResponse::from(&project)))
}

/// DELETE /api/v1/orgs/:oid/projects/:pid — delete project (admin+)
pub async fn delete_project(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id)): Path<(String, String)>,
) -> ApiResult<axum::http::StatusCode> {
    load_org_and_check_role(&state, &org_id, &claims.sub, Role::Admin).await?;

    let project = state
        .store
        .get_project(&org_id, &project_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Project not found"))?;

    sync_tenant_delete(&state, &project.id).await?;

    let deleted = state.store.delete_project(&org_id, &project_id).await;

    let deleted = match deleted {
        Ok(deleted) => deleted,
        Err(err) => {
            let _ = sync_tenant_upsert(&state, &project.id, &project.name, true).await;
            return Err(PlatformError::Internal(err));
        }
    };

    if !deleted {
        let _ = sync_tenant_upsert(&state, &project.id, &project.name, true).await;
        return Err(PlatformError::not_found("Project not found"));
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ── Engine integration ────────────────────────────────────────────────────────

/// Register a new tenant in ordo-server when a project is created.
pub(crate) async fn register_tenant_in_engine(
    state: &AppState,
    tenant_id: &str,
    name: &str,
) -> ApiResult<()> {
    sync_tenant_upsert(state, tenant_id, name, true).await
}

pub(crate) async fn sync_tenant_upsert(
    state: &AppState,
    tenant_id: &str,
    name: &str,
    enabled: bool,
) -> ApiResult<()> {
    if let Some(publisher) = &state.sync_publisher {
        publisher
            .publish(SyncEvent::TenantUpsert {
                tenant_id: tenant_id.to_string(),
                name: name.to_string(),
                enabled,
            })
            .await
            .map_err(|e| {
                PlatformError::internal(format!(
                    "Failed to publish tenant registration to NATS: {}",
                    e
                ))
            })?;
        return Ok(());
    }

    let url = format!("{}/api/v1/tenants", state.config.engine_url);
    let body = serde_json::json!({
        "id": tenant_id,
        "name": name,
        "enabled": enabled
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

pub(crate) async fn sync_tenant_delete(state: &AppState, tenant_id: &str) -> ApiResult<()> {
    if let Some(publisher) = &state.sync_publisher {
        publisher
            .publish(SyncEvent::TenantDeleted {
                tenant_id: tenant_id.to_string(),
            })
            .await
            .map_err(|e| {
                PlatformError::internal(format!("Failed to publish tenant deletion to NATS: {}", e))
            })?;
        return Ok(());
    }

    let url = format!("{}/api/v1/tenants/{}", state.config.engine_url, tenant_id);
    match state.http_client.delete(&url).send().await {
        Ok(resp)
            if resp.status().is_success() || resp.status() == reqwest::StatusCode::NOT_FOUND =>
        {
            Ok(())
        }
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            Err(PlatformError::internal(format!(
                "Engine rejected tenant deletion: HTTP {} — {}",
                status, body
            )))
        }
        Err(e) => Err(PlatformError::internal(format!(
            "Could not reach engine to delete tenant '{}': {}",
            tenant_id, e
        ))),
    }
}

/// Push a RuleSet JSON blob to ordo-server for the given tenant.
///
/// Used by the templates API when cloning a template into a new project.
/// The caller is responsible for setting `config.tenant_id` in the payload
/// before calling — ordo-server validates that the body's tenant_id matches
/// the `X-Tenant-ID` header.
pub(crate) async fn push_ruleset_to_engine(
    state: &AppState,
    tenant_id: &str,
    ruleset: &serde_json::Value,
) -> ApiResult<()> {
    if let Some(publisher) = &state.sync_publisher {
        let name = ruleset
            .get("config")
            .and_then(|config| config.get("name"))
            .and_then(|name| name.as_str())
            .ok_or_else(|| PlatformError::bad_request("ruleset.config.name is required"))?;
        let version = ruleset
            .get("config")
            .and_then(|config| config.get("version"))
            .and_then(|version| version.as_str())
            .unwrap_or("0.0.0");

        publisher
            .publish(SyncEvent::RulePut {
                tenant_id: tenant_id.to_string(),
                name: name.to_string(),
                ruleset_json: serde_json::to_string(ruleset).map_err(|e| {
                    PlatformError::internal(format!("Failed to serialize ruleset for NATS: {}", e))
                })?,
                version: version.to_string(),
            })
            .await
            .map_err(|e| {
                PlatformError::internal(format!("Failed to publish ruleset update to NATS: {}", e))
            })?;
        return Ok(());
    }

    let url = format!("{}/api/v1/rulesets", state.config.engine_url);
    match state
        .http_client
        .post(&url)
        .header("X-Tenant-ID", tenant_id)
        .json(ruleset)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => Ok(()),
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            tracing::error!(
                "Engine rejected template ruleset for tenant {}: {} — {}",
                tenant_id,
                status,
                body
            );
            Err(PlatformError::internal(format!(
                "Engine rejected template ruleset: HTTP {}",
                status
            )))
        }
        Err(e) => {
            tracing::error!("Failed to push template ruleset to engine: {}", e);
            Err(PlatformError::internal(format!(
                "Could not reach engine to push template ruleset: {}",
                e
            )))
        }
    }
}
