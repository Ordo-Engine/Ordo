//! Decision Contract handlers (ordo-book Ch13 契约维度)
//!
//! A Decision Contract formally documents the input/output schema and SLA
//! guarantees for a ruleset. Contracts are keyed by ruleset_name.
//!
//! Permissions:
//!   - GET  → any project member
//!   - PUT/DELETE → admin+

use crate::{
    catalog::resolve_project,
    error::{ApiResult, PlatformError},
    models::{Claims, ContractField, DecisionContract, FactDataType, Role},
    AppState,
};
use axum::{
    extract::{Path, State},
    Extension, Json,
};
use chrono::Utc;
use serde::Deserialize;

// ── Request types ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct UpsertContractRequest {
    pub version_pattern: String,
    pub owner: String,
    pub sla_p99_ms: Option<u32>,
    pub input_fields: Vec<ContractField>,
    pub output_fields: Vec<ContractField>,
    pub notes: Option<String>,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// GET /api/v1/projects/:pid/contracts — list all contracts (any member)
pub async fn list_contracts(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> ApiResult<Json<Vec<DecisionContract>>> {
    let (org_id, _role) = resolve_project(&state, &project_id, &claims.sub, None).await?;

    let contracts = state
        .store
        .get_contracts(&org_id, &project_id)
        .await
        .map_err(PlatformError::Internal)?;

    Ok(Json(contracts))
}

/// PUT /api/v1/projects/:pid/contracts/:ruleset_name — upsert a contract (admin+)
pub async fn upsert_contract(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((project_id, ruleset_name)): Path<(String, String)>,
    Json(req): Json<UpsertContractRequest>,
) -> ApiResult<Json<DecisionContract>> {
    let (org_id, _role) =
        resolve_project(&state, &project_id, &claims.sub, Some(Role::Admin)).await?;

    let mut contracts = state
        .store
        .get_contracts(&org_id, &project_id)
        .await
        .map_err(PlatformError::Internal)?;

    let now = Utc::now();

    if let Some(existing) = contracts.iter_mut().find(|c| c.ruleset_name == ruleset_name) {
        existing.version_pattern = req.version_pattern;
        existing.owner = req.owner;
        existing.sla_p99_ms = req.sla_p99_ms;
        existing.input_fields = req.input_fields;
        existing.output_fields = req.output_fields;
        existing.notes = req.notes;
        existing.updated_at = now;
        let updated = existing.clone();
        state
            .store
            .save_contracts(&org_id, &project_id, &contracts)
            .await
            .map_err(PlatformError::Internal)?;
        Ok(Json(updated))
    } else {
        let contract = DecisionContract {
            ruleset_name,
            version_pattern: req.version_pattern,
            owner: req.owner,
            sla_p99_ms: req.sla_p99_ms,
            input_fields: req.input_fields,
            output_fields: req.output_fields,
            notes: req.notes,
            updated_at: now,
        };
        contracts.push(contract.clone());
        state
            .store
            .save_contracts(&org_id, &project_id, &contracts)
            .await
            .map_err(PlatformError::Internal)?;
        Ok(Json(contract))
    }
}

/// DELETE /api/v1/projects/:pid/contracts/:ruleset_name — delete contract (admin+)
pub async fn delete_contract(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((project_id, ruleset_name)): Path<(String, String)>,
) -> ApiResult<axum::http::StatusCode> {
    let (org_id, _role) =
        resolve_project(&state, &project_id, &claims.sub, Some(Role::Admin)).await?;

    let mut contracts = state
        .store
        .get_contracts(&org_id, &project_id)
        .await
        .map_err(PlatformError::Internal)?;

    let before = contracts.len();
    contracts.retain(|c| c.ruleset_name != ruleset_name);

    if contracts.len() == before {
        return Err(PlatformError::not_found("Contract not found"));
    }

    state
        .store
        .save_contracts(&org_id, &project_id, &contracts)
        .await
        .map_err(PlatformError::Internal)?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// Ensure FactDataType is available for ContractField deserialization
// (ContractField uses FactDataType, which is in models.rs)
const _: fn() = || {
    let _: FactDataType = FactDataType::String;
};
