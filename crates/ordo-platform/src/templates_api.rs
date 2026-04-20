//! Template API handlers.

use crate::{
    error::{ApiResult, PlatformError},
    models::{Claims, Project, Role, TemplateDetail, TemplateMetadata},
    org::load_org_and_check_role,
    project::{register_project_tenant, ProjectResponse},
    ruleset_history::append_history_entry_for_actor,
    template::extract_locale,
    AppState,
};
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Extension, Json,
};
use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

fn locale_from_headers(headers: &HeaderMap) -> &'static str {
    let raw = headers
        .get(axum::http::header::ACCEPT_LANGUAGE)
        .and_then(|v| v.to_str().ok());
    extract_locale(raw)
}

/// GET /api/v1/templates — list all templates (any authenticated user).
pub async fn list_templates(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(_claims): Extension<Claims>,
) -> ApiResult<Json<Vec<TemplateMetadata>>> {
    let locale = locale_from_headers(&headers);
    Ok(Json(state.templates.list(locale)))
}

/// GET /api/v1/templates/:id — full template detail (any authenticated user).
pub async fn get_template(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(_claims): Extension<Claims>,
    Path(id): Path<String>,
) -> ApiResult<Json<TemplateDetail>> {
    let locale = locale_from_headers(&headers);
    state
        .templates
        .get(&id, locale)
        .map(Json)
        .ok_or_else(|| PlatformError::not_found("Template not found"))
}

// ── Shared Installation ──────────────────────────────────────────────────────

/// Creates a project and persists the template payload into platform storage.
pub async fn install_template_detail(
    state: &AppState,
    claims: &Claims,
    org_id: &str,
    project_name: &str,
    project_description: Option<&str>,
    tpl: TemplateDetail,
    source_label: &str,
) -> ApiResult<Json<ProjectResponse>> {
    let project = Project {
        id: Uuid::new_v4().to_string(),
        name: project_name.to_string(),
        description: project_description.map(str::to_string),
        org_id: org_id.to_string(),
        created_at: Utc::now(),
        created_by: claims.sub.clone(),
        server_id: None,
    };
    state
        .store
        .save_project(&project)
        .await
        .map_err(PlatformError::Internal)?;

    macro_rules! rollback {
        ($err:expr) => {{
            let _ = state.store.delete_project(org_id, &project.id).await;
            return Err(PlatformError::Internal($err));
        }};
    }

    if let Err(e) = state
        .store
        .save_facts(org_id, &project.id, &tpl.facts)
        .await
    {
        rollback!(e)
    }
    if let Err(e) = state
        .store
        .save_concepts(org_id, &project.id, &tpl.concepts)
        .await
    {
        rollback!(e)
    }

    let mut ruleset = tpl.ruleset.clone();
    if let Some(cfg) = ruleset.get_mut("config").and_then(|c| c.as_object_mut()) {
        cfg.insert(
            "tenant_id".to_string(),
            serde_json::Value::String(project.id.clone()),
        );
    }
    let ruleset_name = ruleset
        .get("config")
        .and_then(|c| c.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or(source_label)
        .to_string();

    if let Some(mut contract) = tpl.contract {
        contract.ruleset_name = ruleset_name.clone();
        contract.updated_at = Utc::now();
        if let Err(e) = state
            .store
            .save_contracts(org_id, &project.id, &[contract])
            .await
        {
            rollback!(e)
        }
    }

    if !tpl.tests.is_empty() {
        let now = Utc::now();
        let tests: Vec<crate::models::TestCase> = tpl
            .tests
            .into_iter()
            .map(|mut tc| {
                tc.id = Uuid::new_v4().to_string();
                tc.created_by = claims.sub.clone();
                tc.created_at = now;
                tc.updated_at = now;
                tc
            })
            .collect();
        if let Err(e) = state
            .store
            .save_tests(org_id, &project.id, &ruleset_name, &tests)
            .await
        {
            rollback!(e)
        }
    }

    if let Err(e) = state
        .store
        .save_draft_ruleset(
            &Uuid::new_v4().to_string(),
            &project.id,
            &ruleset_name,
            &ruleset,
            0,
            &claims.sub,
        )
        .await
    {
        rollback!(e)
    }

    if let Err(e) = append_history_entry_for_actor(
        state,
        org_id,
        &project.id,
        &ruleset_name,
        crate::models::RulesetHistorySource::Create,
        format!("Created from '{}'", source_label),
        ruleset.clone(),
        &claims.sub,
        &claims.email,
    )
    .await
    {
        let _ = state.store.delete_project(org_id, &project.id).await;
        return Err(e);
    }

    if let Err(e) = register_project_tenant(state, &project.id, &project.name).await {
        let _ = state.store.delete_project(org_id, &project.id).await;
        return Err(e);
    }

    Ok(Json(ProjectResponse::from(&project)))
}

// ── From Template ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateFromTemplateRequest {
    pub template_id: String,
    pub project_name: String,
    #[serde(default)]
    pub project_description: Option<String>,
}

/// POST /api/v1/orgs/:oid/projects/from-template
pub async fn create_from_template(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    headers: HeaderMap,
    Path(org_id): Path<String>,
    Json(req): Json<CreateFromTemplateRequest>,
) -> ApiResult<Json<ProjectResponse>> {
    load_org_and_check_role(&state, &org_id, &claims.sub, Role::Admin).await?;

    let name = req.project_name.trim();
    if name.is_empty() {
        return Err(PlatformError::bad_request("Project name is required"));
    }

    let locale = locale_from_headers(&headers);
    let tpl = state
        .templates
        .get(&req.template_id, locale)
        .ok_or_else(|| PlatformError::not_found("Template not found"))?;

    install_template_detail(
        &state,
        &claims,
        &org_id,
        name,
        req.project_description.as_deref(),
        tpl,
        &req.template_id,
    )
    .await
}
