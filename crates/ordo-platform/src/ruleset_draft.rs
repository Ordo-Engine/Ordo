//! Draft ruleset CRUD, publish (NATS), and deployment history handlers.

use ordo_core::{
    context::Value as CoreValue,
    expr::{Expr, ExprParser},
    rule::{
        Action, ActionKind, Condition, ExecutionOptions, RuleExecutor, RuleSet, Step, StepKind,
    },
    trace::ExecutionTrace,
};
use ordo_protocol::{
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
    /// Studio-format ruleset (converted to engine format by ordo-protocol on the backend).
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
            .map_err(|e: ordo_protocol::ConvertError| {
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
            .map_err(|e: ordo_protocol::ConvertError| {
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

    // Inline referenced sub-rule assets, then convert studio → engine format.
    let inlined =
        inline_sub_rules_into_draft(&state, &org_id, &project_id, draft.draft.clone()).await?;
    let concepts = state
        .store
        .get_concepts(&org_id, &project_id)
        .await
        .map_err(PlatformError::Internal)?;
    let engine_json = studio_draft_to_engine_json_with_concepts(&inlined, &concepts)?;

    // Publish via NATS
    let publish_result = publish_via_nats(
        &state,
        &env,
        &project_id,
        &ruleset_name,
        &engine_json,
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

    // Redeploy uses the stored snapshot (already contains inlined sub-rules).
    let concepts = state
        .store
        .get_concepts(&org_id, &project_id)
        .await
        .map_err(PlatformError::Internal)?;
    let snapshot_engine_json =
        studio_draft_to_engine_json_with_concepts(&original.snapshot, &concepts)?;

    let result = publish_via_nats(
        &state,
        &env,
        &project_id,
        &ruleset_name,
        &snapshot_engine_json,
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

pub(crate) async fn inline_sub_rules_with_manifest(
    state: &AppState,
    org_id: &str,
    project_id: &str,
    draft: serde_json::Value,
) -> ApiResult<InlineSubRuleSnapshot> {
    use crate::models::SubRuleScope;
    use ordo_protocol::types::{
        ruleset::StudioSubRuleGraph,
        step::{StudioStep, StudioStepKind},
    };

    const MAX_DEPTH: usize = 8;
    const MAX_REFS: usize = 64;

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

const CONCEPT_PRELUDE_STEP_ID: &str = "__ordo_concepts_prelude";

fn normalize_concept_ref(path: &str) -> String {
    path.strip_prefix("$.")
        .or_else(|| path.strip_prefix('$'))
        .unwrap_or(path)
        .to_string()
}

fn scan_expression_refs(expression: &str, refs: &mut std::collections::HashSet<String>) {
    let mut chars = expression.char_indices().peekable();
    let mut quote: Option<char> = None;

    while let Some((idx, ch)) = chars.next() {
        if let Some(q) = quote {
            if ch == '\\' {
                chars.next();
            } else if ch == q {
                quote = None;
            }
            continue;
        }

        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            continue;
        }

        if !(ch.is_ascii_alphabetic() || ch == '_' || ch == '$') {
            continue;
        }

        let mut end = idx + ch.len_utf8();
        while let Some((next_idx, next)) = chars.peek().copied() {
            if next.is_ascii_alphanumeric() || next == '_' || next == '.' || next == '$' {
                end = next_idx + next.len_utf8();
                chars.next();
            } else {
                break;
            }
        }

        let token = &expression[idx..end];
        let normalized = normalize_concept_ref(token);
        let next_non_ws = expression[end..].chars().find(|c| !c.is_whitespace());
        if matches!(
            normalized.as_str(),
            "true" | "false" | "null" | "undefined" | "and" | "or" | "not" | "in"
        ) || next_non_ws == Some('(')
        {
            continue;
        }
        refs.insert(normalized);
    }
}

fn collect_expr_refs(expr: &Expr, refs: &mut std::collections::HashSet<String>) {
    match expr {
        Expr::Field(path) => {
            refs.insert(normalize_concept_ref(path));
        }
        Expr::Binary { left, right, .. } => {
            collect_expr_refs(left, refs);
            collect_expr_refs(right, refs);
        }
        Expr::Unary { operand, .. } => collect_expr_refs(operand, refs),
        Expr::Call { args, .. } | Expr::Array(args) | Expr::Coalesce(args) => {
            for arg in args {
                collect_expr_refs(arg, refs);
            }
        }
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr_refs(condition, refs);
            collect_expr_refs(then_branch, refs);
            collect_expr_refs(else_branch, refs);
        }
        Expr::Object(entries) => {
            for (_, value) in entries {
                collect_expr_refs(value, refs);
            }
        }
        Expr::Literal(_) | Expr::Exists(_) => {}
    }
}

fn collect_condition_refs(condition: &Condition, refs: &mut std::collections::HashSet<String>) {
    match condition {
        Condition::Always => {}
        Condition::Expression(expr) => collect_expr_refs(expr, refs),
        Condition::ExpressionString(expression) => match ExprParser::parse(expression) {
            Ok(expr) => collect_expr_refs(&expr, refs),
            Err(_) => scan_expression_refs(expression, refs),
        },
    }
}

fn collect_action_refs(action: &Action, refs: &mut std::collections::HashSet<String>) {
    match &action.kind {
        ActionKind::SetVariable { value, .. } | ActionKind::Metric { value, .. } => {
            collect_expr_refs(value, refs);
        }
        ActionKind::CallRuleSet { input_mapping, .. } => {
            if let Some(expr) = input_mapping {
                collect_expr_refs(expr, refs);
            }
        }
        ActionKind::ExternalCall { params, .. } => {
            for (_, expr) in params {
                collect_expr_refs(expr, refs);
            }
        }
        ActionKind::Log { .. } => {}
    }
}

fn collect_step_refs(
    steps: &hashbrown::HashMap<String, Step>,
) -> std::collections::HashSet<String> {
    let mut refs = std::collections::HashSet::new();

    for step in steps.values() {
        match &step.kind {
            StepKind::Decision { branches, .. } => {
                for branch in branches {
                    collect_condition_refs(&branch.condition, &mut refs);
                    for action in &branch.actions {
                        collect_action_refs(action, &mut refs);
                    }
                }
            }
            StepKind::Action { actions, .. } => {
                for action in actions {
                    collect_action_refs(action, &mut refs);
                }
            }
            StepKind::Terminal { result } => {
                for (_, expr) in &result.output {
                    collect_expr_refs(expr, &mut refs);
                }
            }
            StepKind::SubRule { bindings, .. } => {
                for (_, expr) in bindings {
                    collect_expr_refs(expr, &mut refs);
                }
            }
        }
    }

    refs
}

fn rewrite_expression_string_concept_refs(
    expression: &str,
    concept_names: &std::collections::HashSet<String>,
) -> String {
    let mut output = String::with_capacity(expression.len());
    let mut chars = expression.char_indices().peekable();
    let mut quote: Option<char> = None;

    while let Some((idx, ch)) = chars.next() {
        if let Some(q) = quote {
            output.push(ch);
            if ch == '\\' {
                if let Some((_, escaped)) = chars.next() {
                    output.push(escaped);
                }
            } else if ch == q {
                quote = None;
            }
            continue;
        }

        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            output.push(ch);
            continue;
        }

        if !(ch.is_ascii_alphabetic() || ch == '_' || ch == '$') {
            output.push(ch);
            continue;
        }

        let mut end = idx + ch.len_utf8();
        while let Some((next_idx, next)) = chars.peek().copied() {
            if next.is_ascii_alphanumeric() || next == '_' || next == '.' || next == '$' {
                end = next_idx + next.len_utf8();
                chars.next();
            } else {
                break;
            }
        }

        let token = &expression[idx..end];
        let normalized = normalize_concept_ref(token);
        let next_non_ws = expression[end..].chars().find(|c| !c.is_whitespace());
        if concept_names.contains(&normalized)
            && token != format!("${}", normalized)
            && next_non_ws != Some('(')
        {
            output.push('$');
            output.push_str(&normalized);
        } else {
            output.push_str(token);
        }
    }

    output
}

fn rewrite_expr_concept_refs(expr: &mut Expr, concept_names: &std::collections::HashSet<String>) {
    match expr {
        Expr::Field(path) => {
            let normalized = normalize_concept_ref(path);
            if concept_names.contains(&normalized) && *path != format!("${}", normalized) {
                *path = format!("${}", normalized);
            }
        }
        Expr::Binary { left, right, .. } => {
            rewrite_expr_concept_refs(left, concept_names);
            rewrite_expr_concept_refs(right, concept_names);
        }
        Expr::Unary { operand, .. } => rewrite_expr_concept_refs(operand, concept_names),
        Expr::Call { args, .. } | Expr::Array(args) | Expr::Coalesce(args) => {
            for arg in args {
                rewrite_expr_concept_refs(arg, concept_names);
            }
        }
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => {
            rewrite_expr_concept_refs(condition, concept_names);
            rewrite_expr_concept_refs(then_branch, concept_names);
            rewrite_expr_concept_refs(else_branch, concept_names);
        }
        Expr::Object(entries) => {
            for (_, value) in entries {
                rewrite_expr_concept_refs(value, concept_names);
            }
        }
        Expr::Literal(_) | Expr::Exists(_) => {}
    }
}

fn rewrite_condition_concept_refs(
    condition: &mut Condition,
    concept_names: &std::collections::HashSet<String>,
) {
    match condition {
        Condition::Always => {}
        Condition::Expression(expr) => rewrite_expr_concept_refs(expr, concept_names),
        Condition::ExpressionString(expression) => {
            *expression = rewrite_expression_string_concept_refs(expression, concept_names);
        }
    }
}

fn rewrite_action_concept_refs(
    action: &mut Action,
    concept_names: &std::collections::HashSet<String>,
) {
    match &mut action.kind {
        ActionKind::SetVariable { value, .. } | ActionKind::Metric { value, .. } => {
            rewrite_expr_concept_refs(value, concept_names);
        }
        ActionKind::CallRuleSet { input_mapping, .. } => {
            if let Some(expr) = input_mapping {
                rewrite_expr_concept_refs(expr, concept_names);
            }
        }
        ActionKind::ExternalCall { params, .. } => {
            for (_, expr) in params {
                rewrite_expr_concept_refs(expr, concept_names);
            }
        }
        ActionKind::Log { .. } => {}
    }
}

fn rewrite_steps_concept_refs(
    steps: &mut hashbrown::HashMap<String, Step>,
    concept_names: &std::collections::HashSet<String>,
) {
    for step in steps.values_mut() {
        match &mut step.kind {
            StepKind::Decision { branches, .. } => {
                for branch in branches {
                    rewrite_condition_concept_refs(&mut branch.condition, concept_names);
                    for action in &mut branch.actions {
                        rewrite_action_concept_refs(action, concept_names);
                    }
                }
            }
            StepKind::Action { actions, .. } => {
                for action in actions {
                    rewrite_action_concept_refs(action, concept_names);
                }
            }
            StepKind::Terminal { result } => {
                for (_, expr) in &mut result.output {
                    rewrite_expr_concept_refs(expr, concept_names);
                }
            }
            StepKind::SubRule { bindings, .. } => {
                for (_, expr) in bindings {
                    rewrite_expr_concept_refs(expr, concept_names);
                }
            }
        }
    }
}

fn concept_expression_refs(concept: &ConceptDefinition) -> std::collections::HashSet<String> {
    let mut refs = concept
        .dependencies
        .iter()
        .map(|dep| normalize_concept_ref(dep))
        .collect::<std::collections::HashSet<_>>();
    match ExprParser::parse(&concept.expression) {
        Ok(expr) => collect_expr_refs(&expr, &mut refs),
        Err(_) => scan_expression_refs(&concept.expression, &mut refs),
    }
    refs
}

fn resolve_concept_order(
    roots: &std::collections::HashSet<String>,
    concepts: &[ConceptDefinition],
) -> ApiResult<Vec<ConceptDefinition>> {
    let by_name = concepts
        .iter()
        .cloned()
        .map(|concept| (concept.name.clone(), concept))
        .collect::<std::collections::HashMap<_, _>>();
    let mut order = Vec::new();
    let mut visiting = std::collections::HashSet::<String>::new();
    let mut visited = std::collections::HashSet::<String>::new();

    fn visit(
        name: &str,
        by_name: &std::collections::HashMap<String, ConceptDefinition>,
        visiting: &mut std::collections::HashSet<String>,
        visited: &mut std::collections::HashSet<String>,
        order: &mut Vec<ConceptDefinition>,
    ) -> ApiResult<()> {
        let Some(concept) = by_name.get(name) else {
            return Ok(());
        };
        if visited.contains(name) {
            return Ok(());
        }
        if !visiting.insert(name.to_string()) {
            return Err(PlatformError::bad_request(format!(
                "Concept dependency cycle detected at '{}'",
                name
            )));
        }
        for dep in concept_expression_refs(concept) {
            if by_name.contains_key(&dep) {
                visit(&dep, by_name, visiting, visited, order)?;
            }
        }
        visiting.remove(name);
        visited.insert(name.to_string());
        order.push(concept.clone());
        Ok(())
    }

    for root in roots {
        visit(root, &by_name, &mut visiting, &mut visited, &mut order)?;
    }

    Ok(order)
}

fn materialize_concepts_for_step_graph(
    entry_step: &mut String,
    steps: &mut hashbrown::HashMap<String, Step>,
    concepts: &[ConceptDefinition],
) -> ApiResult<()> {
    if concepts.is_empty() {
        return Ok(());
    }

    steps.remove(CONCEPT_PRELUDE_STEP_ID);
    if *entry_step == CONCEPT_PRELUDE_STEP_ID {
        *entry_step = steps.keys().next().cloned().unwrap_or_default();
    }

    let concept_names = concepts
        .iter()
        .map(|concept| concept.name.clone())
        .collect::<std::collections::HashSet<_>>();
    let roots = collect_step_refs(steps)
        .into_iter()
        .filter(|name| concept_names.contains(name))
        .collect::<std::collections::HashSet<_>>();
    let order = resolve_concept_order(&roots, concepts)?;

    rewrite_steps_concept_refs(steps, &concept_names);

    if order.is_empty() {
        return Ok(());
    }

    let mut actions = Vec::with_capacity(order.len());
    for concept in order {
        let mut expr = ExprParser::parse(&concept.expression).map_err(|e| {
            PlatformError::bad_request(format!(
                "Concept '{}' expression failed to parse: {}",
                concept.name, e
            ))
        })?;
        rewrite_expr_concept_refs(&mut expr, &concept_names);
        actions.push(Action::set_var(concept.name, expr));
    }

    let original_entry = entry_step.clone();
    steps.insert(
        CONCEPT_PRELUDE_STEP_ID.to_string(),
        Step::action(
            CONCEPT_PRELUDE_STEP_ID,
            "Compute Concepts",
            actions,
            original_entry,
        ),
    );
    *entry_step = CONCEPT_PRELUDE_STEP_ID.to_string();
    Ok(())
}

fn materialize_concepts_into_engine_ruleset(
    ruleset: &mut RuleSet,
    concepts: &[ConceptDefinition],
) -> ApiResult<()> {
    materialize_concepts_for_step_graph(
        &mut ruleset.config.entry_step,
        &mut ruleset.steps,
        concepts,
    )?;

    for graph in ruleset.sub_rules.values_mut() {
        materialize_concepts_for_step_graph(&mut graph.entry_step, &mut graph.steps, concepts)?;
    }

    Ok(())
}

pub(crate) fn studio_draft_to_engine_json_with_concepts(
    draft: &serde_json::Value,
    concepts: &[ConceptDefinition],
) -> ApiResult<serde_json::Value> {
    let mut studio: StudioRuleSet = serde_json::from_value(draft.clone()).map_err(|e| {
        PlatformError::bad_request(format!("Draft is not valid studio format: {}", e))
    })?;
    materialize_sub_rule_runtime(&mut studio);
    let mut engine: RuleSet = studio
        .try_into()
        .map_err(|e: ordo_protocol::ConvertError| {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::FactDataType;

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
