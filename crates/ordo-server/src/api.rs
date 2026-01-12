//! API handlers

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use ordo_core::prelude::*;
use serde::{Deserialize, Serialize};
use std::time::Instant;

use crate::error::ApiError;
use crate::metrics;
use crate::AppState;

/// API Result type
type ApiResult<T> = std::result::Result<T, ApiError>;

// ==================== Request/Response types ====================

/// Execute request
#[derive(Deserialize)]
pub struct ExecuteRequest {
    /// Input data
    pub input: Value,
    /// Whether to include trace
    #[serde(default)]
    pub trace: bool,
}

/// Execute response
#[derive(Serialize)]
pub struct ExecuteResponse {
    /// Result code
    pub code: String,
    /// Result message
    pub message: String,
    /// Output data
    pub output: Value,
    /// Execution duration in microseconds
    pub duration_us: u64,
    /// Execution trace (if requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<TraceInfo>,
}

/// Trace info
#[derive(Serialize)]
pub struct TraceInfo {
    pub path: String,
    pub steps: Vec<StepInfo>,
}

/// Step info
#[derive(Serialize)]
pub struct StepInfo {
    pub id: String,
    pub name: String,
    pub duration_us: u64,
}

/// Expression evaluation request
#[derive(Deserialize)]
pub struct EvalRequest {
    /// Expression to evaluate
    pub expression: String,
    /// Context data
    #[serde(default)]
    pub context: Value,
}

/// Expression evaluation response
#[derive(Serialize)]
pub struct EvalResponse {
    /// Result value
    pub result: Value,
    /// Parsed expression (for debugging)
    pub parsed: String,
}

/// Create ruleset request
#[derive(Deserialize)]
pub struct CreateRuleSetRequest {
    /// RuleSet definition (JSON)
    #[serde(flatten)]
    pub ruleset: RuleSet,
}

// ==================== Handlers ====================

/// List all rulesets
pub async fn list_rulesets(
    State(state): State<AppState>,
) -> ApiResult<Json<Vec<crate::store::RuleSetInfo>>> {
    let store = state.store.read().await;
    Ok(Json(store.list()))
}

/// Get a ruleset by name
pub async fn get_ruleset(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> ApiResult<Json<RuleSet>> {
    let store = state.store.read().await;
    let ruleset = store
        .get(&name)
        .ok_or_else(|| ApiError::not_found(format!("RuleSet '{}' not found", name)))?;

    // Clone to return
    Ok(Json((*ruleset).clone()))
}

/// Create or update a ruleset
pub async fn create_ruleset(
    State(state): State<AppState>,
    Json(request): Json<CreateRuleSetRequest>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    let mut store = state.store.write().await;
    let name = request.ruleset.config.name.clone();
    let exists = store.exists(&name);

    store
        .put(request.ruleset)
        .map_err(|errors| ApiError::bad_request(format!("Validation errors: {:?}", errors)))?;

    let status = if exists {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };

    Ok((
        status,
        Json(serde_json::json!({
            "status": if exists { "updated" } else { "created" },
            "name": name,
        })),
    ))
}

/// Delete a ruleset
pub async fn delete_ruleset(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> ApiResult<StatusCode> {
    let mut store = state.store.write().await;
    if store.delete(&name) {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found(format!("RuleSet '{}' not found", name)))
    }
}

/// Execute a ruleset
pub async fn execute_ruleset(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(request): Json<ExecuteRequest>,
) -> ApiResult<Json<ExecuteResponse>> {
    let start = Instant::now();
    let store = state.store.read().await;

    // Get ruleset
    let ruleset = store
        .get(&name)
        .ok_or_else(|| ApiError::not_found(format!("RuleSet '{}' not found", name)))?;

    // Execute
    let result = match store.executor().execute(&ruleset, request.input) {
        Ok(result) => {
            // Record success metrics
            let duration_secs = start.elapsed().as_secs_f64();
            metrics::record_execution_success(&name, duration_secs);
            result
        }
        Err(e) => {
            // Record error metrics
            let duration_secs = start.elapsed().as_secs_f64();
            metrics::record_execution_error(&name, duration_secs);
            return Err(e.into());
        }
    };

    // Build response
    let trace = if request.trace {
        result.trace.as_ref().map(|t| TraceInfo {
            path: t.path_string(),
            steps: t
                .steps
                .iter()
                .map(|s| StepInfo {
                    id: s.step_id.clone(),
                    name: s.step_name.clone(),
                    duration_us: s.duration_us,
                })
                .collect(),
        })
    } else {
        None
    };

    Ok(Json(ExecuteResponse {
        code: result.code,
        message: result.message,
        output: result.output,
        duration_us: result.duration_us,
        trace,
    }))
}

/// Evaluate an expression (debug endpoint)
pub async fn eval_expression(Json(request): Json<EvalRequest>) -> ApiResult<Json<EvalResponse>> {
    let start = Instant::now();

    // Parse expression
    let expr = match ExprParser::parse(&request.expression) {
        Ok(expr) => expr,
        Err(e) => {
            let duration_secs = start.elapsed().as_secs_f64();
            metrics::record_eval_error(duration_secs);
            return Err(e.into());
        }
    };

    // Create context
    let ctx = Context::new(request.context);

    // Evaluate
    let evaluator = Evaluator::new();
    let result = match evaluator.eval(&expr, &ctx) {
        Ok(result) => {
            let duration_secs = start.elapsed().as_secs_f64();
            metrics::record_eval_success(duration_secs);
            result
        }
        Err(e) => {
            let duration_secs = start.elapsed().as_secs_f64();
            metrics::record_eval_error(duration_secs);
            return Err(e.into());
        }
    };

    Ok(Json(EvalResponse {
        result,
        parsed: format!("{:?}", expr),
    }))
}
