//! Draft ruleset CRUD, publish (NATS), and deployment history handlers.

use ordo_core::{
    context::Value as CoreValue,
    rule::{ExecutionOptions, RuleExecutor, RuleSet},
    trace::ExecutionTrace,
};
use ordo_studio_format::{
    types::{
        condition::StudioCondition,
        expr::StudioExpr,
        ruleset::StudioSubRuleGraph,
        step::{
            StudioAssignment, StudioBranch, StudioOutputField, StudioStep, StudioStepKind,
            StudioSubRuleOutput,
        },
    },
    StudioRuleSet,
};

use crate::{
    error::{ApiResult, PlatformError},
    models::{
        Claims, ConceptDefinition, DeploymentStatus, DraftConflictResponse, ProjectRuleset,
        ProjectRulesetMeta, PublishRequest, RedeployRequest, ReleaseSubRuleDependency,
        RulesetDeployment, RulesetHistoryEntry, RulesetHistorySource, SaveDraftRequest,
    },
    rbac::{
        require_project_permission, PERM_DEPLOYMENT_REDEPLOY, PERM_DEPLOYMENT_VIEW,
        PERM_RULESET_EDIT, PERM_RULESET_PUBLISH, PERM_RULESET_VIEW,
    },
    release::hash_json_value,
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
    let ruleset_value = serde_json::to_value(&req.ruleset).map_err(|e| {
        DraftSaveResponse::Err(PlatformError::internal(format!(
            "Serialization error: {}",
            e
        )))
    })?;
    match state
        .store
        .save_draft_ruleset(
            &id,
            &project_id,
            &ruleset_name,
            &ruleset_value,
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
    /// Studio-format ruleset (converted to engine format by ordo-studio-format on the backend).
    pub ruleset: StudioRuleSet,
    pub input: serde_json::Value,
}

#[derive(serde::Deserialize)]
pub struct ConvertRulesetRequest {
    pub ruleset: StudioRuleSet,
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

    // Convert studio-format ruleset to engine format, then compile.
    let mut ruleset: RuleSet =
        req.ruleset
            .try_into()
            .map_err(|e: ordo_studio_format::ConvertError| {
                PlatformError::bad_request(format!("Ruleset conversion failed: {}", e))
            })?;
    let concepts = state
        .store
        .get_concepts(&org_id, &project_id)
        .await
        .map_err(PlatformError::Internal)?;
    materialize_concepts_into_engine_ruleset(&mut ruleset, &concepts)?;

    ruleset
        .compile()
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

    let trace = result
        .trace
        .as_ref()
        .map(|t: &ExecutionTrace| TraceResponseTrace {
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

/// POST /api/v1/orgs/:oid/projects/:pid/rulesets/:name/convert
///
/// Converts a studio-format ruleset to engine format without executing it.
pub async fn convert_draft_ruleset(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id, _ruleset_name)): Path<(String, String, String)>,
    Json(req): Json<ConvertRulesetRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    require_project_permission(&state, &org_id, &project_id, &claims.sub, PERM_RULESET_VIEW)
        .await?;

    let mut engine: RuleSet =
        req.ruleset
            .try_into()
            .map_err(|e: ordo_studio_format::ConvertError| {
                PlatformError::bad_request(format!("Ruleset conversion failed: {}", e))
            })?;
    let concepts = state
        .store
        .get_concepts(&org_id, &project_id)
        .await
        .map_err(PlatformError::Internal)?;
    materialize_concepts_into_engine_ruleset(&mut engine, &concepts)?;

    let engine_json = serde_json::to_value(&engine)
        .map_err(|e| PlatformError::internal(format!("Engine serialization failed: {}", e)))?;

    Ok(Json(engine_json))
}

// ── Publish ───────────────────────────────────────────────────────────────────

/// POST /api/v1/orgs/:oid/projects/:pid/rulesets/:name/publish
///
/// Publishing is **asynchronous** and runs through the release engine: it synthesizes
/// an auto-approved, all-at-once internal release targeting every server bound to the
/// environment, hands it to the `ordo-platform-worker`, and returns a `Dispatched`
/// deployment immediately. The worker pushes the rule over NATS, waits for server
/// acks, and flips the deployment to `Success`/`Failed` — clients poll
/// `list_ruleset_deployments` for the final outcome. (Publishing therefore requires
/// the worker to be running.)
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

    let draft = state
        .store
        .get_draft_ruleset(&project_id, &ruleset_name)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Draft ruleset not found"))?;

    let env = state
        .store
        .get_environment(&project_id, &req.environment_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Environment not found"))?;

    let version = draft
        .draft
        .get("config")
        .and_then(|c| c.get("version"))
        .and_then(|v| v.as_str())
        .unwrap_or("0.0.0")
        .to_string();

    // Create the deployment row up front as `Dispatched`; the worker running the
    // synthesized release flips it to `Success`/`Failed`.
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
        status: DeploymentStatus::Dispatched,
    };
    state
        .store
        .create_deployment(&deployment)
        .await
        .map_err(PlatformError::Internal)?;

    // Inline sub-rules + enqueue the release. If either fails, the `Dispatched`
    // deployment created above would otherwise orphan in "published, unconfirmed"
    // forever, so settle it to `Failed` before returning the error.
    let enqueue = async {
        let inlined =
            inline_sub_rules_into_draft(&state, &org_id, &project_id, draft.draft.clone()).await?;
        enqueue_publish_release(
            &state,
            &org_id,
            &project_id,
            &ruleset_name,
            &version,
            inlined,
            &dep_id,
            &env,
            &claims,
            req.release_note.clone(),
        )
        .await
    }
    .await;
    if let Err(e) = enqueue {
        let _ = state
            .store
            .update_deployment_status(&dep_id, DeploymentStatus::Failed)
            .await;
        return Err(e);
    }

    let deployment = state
        .store
        .get_deployment(&project_id, &dep_id)
        .await
        .map_err(PlatformError::Internal)?
        .expect("just created");

    Ok(Json(deployment))
}

/// Roll a freshly-published draft out through the release engine: synthesize an
/// auto-approved, all-at-once internal release targeting every server bound to the
/// environment and enqueue it for the worker, which flips `deployment_id` (created
/// `Dispatched`) to `Success`/`Failed`.
///
/// When the environment has no bound servers there is nothing for the worker to target
/// (the release engine requires ≥1 server), so we broadcast the rule directly over
/// NATS and record the publish here — it stays `Dispatched`, as there is no server to
/// confirm against.
#[allow(clippy::too_many_arguments)]
async fn enqueue_publish_release(
    state: &AppState,
    org_id: &str,
    project_id: &str,
    ruleset_name: &str,
    version: &str,
    inlined_studio: serde_json::Value,
    deployment_id: &str,
    env: &crate::models::ProjectEnvironment,
    claims: &Claims,
    release_note: Option<String>,
) -> ApiResult<()> {
    if state.sync_publisher.is_none() {
        return Err(PlatformError::internal(
            "NATS is not configured; cannot publish",
        ));
    }

    // No bound servers → broadcast directly and record the publish (stays Dispatched);
    // there is no target for a release execution and no ack to wait on.
    if env.server_ids.is_empty() {
        let concepts = state
            .store
            .get_concepts(org_id, project_id)
            .await
            .map_err(PlatformError::Internal)?;
        let engine_json = studio_draft_to_engine_json_with_concepts(&inlined_studio, &concepts)?;
        publish_via_nats(state, env, project_id, ruleset_name, &engine_json, version)
            .await
            .map_err(|e| PlatformError::internal(format!("NATS publish failed: {e}")))?;

        state
            .store
            .mark_ruleset_published(project_id, ruleset_name, version)
            .await
            .map_err(PlatformError::Internal)?;
        if let Some(user) = state
            .store
            .get_user(&claims.sub)
            .await
            .map_err(PlatformError::Internal)?
        {
            let entry = RulesetHistoryEntry {
                id: Uuid::new_v4().to_string(),
                ruleset_name: ruleset_name.to_string(),
                action: format!("published to {}", env.name),
                source: RulesetHistorySource::Publish,
                created_at: Utc::now(),
                author_id: claims.sub.clone(),
                author_email: user.email,
                author_display_name: user.display_name,
                snapshot: inlined_studio,
            };
            let _ = state
                .store
                .append_ruleset_history(org_id, project_id, ruleset_name, &[entry])
                .await;
        }
        return Ok(());
    }

    let mut bound_servers = Vec::with_capacity(env.server_ids.len());
    for server_id in &env.server_ids {
        let server = state
            .store
            .get_server(server_id)
            .await
            .map_err(PlatformError::Internal)?
            .ok_or_else(|| PlatformError::not_found("Bound server not found"))?;
        bound_servers.push(server);
    }

    let request = build_internal_publish_request(
        org_id,
        project_id,
        ruleset_name,
        version,
        inlined_studio,
        deployment_id,
        env,
        claims,
        release_note,
    );
    state
        .store
        .create_internal_release_request(&request, None)
        .await
        .map_err(PlatformError::Internal)?;

    let user = state
        .store
        .get_user(&claims.sub)
        .await
        .map_err(PlatformError::Internal)?;
    let actor = crate::release::user_history_actor(claims, user.as_ref());
    crate::release::enqueue_release_execution(
        state,
        &request,
        env,
        &bound_servers,
        &actor,
        &claims.sub,
    )
    .await
    .map_err(PlatformError::Internal)?;

    Ok(())
}

/// Build the auto-approved, all-at-once internal `ReleaseRequest` that backs a direct
/// publish. `publish_deployment_id` links it to the deployment row the worker settles;
/// `target_ruleset_snapshot` carries the inlined studio draft the worker pushes.
#[allow(clippy::too_many_arguments)]
fn build_internal_publish_request(
    org_id: &str,
    project_id: &str,
    ruleset_name: &str,
    version: &str,
    inlined_studio: serde_json::Value,
    deployment_id: &str,
    env: &crate::models::ProjectEnvironment,
    claims: &Claims,
    release_note: Option<String>,
) -> crate::models::ReleaseRequest {
    use crate::models::{
        ReleaseContentDiffSummary, ReleaseRequest, ReleaseRequestSnapshot, ReleaseRequestStatus,
        ReleaseVersionDiff, RollbackPolicy, RolloutStrategy, RolloutStrategyKind,
    };

    let strategy = RolloutStrategy {
        kind: Some(RolloutStrategyKind::AllAtOnce),
        ..Default::default()
    };
    let affected = env.server_ids.len() as i32;
    let now = Utc::now();

    ReleaseRequest {
        id: Uuid::new_v4().to_string(),
        org_id: org_id.to_string(),
        project_id: project_id.to_string(),
        ruleset_name: ruleset_name.to_string(),
        version: version.to_string(),
        environment_id: env.id.clone(),
        environment_name: Some(env.name.clone()),
        policy_id: None,
        // Born already executing — direct publish is an ungoverned fast path; the
        // worker drives it to Completed/Failed. (Governance is intentionally skipped;
        // the call is still gated by PERM_RULESET_PUBLISH.)
        status: ReleaseRequestStatus::Executing,
        title: format!("Publish {ruleset_name} {version}"),
        change_summary: "Direct publish".to_string(),
        release_note: release_note.clone(),
        affected_instance_count: affected,
        rollout_strategy: strategy.clone(),
        rollback_version: None,
        created_by: claims.sub.clone(),
        created_by_name: None,
        created_by_email: None,
        created_at: now,
        updated_at: now,
        version_diff: ReleaseVersionDiff {
            to_version: version.to_string(),
            changed: true,
            ..Default::default()
        },
        content_diff: ReleaseContentDiffSummary::default(),
        request_snapshot: ReleaseRequestSnapshot {
            requester_id: claims.sub.clone(),
            environment_name: Some(env.name.clone()),
            rollout_strategy: strategy,
            rollback_policy: RollbackPolicy {
                auto_rollback: false,
                ..Default::default()
            },
            affected_instance_count: affected,
            target_ruleset_snapshot: Some(inlined_studio),
            publish_deployment_id: Some(deployment_id.to_string()),
            ..Default::default()
        },
        execution_attempts: 0,
        max_execution_attempts: 1,
        is_closed: false,
        approvals: Vec::new(),
    }
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
        release_note: req.release_note.clone(),
        snapshot: original.snapshot.clone(),
        deployed_at: Utc::now(),
        deployed_by: Some(claims.sub.clone()),
        status: DeploymentStatus::Dispatched,
    };
    state
        .store
        .create_deployment(&deployment)
        .await
        .map_err(PlatformError::Internal)?;

    // The stored deployment snapshot already contains inlined sub-rules — roll it out
    // through the release engine, exactly like a fresh publish (async, worker-settled).
    // On enqueue failure, settle the `Dispatched` row to `Failed` so it doesn't orphan.
    if let Err(e) = enqueue_publish_release(
        &state,
        &org_id,
        &project_id,
        &ruleset_name,
        &original.version,
        original.snapshot.clone(),
        &dep_id,
        &env,
        &claims,
        req.release_note.clone(),
    )
    .await
    {
        let _ = state
            .store
            .update_deployment_status(&dep_id, DeploymentStatus::Failed)
            .await;
        return Err(e);
    }

    let deployment = state
        .store
        .get_deployment(&project_id, &dep_id)
        .await
        .map_err(PlatformError::Internal)?
        .expect("just created");

    Ok(Json(deployment))
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Scan a studio-format ruleset draft, fetch all referenced sub-rule assets from
/// the platform store, and inline them as `subRules` entries.  The result is a
/// self-contained studio JSON that `studio_draft_to_engine_json` can convert
/// without any missing sub-rule references.
///
/// Sub-sub-rules are resolved iteratively up to MAX_SUBRULE_INLINE_DEPTH levels.
pub(crate) struct InlineSubRuleSnapshot {
    pub draft: serde_json::Value,
    pub dependencies: Vec<ReleaseSubRuleDependency>,
}

/// A draft is in engine format (vs studio format) when its `steps` is a map keyed
/// by step id rather than an array. Drafts seeded from a template are stored this
/// way; studio-saved drafts use an array. Used to route conversion correctly.
pub(crate) fn draft_is_engine_format(draft: &serde_json::Value) -> bool {
    draft.get("steps").map(|s| s.is_object()).unwrap_or(false)
}

pub(crate) async fn inline_sub_rules_with_manifest(
    state: &AppState,
    org_id: &str,
    project_id: &str,
    draft: serde_json::Value,
) -> ApiResult<InlineSubRuleSnapshot> {
    use crate::models::SubRuleScope;
    use ordo_studio_format::types::{
        ruleset::StudioSubRuleGraph,
        step::{StudioStep, StudioStepKind},
    };

    const MAX_DEPTH: usize = 8;
    const MAX_REFS: usize = 64;

    // Engine-format drafts (e.g. seeded from a template — steps as a map) have no
    // studio `subRules` graph to inline; pass them through untouched.
    if draft_is_engine_format(&draft) {
        return Ok(InlineSubRuleSnapshot {
            draft,
            dependencies: Vec::new(),
        });
    }

    let mut ruleset: StudioRuleSet = serde_json::from_value(draft).map_err(|e| {
        PlatformError::bad_request(format!("Draft is not valid studio format: {}", e))
    })?;

    // Collect sub-rule refNames from a step list.
    fn collect_refs(steps: &[StudioStep]) -> Vec<String> {
        steps
            .iter()
            .filter_map(|s| {
                if let StudioStepKind::SubRule { ref_name, .. } = &s.kind {
                    Some(ref_name.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    let mut queue: Vec<(String, usize)> = collect_refs(&ruleset.steps)
        .into_iter()
        .map(|n| (n, 0))
        .collect();
    let mut visited = std::collections::HashSet::new();
    let mut dependencies = Vec::new();

    while let Some((ref_name, depth)) = queue.pop() {
        if depth >= MAX_DEPTH
            || visited.contains(&ref_name)
            || ruleset.sub_rules.contains_key(&ref_name)
        {
            continue;
        }
        if visited.len() >= MAX_REFS {
            return Err(PlatformError::bad_request(
                "Too many sub-rule references (max 64)",
            ));
        }
        visited.insert(ref_name.clone());

        // Try project scope first, then org scope.
        let (asset, scope) = match state
            .store
            .get_sub_rule_asset(org_id, SubRuleScope::Project, Some(project_id), &ref_name)
            .await
            .map_err(PlatformError::Internal)?
        {
            Some(asset) => (asset, SubRuleScope::Project),
            None => (
                state
                    .store
                    .get_sub_rule_asset(org_id, SubRuleScope::Org, None, &ref_name)
                    .await
                    .map_err(PlatformError::Internal)?
                    .ok_or_else(|| {
                        PlatformError::bad_request(format!("Sub-rule '{}' not found", ref_name))
                    })?,
                SubRuleScope::Org,
            ),
        };

        let asset_draft = asset.draft.clone();
        let sub: StudioRuleSet = serde_json::from_value(asset_draft.clone()).map_err(|e| {
            PlatformError::bad_request(format!("Sub-rule '{}' has invalid draft: {}", ref_name, e))
        })?;

        dependencies.push(ReleaseSubRuleDependency {
            name: ref_name.clone(),
            display_name: asset.meta.display_name.clone(),
            scope,
            asset_id: asset.meta.id.clone(),
            draft_seq: asset.meta.draft_seq,
            content_hash: hash_json_value(&asset_draft),
        });

        // Enqueue nested references.
        for nested in collect_refs(&sub.steps) {
            queue.push((nested, depth + 1));
        }

        ruleset.sub_rules.insert(
            ref_name,
            StudioSubRuleGraph {
                entry_step: sub.start_step_id,
                steps: sub.steps,
                input_schema: None,
                output_schema: None,
            },
        );
    }

    let draft = serde_json::to_value(&ruleset)
        .map_err(|e| PlatformError::internal(format!("Serialization failed: {}", e)))?;
    dependencies.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(InlineSubRuleSnapshot {
        draft,
        dependencies,
    })
}

pub(crate) async fn inline_sub_rules_into_draft(
    state: &AppState,
    org_id: &str,
    project_id: &str,
    draft: serde_json::Value,
) -> ApiResult<serde_json::Value> {
    Ok(
        inline_sub_rules_with_manifest(state, org_id, project_id, draft)
            .await?
            .draft,
    )
}

fn safe_runtime_name(value: &str) -> String {
    let cleaned = value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();
    if cleaned.is_empty() {
        "value".to_string()
    } else {
        cleaned
    }
}

fn literal_string_expr(value: impl Into<String>) -> StudioExpr {
    StudioExpr::Literal {
        value: serde_json::Value::String(value.into()),
        value_type: Some("string".to_string()),
    }
}

fn variable_expr(path: impl Into<String>) -> StudioExpr {
    StudioExpr::Variable { path: path.into() }
}

struct TerminalReturnBridge {
    child_steps: Vec<StudioStep>,
    parent_steps: Vec<StudioStep>,
    next_step_id: String,
    outputs: Vec<StudioSubRuleOutput>,
}

fn create_terminal_return_bridge(
    sub_rule_step_id: &str,
    source_steps: &[StudioStep],
) -> Option<TerminalReturnBridge> {
    let terminals: Vec<StudioStep> = source_steps
        .iter()
        .filter(|step| matches!(step.kind, StudioStepKind::Terminal { .. }))
        .cloned()
        .collect();
    if terminals.is_empty() {
        return None;
    }

    let prefix = format!("__ordo_sub_{}", safe_runtime_name(sub_rule_step_id));
    let terminal_id_var = format!("{}_terminal_id", prefix);
    let return_step_id = format!("{}__return_to_parent", sub_rule_step_id);
    let mut outputs = Vec::new();
    let mut output_var_by_terminal = std::collections::HashMap::<String, Vec<String>>::new();

    let mut child_steps = Vec::with_capacity(source_steps.len() + 1);
    for step in source_steps.iter().cloned() {
        if let StudioStepKind::Terminal { output, .. } = &step.kind {
            let mut assignments = Vec::new();
            if terminals.len() > 1 {
                assignments.push(StudioAssignment {
                    name: terminal_id_var.clone(),
                    value: literal_string_expr(step.id.clone()),
                });
                outputs.push(StudioSubRuleOutput {
                    parent_var: terminal_id_var.clone(),
                    child_var: terminal_id_var.clone(),
                });
            }

            let mut output_vars = Vec::new();
            for (index, field) in output.iter().enumerate() {
                let output_var = format!(
                    "{}_{}_{}_{}",
                    prefix,
                    safe_runtime_name(&step.id),
                    safe_runtime_name(&field.name),
                    index
                );
                output_vars.push(output_var.clone());
                outputs.push(StudioSubRuleOutput {
                    parent_var: output_var.clone(),
                    child_var: output_var.clone(),
                });
                assignments.push(StudioAssignment {
                    name: output_var,
                    value: field.value.clone(),
                });
            }
            output_var_by_terminal.insert(step.id.clone(), output_vars);

            child_steps.push(StudioStep {
                id: step.id,
                name: step.name,
                description: step.description,
                position: step.position,
                system_generated: Some("sub_rule_runtime".to_string()),
                kind: StudioStepKind::Action {
                    assignments,
                    external_calls: Vec::new(),
                    logging: None,
                    next_step_id: return_step_id.clone(),
                },
            });
        } else {
            child_steps.push(step);
        }
    }

    child_steps.push(StudioStep {
        id: return_step_id.clone(),
        name: "Return to parent".to_string(),
        description: None,
        position: None,
        system_generated: Some("sub_rule_runtime".to_string()),
        kind: StudioStepKind::Terminal {
            code: "OK".to_string(),
            message: None,
            output: Vec::new(),
        },
    });

    let parent_terminals = terminals
        .iter()
        .map(|terminal| {
            let StudioStepKind::Terminal {
                code,
                message,
                output,
            } = &terminal.kind
            else {
                unreachable!();
            };
            let output_vars = output_var_by_terminal
                .get(&terminal.id)
                .cloned()
                .unwrap_or_default();
            StudioStep {
                id: format!(
                    "{}__terminal_{}",
                    sub_rule_step_id,
                    safe_runtime_name(&terminal.id)
                ),
                name: terminal.name.clone(),
                description: terminal.description.clone(),
                position: None,
                system_generated: Some("sub_rule_runtime".to_string()),
                kind: StudioStepKind::Terminal {
                    code: code.clone(),
                    message: message.clone(),
                    output: output
                        .iter()
                        .enumerate()
                        .map(|(index, field)| StudioOutputField {
                            name: field.name.clone(),
                            value: variable_expr(format!("${}", output_vars[index])),
                        })
                        .collect(),
                },
            }
        })
        .collect::<Vec<_>>();

    if parent_terminals.len() == 1 {
        return Some(TerminalReturnBridge {
            child_steps,
            parent_steps: parent_terminals.clone(),
            next_step_id: parent_terminals[0].id.clone(),
            outputs,
        });
    }

    let dispatcher_id = format!("{}__return_dispatch", sub_rule_step_id);
    let dispatcher = StudioStep {
        id: dispatcher_id.clone(),
        name: "Sub-rule return dispatch".to_string(),
        description: None,
        position: None,
        system_generated: Some("sub_rule_runtime".to_string()),
        kind: StudioStepKind::Decision {
            branches: terminals
                .iter()
                .skip(1)
                .enumerate()
                .map(|(index, terminal)| StudioBranch {
                    id: format!("{}_b_{}", dispatcher_id, index),
                    label: Some(match &terminal.kind {
                        StudioStepKind::Terminal { code, .. } => code.clone(),
                        _ => String::new(),
                    }),
                    condition: StudioCondition::Simple {
                        left: variable_expr(format!("${}", terminal_id_var)),
                        operator: "eq".to_string(),
                        right: literal_string_expr(terminal.id.clone()),
                    },
                    next_step_id: format!(
                        "{}__terminal_{}",
                        sub_rule_step_id,
                        safe_runtime_name(&terminal.id)
                    ),
                })
                .collect(),
            default_next_step_id: Some(parent_terminals[0].id.clone()),
        },
    };

    Some(TerminalReturnBridge {
        child_steps,
        parent_steps: std::iter::once(dispatcher)
            .chain(parent_terminals)
            .collect(),
        next_step_id: dispatcher_id,
        outputs,
    })
}

fn merge_sub_rule_outputs(
    existing: Vec<StudioSubRuleOutput>,
    generated: Vec<StudioSubRuleOutput>,
) -> Vec<StudioSubRuleOutput> {
    let mut merged = existing;
    for output in generated {
        if !merged
            .iter()
            .any(|item| item.parent_var == output.parent_var && item.child_var == output.child_var)
        {
            merged.push(output);
        }
    }
    merged
}

fn materialize_terminal_propagation_for_steps(
    steps: &mut Vec<StudioStep>,
    sub_rules: &mut hashbrown::HashMap<String, StudioSubRuleGraph>,
) {
    let mut step_ids = steps
        .iter()
        .map(|step| step.id.clone())
        .collect::<std::collections::HashSet<_>>();
    let mut appended_steps = Vec::new();

    for step in steps.iter_mut() {
        let StudioStepKind::SubRule {
            ref mut ref_name,
            bindings: _,
            ref mut outputs,
            ref mut return_policy,
            ref mut next_step_id,
        } = step.kind
        else {
            continue;
        };

        let should_propagate_terminal =
            matches!(return_policy.as_deref(), Some("propagate_terminal"))
                || next_step_id.is_empty()
                || !step_ids.contains(next_step_id);
        if !should_propagate_terminal {
            continue;
        }

        let Some(graph) = sub_rules.get(ref_name).cloned() else {
            continue;
        };
        if !graph
            .steps
            .iter()
            .any(|child| matches!(child.kind, StudioStepKind::Terminal { .. }))
        {
            continue;
        }

        let Some(bridge) = create_terminal_return_bridge(&step.id, &graph.steps) else {
            continue;
        };

        let bridged_ref_name = format!(
            "{}__{}_terminal_return",
            ref_name,
            safe_runtime_name(&step.id)
        );
        sub_rules.insert(
            bridged_ref_name.clone(),
            StudioSubRuleGraph {
                entry_step: graph.entry_step,
                steps: bridge.child_steps,
                input_schema: graph.input_schema,
                output_schema: graph.output_schema,
            },
        );

        for parent_step in &bridge.parent_steps {
            step_ids.insert(parent_step.id.clone());
        }
        appended_steps.extend(bridge.parent_steps);
        *ref_name = bridged_ref_name;
        *return_policy = Some("continue".to_string());
        *next_step_id = bridge.next_step_id;
        *outputs = merge_sub_rule_outputs(outputs.clone(), bridge.outputs);
    }

    if !appended_steps.is_empty() {
        steps.extend(appended_steps);
    }
}

fn materialize_sub_rule_runtime(draft: &mut StudioRuleSet) {
    const MAX_DEPTH: usize = 8;

    for _ in 0..MAX_DEPTH {
        let before = draft.sub_rules.len();
        materialize_terminal_propagation_for_steps(&mut draft.steps, &mut draft.sub_rules);

        let names = draft.sub_rules.keys().cloned().collect::<Vec<_>>();
        for name in names {
            if let Some(mut graph) = draft.sub_rules.remove(&name) {
                materialize_terminal_propagation_for_steps(&mut graph.steps, &mut draft.sub_rules);
                draft.sub_rules.insert(name, graph);
            }
        }

        if before == draft.sub_rules.len() {
            break;
        }
    }
}

/// Inline concepts into an engine ruleset. The materialization logic is the
/// single source of truth in `ordo_studio_format::concepts` (shared with the
/// CLI); here we only map the DB concept model to that crate's authoring shape.
fn materialize_concepts_into_engine_ruleset(
    ruleset: &mut RuleSet,
    concepts: &[ConceptDefinition],
) -> ApiResult<()> {
    let shared: Vec<ordo_studio_format::ConceptDefinition> = concepts
        .iter()
        .map(|c| ordo_studio_format::ConceptDefinition {
            name: c.name.clone(),
            expression: c.expression.clone(),
            dependencies: c.dependencies.clone(),
            data_type: None,
            description: None,
        })
        .collect();
    ordo_studio_format::materialize_concepts(ruleset, &shared)
        .map_err(|e| PlatformError::bad_request(e.to_string()))
}

pub(crate) fn studio_draft_to_engine_json_with_concepts(
    draft: &serde_json::Value,
    concepts: &[ConceptDefinition],
) -> ApiResult<serde_json::Value> {
    // Drafts seeded from a template are stored in engine format (steps as a map),
    // while studio-saved drafts are studio format (steps as an array). If the draft
    // is already engine format, parse it directly instead of through the studio
    // converter (which would fail with "expected a sequence").
    if draft_is_engine_format(draft) {
        let mut engine: RuleSet = serde_json::from_value(draft.clone()).map_err(|e| {
            PlatformError::bad_request(format!("Draft is not valid engine format: {}", e))
        })?;
        materialize_concepts_into_engine_ruleset(&mut engine, concepts)?;
        return serde_json::to_value(&engine)
            .map_err(|e| PlatformError::internal(format!("Engine serialization failed: {}", e)));
    }

    let mut studio: StudioRuleSet = serde_json::from_value(draft.clone()).map_err(|e| {
        PlatformError::bad_request(format!("Draft is not valid studio format: {}", e))
    })?;
    materialize_sub_rule_runtime(&mut studio);
    let mut engine: RuleSet =
        studio
            .try_into()
            .map_err(|e: ordo_studio_format::ConvertError| {
                PlatformError::bad_request(format!("Draft conversion failed: {}", e))
            })?;
    materialize_concepts_into_engine_ruleset(&mut engine, concepts)?;
    serde_json::to_value(&engine)
        .map_err(|e| PlatformError::internal(format!("Engine serialization failed: {}", e)))
}

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

    let prefix = env
        .nats_subject_prefix
        .as_deref()
        .unwrap_or(&state.config.nats_subject_prefix);

    publisher
        .publish_rule_put(
            prefix,
            project_id,
            ruleset_name,
            json_str,
            version,
            None,
            None,
        )
        .await
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::FactDataType;
    use ordo_studio_format::CONCEPT_PRELUDE_STEP_ID;

    #[test]
    fn studio_conversion_materializes_referenced_concepts() {
        let draft = serde_json::json!({
            "config": {
                "name": "coupon",
                "version": "1.0.0"
            },
            "startStepId": "start",
            "steps": [
                {
                    "id": "start",
                    "name": "Check",
                    "type": "decision",
                    "branches": [
                        {
                            "id": "b1",
                            "condition": {
                                "type": "expression",
                                "expression": "$.vip_eligible == true"
                            },
                            "nextStepId": "ok"
                        }
                    ],
                    "defaultNextStepId": "no"
                },
                {
                    "id": "ok",
                    "name": "OK",
                    "type": "terminal",
                    "code": "OK",
                    "message": {
                        "type": "literal",
                        "value": "ok",
                        "valueType": "string"
                    },
                    "output": []
                },
                {
                    "id": "no",
                    "name": "NO",
                    "type": "terminal",
                    "code": "NO",
                    "message": {
                        "type": "literal",
                        "value": "no",
                        "valueType": "string"
                    },
                    "output": []
                }
            ]
        });
        let concepts = vec![ConceptDefinition {
            name: "vip_eligible".to_string(),
            data_type: FactDataType::Boolean,
            expression: "user_vip_level >= 2 && cart_amount >= 200".to_string(),
            dependencies: vec![],
            description: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }];

        let engine = studio_draft_to_engine_json_with_concepts(&draft, &concepts).unwrap();

        assert_eq!(
            engine["config"]["entry_step"].as_str(),
            Some(CONCEPT_PRELUDE_STEP_ID)
        );
        assert_eq!(
            engine["steps"][CONCEPT_PRELUDE_STEP_ID]["next_step"].as_str(),
            Some("start")
        );
        assert_eq!(
            engine["steps"][CONCEPT_PRELUDE_STEP_ID]["actions"][0]["name"].as_str(),
            Some("vip_eligible")
        );
        assert_eq!(
            engine["steps"]["start"]["branches"][0]["condition"].as_str(),
            Some("$vip_eligible == true")
        );
    }
}
