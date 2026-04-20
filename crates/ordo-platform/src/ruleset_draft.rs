//! Draft ruleset CRUD, publish (NATS), and deployment history handlers.

use ordo_core::{
    context::Value as CoreValue,
    rule::{ExecutionOptions, RuleExecutor, RuleSet},
    trace::ExecutionTrace,
};

use crate::{
    error::{ApiResult, PlatformError},
    models::{
        Claims, DeploymentStatus, DraftConflictResponse, ProjectRuleset, ProjectRulesetMeta,
        PublishRequest, RedeployRequest, RulesetDeployment, RulesetHistoryEntry,
        RulesetHistorySource, SaveDraftRequest,
    },
    rbac::{
        require_project_permission, PERM_DEPLOYMENT_REDEPLOY, PERM_DEPLOYMENT_VIEW,
        PERM_RULESET_EDIT, PERM_RULESET_PUBLISH, PERM_RULESET_VIEW,
    },
    sync::SyncEvent,
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

// ── Draft CRUD ────────────────────────────────────────────────────────────────

/// GET /api/v1/orgs/:oid/projects/:pid/rulesets
pub async fn list_drafts(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id)): Path<(String, String)>,
) -> ApiResult<Json<Vec<ProjectRulesetMeta>>> {
    require_project_permission(&state, &org_id, &project_id, &claims.sub, PERM_RULESET_VIEW)
        .await?;
    let metas = state
        .store
        .list_draft_rulesets(&project_id)
        .await
        .map_err(PlatformError::Internal)?;
    Ok(Json(metas))
}

/// GET /api/v1/orgs/:oid/projects/:pid/rulesets/:name
pub async fn get_draft(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id, ruleset_name)): Path<(String, String, String)>,
) -> ApiResult<Json<ProjectRuleset>> {
    require_project_permission(&state, &org_id, &project_id, &claims.sub, PERM_RULESET_VIEW)
        .await?;

    if let Some(draft) = state
        .store
        .get_draft_ruleset(&project_id, &ruleset_name)
        .await
        .map_err(PlatformError::Internal)?
    {
        return Ok(Json(draft));
    }

    let seeded = seed_draft_from_history(&state, &project_id, &ruleset_name, &claims.sub)
        .await
        .map_err(PlatformError::Internal)?;

    seeded
        .map(Json)
        .ok_or_else(|| PlatformError::not_found("Ruleset not found"))
}

/// PUT /api/v1/orgs/:oid/projects/:pid/rulesets/:name
///
/// Returns 409 on optimistic-lock conflict (client must merge manually).
pub async fn save_draft(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id, ruleset_name)): Path<(String, String, String)>,
    Json(req): Json<SaveDraftRequest>,
) -> Result<Json<ProjectRuleset>, DraftSaveResponse> {
    require_project_permission(&state, &org_id, &project_id, &claims.sub, PERM_RULESET_EDIT)
        .await
        .map_err(DraftSaveResponse::Err)?;

    let id = Uuid::new_v4().to_string();
    match state
        .store
        .save_draft_ruleset(
            &id,
            &project_id,
            &ruleset_name,
            &req.ruleset,
            req.expected_seq,
            &claims.sub,
        )
        .await
    {
        Ok(draft) => Ok(Json(draft)),
        Err(e) if e.to_string() == "conflict" => {
            // Return 409 with the current server draft so the client can diff
            let current = state
                .store
                .get_draft_ruleset(&project_id, &ruleset_name)
                .await
                .ok()
                .flatten();
            let (server_draft, server_seq) = if let Some(ref d) = current {
                (d.draft.clone(), d.meta.draft_seq)
            } else {
                (serde_json::Value::Null, 0)
            };
            Err(DraftSaveResponse::Conflict(Json(DraftConflictResponse {
                conflict: true,
                server_draft,
                server_seq,
            })))
        }
        Err(e) => Err(DraftSaveResponse::Err(PlatformError::Internal(e))),
    }
}

