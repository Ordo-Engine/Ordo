//! Project environment CRUD handlers.
//!
//! Each environment binds a project to an ordo-server and optionally configures
//! a canary traffic split.

use crate::{
    error::{ApiResult, PlatformError},
    models::{
        Claims, CreateEnvironmentRequest, ProjectEnvironment, SetCanaryRequest,
        UpdateEnvironmentRequest,
    },
    rbac::{
        require_project_permission, PERM_CANARY_MANAGE, PERM_ENVIRONMENT_MANAGE,
        PERM_ENVIRONMENT_VIEW,
    },
    AppState,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use uuid::Uuid;

// ── Handlers ──────────────────────────────────────────────────────────────────

/// GET /api/v1/orgs/:oid/projects/:pid/environments
pub async fn list_environments(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id)): Path<(String, String)>,
) -> ApiResult<Json<Vec<ProjectEnvironment>>> {
    require_project_permission(
        &state,
        &org_id,
        &project_id,
        &claims.sub,
        PERM_ENVIRONMENT_VIEW,
    )
    .await?;
    let envs = state
        .store
        .list_environments(&project_id)
        .await
        .map_err(PlatformError::Internal)?;
    Ok(Json(envs))
}

/// POST /api/v1/orgs/:oid/projects/:pid/environments
pub async fn create_environment(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id)): Path<(String, String)>,
    Json(req): Json<CreateEnvironmentRequest>,
) -> ApiResult<Json<ProjectEnvironment>> {
    require_project_permission(
        &state,
        &org_id,
        &project_id,
        &claims.sub,
        PERM_ENVIRONMENT_MANAGE,
    )
    .await?;

    if req.name.trim().is_empty() {
        return Err(PlatformError::bad_request("Environment name is required"));
    }

    let id = Uuid::new_v4().to_string();
    let env = state
        .store
        .create_environment(&id, &project_id, &req, false)
        .await
        .map_err(PlatformError::Internal)?;
    Ok(Json(env))
}

/// PUT /api/v1/orgs/:oid/projects/:pid/environments/:eid
pub async fn update_environment(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id, env_id)): Path<(String, String, String)>,
    Json(req): Json<UpdateEnvironmentRequest>,
) -> ApiResult<Json<ProjectEnvironment>> {
    require_project_permission(
        &state,
        &org_id,
        &project_id,
        &claims.sub,
        PERM_ENVIRONMENT_MANAGE,
    )
    .await?;

    let updated = state
        .store
        .update_environment(&project_id, &env_id, &req)
        .await
        .map_err(PlatformError::Internal)?;

    if !updated {
        return Err(PlatformError::not_found("Environment not found"));
    }

    let env = state
        .store
        .get_environment(&project_id, &env_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Environment not found"))?;
    Ok(Json(env))
}

/// DELETE /api/v1/orgs/:oid/projects/:pid/environments/:eid
pub async fn delete_environment(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id, env_id)): Path<(String, String, String)>,
) -> ApiResult<StatusCode> {
    require_project_permission(
        &state,
        &org_id,
        &project_id,
        &claims.sub,
        PERM_ENVIRONMENT_MANAGE,
    )
    .await?;

    let deleted = state
        .store
        .delete_environment(&project_id, &env_id)
        .await
        .map_err(PlatformError::Internal)?;

    if !deleted {
        return Err(PlatformError::bad_request(
            "Cannot delete default environment or environment not found",
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// PUT /api/v1/orgs/:oid/projects/:pid/environments/:eid/canary
pub async fn set_canary(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id, env_id)): Path<(String, String, String)>,
    Json(req): Json<SetCanaryRequest>,
) -> ApiResult<Json<ProjectEnvironment>> {
    require_project_permission(
        &state,
        &org_id,
        &project_id,
        &claims.sub,
        PERM_CANARY_MANAGE,
    )
    .await?;

    if !(0..=100).contains(&req.canary_percentage) {
        return Err(PlatformError::bad_request(
            "canary_percentage must be between 0 and 100",
        ));
    }

    state
        .store
        .set_canary(
            &project_id,
            &env_id,
            req.canary_target_env_id.as_deref(),
            req.canary_percentage,
        )
        .await
        .map_err(PlatformError::Internal)?;

    let env = state
        .store
        .get_environment(&project_id, &env_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Environment not found"))?;
    Ok(Json(env))
}
