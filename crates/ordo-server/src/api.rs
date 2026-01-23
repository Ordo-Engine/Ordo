//! API handlers

use axum::{
    extract::{ConnectInfo, Extension, Path, State},
    http::StatusCode,
    Json,
};
use futures::future::join_all;
use ordo_core::prelude::*;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use crate::error::ApiError;
use crate::metrics;
use crate::middleware::tenant::TenantContext;
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

// ==================== Batch Execution Types ====================

/// Batch execution options
#[derive(Debug, Clone, Deserialize, Default)]
pub struct BatchExecuteOptions {
    /// Whether to execute in parallel (default: true)
    #[serde(default = "default_parallel")]
    pub parallel: bool,
    /// Whether to stop on first error (default: false)
    /// Note: This option is reserved for future implementation
    #[serde(default)]
    #[allow(dead_code)]
    pub stop_on_error: bool,
    /// Whether to include trace in results (default: false)
    #[serde(default)]
    pub trace: bool,
}

fn default_parallel() -> bool {
    true
}

/// Batch execute request
#[derive(Deserialize)]
pub struct BatchExecuteRequest {
    /// List of inputs to execute
    pub inputs: Vec<Value>,
    /// Execution options
    #[serde(default)]
    pub options: BatchExecuteOptions,
}

