//! Managed SubRule asset API.
//!
//! Sub-rules are standalone decision graphs referenced by parent rulesets.
//! Their content is snapshotted inline when the parent ruleset is published —
//! no separate sub-rule publish/version flow is needed.

use crate::{
    error::{ApiResult, PlatformError},
    models::{Claims, SaveSubRuleAssetRequest, SubRuleAsset, SubRuleAssetMeta, SubRuleScope},
    rbac::{require_permission, require_project_permission, PERM_RULESET_EDIT, PERM_RULESET_VIEW},
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use uuid::Uuid;

#[derive(Debug, serde::Deserialize)]
pub struct ListProjectSubRulesQuery {
    #[serde(default = "default_include_org")]
    pub include_org: bool,
}

fn default_include_org() -> bool {
    true
}

/// GET /api/v1/orgs/:oid/sub-rules
pub async fn list_org_sub_rules(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
) -> ApiResult<Json<Vec<SubRuleAssetMeta>>> {
    require_permission(&state, &org_id, &claims.sub, PERM_RULESET_VIEW).await?;
    let assets = state
        .store
        .list_org_sub_rules(&org_id)
        .await
        .map_err(PlatformError::Internal)?;
    Ok(Json(assets))
}

/// PUT /api/v1/orgs/:oid/sub-rules/:name
pub async fn save_org_sub_rule(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, name)): Path<(String, String)>,
    Json(req): Json<SaveSubRuleAssetRequest>,
) -> ApiResult<Json<SubRuleAsset>> {
    require_permission(&state, &org_id, &claims.sub, PERM_RULESET_EDIT).await?;
    let name = normalize_name(&name, &req.name)?;
    let asset = state
        .store
        .upsert_sub_rule_asset(
            &Uuid::new_v4().to_string(),
            &org_id,
            None,
            SubRuleScope::Org,
            &name,
            req.display_name.as_deref(),
            req.description.as_deref(),
            &req.draft,
            &req.input_schema,
            &req.output_schema,
            req.expected_seq,
            &claims.sub,
        )
        .await
        .map_err(map_conflict)?;
    Ok(Json(asset))
}

/// GET /api/v1/orgs/:oid/sub-rules/:name
pub async fn get_org_sub_rule(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, name)): Path<(String, String)>,
) -> ApiResult<Json<SubRuleAsset>> {
    require_permission(&state, &org_id, &claims.sub, PERM_RULESET_VIEW).await?;
    let asset = state
        .store
        .get_sub_rule_asset(&org_id, SubRuleScope::Org, None, &name)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("SubRule not found"))?;
    Ok(Json(asset))
}

/// DELETE /api/v1/orgs/:oid/sub-rules/:name
pub async fn delete_org_sub_rule(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, name)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    require_permission(&state, &org_id, &claims.sub, PERM_RULESET_EDIT).await?;
    let deleted = state
        .store
        .delete_sub_rule_asset(&org_id, SubRuleScope::Org, None, &name)
        .await
        .map_err(PlatformError::Internal)?;
    if !deleted {
        return Err(PlatformError::not_found("SubRule not found"));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/v1/orgs/:oid/projects/:pid/sub-rules
pub async fn list_project_sub_rules(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id)): Path<(String, String)>,
    Query(q): Query<ListProjectSubRulesQuery>,
) -> ApiResult<Json<Vec<SubRuleAssetMeta>>> {
    require_project_permission(&state, &org_id, &project_id, &claims.sub, PERM_RULESET_VIEW)
        .await?;
    let mut assets = state
        .store
        .list_project_sub_rules(&org_id, &project_id)
        .await
        .map_err(PlatformError::Internal)?;
    if !q.include_org {
        assets.retain(|a| a.scope == SubRuleScope::Project);
    }
    Ok(Json(assets))
}

/// PUT /api/v1/orgs/:oid/projects/:pid/sub-rules/:name
pub async fn save_project_sub_rule(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id, name)): Path<(String, String, String)>,
    Json(req): Json<SaveSubRuleAssetRequest>,
) -> ApiResult<Json<SubRuleAsset>> {
    require_project_permission(&state, &org_id, &project_id, &claims.sub, PERM_RULESET_EDIT)
        .await?;
    let name = normalize_name(&name, &req.name)?;
    let asset = state
        .store
        .upsert_sub_rule_asset(
            &Uuid::new_v4().to_string(),
            &org_id,
            Some(&project_id),
            SubRuleScope::Project,
            &name,
            req.display_name.as_deref(),
            req.description.as_deref(),
            &req.draft,
            &req.input_schema,
            &req.output_schema,
            req.expected_seq,
            &claims.sub,
        )
        .await
        .map_err(map_conflict)?;
    Ok(Json(asset))
}

/// GET /api/v1/orgs/:oid/projects/:pid/sub-rules/:name
pub async fn get_project_sub_rule(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id, name)): Path<(String, String, String)>,
) -> ApiResult<Json<SubRuleAsset>> {
    require_project_permission(&state, &org_id, &project_id, &claims.sub, PERM_RULESET_VIEW)
        .await?;
    let asset = state
        .store
        .get_sub_rule_asset(&org_id, SubRuleScope::Project, Some(&project_id), &name)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("SubRule not found"))?;
    Ok(Json(asset))
}

/// DELETE /api/v1/orgs/:oid/projects/:pid/sub-rules/:name
pub async fn delete_project_sub_rule(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id, name)): Path<(String, String, String)>,
) -> ApiResult<StatusCode> {
    require_project_permission(&state, &org_id, &project_id, &claims.sub, PERM_RULESET_EDIT)
        .await?;
    let deleted = state
        .store
        .delete_sub_rule_asset(&org_id, SubRuleScope::Project, Some(&project_id), &name)
        .await
        .map_err(PlatformError::Internal)?;
    if !deleted {
        return Err(PlatformError::not_found("SubRule not found"));
    }
    Ok(StatusCode::NO_CONTENT)
}

fn normalize_name(path_name: &str, body_name: &str) -> ApiResult<String> {
    let path_name = path_name.trim();
    let body_name = body_name.trim();
    if path_name.is_empty() || body_name.is_empty() {
        return Err(PlatformError::bad_request("SubRule name is required"));
    }
    if path_name != body_name {
        return Err(PlatformError::bad_request(
            "SubRule path name and body name must match",
        ));
    }
    Ok(path_name.to_string())
}

fn map_conflict(err: anyhow::Error) -> PlatformError {
    let message = err.to_string();
    if message == "conflict" {
        PlatformError::conflict("SubRule draft has changed")
    } else {
        PlatformError::Internal(err)
    }
}
