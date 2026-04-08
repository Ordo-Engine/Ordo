//! Fact Catalog and Concept Registry handlers
//!
//! Fact Catalog  — project-scoped registry of raw input fields (ordo-book Ch7 事实五元组)
//! Concept Registry — project-scoped registry of derived fields (ordo-book Ch7 概念 DAG)
//!
//! Both share the same permission model:
//!   - GET  → any project member
//!   - POST/DELETE → admin+ role

use crate::{
    error::{ApiResult, PlatformError},
    models::{Claims, ConceptDefinition, FactDefinition, NullPolicy, FactDataType, Role},
    proxy::find_project_membership,
    AppState,
};
use axum::{
    extract::{Path, State},
    Extension, Json,
};
use chrono::Utc;
use serde::Deserialize;

// ── Fact request types ────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct UpsertFactRequest {
    pub name: String,
    pub data_type: FactDataType,
    pub source: String,
    pub latency_ms: Option<u32>,
    pub null_policy: NullPolicy,
    pub description: Option<String>,
    pub owner: Option<String>,
}

// ── Concept request types ─────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct UpsertConceptRequest {
    pub name: String,
    pub data_type: FactDataType,
    pub expression: String,
    pub dependencies: Vec<String>,
    pub description: Option<String>,
}

// ── Fact handlers ─────────────────────────────────────────────────────────────

/// GET /api/v1/projects/:pid/facts — list all facts (any member)
pub async fn list_facts(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> ApiResult<Json<Vec<FactDefinition>>> {
    let (org_id, _role) = resolve_project(&state, &project_id, &claims.sub, None).await?;

    let facts = state
        .store
        .get_facts(&org_id, &project_id)
        .await
        .map_err(PlatformError::Internal)?;

    Ok(Json(facts))
}

/// POST /api/v1/projects/:pid/facts — upsert a fact (admin+)
pub async fn upsert_fact(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Json(req): Json<UpsertFactRequest>,
) -> ApiResult<Json<FactDefinition>> {
    let (org_id, _role) =
        resolve_project(&state, &project_id, &claims.sub, Some(Role::Admin)).await?;

    if req.name.trim().is_empty() {
        return Err(PlatformError::bad_request("Fact name is required"));
    }

    let mut facts = state
        .store
        .get_facts(&org_id, &project_id)
        .await
        .map_err(PlatformError::Internal)?;

    let now = Utc::now();
    let name = req.name.trim().to_string();

    if let Some(existing) = facts.iter_mut().find(|f| f.name == name) {
        // Update existing
        existing.data_type = req.data_type;
        existing.source = req.source;
        existing.latency_ms = req.latency_ms;
        existing.null_policy = req.null_policy;
        existing.description = req.description;
        existing.owner = req.owner;
        existing.updated_at = now;
        let updated = existing.clone();
        state
            .store
            .save_facts(&org_id, &project_id, &facts)
            .await
            .map_err(PlatformError::Internal)?;
        Ok(Json(updated))
    } else {
        // Insert new
        let fact = FactDefinition {
            name,
            data_type: req.data_type,
            source: req.source,
            latency_ms: req.latency_ms,
            null_policy: req.null_policy,
            description: req.description,
            owner: req.owner,
            created_at: now,
            updated_at: now,
        };
        facts.push(fact.clone());
        state
            .store
            .save_facts(&org_id, &project_id, &facts)
            .await
            .map_err(PlatformError::Internal)?;
        Ok(Json(fact))
    }
}

/// DELETE /api/v1/projects/:pid/facts/:name — delete a fact (admin+)
pub async fn delete_fact(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((project_id, name)): Path<(String, String)>,
) -> ApiResult<axum::http::StatusCode> {
    let (org_id, _role) =
        resolve_project(&state, &project_id, &claims.sub, Some(Role::Admin)).await?;

    let mut facts = state
        .store
        .get_facts(&org_id, &project_id)
        .await
        .map_err(PlatformError::Internal)?;

    let before = facts.len();
    facts.retain(|f| f.name != name);

    if facts.len() == before {
        return Err(PlatformError::not_found("Fact not found"));
    }

    state
        .store
        .save_facts(&org_id, &project_id, &facts)
        .await
        .map_err(PlatformError::Internal)?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ── Concept handlers ──────────────────────────────────────────────────────────

/// GET /api/v1/projects/:pid/concepts — list all concepts (any member)
pub async fn list_concepts(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> ApiResult<Json<Vec<ConceptDefinition>>> {
    let (org_id, _role) = resolve_project(&state, &project_id, &claims.sub, None).await?;

    let concepts = state
        .store
        .get_concepts(&org_id, &project_id)
        .await
        .map_err(PlatformError::Internal)?;

    Ok(Json(concepts))
}

/// POST /api/v1/projects/:pid/concepts — upsert a concept (admin+)
pub async fn upsert_concept(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Json(req): Json<UpsertConceptRequest>,
) -> ApiResult<Json<ConceptDefinition>> {
    let (org_id, _role) =
        resolve_project(&state, &project_id, &claims.sub, Some(Role::Admin)).await?;

    if req.name.trim().is_empty() {
        return Err(PlatformError::bad_request("Concept name is required"));
    }

    let mut concepts = state
        .store
        .get_concepts(&org_id, &project_id)
        .await
        .map_err(PlatformError::Internal)?;

    let now = Utc::now();
    let name = req.name.trim().to_string();

    if let Some(existing) = concepts.iter_mut().find(|c| c.name == name) {
        existing.data_type = req.data_type;
        existing.expression = req.expression;
        existing.dependencies = req.dependencies;
        existing.description = req.description;
        existing.updated_at = now;
        let updated = existing.clone();
        state
            .store
            .save_concepts(&org_id, &project_id, &concepts)
            .await
            .map_err(PlatformError::Internal)?;
        Ok(Json(updated))
    } else {
        let concept = ConceptDefinition {
            name,
            data_type: req.data_type,
            expression: req.expression,
            dependencies: req.dependencies,
            description: req.description,
            created_at: now,
            updated_at: now,
        };
        concepts.push(concept.clone());
        state
            .store
            .save_concepts(&org_id, &project_id, &concepts)
            .await
            .map_err(PlatformError::Internal)?;
        Ok(Json(concept))
    }
}

/// DELETE /api/v1/projects/:pid/concepts/:name — delete a concept (admin+)
pub async fn delete_concept(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((project_id, name)): Path<(String, String)>,
) -> ApiResult<axum::http::StatusCode> {
    let (org_id, _role) =
        resolve_project(&state, &project_id, &claims.sub, Some(Role::Admin)).await?;

    let mut concepts = state
        .store
        .get_concepts(&org_id, &project_id)
        .await
        .map_err(PlatformError::Internal)?;

    let before = concepts.len();
    concepts.retain(|c| c.name != name);

    if concepts.len() == before {
        return Err(PlatformError::not_found("Concept not found"));
    }

    state
        .store
        .save_concepts(&org_id, &project_id, &concepts)
        .await
        .map_err(PlatformError::Internal)?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ── Shared helper ─────────────────────────────────────────────────────────────

/// Resolve project membership; optionally require a minimum role.
/// Returns (org_id, role).
pub async fn resolve_project(
    state: &AppState,
    project_id: &str,
    user_id: &str,
    required_role: Option<Role>,
) -> ApiResult<(String, Role)> {
    let (role, org_id) = find_project_membership(state, project_id, user_id).await?;

    if let Some(required) = required_role {
        if role < required {
            return Err(PlatformError::forbidden(
                "Insufficient role for this operation",
            ));
        }
    }

    Ok((org_id, role))
}
