//! gRPC service implementation
//!
//! This module provides gRPC service implementation for the Ordo rule engine.
//!
//! ## Multi-tenancy
//!
//! All methods support multi-tenancy through gRPC metadata. Pass `x-tenant-id`
//! in the metadata to specify the tenant. If not provided, the server's default
//! tenant will be used.
//!
//! ## Batch Execution
//!
//! The `BatchExecute` method allows executing a ruleset with multiple inputs
//! in a single RPC call, which is more efficient than calling `Execute` multiple times.

use std::sync::Arc;
use std::time::Instant;

use futures::future::join_all;
use ordo_core::prelude::*;
use ordo_core::rule::{ExecutionOptions, RuleExecutor};
use ordo_proto::{
    health_response, ordo_service_server::OrdoService, BatchExecuteOptions, BatchExecuteRequest,
    BatchExecuteResponse, BatchExecuteResultItem, BatchExecuteSummary, EvalRequest, EvalResponse,
    ExecuteRequest, ExecuteResponse, ExecutionTrace, GetRuleSetRequest, GetRuleSetResponse,
    HealthRequest, HealthResponse, ListRuleSetsRequest, ListRuleSetsResponse, RuleSetSummary,
    StepTrace,
};
use tokio::sync::RwLock;
use tonic::{Request, Response, Status};

use crate::rate_limiter::RateLimiter;
use crate::store::RuleStore;
use crate::tenant::{TenantConfig, TenantManager};

/// Metadata key for tenant ID
const METADATA_TENANT_ID: &str = "x-tenant-id";

/// Maximum batch size limit
const MAX_BATCH_SIZE: usize = 1000;

/// gRPC service implementation with multi-tenancy support
pub struct OrdoGrpcService {
    store: Arc<RwLock<RuleStore>>,
    executor: Arc<RuleExecutor>,
    start_time: Instant,
    default_tenant: String,
    tenant_manager: Arc<TenantManager>,
    rate_limiter: Arc<RateLimiter>,
    multi_tenancy_enabled: bool,
}

impl OrdoGrpcService {
    /// Create a new gRPC service with multi-tenancy support
    pub fn new(
        store: Arc<RwLock<RuleStore>>,
        executor: Arc<RuleExecutor>,
        default_tenant: String,
        tenant_manager: Arc<TenantManager>,
        rate_limiter: Arc<RateLimiter>,
        multi_tenancy_enabled: bool,
    ) -> Self {
        Self {
            store,
            executor,
            start_time: Instant::now(),
            default_tenant,
            tenant_manager,
            rate_limiter,
            multi_tenancy_enabled,
        }
    }

    /// Extract tenant ID from gRPC metadata
    fn extract_tenant_id<T>(&self, request: &Request<T>) -> String {
        request
            .metadata()
            .get(METADATA_TENANT_ID)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.default_tenant.clone())
    }

    /// Validate tenant and check rate limit
    async fn validate_tenant(&self, tenant_id: &str) -> std::result::Result<TenantConfig, Status> {
        if !self.multi_tenancy_enabled {
            return Ok(TenantConfig::default_for_id(
                tenant_id,
                self.tenant_manager.defaults(),
            ));
        }

        let config = self
            .tenant_manager
            .validate_enabled(tenant_id)
            .await
            .map_err(Status::not_found)?;

        // Check rate limit
        let allowed = self
            .rate_limiter
            .allow(tenant_id, config.qps_limit, config.burst_limit);
        if !allowed {
            return Err(Status::resource_exhausted(format!(
                "Rate limit exceeded for tenant '{}'",
                tenant_id
            )));
        }

        Ok(config)
    }
}

/// Build execution trace for gRPC response
fn build_execution_trace(
    trace: Option<&ordo_core::trace::ExecutionTrace>,
    enabled: bool,
) -> Option<ExecutionTrace> {
    if !enabled {
        return None;
    }
    trace.map(|t| ExecutionTrace {
        path: t.path_string(),
        steps: t
            .steps
            .iter()
            .map(|s| StepTrace {
                step_id: s.step_id.clone(),
                step_name: s.step_name.clone(),
                duration_us: s.duration_us,
                result: if s.is_terminal {
                    "terminal".to_string()
                } else {
                    s.next_step.clone().unwrap_or_default()
                },
            })
            .collect(),
    })
}

