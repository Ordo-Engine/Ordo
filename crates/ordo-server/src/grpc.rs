//! gRPC service implementation

use std::sync::Arc;
use std::time::Instant;

use ordo_core::prelude::*;
use ordo_proto::{
    health_response, ordo_service_server::OrdoService, EvalRequest, EvalResponse, ExecuteRequest,
    ExecuteResponse, ExecutionTrace, GetRuleSetRequest, GetRuleSetResponse, HealthRequest,
    HealthResponse, ListRuleSetsRequest, ListRuleSetsResponse, RuleSetSummary, StepTrace,
};
use tokio::sync::RwLock;
use tonic::{Request, Response, Status};

use crate::store::RuleStore;

/// gRPC service implementation
pub struct OrdoGrpcService {
    store: Arc<RwLock<RuleStore>>,
    start_time: Instant,
}

impl OrdoGrpcService {
    /// Create a new gRPC service
    pub fn new(store: Arc<RwLock<RuleStore>>) -> Self {
        Self {
            store,
            start_time: Instant::now(),
        }
    }
}

#[tonic::async_trait]
impl OrdoService for OrdoGrpcService {
    /// Execute a ruleset
    async fn execute(
        &self,
        request: Request<ExecuteRequest>,
    ) -> std::result::Result<Response<ExecuteResponse>, Status> {
        let req = request.into_inner();

        // Parse input JSON
        let input: Value = serde_json::from_str(&req.input_json)
            .map_err(|e| Status::invalid_argument(format!("Invalid input JSON: {}", e)))?;

        // Get ruleset
        let store = self.store.read().await;
        let ruleset = store.get(&req.ruleset_name).ok_or_else(|| {
            Status::not_found(format!("RuleSet '{}' not found", req.ruleset_name))
        })?;

        // Execute
        let result = store
            .executor()
            .execute(&ruleset, input)
            .map_err(|e| Status::internal(format!("Execution error: {}", e)))?;

        // Build response
        let trace = if req.include_trace {
            result.trace.as_ref().map(|t| ExecutionTrace {
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
        } else {
            None
        };

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

    /// Get a ruleset by name
    async fn get_rule_set(
        &self,
        request: Request<GetRuleSetRequest>,
    ) -> std::result::Result<Response<GetRuleSetResponse>, Status> {
        let req = request.into_inner();

        let store = self.store.read().await;
        let ruleset = store
            .get(&req.name)
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

    /// List all rulesets
    async fn list_rule_sets(
        &self,
        request: Request<ListRuleSetsRequest>,
    ) -> std::result::Result<Response<ListRuleSetsResponse>, Status> {
        let req = request.into_inner();

        let store = self.store.read().await;
        let all_rulesets = store.list();

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

    /// Evaluate an expression
    async fn eval(
        &self,
        request: Request<EvalRequest>,
    ) -> std::result::Result<Response<EvalResponse>, Status> {
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

    /// Health check
    async fn health(
        &self,
        _request: Request<HealthRequest>,
    ) -> std::result::Result<Response<HealthResponse>, Status> {
        let store = self.store.read().await;
        let ruleset_count = store.list().len() as u32;

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

    async fn create_test_service() -> OrdoGrpcService {
        let store = Arc::new(RwLock::new(RuleStore::new()));

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

        OrdoGrpcService::new(store)
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
