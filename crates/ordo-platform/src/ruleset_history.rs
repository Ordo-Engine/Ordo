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
use uuid::Uuid;

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

pub(crate) async fn append_history_entry_for_actor(
    state: &AppState,
    org_id: &str,
    project_id: &str,
    ruleset_name: &str,
    source: RulesetHistorySource,
    action: impl Into<String>,
    snapshot: serde_json::Value,
    author_id: &str,
    author_email: &str,
) -> ApiResult<()> {
    let display_name = state
        .store
        .get_user(author_id)
        .await
        .map_err(PlatformError::Internal)?
        .map(|user| user.display_name)
        .unwrap_or_else(|| author_email.to_string());

    let entry = RulesetHistoryEntry {
        id: Uuid::new_v4().to_string(),
        ruleset_name: ruleset_name.to_string(),
        action: action.into(),
        source,
        created_at: Utc::now(),
        author_id: author_id.to_string(),
        author_email: author_email.to_string(),
        author_display_name: display_name,
        snapshot,
    };

    state
        .store
        .append_ruleset_history(org_id, project_id, ruleset_name, &[entry])
        .await
        .map_err(PlatformError::Internal)?;

    Ok(())
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
