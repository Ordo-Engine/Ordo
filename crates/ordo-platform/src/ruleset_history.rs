use crate::{
    error::{ApiResult, PlatformError},
    models::{Claims, Role, RulesetHistoryEntry, RulesetHistorySource},
    proxy::find_project_membership,
    AppState,
};
use axum::{
    extract::{Path, State},
    Extension, Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct AppendRulesetHistoryRequest {
    pub entries: Vec<AppendRulesetHistoryEntry>,
}

#[derive(Debug, Deserialize)]
pub struct AppendRulesetHistoryEntry {
    pub id: String,
    pub action: String,
    pub source: RulesetHistorySource,
    pub created_at: Option<DateTime<Utc>>,
    pub snapshot: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct RulesetHistoryResponse {
    pub ruleset_name: String,
    pub entries: Vec<RulesetHistoryEntry>,
}

/// GET /api/v1/projects/:pid/rulesets/:name/history — list persisted history (member)
pub async fn list_ruleset_history(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((project_id, ruleset_name)): Path<(String, String)>,
) -> ApiResult<Json<RulesetHistoryResponse>> {
    let (_role, org_id) = find_project_membership(&state, &project_id, &claims.sub).await?;

    let entries = state
        .store
        .get_ruleset_history(&org_id, &project_id, &ruleset_name)
        .await
        .map_err(PlatformError::Internal)?;

    Ok(Json(RulesetHistoryResponse {
        ruleset_name,
        entries,
    }))
}

/// POST /api/v1/projects/:pid/rulesets/:name/history — append persisted history entries (editor+)
pub async fn append_ruleset_history(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((project_id, ruleset_name)): Path<(String, String)>,
    Json(req): Json<AppendRulesetHistoryRequest>,
) -> ApiResult<Json<RulesetHistoryResponse>> {
    let (role, org_id) = find_project_membership(&state, &project_id, &claims.sub).await?;

    if role < Role::Editor {
        return Err(PlatformError::forbidden(
            "Editor role required to write ruleset history",
        ));
    }

    if req.entries.is_empty() {
        return Err(PlatformError::bad_request(
            "At least one history entry is required",
        ));
    }

    let user = state
        .store
        .get_user(&claims.sub)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("User not found"))?;

    let entries: Vec<RulesetHistoryEntry> = req
        .entries
        .into_iter()
        .map(|entry| RulesetHistoryEntry {
            id: entry.id,
            ruleset_name: ruleset_name.clone(),
            action: entry.action,
            source: entry.source,
            created_at: entry.created_at.unwrap_or_else(Utc::now),
            author_id: user.id.clone(),
            author_email: user.email.clone(),
            author_display_name: user.display_name.clone(),
            snapshot: entry.snapshot,
        })
        .collect();

    let history = state
        .store
        .append_ruleset_history(&org_id, &project_id, &ruleset_name, &entries)
        .await
        .map_err(PlatformError::Internal)?;

    Ok(Json(RulesetHistoryResponse {
        ruleset_name,
        entries: history,
    }))
}