/// DELETE /api/v1/orgs/:oid/projects/:pid/rulesets/:name
pub async fn delete_draft(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id, ruleset_name)): Path<(String, String, String)>,
) -> ApiResult<StatusCode> {
    require_project_permission(&state, &org_id, &project_id, &claims.sub, PERM_RULESET_EDIT)
        .await?;
    state
        .store
        .delete_draft_ruleset(&project_id, &ruleset_name)
        .await
        .map_err(PlatformError::Internal)?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Inline trace execution ────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct TraceRequest {
    /// Engine-format ruleset (pre-converted by frontend adapter from editor format).
    pub ruleset: serde_json::Value,
    pub input: serde_json::Value,
}

#[derive(serde::Serialize)]
pub struct TraceResponse {
    pub code: String,
    pub message: String,
    pub output: serde_json::Value,
    pub duration_us: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<TraceResponseTrace>,
}

#[derive(serde::Serialize)]
pub struct TraceResponseTrace {
    pub path: String,
    pub steps: Vec<TraceResponseStep>,
}

#[derive(serde::Serialize)]
pub struct TraceResponseStep {
    pub id: String,
    pub name: String,
    pub duration_us: u64,
}

/// POST /api/v1/orgs/:oid/projects/:pid/rulesets/:name/trace
///
/// Executes the ruleset inline on the platform. The request body must include a
/// `ruleset` field already in engine format (steps as map, config.entry_step set),
/// produced by the frontend adapter (`convertToEngineFormat`).
pub async fn trace_draft(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id, _ruleset_name)): Path<(String, String, String)>,
    Json(req): Json<TraceRequest>,
) -> ApiResult<Json<TraceResponse>> {
    require_project_permission(&state, &org_id, &project_id, &claims.sub, PERM_RULESET_VIEW)
        .await?;

    // Use the engine-format ruleset provided by the frontend (already converted from editor format).
    let ruleset_json = serde_json::to_string(&req.ruleset)
        .map_err(|e| PlatformError::internal(format!("Failed to serialize ruleset: {}", e)))?;

    let ruleset = RuleSet::from_json_compiled(&ruleset_json)
        .map_err(|e| PlatformError::bad_request(format!("Failed to compile ruleset: {}", e)))?;

    // Convert input to ordo-core Value
    let input: CoreValue = serde_json::from_value(req.input)
        .map_err(|e| PlatformError::bad_request(format!("Invalid input: {}", e)))?;

    // Execute with trace enabled
    let executor = RuleExecutor::new();
    let options = ExecutionOptions::default().trace(true);
    let result = executor
        .execute_with_options(&ruleset, input, Some(&options))
        .map_err(|e| PlatformError::internal(format!("Execution failed: {}", e)))?;

    // Convert output
    let output: serde_json::Value = serde_json::to_value(&result.output)
        .unwrap_or(serde_json::Value::Object(Default::default()));

    let trace = result.trace.as_ref().map(|t: &ExecutionTrace| TraceResponseTrace {
        path: t.path_string(),
        steps: t
            .steps
            .iter()
            .map(|s| TraceResponseStep {
                id: s.step_id.clone(),
                name: s.step_name.clone(),
                duration_us: s.duration_us,
            })
            .collect(),
    });

    Ok(Json(TraceResponse {
        code: result.code,
        message: result.message,
        output,
        duration_us: result.duration_us,
        trace,
    }))
}

// ── Publish ───────────────────────────────────────────────────────────────────