#[tonic::async_trait]
impl OrdoService for OrdoGrpcService {
    /// Execute a ruleset with multi-tenancy support
    async fn execute(
        &self,
        request: Request<ExecuteRequest>,
    ) -> std::result::Result<Response<ExecuteResponse>, Status> {
        // Extract and validate tenant
        let tenant_id = self.extract_tenant_id(&request);
        let tenant_config = self.validate_tenant(&tenant_id).await?;

        let req = request.into_inner();

        // Parse input JSON
        let input: Value = serde_json::from_str(&req.input_json)
            .map_err(|e| Status::invalid_argument(format!("Invalid input JSON: {}", e)))?;

        // Get ruleset for the tenant
        let ruleset = {
            let store = self.store.read().await;
            store
                .get_for_tenant(&tenant_id, &req.ruleset_name)
                .ok_or_else(|| {
                    Status::not_found(format!("RuleSet '{}' not found", req.ruleset_name))
                })?
        };

        // Build execution options for tenant-specific overrides
        let exec_options = if tenant_config.execution_timeout_ms > 0 || req.include_trace {
            Some(ExecutionOptions {
                timeout_ms: if tenant_config.execution_timeout_ms > 0 {
                    Some(tenant_config.execution_timeout_ms)
                } else {
                    None
                },
                enable_trace: if req.include_trace { Some(true) } else { None },
                max_depth: None,
            })
        } else {
            None
        };

        // Execute
        let result = self
            .executor
            .execute_with_options(&ruleset, input, exec_options.as_ref())
            .map_err(|e| Status::internal(format!("Execution error: {}", e)))?;

        // Build response
        let trace = build_execution_trace(result.trace.as_ref(), req.include_trace);

        // Serialize output
        let output_json = serde_json::to_string(&result.output)
            .map_err(|e| Status::internal(format!("Failed to serialize output: {}", e)))?;

        Ok(Response::new(ExecuteResponse {
            code: result.code,
            message: result.message,
            output_json,
            duration_us: result.duration_us,
            trace,
        }))
    }

