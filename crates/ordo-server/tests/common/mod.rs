//! Common test utilities

use std::sync::Arc;
use std::time::Duration;

use ordo_core::prelude::*;
use tokio::sync::RwLock;

/// Rule store for server
pub struct RuleStore {
    rulesets: std::collections::HashMap<String, Arc<RuleSet>>,
    executor: RuleExecutor,
}

impl RuleStore {
    pub fn new() -> Self {
        Self {
            rulesets: std::collections::HashMap::new(),
            executor: RuleExecutor::with_trace(TraceConfig::minimal()),
        }
    }

    pub fn put(&mut self, ruleset: RuleSet) -> std::result::Result<(), Vec<String>> {
        ruleset.validate()?;
        let name = ruleset.config.name.clone();
        self.rulesets.insert(name, Arc::new(ruleset));
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<Arc<RuleSet>> {
        self.rulesets.get(name).cloned()
    }

    pub fn list(&self) -> Vec<RuleSetInfo> {
        self.rulesets
            .values()
            .map(|rs| RuleSetInfo {
                name: rs.config.name.clone(),
                version: rs.config.version.clone(),
                description: rs.config.description.clone(),
                step_count: rs.steps.len(),
            })
            .collect()
    }

    pub fn executor(&self) -> &RuleExecutor {
        &self.executor
    }
}

#[derive(serde::Serialize)]
pub struct RuleSetInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub step_count: usize,
}

/// Create a test store with a sample ruleset
pub async fn create_test_store() -> Arc<RwLock<RuleStore>> {
    let store = Arc::new(RwLock::new(RuleStore::new()));

    // Create test ruleset
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

    store
}

/// Create a gRPC service for testing
pub fn create_grpc_service(store: Arc<RwLock<RuleStore>>) -> OrdoGrpcServiceImpl {
    OrdoGrpcServiceImpl::new(store)
}

/// gRPC service implementation for tests
pub struct OrdoGrpcServiceImpl {
    store: Arc<RwLock<RuleStore>>,
    start_time: std::time::Instant,
}

impl OrdoGrpcServiceImpl {
    pub fn new(store: Arc<RwLock<RuleStore>>) -> Self {
        Self {
            store,
            start_time: std::time::Instant::now(),
        }
    }
}

use ordo_proto::{
    health_response, ordo_service_server::OrdoService, EvalRequest, EvalResponse, ExecuteRequest,
    ExecuteResponse, ExecutionTrace, GetRuleSetRequest, GetRuleSetResponse, HealthRequest,
    HealthResponse, ListRuleSetsRequest, ListRuleSetsResponse, RuleSetSummary, StepTrace,
};
use tonic::{Request, Response, Status};

#[tonic::async_trait]
impl OrdoService for OrdoGrpcServiceImpl {
    async fn execute(
        &self,
        request: Request<ExecuteRequest>,
    ) -> std::result::Result<Response<ExecuteResponse>, Status> {
        let req = request.into_inner();

        let input: Value = serde_json::from_str(&req.input_json)
            .map_err(|e| Status::invalid_argument(format!("Invalid input JSON: {}", e)))?;

        let store = self.store.read().await;
        let ruleset = store.get(&req.ruleset_name).ok_or_else(|| {
            Status::not_found(format!("RuleSet '{}' not found", req.ruleset_name))
        })?;

        let result = store
            .executor()
            .execute(&ruleset, input)
            .map_err(|e| Status::internal(format!("Execution error: {}", e)))?;

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

    async fn list_rule_sets(
        &self,
        request: Request<ListRuleSetsRequest>,
    ) -> std::result::Result<Response<ListRuleSetsResponse>, Status> {
        let req = request.into_inner();

        let store = self.store.read().await;
        let all_rulesets = store.list();

        let filtered: Vec<_> = if req.name_prefix.is_empty() {
            all_rulesets
        } else {
            all_rulesets
                .into_iter()
                .filter(|r| r.name.starts_with(&req.name_prefix))
                .collect()
        };

        let limited: Vec<_> = if req.limit > 0 {
            filtered.into_iter().take(req.limit as usize).collect()
        } else {
            filtered
        };

        let total_count = limited.len() as u32;
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

    async fn eval(
        &self,
        request: Request<EvalRequest>,
    ) -> std::result::Result<Response<EvalResponse>, Status> {
        let req = request.into_inner();

        let expr = ExprParser::parse(&req.expression)
            .map_err(|e| Status::invalid_argument(format!("Invalid expression: {}", e)))?;

        let context_value: Value = if req.context_json.is_empty() {
            Value::object(std::collections::HashMap::new())
        } else {
            serde_json::from_str(&req.context_json)
                .map_err(|e| Status::invalid_argument(format!("Invalid context JSON: {}", e)))?
        };

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

/// Wait for gRPC server to be available over TCP
#[allow(dead_code)]
pub async fn wait_for_server(addr: &str) {
    for _ in 0..50 {
        if ordo_proto::ordo_service_client::OrdoServiceClient::connect(addr.to_string())
            .await
            .is_ok()
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("Server did not start in time");
}