/// POST /api/v1/orgs/:oid/projects/:pid/rulesets/:name/publish
pub async fn publish_draft(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id, ruleset_name)): Path<(String, String, String)>,
    Json(req): Json<PublishRequest>,
) -> ApiResult<Json<RulesetDeployment>> {
    require_project_permission(
        &state,
        &org_id,
        &project_id,
        &claims.sub,
        PERM_RULESET_PUBLISH,
    )
    .await?;

    // Load draft
    let draft = state
        .store
        .get_draft_ruleset(&project_id, &ruleset_name)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Draft ruleset not found"))?;

    // Load target environment
    let env = state
        .store
        .get_environment(&project_id, &req.environment_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Environment not found"))?;

    // Extract version from draft
    let version = draft
        .draft
        .get("config")
        .and_then(|c| c.get("version"))
        .and_then(|v| v.as_str())
        .unwrap_or("0.0.0")
        .to_string();

    // Create deployment record (queued)
    let dep_id = Uuid::new_v4().to_string();
    let deployment = RulesetDeployment {
        id: dep_id.clone(),
        project_id: project_id.clone(),
        environment_id: env.id.clone(),
        environment_name: Some(env.name.clone()),
        ruleset_name: ruleset_name.clone(),
        version: version.clone(),
        release_note: req.release_note.clone(),
        snapshot: draft.draft.clone(),
        deployed_at: Utc::now(),
        deployed_by: Some(claims.sub.clone()),
        status: DeploymentStatus::Queued,
    };
    state
        .store
        .create_deployment(&deployment)
        .await
        .map_err(PlatformError::Internal)?;

    // Publish via NATS
    let publish_result = publish_via_nats(
        &state,
        &env,
        &project_id,
        &ruleset_name,
        &draft.draft,
        &version,
    )
    .await;

    let final_status = match publish_result {
        Ok(()) => DeploymentStatus::Success,
        Err(ref e) => {
            tracing::error!("NATS publish failed for deployment {}: {}", dep_id, e);
            DeploymentStatus::Failed
        }
    };

    state
        .store
        .update_deployment_status(&dep_id, final_status.clone())
        .await
        .map_err(PlatformError::Internal)?;

    if let DeploymentStatus::Success = final_status {
        // Update draft's published metadata
        state
            .store
            .mark_ruleset_published(&project_id, &ruleset_name, &version)
            .await
            .map_err(PlatformError::Internal)?;

        // Append history entry
        let user = state
            .store
            .get_user(&claims.sub)
            .await
            .map_err(PlatformError::Internal)?;
        if let Some(user) = user {
            let entry = RulesetHistoryEntry {
                id: Uuid::new_v4().to_string(),
                ruleset_name: ruleset_name.clone(),
                action: format!("published to {}", env.name),
                source: RulesetHistorySource::Publish,
                created_at: Utc::now(),
                author_id: claims.sub.clone(),
                author_email: user.email,
                author_display_name: user.display_name,
                snapshot: draft.draft.clone(),
            };
            let _ = state
                .store
                .append_ruleset_history(&org_id, &project_id, &ruleset_name, &[entry])
                .await;
        }
    }

    let deployment = state
        .store
        .get_deployment(&project_id, &dep_id)
        .await
        .map_err(PlatformError::Internal)?
        .expect("just created");

    if let DeploymentStatus::Failed = deployment.status {
        return Err(PlatformError::internal(
            "Publish queued but NATS push failed; deployment marked as failed",
        ));
    }

    Ok(Json(deployment))
}

// ── Deployment History ────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct DeploymentListQuery {
    pub limit: Option<i64>,
}

/// GET /api/v1/orgs/:oid/projects/:pid/rulesets/:name/deployments
pub async fn list_ruleset_deployments(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id, ruleset_name)): Path<(String, String, String)>,
    Query(q): Query<DeploymentListQuery>,
) -> ApiResult<Json<Vec<RulesetDeployment>>> {
    require_project_permission(
        &state,
        &org_id,
        &project_id,
        &claims.sub,
        PERM_DEPLOYMENT_VIEW,
    )
    .await?;
    let deployments = state
        .store
        .list_deployments(&project_id, Some(&ruleset_name), q.limit.unwrap_or(50))
        .await
        .map_err(PlatformError::Internal)?;
    Ok(Json(deployments))
}

/// GET /api/v1/orgs/:oid/projects/:pid/deployments
pub async fn list_project_deployments(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id)): Path<(String, String)>,
    Query(q): Query<DeploymentListQuery>,
) -> ApiResult<Json<Vec<RulesetDeployment>>> {
    require_project_permission(
        &state,
        &org_id,
        &project_id,
        &claims.sub,
        PERM_DEPLOYMENT_VIEW,
    )
    .await?;
    let deployments = state
        .store
        .list_deployments(&project_id, None, q.limit.unwrap_or(100))
        .await
        .map_err(PlatformError::Internal)?;
    Ok(Json(deployments))
}