    /// Execute a ruleset with multiple inputs (batch execution) with multi-tenancy support
    async fn batch_execute(
        &self,
        request: Request<BatchExecuteRequest>,
    ) -> std::result::Result<Response<BatchExecuteResponse>, Status> {
        // Extract and validate tenant
        let tenant_id = self.extract_tenant_id(&request);
        let tenant_config = self.validate_tenant(&tenant_id).await?;

        let req = request.into_inner();

        // Validate batch size
        if req.inputs_json.is_empty() {
            return Err(Status::invalid_argument(
                "inputs_json array cannot be empty",
            ));
        }
        if req.inputs_json.len() > MAX_BATCH_SIZE {
            return Err(Status::invalid_argument(format!(
                "batch size {} exceeds maximum allowed size {}",
                req.inputs_json.len(),
                MAX_BATCH_SIZE
            )));
        }

        let batch_size = req.inputs_json.len();
        let options = req.options.unwrap_or(BatchExecuteOptions {
            parallel: true,
            include_trace: false,
        });

        // Get ruleset for the tenant (single lock acquisition for entire batch)
        let ruleset = {
            let store = self.store.read().await;
            store
                .get_for_tenant(&tenant_id, &req.ruleset_name)
                .ok_or_else(|| {
                    Status::not_found(format!("RuleSet '{}' not found", req.ruleset_name))
                })?
        };

        // Build execution options
        let exec_options = Arc::new(ExecutionOptions {
            timeout_ms: if tenant_config.execution_timeout_ms > 0 {
                Some(tenant_config.execution_timeout_ms)
            } else {
                None
            },
            enable_trace: if options.include_trace {
                Some(true)
            } else {
                None
            },
            max_depth: None,
        });

        let executor = self.executor.clone();
        let trace_enabled = options.include_trace;

        // Execute batch
        let results: Vec<BatchExecuteResultItem> = if options.parallel {
            // Parallel execution using tokio::spawn_blocking
            let futures = req.inputs_json.into_iter().map(|input_json| {
                let ruleset = Arc::clone(&ruleset);
                let executor = executor.clone();
                let exec_options = Arc::clone(&exec_options);

                tokio::task::spawn_blocking(move || {
                    let start_one = Instant::now();

                    // Parse input
                    let input: Value = match serde_json::from_str(&input_json) {
                        Ok(v) => v,
                        Err(e) => {
                            return BatchExecuteResultItem {
                                code: "error".to_string(),
                                message: "Invalid input JSON".to_string(),
                                output_json: "null".to_string(),
                                duration_us: start_one.elapsed().as_micros() as u64,
                                trace: None,
                                error: format!("Invalid input JSON: {}", e),
                            }
                        }
                    };

                    // Execute
                    match executor.execute_with_options(&ruleset, input, Some(&exec_options)) {
                        Ok(result) => {
                            let trace = build_execution_trace(result.trace.as_ref(), trace_enabled);
                            let output_json = serde_json::to_string(&result.output)
                                .unwrap_or_else(|_| "null".to_string());
                            BatchExecuteResultItem {
                                code: result.code,
                                message: result.message,
                                output_json,
                                duration_us: result.duration_us,
                                trace,
                                error: String::new(),
                            }
                        }
                        Err(e) => BatchExecuteResultItem {
                            code: "error".to_string(),
                            message: e.to_string(),
                            output_json: "null".to_string(),
                            duration_us: start_one.elapsed().as_micros() as u64,
                            trace: None,
                            error: e.to_string(),
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
                        output_json: "null".to_string(),
                        duration_us: 0,
                        trace: None,
                        error: err.to_string(),
                    },
                })
                .collect()
        } else {
            // Sequential execution
            req.inputs_json
                .into_iter()
                .map(|input_json| {
                    let start_one = Instant::now();

                    // Parse input
                    let input: Value = match serde_json::from_str(&input_json) {
                        Ok(v) => v,
                        Err(e) => {
                            return BatchExecuteResultItem {
                                code: "error".to_string(),
                                message: "Invalid input JSON".to_string(),
                                output_json: "null".to_string(),
                                duration_us: start_one.elapsed().as_micros() as u64,
                                trace: None,
                                error: format!("Invalid input JSON: {}", e),
                            }
                        }
                    };

                    // Execute
                    match self
                        .executor
                        .execute_with_options(&ruleset, input, Some(&exec_options))
                    {
                        Ok(result) => {
                            let trace = build_execution_trace(result.trace.as_ref(), trace_enabled);
                            let output_json = serde_json::to_string(&result.output)
                                .unwrap_or_else(|_| "null".to_string());
                            BatchExecuteResultItem {
                                code: result.code,
                                message: result.message,
                                output_json,
                                duration_us: result.duration_us,
                                trace,
                                error: String::new(),
                            }
                        }
                        Err(e) => BatchExecuteResultItem {
                            code: "error".to_string(),
                            message: e.to_string(),
                            output_json: "null".to_string(),
                            duration_us: start_one.elapsed().as_micros() as u64,
                            trace: None,
                            error: e.to_string(),
                        },
                    }
                })
                .collect()
        };

        // Build summary
        let mut success: u32 = 0;
        let mut failed: u32 = 0;
        let mut total_duration_us: u64 = 0;
        for result in &results {
            total_duration_us += result.duration_us;
            if result.error.is_empty() {
                success += 1;
            } else {
                failed += 1;
            }
        }

        Ok(Response::new(BatchExecuteResponse {
            results,
            summary: Some(BatchExecuteSummary {
                total: batch_size as u32,
                success,
                failed,
                total_duration_us,
            }),
        }))
    }

    /// Get a ruleset by name with multi-tenancy support
    async fn get_rule_set(
        &self,
        request: Request<GetRuleSetRequest>,
    ) -> std::result::Result<Response<GetRuleSetResponse>, Status> {
        // Extract and validate tenant
        let tenant_id = self.extract_tenant_id(&request);
        let _tenant_config = self.validate_tenant(&tenant_id).await?;

        let req = request.into_inner();

        let store = self.store.read().await;
        let ruleset = store
            .get_for_tenant(&tenant_id, &req.name)
            .ok_or_else(|| Status::not_found(format!("RuleSet '{}' not found", req.name)))?;

        let ruleset_json = serde_json::to_string(&*ruleset)
            .map_err(|e| Status::internal(format!("Failed to serialize ruleset: {}", e)))?;

        Ok(Response::new(GetRuleSetResponse {
            ruleset_json,
            version: ruleset.config.version.clone(),
            description: ruleset.config.description.clone(),
            step_count: ruleset.steps.len() as u32,
        }))
    }

    /// List all rulesets with multi-tenancy support
    async fn list_rule_sets(
        &self,
        request: Request<ListRuleSetsRequest>,
    ) -> std::result::Result<Response<ListRuleSetsResponse>, Status> {
        // Extract and validate tenant
        let tenant_id = self.extract_tenant_id(&request);
        let _tenant_config = self.validate_tenant(&tenant_id).await?;

        let req = request.into_inner();

        let store = self.store.read().await;
        let all_rulesets = store.list_for_tenant(&tenant_id);

        // Filter by prefix if specified
        let filtered: Vec<_> = if req.name_prefix.is_empty() {
            all_rulesets
        } else {
            all_rulesets
                .into_iter()
                .filter(|r| r.name.starts_with(&req.name_prefix))
                .collect()
        };

        // Capture total count BEFORE applying limit (for pagination)
        let total_count = filtered.len() as u32;

        // Apply limit if specified
        let limited: Vec<_> = if req.limit > 0 {
            filtered.into_iter().take(req.limit as usize).collect()
        } else {
            filtered
        };
        let rulesets = limited
            .into_iter()
            .map(|r| RuleSetSummary {
                name: r.name,
                version: r.version,
                description: r.description,
                step_count: r.step_count as u32,
            })
            .collect();

        Ok(Response::new(ListRuleSetsResponse {
            rulesets,
            total_count,
        }))
    }

    /// Evaluate an expression with multi-tenancy support
    async fn eval(
        &self,
        request: Request<EvalRequest>,
    ) -> std::result::Result<Response<EvalResponse>, Status> {
        // Extract and validate tenant (for rate limiting)
        let tenant_id = self.extract_tenant_id(&request);
        let _tenant_config = self.validate_tenant(&tenant_id).await?;

        let req = request.into_inner();

        // Parse expression
        let expr = ExprParser::parse(&req.expression)
            .map_err(|e| Status::invalid_argument(format!("Invalid expression: {}", e)))?;

        // Parse context
        let context_value: Value = if req.context_json.is_empty() {
            Value::object(std::collections::HashMap::new())
        } else {
            serde_json::from_str(&req.context_json)
                .map_err(|e| Status::invalid_argument(format!("Invalid context JSON: {}", e)))?
        };

        // Evaluate
        let ctx = Context::new(context_value);
        let evaluator = Evaluator::new();
        let result = evaluator
            .eval(&expr, &ctx)
            .map_err(|e| Status::internal(format!("Evaluation error: {}", e)))?;

        let result_json = serde_json::to_string(&result)
            .map_err(|e| Status::internal(format!("Failed to serialize result: {}", e)))?;

        Ok(Response::new(EvalResponse {
            result_json,
            parsed_expression: format!("{:?}", expr),
        }))
    }

    /// Health check (does not require tenant validation)
    async fn health(
        &self,
        request: Request<HealthRequest>,
    ) -> std::result::Result<Response<HealthResponse>, Status> {
        // For health check, we use the tenant from metadata if provided, else default
        let tenant_id = self.extract_tenant_id(&request);

        let store = self.store.read().await;
        let ruleset_count = store.list_for_tenant(&tenant_id).len() as u32;

        Ok(Response::new(HealthResponse {
            status: health_response::Status::Serving as i32,
            version: env!("CARGO_PKG_VERSION").to_string(),
            ruleset_count,
            uptime_seconds: self.start_time.elapsed().as_secs(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rate_limiter::RateLimiter;
    use crate::tenant::{TenantDefaults, TenantManager};

    async fn create_test_service() -> OrdoGrpcService {
        let store = Arc::new(RwLock::new(RuleStore::new()));
        let executor = Arc::new(RuleExecutor::new());
        let defaults = TenantDefaults {
            default_qps_limit: Some(1000),
            default_burst_limit: Some(100),
            default_timeout_ms: 100,
        };
        let tenant_manager = Arc::new(TenantManager::new(None, defaults).await.unwrap());
        tenant_manager.ensure_default("default").await.unwrap();
        let rate_limiter = Arc::new(RateLimiter::new());

        // Add a test ruleset
        let mut ruleset = RuleSet::new("test_rule", "start");
        ruleset.add_step(
            Step::decision("start", "Start")
                .branch(Condition::from_string("value > 50"), "high")
                .default("low")
                .build(),
        );
        ruleset.add_step(Step::terminal(
            "high",
            "High",
            TerminalResult::new("HIGH").with_output("level", Expr::literal("high")),
        ));
        ruleset.add_step(Step::terminal(
            "low",
            "Low",
            TerminalResult::new("LOW").with_output("level", Expr::literal("low")),
        ));

        store.write().await.put(ruleset).unwrap();

        OrdoGrpcService::new(
            store,
            executor,
            "default".to_string(),
            tenant_manager,
            rate_limiter,
            false, // multi_tenancy_enabled = false for basic tests
        )
    }

    fn request_with_tenant<T>(inner: T, tenant_id: &str) -> Request<T> {
        let mut request = Request::new(inner);
        request
            .metadata_mut()
            .insert("x-tenant-id", tenant_id.parse().unwrap());
        request
    }

    #[tokio::test]
    async fn test_execute() {
        let service = create_test_service().await;

        let request = Request::new(ExecuteRequest {
            ruleset_name: "test_rule".to_string(),
            input_json: r#"{"value": 75}"#.to_string(),
            include_trace: true,
        });

        let response = service.execute(request).await.unwrap();
        let resp = response.into_inner();

        assert_eq!(resp.code, "HIGH");
        assert!(resp.trace.is_some());
    }

    #[tokio::test]
    async fn test_execute_with_tenant_metadata() {
        let service = create_test_service().await;

        // Test with explicit tenant metadata
        let request = request_with_tenant(
            ExecuteRequest {
                ruleset_name: "test_rule".to_string(),
                input_json: r#"{"value": 75}"#.to_string(),
                include_trace: false,
            },
            "default",
        );

        let response = service.execute(request).await.unwrap();
        let resp = response.into_inner();

        assert_eq!(resp.code, "HIGH");
    }

    #[tokio::test]
    async fn test_execute_not_found() {
        let service = create_test_service().await;

        let request = Request::new(ExecuteRequest {
            ruleset_name: "nonexistent".to_string(),
            input_json: r#"{}"#.to_string(),
            include_trace: false,
        });

        let result = service.execute(request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn test_batch_execute() {
        let service = create_test_service().await;

        let request = Request::new(BatchExecuteRequest {
            ruleset_name: "test_rule".to_string(),
            inputs_json: vec![
                r#"{"value": 75}"#.to_string(),
                r#"{"value": 25}"#.to_string(),
                r#"{"value": 100}"#.to_string(),
            ],
            options: Some(BatchExecuteOptions {
                parallel: true,
                include_trace: false,
            }),
        });

        let response = service.batch_execute(request).await.unwrap();
        let resp = response.into_inner();

        assert_eq!(resp.results.len(), 3);
        assert_eq!(resp.results[0].code, "HIGH");
        assert_eq!(resp.results[1].code, "LOW");
        assert_eq!(resp.results[2].code, "HIGH");

        let summary = resp.summary.unwrap();
        assert_eq!(summary.total, 3);
        assert_eq!(summary.success, 3);
        assert_eq!(summary.failed, 0);
    }

    #[tokio::test]
    async fn test_batch_execute_sequential() {
        let service = create_test_service().await;

        let request = Request::new(BatchExecuteRequest {
            ruleset_name: "test_rule".to_string(),
            inputs_json: vec![
                r#"{"value": 30}"#.to_string(),
                r#"{"value": 60}"#.to_string(),
            ],
            options: Some(BatchExecuteOptions {
                parallel: false,
                include_trace: true,
            }),
        });

        let response = service.batch_execute(request).await.unwrap();
        let resp = response.into_inner();

        assert_eq!(resp.results.len(), 2);
        assert_eq!(resp.results[0].code, "LOW");
        assert_eq!(resp.results[1].code, "HIGH");

        // Check traces are included
        assert!(resp.results[0].trace.is_some());
        assert!(resp.results[1].trace.is_some());
    }

    #[tokio::test]
    async fn test_batch_execute_with_invalid_input() {
        let service = create_test_service().await;

        let request = Request::new(BatchExecuteRequest {
            ruleset_name: "test_rule".to_string(),
            inputs_json: vec![
                r#"{"value": 75}"#.to_string(),
                r#"invalid json"#.to_string(), // Invalid JSON
                r#"{"value": 25}"#.to_string(),
            ],
            options: None,
        });

        let response = service.batch_execute(request).await.unwrap();
        let resp = response.into_inner();

        assert_eq!(resp.results.len(), 3);
        assert_eq!(resp.results[0].code, "HIGH");
        assert_eq!(resp.results[1].code, "error");
        assert!(!resp.results[1].error.is_empty());
        assert_eq!(resp.results[2].code, "LOW");

        let summary = resp.summary.unwrap();
        assert_eq!(summary.total, 3);
        assert_eq!(summary.success, 2);
        assert_eq!(summary.failed, 1);
    }

    #[tokio::test]
    async fn test_batch_execute_empty_inputs() {
        let service = create_test_service().await;

        let request = Request::new(BatchExecuteRequest {
            ruleset_name: "test_rule".to_string(),
            inputs_json: vec![],
            options: None,
        });

        let result = service.batch_execute(request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn test_list_rulesets() {
        let service = create_test_service().await;

        let request = Request::new(ListRuleSetsRequest {
            name_prefix: String::new(),
            limit: 0,
        });

        let response = service.list_rule_sets(request).await.unwrap();
        let resp = response.into_inner();

        assert_eq!(resp.total_count, 1);
        assert_eq!(resp.rulesets[0].name, "test_rule");
    }

    #[tokio::test]
    async fn test_get_ruleset() {
        let service = create_test_service().await;

        let request = Request::new(GetRuleSetRequest {
            name: "test_rule".to_string(),
        });

        let response = service.get_rule_set(request).await.unwrap();
        let resp = response.into_inner();

        assert!(!resp.ruleset_json.is_empty());
        assert_eq!(resp.step_count, 3);
    }

    #[tokio::test]
    async fn test_eval() {
        let service = create_test_service().await;

        let request = Request::new(EvalRequest {
            expression: "age >= 18 && status == \"active\"".to_string(),
            context_json: r#"{"age": 25, "status": "active"}"#.to_string(),
        });

        let response = service.eval(request).await.unwrap();
        let resp = response.into_inner();

        assert_eq!(resp.result_json, "true");
    }

    #[tokio::test]
    async fn test_health() {
        let service = create_test_service().await;

        let request = Request::new(HealthRequest {
            service: String::new(),
        });

        let response = service.health(request).await.unwrap();
        let resp = response.into_inner();

        assert_eq!(resp.status, health_response::Status::Serving as i32);
        assert_eq!(resp.ruleset_count, 1);
    }
}
