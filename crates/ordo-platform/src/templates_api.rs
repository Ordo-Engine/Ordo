//! Template API handlers (M1.1).
//!
//! Three endpoints exposed to Studio:
//!
//! - `GET  /api/v1/templates`                              — list metadata
//! - `GET  /api/v1/templates/:id`                          — full detail
//! - `POST /api/v1/orgs/:oid/projects/from-template`       — clone template → new project
//!
//! The locale is derived from the request's `Accept-Language` header; see
//! [`crate::template::extract_locale`].

use crate::{
    error::{ApiResult, PlatformError},
    models::{Claims, Project, Role, TemplateDetail, TemplateMetadata, TestCase},
    org::load_org_and_check_role,
    project::{push_ruleset_to_engine, register_tenant_in_engine, ProjectResponse},
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

// ── from-template: clone → project ──────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateFromTemplateRequest {
    pub template_id: String,
    pub project_name: String,
    #[serde(default)]
    pub project_description: Option<String>,
}

/// POST /api/v1/orgs/:oid/projects/from-template
///
/// Flow (best-effort, matches existing engine-integration semantics):
///   1. Verify admin on target org
///   2. Load raw (un-localised) template
///   3. Create the Project in the platform store
///   4. Register tenant in the engine
///   5. Persist facts / concepts to the platform store
///   6. Rewrite ruleset `config.tenant_id` to the new project id + push to engine
///
/// If step 5 or 6 fails, the project row is left behind (500 returned) — the
/// user can retry or delete it from Studio. See plan `docs/M1.1-DOGFOODING.md`.
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

    // Resolve the template in the user's locale so the cloned project has
    // localized step names, fact descriptions, etc. baked in. The engine
    // stores the ruleset as-is — this means the user sees their preferred
    // language in Studio immediately after cloning.
    let locale = locale_from_headers(&headers);
    let tpl = state
        .templates
        .get(&req.template_id, locale)
        .ok_or_else(|| PlatformError::not_found("Template not found"))?;

    // 1. Create project row
    let project = Project {
        id: Uuid::new_v4().to_string(),
        name: name.to_string(),
        description: req.project_description.clone(),
        org_id: org_id.clone(),
        created_at: Utc::now(),
        created_by: claims.sub.clone(),
    };
    state
        .store
        .save_project(&project)
        .await
        .map_err(PlatformError::Internal)?;

    // 2. Register the new tenant in the engine
    register_tenant_in_engine(&state, &project.id, &project.name).await?;

    // 3. Persist facts & concepts
    state
        .store
        .save_facts(&org_id, &project.id, &tpl.facts)
        .await
        .map_err(PlatformError::Internal)?;

    state
        .store
        .save_concepts(&org_id, &project.id, &tpl.concepts)
        .await
        .map_err(PlatformError::Internal)?;

    // 4. Rewrite tenant_id in the ruleset JSON and push to engine
    let mut ruleset = tpl.ruleset.clone();
    if let Some(config) = ruleset.get_mut("config").and_then(|c| c.as_object_mut()) {
        config.insert(
            "tenant_id".to_string(),
            serde_json::Value::String(project.id.clone()),
        );
    }
    push_ruleset_to_engine(&state, &project.id, &ruleset).await?;

    // 5. Save the decision contract (best-effort — template may not include one)
    let ruleset_name = ruleset
        .get("config")
        .and_then(|c| c.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or(&req.template_id)
        .to_string();

    if let Some(mut contract) = tpl.contract {
        contract.ruleset_name = ruleset_name.clone();
        contract.updated_at = chrono::Utc::now();
        state
            .store
            .save_contracts(&org_id, &project.id, &[contract])
            .await
            .map_err(PlatformError::Internal)?;
    }

    // 6. Save bundled test cases (best-effort — re-stamp ids and created_by)
    if !tpl.tests.is_empty() {
        let now = Utc::now();
        let tests: Vec<TestCase> = tpl
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
        state
            .store
            .save_tests(&org_id, &project.id, &ruleset_name, &tests)
            .await
            .map_err(PlatformError::Internal)?;
    }

    Ok(Json(ProjectResponse::from(&project)))
}