/// Single result in batch execution
#[derive(Serialize)]
pub struct BatchExecuteResultItem {
    /// Result code (or "error" if failed)
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
    /// Error message (if failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Batch execution summary
#[derive(Serialize)]
pub struct BatchExecuteSummary {
    /// Total number of inputs
    pub total: usize,
    /// Number of successful executions
    pub success: usize,
    /// Number of failed executions
    pub failed: usize,
    /// Total execution time in microseconds
    pub total_duration_us: u64,
}

/// Batch execute response
#[derive(Serialize)]
pub struct BatchExecuteResponse {
    /// Results for each input (in order)
    pub results: Vec<BatchExecuteResultItem>,
    /// Summary statistics
    pub summary: BatchExecuteSummary,
}

/// Maximum batch size limit
const MAX_BATCH_SIZE: usize = 1000;

// ==================== Handlers ====================

/// List all rulesets
pub async fn list_rulesets(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantContext>,
) -> ApiResult<Json<Vec<crate::store::RuleSetInfo>>> {
    let store = state.store.read().await;
    Ok(Json(store.list_for_tenant(&tenant.id)))
}

/// Get a ruleset by name
pub async fn get_ruleset(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantContext>,
    Path(name): Path<String>,
) -> ApiResult<Json<RuleSet>> {
    let store = state.store.read().await;
    let ruleset = store
        .get_for_tenant(&tenant.id, &name)
        .ok_or_else(|| ApiError::not_found(format!("RuleSet '{}' not found", name)))?;

    // Clone to return
    Ok(Json((*ruleset).clone()))
}

/// Create or update a ruleset
pub async fn create_ruleset(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantContext>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    Json(request): Json<CreateRuleSetRequest>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    let mut store = state.store.write().await;
    let mut ruleset = request.ruleset;
    let name = ruleset.config.name.clone();
    let new_version = ruleset.config.version.clone();
    let exists = store.exists_for_tenant(&tenant.id, &name);

    if let Some(config_tenant) = ruleset.config.tenant_id.as_deref() {
        if config_tenant != tenant.id {
            return Err(ApiError::bad_request(format!(
                "Tenant mismatch: payload tenant '{}' does not match request tenant '{}'",
                config_tenant, tenant.id
            )));
        }
    }
    ruleset.config.tenant_id = Some(tenant.id.clone());

    // Get old version before update
    let old_version = if exists {
        store
            .get_for_tenant(&tenant.id, &name)
            .map(|r| r.config.version.clone())
    } else {
        None
    };

    store
        .put_for_tenant(&tenant.id, ruleset)
        .map_err(|errors| ApiError::bad_request(format!("Validation errors: {:?}", errors)))?;

    metrics::set_tenant_rules_count(&tenant.id, store.list_for_tenant(&tenant.id).len() as i64);

    // Log audit event
    let source_ip = connect_info.map(|ci| ci.0.ip().to_string());
    if exists {
        let rule_id = format!("{}/{}", tenant.id, name);
        state.audit_logger.log_rule_updated(
            &rule_id,
            &old_version.unwrap_or_default(),
            &new_version,
            source_ip,
        );
    } else {
        let rule_id = format!("{}/{}", tenant.id, name);
        state
            .audit_logger
            .log_rule_created(&rule_id, &new_version, source_ip);
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
    Extension(tenant): Extension<TenantContext>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    Path(name): Path<String>,
) -> ApiResult<StatusCode> {
    let mut store = state.store.write().await;
    if store.delete_for_tenant(&tenant.id, &name) {
        metrics::set_tenant_rules_count(&tenant.id, store.list_for_tenant(&tenant.id).len() as i64);
        // Log audit event
        let source_ip = connect_info.map(|ci| ci.0.ip().to_string());
        let rule_id = format!("{}/{}", tenant.id, name);
        state.audit_logger.log_rule_deleted(&rule_id, source_ip);
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found(format!("RuleSet '{}' not found", name)))
    }
}

/// Execute a ruleset
pub async fn execute_ruleset(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantContext>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    Path(name): Path<String>,
    Json(request): Json<ExecuteRequest>,
) -> ApiResult<Json<ExecuteResponse>> {
    let start = Instant::now();

    // Track active executions
    metrics::inc_active_executions();

    // Get ruleset with minimal lock hold time
    // We get Arc<RuleSet> and immediately release the lock
    let ruleset = {
        let store = state.store.read().await;
        store.get_for_tenant(&tenant.id, &name).ok_or_else(|| {
            metrics::dec_active_executions();
            ApiError::not_found(format!("RuleSet '{}' not found", name))
        })?
        // Lock is released here when store goes out of scope
    };

    // Execute without holding the lock (uses shared executor from AppState)
    let mut ruleset = (*ruleset).clone();
    if tenant.config.execution_timeout_ms > 0 {
        ruleset.config.timeout_ms = tenant.config.execution_timeout_ms;
    }
    let result = match state.executor.execute(&ruleset, request.input) {
        Ok(result) => {
            // Record success metrics
            let duration_secs = start.elapsed().as_secs_f64();
            metrics::record_execution_success(&name, duration_secs);
            metrics::record_tenant_execution_success(&tenant.id, &name, duration_secs);

            // Record terminal result distribution
            metrics::record_terminal_result(&name, &result.code);

            // Log audit event (with sampling)
            let source_ip = connect_info.map(|ci| ci.0.ip().to_string());
            let rule_id = format!("{}/{}", tenant.id, name);
            state
                .audit_logger
                .log_execution(&rule_id, result.duration_us, &result.code, source_ip);

            result
        }
        Err(e) => {
            // Record error metrics
            let duration_secs = start.elapsed().as_secs_f64();
            metrics::record_execution_error(&name, duration_secs);
            metrics::record_tenant_execution_error(&tenant.id, &name, duration_secs);

            // Record terminal result for errors
            metrics::record_terminal_result(&name, "error");

            // Log audit event for errors (with sampling)
            let source_ip = connect_info.map(|ci| ci.0.ip().to_string());
            let rule_id = format!("{}/{}", tenant.id, name);
            state.audit_logger.log_execution(
                &rule_id,
                start.elapsed().as_micros() as u64,
                "error",
                source_ip,
            );

            metrics::dec_active_executions();
            return Err(e.into());
        }
    };

    // Decrement active executions
    metrics::dec_active_executions();

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

/// Execute a ruleset with multiple inputs (batch execution)
///
/// This endpoint is more efficient than calling /execute/:name multiple times:
/// - Single HTTP request for all inputs
/// - Single lock acquisition for ruleset lookup
/// - Optional parallel execution
pub async fn execute_ruleset_batch(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantContext>,
    Path(name): Path<String>,
    Json(request): Json<BatchExecuteRequest>,
) -> ApiResult<Json<BatchExecuteResponse>> {
    let start = Instant::now();

    // Validate batch size
    if request.inputs.is_empty() {
        return Err(ApiError::bad_request(
            "inputs array cannot be empty".to_string(),
        ));
    }
    if request.inputs.len() > MAX_BATCH_SIZE {
        return Err(ApiError::bad_request(format!(
            "batch size {} exceeds maximum allowed size {}",
            request.inputs.len(),
            MAX_BATCH_SIZE
        )));
    }

    let batch_size = request.inputs.len();

    // Track active executions (count as batch_size concurrent executions)
    metrics::inc_active_executions();

    // Get ruleset with minimal lock hold time (only once for the entire batch)
    let ruleset = {
        let store = state.store.read().await;
        store.get_for_tenant(&tenant.id, &name).ok_or_else(|| {
            metrics::dec_active_executions();
            ApiError::not_found(format!("RuleSet '{}' not found", name))
        })?
    };
    let mut ruleset = (*ruleset).clone();
    if tenant.config.execution_timeout_ms > 0 {
        ruleset.config.timeout_ms = tenant.config.execution_timeout_ms;
    }
    let ruleset = Arc::new(ruleset);
    let executor = state.executor.clone();
    let trace_enabled = request.options.trace;

    let run_one = |input: Value| {
        let start_one = Instant::now();
        match executor.execute(&ruleset, input) {
            Ok(result) => {
                let trace = if trace_enabled {
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
                BatchExecuteResultItem {
                    code: result.code,
                    message: result.message,
                    output: result.output,
                    duration_us: result.duration_us,
                    trace,
                    error: None,
                }
            }
            Err(err) => BatchExecuteResultItem {
                code: "error".to_string(),
                message: err.to_string(),
                output: Value::Null,
                duration_us: start_one.elapsed().as_micros() as u64,
                trace: None,
                error: Some(err.to_string()),
            },
        }
    };

    let results: Vec<BatchExecuteResultItem> = if request.options.parallel {
        let futures = request.inputs.into_iter().map(|input| {
            let ruleset = Arc::clone(&ruleset);
            let executor = executor.clone();
            tokio::task::spawn_blocking(move || {
                let start_one = Instant::now();
                match executor.execute(&ruleset, input) {
                    Ok(result) => {
                        let trace = if trace_enabled {
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
                        BatchExecuteResultItem {
                            code: result.code,
                            message: result.message,
                            output: result.output,
                            duration_us: result.duration_us,
                            trace,
                            error: None,
                        }
                    }
                    Err(err) => BatchExecuteResultItem {
                        code: "error".to_string(),
                        message: err.to_string(),
                        output: Value::Null,
                        duration_us: start_one.elapsed().as_micros() as u64,
                        trace: None,
                        error: Some(err.to_string()),
                    },
                }
            })
        });

        join_all(futures)
            .await
            .into_iter()
            .map(|result| match result {
                Ok(item) => item,
                Err(err) => BatchExecuteResultItem {
                    code: "error".to_string(),
                    message: "batch task failed".to_string(),
                    output: Value::Null,
                    duration_us: 0,
                    trace: None,
                    error: Some(err.to_string()),
                },
            })
            .collect()
    } else {
        request.inputs.into_iter().map(run_one).collect()
    };

    let mut success = 0;
    let mut failed = 0;
    let mut total_duration_us = 0;
    for result in &results {
        total_duration_us += result.duration_us;
        if result.error.is_some() {
            failed += 1;
        } else {
            success += 1;
        }
        metrics::record_terminal_result(&name, &result.code);
    }

    // Record metrics
    let duration_secs = start.elapsed().as_secs_f64();
    metrics::record_batch_execution(&name, batch_size, success, failed, duration_secs);
    metrics::record_tenant_batch_results(&tenant.id, &name, success, failed);

    metrics::dec_active_executions();

    Ok(Json(BatchExecuteResponse {
        results,
        summary: BatchExecuteSummary {
            total: batch_size,
            success,
            failed,
            total_duration_us,
        },
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
    Extension(tenant): Extension<TenantContext>,
    Path(name): Path<String>,
) -> ApiResult<Json<crate::store::VersionListResponse>> {
    let store = state.store.read().await;

    // Check if ruleset exists
    if !store.exists_for_tenant(&tenant.id, &name) {
        return Err(ApiError::not_found(format!("RuleSet '{}' not found", name)));
    }

    // Check if persistence is enabled
    if !store.persistence_enabled() {
        // Return empty version list for in-memory mode
        return Ok(Json(crate::store::VersionListResponse {
            name: name.clone(),
            current_version: store
                .get_for_tenant(&tenant.id, &name)
                .map(|r| r.config.version.clone())
                .unwrap_or_default(),
            versions: vec![],
        }));
    }

    let versions = store
        .list_versions_for_tenant(&tenant.id, &name)
        .map_err(|e| ApiError::internal(format!("Failed to list versions: {}", e)))?;

    Ok(Json(versions))
}

/// Rollback a ruleset to a specific version
pub async fn rollback_ruleset(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantContext>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    Path(name): Path<String>,
    Json(request): Json<RollbackRequest>,
) -> ApiResult<Json<RollbackResponse>> {
    let mut store = state.store.write().await;

    // Check if ruleset exists
    if !store.exists_for_tenant(&tenant.id, &name) {
        return Err(ApiError::not_found(format!("RuleSet '{}' not found", name)));
    }

    // Check if persistence is enabled
    if !store.persistence_enabled() {
        return Err(ApiError::bad_request(
            "Version rollback not available in memory-only mode".to_string(),
        ));
    }

    // Perform rollback
    match store.rollback_to_version_for_tenant(&tenant.id, &name, request.seq) {
        Ok(Some((from_version, to_version))) => {
            // Log audit event
            let source_ip = connect_info.map(|ci| ci.0.ip().to_string());
            let rule_id = format!("{}/{}", tenant.id, name);
            state.audit_logger.log_rule_rollback(
                &rule_id,
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

// ==================== Tenant Management ====================

/// Create tenant request
#[derive(Deserialize)]
pub struct CreateTenantRequest {
    /// Tenant ID
    pub id: String,
    /// Tenant name
    pub name: String,
    /// Whether tenant is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// QPS limit (None = unlimited)
    #[serde(default)]
    pub qps_limit: Option<u32>,
    /// Burst limit (None = use qps_limit)
    #[serde(default)]
    pub burst_limit: Option<u32>,
    /// Execution timeout in milliseconds
    #[serde(default)]
    pub execution_timeout_ms: Option<u64>,
    /// Maximum number of rules (None = unlimited)
    #[serde(default)]
    pub max_rules: Option<usize>,
}

fn default_enabled() -> bool {
    true
}

/// Update tenant request
#[derive(Deserialize)]
pub struct UpdateTenantRequest {
    /// Tenant name
    #[serde(default)]
    pub name: Option<String>,
    /// Whether tenant is enabled
    #[serde(default)]
    pub enabled: Option<bool>,
    /// QPS limit (None = unlimited)
    #[serde(default)]
    pub qps_limit: Option<Option<u32>>,
    /// Burst limit (None = use qps_limit)
    #[serde(default)]
    pub burst_limit: Option<Option<u32>>,
    /// Execution timeout in milliseconds
    #[serde(default)]
    pub execution_timeout_ms: Option<u64>,
    /// Maximum number of rules (None = unlimited)
    #[serde(default)]
    pub max_rules: Option<Option<usize>>,
}

/// Tenant info response
#[derive(Serialize)]
pub struct TenantInfo {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub qps_limit: Option<u32>,
    pub burst_limit: Option<u32>,
    pub execution_timeout_ms: u64,
    pub max_rules: Option<usize>,
    pub rules_count: usize,
}

/// List all tenants
pub async fn list_tenants(State(state): State<AppState>) -> ApiResult<Json<Vec<TenantInfo>>> {
    let tenants = state.tenant_manager.list().await;
    let store = state.store.read().await;

    let tenant_infos: Vec<TenantInfo> = tenants
        .into_iter()
        .map(|config| {
            let rules_count = store.list_for_tenant(&config.id).len();
            TenantInfo {
                id: config.id,
                name: config.name,
                enabled: config.enabled,
                qps_limit: config.qps_limit,
                burst_limit: config.burst_limit,
                execution_timeout_ms: config.execution_timeout_ms,
                max_rules: config.max_rules,
                rules_count,
            }
        })
        .collect();

    Ok(Json(tenant_infos))
}

/// Get tenant by ID
pub async fn get_tenant(
    State(state): State<AppState>,
    Path(tenant_id): Path<String>,
) -> ApiResult<Json<TenantInfo>> {
    let config = state
        .tenant_manager
        .get(&tenant_id)
        .await
        .ok_or_else(|| ApiError::not_found(format!("Tenant '{}' not found", tenant_id)))?;

    let store = state.store.read().await;
    let rules_count = store.list_for_tenant(&tenant_id).len();

    Ok(Json(TenantInfo {
        id: config.id,
        name: config.name,
        enabled: config.enabled,
        qps_limit: config.qps_limit,
        burst_limit: config.burst_limit,
        execution_timeout_ms: config.execution_timeout_ms,
        max_rules: config.max_rules,
        rules_count,
    }))
}

/// Create a new tenant
pub async fn create_tenant(
    State(state): State<AppState>,
    Json(request): Json<CreateTenantRequest>,
) -> ApiResult<(StatusCode, Json<TenantInfo>)> {
    // Validate tenant ID
    if request.id.is_empty() {
        return Err(ApiError::bad_request(
            "Tenant ID cannot be empty".to_string(),
        ));
    }

    // Check if tenant already exists
    if state.tenant_manager.get(&request.id).await.is_some() {
        return Err(ApiError::bad_request(format!(
            "Tenant '{}' already exists",
            request.id
        )));
    }

    let default_timeout_ms = state.tenant_manager.defaults().default_timeout_ms;
    let config = crate::tenant::TenantConfig {
        id: request.id.clone(),
        name: request.name,
        enabled: request.enabled,
        qps_limit: request.qps_limit,
        burst_limit: request.burst_limit,
        execution_timeout_ms: request.execution_timeout_ms.unwrap_or(default_timeout_ms),
        max_rules: request.max_rules,
        metadata: std::collections::HashMap::new(),
    };

    state
        .tenant_manager
        .upsert(config.clone())
        .await
        .map_err(|e| ApiError::internal(format!("Failed to create tenant: {}", e)))?;

    let store = state.store.read().await;
    let rules_count = store.list_for_tenant(&request.id).len();

    Ok((
        StatusCode::CREATED,
        Json(TenantInfo {
            id: config.id,
            name: config.name,
            enabled: config.enabled,
            qps_limit: config.qps_limit,
            burst_limit: config.burst_limit,
            execution_timeout_ms: config.execution_timeout_ms,
            max_rules: config.max_rules,
            rules_count,
        }),
    ))
}

/// Update tenant configuration
pub async fn update_tenant(
    State(state): State<AppState>,
    Path(tenant_id): Path<String>,
    Json(request): Json<UpdateTenantRequest>,
) -> ApiResult<Json<TenantInfo>> {
    let mut config = state
        .tenant_manager
        .get(&tenant_id)
        .await
        .ok_or_else(|| ApiError::not_found(format!("Tenant '{}' not found", tenant_id)))?;

    // Update fields if provided
    if let Some(name) = request.name {
        config.name = name;
    }
    if let Some(enabled) = request.enabled {
        config.enabled = enabled;
    }
    if let Some(qps_limit) = request.qps_limit {
        config.qps_limit = qps_limit;
    }
    if let Some(burst_limit) = request.burst_limit {
        config.burst_limit = burst_limit;
    }
    if let Some(timeout_ms) = request.execution_timeout_ms {
        config.execution_timeout_ms = timeout_ms;
    }
    if let Some(max_rules) = request.max_rules {
        config.max_rules = max_rules;
    }

    state
        .tenant_manager
        .upsert(config.clone())
        .await
        .map_err(|e| ApiError::internal(format!("Failed to update tenant: {}", e)))?;

    let store = state.store.read().await;
    let rules_count = store.list_for_tenant(&tenant_id).len();

    Ok(Json(TenantInfo {
        id: config.id,
        name: config.name,
        enabled: config.enabled,
        qps_limit: config.qps_limit,
        burst_limit: config.burst_limit,
        execution_timeout_ms: config.execution_timeout_ms,
        max_rules: config.max_rules,
        rules_count,
    }))
}

/// Delete a tenant
pub async fn delete_tenant(
    State(state): State<AppState>,
    Path(tenant_id): Path<String>,
) -> ApiResult<StatusCode> {
    // Check if tenant has rules
    let store = state.store.read().await;
    let rules_count = store.list_for_tenant(&tenant_id).len();
    if rules_count > 0 {
        return Err(ApiError::bad_request(format!(
            "Cannot delete tenant '{}' with {} rules. Delete all rules first.",
            tenant_id, rules_count
        )));
    }
    drop(store); // Release lock before deleting

    let deleted = state
        .tenant_manager
        .delete(&tenant_id)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to delete tenant: {}", e)))?;

    if deleted {
        // Clean up rate limiter cache for this tenant
        state.rate_limiter.remove_tenant(&tenant_id);
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found(format!(
            "Tenant '{}' not found",
            tenant_id
        )))
    }
}
