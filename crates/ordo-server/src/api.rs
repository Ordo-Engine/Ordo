//! API handlers

use axum::{
    extract::{ConnectInfo, Path, State},
    http::StatusCode,
    Json,
};
use ordo_core::prelude::*;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
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
    connect_info: Option<ConnectInfo<SocketAddr>>,
    Json(request): Json<CreateRuleSetRequest>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    let mut store = state.store.write().await;
    let name = request.ruleset.config.name.clone();
    let new_version = request.ruleset.config.version.clone();
    let exists = store.exists(&name);

    // Get old version before update
    let old_version = if exists {
        store.get(&name).map(|r| r.config.version.clone())
    } else {
        None
    };

    store
        .put(request.ruleset)
        .map_err(|errors| ApiError::bad_request(format!("Validation errors: {:?}", errors)))?;

    // Log audit event
    let source_ip = connect_info.map(|ci| ci.0.ip().to_string());
    if exists {
        state.audit_logger.log_rule_updated(
            &name,
            &old_version.unwrap_or_default(),
            &new_version,
            source_ip,
        );
    } else {
        state
            .audit_logger
            .log_rule_created(&name, &new_version, source_ip);
    }

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
    connect_info: Option<ConnectInfo<SocketAddr>>,
    Path(name): Path<String>,
) -> ApiResult<StatusCode> {
    let mut store = state.store.write().await;
    if store.delete(&name) {
        // Log audit event
        let source_ip = connect_info.map(|ci| ci.0.ip().to_string());
        state.audit_logger.log_rule_deleted(&name, source_ip);
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found(format!("RuleSet '{}' not found", name)))
    }
}

/// Execute a ruleset
pub async fn execute_ruleset(
    State(state): State<AppState>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
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

            // Log audit event (with sampling)
            let source_ip = connect_info.map(|ci| ci.0.ip().to_string());
            state
                .audit_logger
                .log_execution(&name, result.duration_us, &result.code, source_ip);

            result
        }
        Err(e) => {
            // Record error metrics
            let duration_secs = start.elapsed().as_secs_f64();
            metrics::record_execution_error(&name, duration_secs);

            // Log audit event for errors (with sampling)
            let source_ip = connect_info.map(|ci| ci.0.ip().to_string());
            state.audit_logger.log_execution(
                &name,
                start.elapsed().as_micros() as u64,
                "error",
                source_ip,
            );

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

// ==================== Version Management ====================

/// Rollback request
#[derive(Deserialize)]
pub struct RollbackRequest {
    /// Version sequence number to rollback to
    pub seq: u32,
}

/// Rollback response
#[derive(Serialize)]
pub struct RollbackResponse {
    /// Status
    pub status: String,
    /// Rule name
    pub name: String,
    /// Version before rollback
    pub from_version: String,
    /// Version after rollback
    pub to_version: String,
}

/// List versions of a ruleset
pub async fn list_versions(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> ApiResult<Json<crate::store::VersionListResponse>> {
    let store = state.store.read().await;

    // Check if ruleset exists
    if !store.exists(&name) {
        return Err(ApiError::not_found(format!("RuleSet '{}' not found", name)));
    }

    // Check if persistence is enabled
    if !store.persistence_enabled() {
        // Return empty version list for in-memory mode
        return Ok(Json(crate::store::VersionListResponse {
            name: name.clone(),
            current_version: store
                .get(&name)
                .map(|r| r.config.version.clone())
                .unwrap_or_default(),
            versions: vec![],
        }));
    }

    let versions = store
        .list_versions(&name)
        .map_err(|e| ApiError::internal(format!("Failed to list versions: {}", e)))?;

    Ok(Json(versions))
}

/// Rollback a ruleset to a specific version
pub async fn rollback_ruleset(
    State(state): State<AppState>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    Path(name): Path<String>,
    Json(request): Json<RollbackRequest>,
) -> ApiResult<Json<RollbackResponse>> {
    let mut store = state.store.write().await;

    // Check if ruleset exists
    if !store.exists(&name) {
        return Err(ApiError::not_found(format!("RuleSet '{}' not found", name)));
    }

    // Check if persistence is enabled
    if !store.persistence_enabled() {
        return Err(ApiError::bad_request(
            "Version rollback not available in memory-only mode".to_string(),
        ));
    }

    // Perform rollback
    match store.rollback_to_version(&name, request.seq) {
        Ok(Some((from_version, to_version))) => {
            // Log audit event
            let source_ip = connect_info.map(|ci| ci.0.ip().to_string());
            state.audit_logger.log_rule_rollback(
                &name,
                &from_version,
                &to_version,
                request.seq,
                source_ip,
            );

            Ok(Json(RollbackResponse {
                status: "rolled_back".to_string(),
                name,
                from_version,
                to_version,
            }))
        }
        Ok(None) => Err(ApiError::not_found(format!(
            "Version {} not found for rule '{}'",
            request.seq, name
        ))),
        Err(e) => Err(ApiError::internal(format!("Rollback failed: {}", e))),
    }
}

// ==================== Audit Configuration ====================

/// Sample rate request
#[derive(Deserialize)]
pub struct SampleRateRequest {
    /// New sample rate (0-100)
    pub sample_rate: u8,
}

/// Sample rate response
#[derive(Serialize)]
pub struct SampleRateResponse {
    /// Current sample rate
    pub sample_rate: u8,
    /// Previous sample rate (only in update response)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<u8>,
}

/// Get current audit sample rate
pub async fn get_audit_sample_rate(State(state): State<AppState>) -> Json<SampleRateResponse> {
    let rate = state.audit_logger.get_sample_rate();
    Json(SampleRateResponse {
        sample_rate: rate,
        previous: None,
    })
}

/// Update audit sample rate
pub async fn set_audit_sample_rate(
    State(state): State<AppState>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    Json(request): Json<SampleRateRequest>,
) -> Json<SampleRateResponse> {
    let source_ip = connect_info.map(|ci| ci.0.ip().to_string());
    let previous = state.audit_logger.set_sample_rate(request.sample_rate);
    let current = state.audit_logger.get_sample_rate();

    // Log the change
    state
        .audit_logger
        .log_sample_rate_changed(previous, current, source_ip);

    Json(SampleRateResponse {
        sample_rate: current,
        previous: Some(previous),
    })
}