/// POST /api/v1/orgs/:oid/projects/:pid/rulesets/:name/deployments/:did/redeploy
pub async fn redeploy(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id, ruleset_name, deployment_id)): Path<(String, String, String, String)>,
    Json(req): Json<RedeployRequest>,
) -> ApiResult<Json<RulesetDeployment>> {
    require_project_permission(
        &state,
        &org_id,
        &project_id,
        &claims.sub,
        PERM_DEPLOYMENT_REDEPLOY,
    )
    .await?;

    // Load original deployment snapshot
    let original = state
        .store
        .get_deployment(&project_id, &deployment_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Deployment not found"))?;

    let env = state
        .store
        .get_environment(&project_id, &req.environment_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Environment not found"))?;

    let dep_id = Uuid::new_v4().to_string();
    let deployment = RulesetDeployment {
        id: dep_id.clone(),
        project_id: project_id.clone(),
        environment_id: env.id.clone(),
        environment_name: Some(env.name.clone()),
        ruleset_name: ruleset_name.clone(),
        version: original.version.clone(),
        release_note: req.release_note,
        snapshot: original.snapshot.clone(),
        deployed_at: Utc::now(),
        deployed_by: Some(claims.sub.clone()),
        status: DeploymentStatus::Queued,
    };
    state
        .store
        .create_deployment(&deployment)
        .await
        .map_err(PlatformError::Internal)?;

    let result = publish_via_nats(
        &state,
        &env,
        &project_id,
        &ruleset_name,
        &original.snapshot,
        &original.version,
    )
    .await;

    let final_status = if result.is_ok() {
        DeploymentStatus::Success
    } else {
        DeploymentStatus::Failed
    };

    state
        .store
        .update_deployment_status(&dep_id, final_status)
        .await
        .map_err(PlatformError::Internal)?;

    let deployment = state
        .store
        .get_deployment(&project_id, &dep_id)
        .await
        .map_err(PlatformError::Internal)?
        .expect("just created");

    Ok(Json(deployment))
}

// ── Internal helpers ──────────────────────────────────────────────────────────

async fn publish_via_nats(
    state: &AppState,
    env: &crate::models::ProjectEnvironment,
    project_id: &str,
    ruleset_name: &str,
    ruleset_json: &serde_json::Value,
    version: &str,
) -> anyhow::Result<()> {
    let publisher = state
        .sync_publisher
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("NATS not configured"))?;

    let json_str = serde_json::to_string(ruleset_json)?;

    let event = SyncEvent::RulePut {
        tenant_id: project_id.to_string(),
        name: ruleset_name.to_string(),
        ruleset_json: json_str,
        version: version.to_string(),
        release_execution_id: None,
        target_server_ids: None,
    };

    let prefix = env
        .nats_subject_prefix
        .as_deref()
        .unwrap_or(&state.config.nats_subject_prefix);

    publisher.publish_to(prefix, event).await
}

async fn seed_draft_from_history(
    state: &AppState,
    project_id: &str,
    ruleset_name: &str,
    user_id: &str,
) -> anyhow::Result<Option<ProjectRuleset>> {
    let Some((ruleset_json, _created_at, author_id)) = state
        .store
        .get_latest_ruleset_history_snapshot(project_id, ruleset_name)
        .await?
    else {
        return Ok(None);
    };
    let id = Uuid::new_v4().to_string();
    let draft = state
        .store
        .save_draft_ruleset(
            &id,
            project_id,
            ruleset_name,
            &ruleset_json,
            0,
            author_id.as_deref().unwrap_or(user_id),
        )
        .await?;

    Ok(Some(draft))
}

// ── Custom response type for conflict handling ────────────────────────────────

pub enum DraftSaveResponse {
    Err(PlatformError),
    Conflict(Json<DraftConflictResponse>),
}

impl axum::response::IntoResponse for DraftSaveResponse {
    fn into_response(self) -> axum::response::Response {
        match self {
            DraftSaveResponse::Err(e) => e.into_response(),
            DraftSaveResponse::Conflict(body) => (StatusCode::CONFLICT, body).into_response(),
        }
    }
}
