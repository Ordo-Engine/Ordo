//! Executor for compiled rulesets

use super::compiled::{
    CompiledAction, CompiledCondition, CompiledRuleSet, CompiledStep, CompiledSubRuleBinding,
    CompiledSubRuleGraph, CompiledSubRuleOutput, FIELD_MISSING_LENIENT,
};
use super::metrics::{MetricSink, NoOpMetricSink};
use super::{ExecutionResult, TerminalResult};
use crate::capability::{CapabilityInvoker, CapabilityRequest};
use crate::context::{Context, IString, Value};
use crate::error::{OrdoError, Result};
use crate::expr::BytecodeVM;
use std::collections::HashMap;
use std::sync::Arc;

// Use web_time for WASM, std::time for native
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

#[cfg(target_arch = "wasm32")]
mod wasm_time {
    #[derive(Clone, Copy)]
    pub struct Instant(f64);

    impl Instant {
        pub fn now() -> Self {
            Instant(0.0)
        }

        pub fn elapsed(&self) -> std::time::Duration {
            std::time::Duration::from_micros(0)
        }
    }
}

#[cfg(target_arch = "wasm32")]
use wasm_time::Instant;

pub struct CompiledRuleExecutor {
    vm: BytecodeVM,
    metric_sink: Arc<dyn MetricSink>,
    capability_invoker: Option<Arc<dyn CapabilityInvoker>>,
    max_call_depth: usize,
}

impl Default for CompiledRuleExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl CompiledRuleExecutor {
    pub fn new() -> Self {
        Self {
            vm: BytecodeVM::new(),
            metric_sink: Arc::new(NoOpMetricSink),
            capability_invoker: None,
            max_call_depth: 10,
        }
    }

    pub fn with_metric_sink(metric_sink: Arc<dyn MetricSink>) -> Self {
        Self {
            vm: BytecodeVM::new(),
            metric_sink,
            capability_invoker: None,
            max_call_depth: 10,
        }
    }

    pub fn set_capability_invoker(&mut self, capability_invoker: Arc<dyn CapabilityInvoker>) {
        self.capability_invoker = Some(capability_invoker);
    }

    pub fn capability_invoker(&self) -> Option<Arc<dyn CapabilityInvoker>> {
        self.capability_invoker.clone()
    }

    pub fn execute(&self, ruleset: &CompiledRuleSet, input: Value) -> Result<ExecutionResult> {
        let start_time = Instant::now();
        let mut ctx = Context::new(input);
        let mut current_step = ruleset.entry_step;
        let mut depth = 0usize;
        let remaining_call_depth = self.max_call_depth;

        loop {
            // Amortized timeout: skip the first 16 steps, then check every 16 steps.
            // Avoids Instant::elapsed() syscall overhead for short rules.
            if ruleset.metadata.timeout_ms > 0
                && depth >= 16
                && depth & 15 == 0
                && start_time.elapsed().as_millis() as u64 >= ruleset.metadata.timeout_ms
            {
                return Err(OrdoError::Timeout {
                    timeout_ms: ruleset.metadata.timeout_ms,
                });
            }

            if depth >= ruleset.metadata.max_depth as usize {
                return Err(OrdoError::MaxDepthExceeded {
                    max_depth: ruleset.metadata.max_depth as usize,
                });
            }

            let step = ruleset.get_step(current_step)?;
            match step {
                CompiledStep::Decision {
                    branches,
                    default_next,
                    ..
                } => {
                    let mut matched = false;
                    for branch in branches {
                        let condition =
                            self.evaluate_condition(ruleset, &branch.condition, &ctx)?;
                        if condition {
                            for action in &branch.actions {
                                self.execute_action(ruleset, action, &mut ctx)?;
                            }
                            current_step = branch.next_step;
                            matched = true;
                            break;
                        }
                    }
                    if matched {
                        depth += 1;
                        continue;
                    }
                    if let Some(next) = default_next {
                        current_step = *next;
                        depth += 1;
                        continue;
                    }
                    return Err(OrdoError::eval_error(
                        "No matching branch and no default branch",
                    ));
                }
                CompiledStep::Action {
                    actions, next_step, ..
                } => {
                    for action in actions {
                        self.execute_action(ruleset, action, &mut ctx)?;
                    }
                    current_step = *next_step;
                    depth += 1;
                }
                CompiledStep::SubRule {
                    ref_name,
                    bindings,
                    outputs,
                    next_step,
                    ..
                } => {
                    let child_ctx = self.execute_sub_rule(
                        ruleset,
                        *ref_name,
                        bindings,
                        &ctx,
                        remaining_call_depth,
                    )?;
                    self.copy_sub_rule_outputs(ruleset, outputs, &child_ctx, &mut ctx)?;
                    current_step = *next_step;
                    depth += 1;
                }
                CompiledStep::Terminal {
                    code,
                    message,
                    outputs,
                    data,
                    ..
                } => {
                    let result = TerminalResult {
                        code: ruleset.get_string(*code)?.to_string(), // Code needs to be owned per interface
                        message: ruleset.get_string(*message)?.to_string(),
                        output: Vec::new(),
                        data: data.clone(),
                    };
                    let output = self.build_output(ruleset, outputs, &result, &ctx)?;
                    return Ok(ExecutionResult {
                        code: result.code,
                        message: result.message,
                        output,
                        trace: None,
                        duration_us: start_time.elapsed().as_micros() as u64,
                    });
                }
            }
        }
    }

    fn execute_sub_graph(
        &self,
        ruleset: &CompiledRuleSet,
        graph: &CompiledSubRuleGraph,
        input: Value,
        remaining_call_depth: usize,
    ) -> Result<Context> {
        let mut ctx = Context::new(input);
        let mut current_step = graph.entry_step;
        let mut depth = 0usize;

        loop {
            if depth >= ruleset.metadata.max_depth as usize {
                return Err(OrdoError::MaxDepthExceeded {
                    max_depth: ruleset.metadata.max_depth as usize,
                });
            }

            let step = graph.get_step(current_step)?;
            match step {
                CompiledStep::Decision {
                    branches,
                    default_next,
                    ..
                } => {
                    let mut matched = false;
                    for branch in branches {
                        if self.evaluate_condition(ruleset, &branch.condition, &ctx)? {
                            for action in &branch.actions {
                                self.execute_action(ruleset, action, &mut ctx)?;
                            }
                            current_step = branch.next_step;
                            matched = true;
                            break;
                        }
                    }
                    if matched {
                        depth += 1;
                        continue;
                    }
                    if let Some(next) = default_next {
                        current_step = *next;
                        depth += 1;
                        continue;
                    }
                    return Err(OrdoError::eval_error(
                        "No matching branch and no default branch",
                    ));
                }
                CompiledStep::Action {
                    actions, next_step, ..
                } => {
                    for action in actions {
                        self.execute_action(ruleset, action, &mut ctx)?;
                    }
                    current_step = *next_step;
                    depth += 1;
                }
                CompiledStep::SubRule {
                    ref_name,
                    bindings,
                    outputs,
                    next_step,
                    ..
                } => {
                    let child_ctx = self.execute_sub_rule(
                        ruleset,
                        *ref_name,
                        bindings,
                        &ctx,
                        remaining_call_depth,
                    )?;
                    self.copy_sub_rule_outputs(ruleset, outputs, &child_ctx, &mut ctx)?;
                    current_step = *next_step;
                    depth += 1;
                }
                CompiledStep::Terminal { .. } => return Ok(ctx),
            }
        }
    }

    fn execute_sub_rule(
        &self,
        ruleset: &CompiledRuleSet,
        ref_name: u32,
        bindings: &[CompiledSubRuleBinding],
        parent_ctx: &Context,
        remaining_call_depth: usize,
    ) -> Result<Context> {
        if remaining_call_depth == 0 {
            let name = ruleset.get_string(ref_name).unwrap_or("<unknown>");
            return Err(OrdoError::eval_error(format!(
                "SubRule max nesting depth ({}) exceeded calling '{}'",
                self.max_call_depth, name
            )));
        }

        let graph = ruleset.get_sub_rule(ref_name)?;
        let mut child_data = std::collections::HashMap::with_capacity(bindings.len());
        for binding in bindings {
            let name = ruleset.get_string(binding.name)?;
            if let Some(value) =
                self.evaluate_sub_rule_binding(ruleset, binding.expr, parent_ctx)?
            {
                child_data.insert(name.to_string(), value);
            }
        }

        self.execute_sub_graph(
            ruleset,
            graph,
            Value::object(child_data),
            remaining_call_depth - 1,
        )
    }

    fn evaluate_sub_rule_binding(
        &self,
        ruleset: &CompiledRuleSet,
        expr_idx: u32,
        ctx: &Context,
    ) -> Result<Option<Value>> {
        match self.evaluate_expr(ruleset, expr_idx, ctx) {
            Ok(value) => Ok(Some(value)),
            Err(OrdoError::FieldNotFound { .. })
                if ruleset.metadata.field_missing == FIELD_MISSING_LENIENT =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    fn copy_sub_rule_outputs(
        &self,
        ruleset: &CompiledRuleSet,
        outputs: &[CompiledSubRuleOutput],
        child_ctx: &Context,
        parent_ctx: &mut Context,
    ) -> Result<()> {
        for output in outputs {
            let child_variable = ruleset.get_string(output.child_variable)?;
            if let Some(value) = child_ctx.variables().get(child_variable) {
                let parent_variable = ruleset.get_string(output.parent_variable)?;
                parent_ctx.set_variable(parent_variable, value.clone());
            }
        }
        Ok(())
    }

    fn evaluate_condition(
        &self,
        ruleset: &CompiledRuleSet,
        condition: &CompiledCondition,
        ctx: &Context,
    ) -> Result<bool> {
        match condition {
            CompiledCondition::Always => Ok(true),
            CompiledCondition::Expr(idx) => {
                let expr = ruleset
                    .expressions
                    .get(*idx as usize)
                    .ok_or_else(|| OrdoError::parse_error("Expression index out of range"))?;
                match self.vm.execute(expr, ctx) {
                    Ok(value) => Ok(value.is_truthy()),
                    Err(OrdoError::FieldNotFound { .. })
                        if ruleset.metadata.field_missing == FIELD_MISSING_LENIENT =>
                    {
                        Ok(false)
                    }
                    Err(e) => Err(e),
                }
            }
        }
    }

    fn execute_action(
        &self,
        ruleset: &CompiledRuleSet,
        action: &CompiledAction,
        ctx: &mut Context,
    ) -> Result<()> {
        match action {
            CompiledAction::SetVariable { name, value } => {
                let expr = ruleset
                    .expressions
                    .get(*value as usize)
                    .ok_or_else(|| OrdoError::parse_error("Expression index out of range"))?;
                let val = self.vm.execute(expr, ctx)?;
                let name = ruleset.get_string(*name)?;
                ctx.set_variable(name, val);
            }
            CompiledAction::Log { message, level } => {
                let msg = ruleset.get_string(*message)?;
                match *level {
                    0 => tracing::debug!(message = %msg, "Rule action"),
                    1 => tracing::info!(message = %msg, "Rule action"),
                    2 => tracing::warn!(message = %msg, "Rule action"),
                    _ => tracing::error!(message = %msg, "Rule action"),
                }
            }
            CompiledAction::Metric { name, value, tags } => {
                let val = self.evaluate_expr(ruleset, *value, ctx)?;
                let metric_value = match &val {
                    Value::Int(i) => *i as f64,
                    Value::Float(f) => *f,
                    Value::Bool(b) => {
                        if *b {
                            1.0
                        } else {
                            0.0
                        }
                    }
                    _ => {
                        tracing::warn!("Cannot convert value to metric");
                        return Ok(());
                    }
                };
                let name = ruleset.get_string(*name)?;
                let tags = tags
                    .iter()
                    .map(|(k, v)| {
                        Ok((
                            ruleset.get_string(*k)?.to_string(),
                            ruleset.get_string(*v)?.to_string(),
                        ))
                    })
                    .collect::<Result<Vec<(String, String)>>>()?;
                self.record_metric(name, metric_value, &tags)?;
            }
            CompiledAction::ExternalCall {
                service,
                method,
                params,
                result_variable,
                timeout_ms,
            } => {
                let capability_invoker = self.capability_invoker.as_ref().ok_or_else(|| {
                    OrdoError::eval_error_static("ExternalCall requires a capability invoker")
                })?;

                let service_name = ruleset.get_string(*service)?;
                let operation = ruleset.get_string(*method)?;
                let mut payload = HashMap::with_capacity(params.len());
                for (name, expr) in params {
                    payload.insert(
                        ruleset.get_string(*name)?.to_string(),
                        self.evaluate_expr(ruleset, *expr, ctx)?,
                    );
                }

                let mut request = CapabilityRequest::new(
                    service_name.to_string(),
                    operation.to_string(),
                    Value::object(payload),
                );
                if *timeout_ms > 0 {
                    request = request.with_timeout(*timeout_ms);
                }

                let response = capability_invoker.invoke(&request)?;
                if let Some(result_variable) = result_variable {
                    let response_obj = build_capability_response_value(
                        service_name,
                        operation,
                        response.payload,
                        response.metadata,
                    );
                    ctx.set_variable(ruleset.get_string(*result_variable)?, response_obj);
                }
            }
        }
        Ok(())
    }

    fn evaluate_expr(
        &self,
        ruleset: &CompiledRuleSet,
        expr_idx: u32,
        ctx: &Context,
    ) -> Result<Value> {
        let expr = ruleset
            .expressions
            .get(expr_idx as usize)
            .ok_or_else(|| OrdoError::parse_error("Expression index out of range"))?;
        self.vm.execute(expr, ctx)
    }

    fn record_metric(&self, name: &str, value: f64, tags: &[(String, String)]) -> Result<()> {
        if let Some(capability_invoker) = &self.capability_invoker {
            let mut tag_values = std::collections::HashMap::with_capacity(tags.len());
            for (key, value) in tags {
                tag_values.insert(key.clone(), Value::string(value));
            }

            let mut payload = std::collections::HashMap::with_capacity(3);
            payload.insert("name".to_string(), Value::string(name));
            payload.insert("value".to_string(), Value::float(value));
            payload.insert("tags".to_string(), Value::object(tag_values));

            let request =
                CapabilityRequest::new("metrics.prometheus", "gauge", Value::object(payload));

            match capability_invoker.invoke(&request) {
                Ok(_) => return Ok(()),
                Err(OrdoError::CapabilityNotFound { .. }) => {}
                Err(error) => return Err(error),
            }
        }

        self.metric_sink.record_gauge(name, value, tags);
        Ok(())
    }

    fn build_output(
        &self,
        ruleset: &CompiledRuleSet,
        outputs: &[super::compiled::CompiledOutput],
        result: &TerminalResult,
        ctx: &Context,
    ) -> Result<Value> {
        let data_len = match &result.data {
            Value::Object(map) => map.len(),
            _ => 0,
        };
        let mut output: hashbrown::HashMap<IString, Value> =
            hashbrown::HashMap::with_capacity(outputs.len() + data_len);

        for item in outputs {
            let expr = ruleset
                .expressions
                .get(item.expr as usize)
                .ok_or_else(|| OrdoError::parse_error("Expression index out of range"))?;
            let value = self.vm.execute(expr, ctx)?;
            let key = ruleset.get_string(item.key)?;
            output.insert(Arc::from(key), value);
        }

        if let Value::Object(data) = &result.data {
            for (k, v) in data {
                output.insert(k.clone(), v.clone());
            }
        }

        Ok(Value::object_optimized(output))
    }
}

fn build_capability_response_value(
    service: &str,
    operation: &str,
    payload: Value,
    metadata: HashMap<String, String>,
) -> Value {
    let metadata = Value::object(
        metadata
            .into_iter()
            .map(|(key, value)| (key, Value::string(value)))
            .collect(),
    );

    Value::object({
        let mut response = HashMap::with_capacity(4);
        response.insert("capability".to_string(), Value::string(service));
        response.insert("operation".to_string(), Value::string(operation));
        response.insert("payload".to_string(), payload);
        response.insert("metadata".to_string(), metadata);
        response
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::{
        CapabilityCategory, CapabilityDescriptor, CapabilityProvider, CapabilityRegistry,
        CapabilityRequest, CapabilityResponse,
    };
    use crate::expr::Expr;
    use crate::rule::metrics::MetricSink;
    use crate::rule::{
        Action, ActionKind, RuleSet, RuleSetCompiler, Step, StepKind, SubRuleGraph, TerminalResult,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestMetricSink {
        gauge_calls: AtomicUsize,
    }

    impl MetricSink for TestMetricSink {
        fn record_gauge(&self, _name: &str, _value: f64, _tags: &[(String, String)]) {
            self.gauge_calls.fetch_add(1, Ordering::SeqCst);
        }

        fn record_counter(&self, _name: &str, _value: f64, _tags: &[(String, String)]) {}
    }

    struct TestMetricCapability {
        calls: AtomicUsize,
    }

    impl CapabilityProvider for TestMetricCapability {
        fn descriptor(&self) -> CapabilityDescriptor {
            CapabilityDescriptor::new("metrics.prometheus", CapabilityCategory::Action)
        }

        fn invoke(&self, _request: &CapabilityRequest) -> Result<CapabilityResponse> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(CapabilityResponse::empty())
        }
    }

    struct EchoCapability;

    impl CapabilityProvider for EchoCapability {
        fn descriptor(&self) -> CapabilityDescriptor {
            CapabilityDescriptor::new("network.http", CapabilityCategory::Network)
        }

        fn invoke(&self, request: &CapabilityRequest) -> Result<CapabilityResponse> {
            let payload = match &request.payload {
                Value::Object(payload) => payload,
                other => {
                    return Err(OrdoError::capability_invocation(
                        "network.http",
                        format!("expected object payload, got {other:?}"),
                    ));
                }
            };

            let url = payload
                .get("url")
                .cloned()
                .ok_or_else(|| OrdoError::capability_invocation("network.http", "missing url"))?;
            let amount = payload.get("amount").cloned().ok_or_else(|| {
                OrdoError::capability_invocation("network.http", "missing amount")
            })?;
            let json_body = payload.get("json_body").cloned();

            Ok(CapabilityResponse::new(Value::object({
                let mut response = HashMap::new();
                response.insert("status".to_string(), Value::int(200));
                response.insert("method".to_string(), Value::string(&request.operation));
                response.insert("url".to_string(), url);
                response.insert("echoed_amount".to_string(), amount);
                if let Some(json_body) = json_body {
                    response.insert("json_body".to_string(), json_body);
                }
                response
            }))
            .with_metadata("provider", "echo"))
        }
    }

    #[test]
    fn compiled_executor_prefers_capability_metrics_when_available() {
        let mut ruleset = RuleSet::new("compiled_metric_test", "record_metric");
        ruleset.add_step(Step::action(
            "record_metric",
            "Record Metric",
            vec![Action {
                kind: ActionKind::Metric {
                    name: "compiled_metric".to_string(),
                    value: Expr::literal(9.0f64),
                    tags: vec![("env".to_string(), "test".to_string())],
                },
                description: String::new(),
            }],
            "done",
        ));
        ruleset.add_step(Step::terminal("done", "Done", TerminalResult::new("OK")));
        let compiled = RuleSetCompiler::compile(&ruleset).unwrap();

        let sink = Arc::new(TestMetricSink {
            gauge_calls: AtomicUsize::new(0),
        });
        let mut executor = CompiledRuleExecutor::with_metric_sink(sink.clone());
        let registry = Arc::new(CapabilityRegistry::new());
        let capability = Arc::new(TestMetricCapability {
            calls: AtomicUsize::new(0),
        });
        let capability_ref = capability.clone();
        registry.register(capability);
        executor.set_capability_invoker(registry);

        let input = serde_json::from_str(r#"{}"#).unwrap();
        let result = executor.execute(&compiled, input).unwrap();

        assert_eq!(result.code, "OK");
        assert_eq!(capability_ref.calls.load(Ordering::SeqCst), 1);
        assert_eq!(sink.gauge_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn compiled_executor_supports_external_call_capabilities() {
        let mut ruleset = RuleSet::new("compiled_external_call_test", "invoke");
        ruleset.add_step(Step::action(
            "invoke",
            "Invoke Capability",
            vec![Action {
                kind: ActionKind::ExternalCall {
                    service: "network.http".to_string(),
                    method: "POST".to_string(),
                    params: vec![
                        (
                            "url".to_string(),
                            Expr::literal("https://example.test/score"),
                        ),
                        ("amount".to_string(), Expr::field("amount")),
                    ],
                    result_variable: Some("http_result".to_string()),
                    timeout_ms: 250,
                },
                description: String::new(),
            }],
            "done",
        ));
        ruleset.add_step(Step::terminal(
            "done",
            "Done",
            TerminalResult::new("OK")
                .with_output("status", Expr::field("$http_result.payload.status"))
                .with_output("method", Expr::field("$http_result.payload.method"))
                .with_output("amount", Expr::field("$http_result.payload.echoed_amount"))
                .with_output("provider", Expr::field("$http_result.metadata.provider")),
        ));

        let compiled = RuleSetCompiler::compile(&ruleset).unwrap();
        let registry = Arc::new(CapabilityRegistry::new());
        registry.register(Arc::new(EchoCapability));

        let mut executor = CompiledRuleExecutor::new();
        executor.set_capability_invoker(registry);

        let input = serde_json::from_str(r#"{"amount": 42}"#).unwrap();
        let result = executor.execute(&compiled, input).unwrap();

        assert_eq!(result.code, "OK");
        assert_eq!(result.output.get_path("status"), Some(&Value::int(200)));
        assert_eq!(
            result.output.get_path("method"),
            Some(&Value::string("POST"))
        );
        assert_eq!(result.output.get_path("amount"), Some(&Value::int(42)));
        assert_eq!(
            result.output.get_path("provider"),
            Some(&Value::string("echo"))
        );
    }

    #[test]
    fn compiled_ruleset_external_call_survives_serialize_roundtrip() {
        let mut ruleset = RuleSet::new("compiled_external_roundtrip", "invoke");
        ruleset.add_step(Step::action(
            "invoke",
            "Invoke Capability",
            vec![Action {
                kind: ActionKind::ExternalCall {
                    service: "network.http".to_string(),
                    method: "POST".to_string(),
                    params: vec![
                        (
                            "url".to_string(),
                            Expr::literal("https://example.test/roundtrip"),
                        ),
                        ("amount".to_string(), Expr::field("amount")),
                    ],
                    result_variable: Some("http_result".to_string()),
                    timeout_ms: 100,
                },
                description: String::new(),
            }],
            "done",
        ));
        ruleset.add_step(Step::terminal(
            "done",
            "Done",
            TerminalResult::new("OK")
                .with_output("amount", Expr::field("$http_result.payload.echoed_amount")),
        ));

        let compiled = RuleSetCompiler::compile(&ruleset).unwrap();
        let bytes = compiled.serialize();
        let decoded = CompiledRuleSet::deserialize(&bytes).unwrap();
        let registry = Arc::new(CapabilityRegistry::new());
        registry.register(Arc::new(EchoCapability));

        let mut executor = CompiledRuleExecutor::new();
        executor.set_capability_invoker(registry);

        let input = serde_json::from_str(r#"{"amount": 17}"#).unwrap();
        let result = executor.execute(&decoded, input).unwrap();
        assert_eq!(result.output.get_path("amount"), Some(&Value::int(17)));
    }

    #[test]
    fn compiled_ruleset_sub_rule_survives_serialize_roundtrip() {
        let mut normalize_steps = hashbrown::HashMap::new();
        normalize_steps.insert(
            "set_score".to_string(),
            Step::action(
                "set_score",
                "Set Score",
                vec![Action {
                    kind: ActionKind::SetVariable {
                        name: "normalized".to_string(),
                        value: Expr::field("raw_score"),
                    },
                    description: String::new(),
                }],
                "done",
            ),
        );
        normalize_steps.insert(
            "done".to_string(),
            Step::terminal("done", "Done", TerminalResult::new("OK")),
        );

        let mut classify_steps = hashbrown::HashMap::new();
        classify_steps.insert(
            "normalize".to_string(),
            Step {
                id: "normalize".to_string(),
                name: "Normalize".to_string(),
                kind: StepKind::SubRule {
                    ref_name: "normalize_score".to_string(),
                    bindings: vec![("raw_score".to_string(), Expr::field("score"))],
                    outputs: vec![("score_for_tier".to_string(), "normalized".to_string())],
                    next_step: "check".to_string(),
                },
            },
        );
        classify_steps.insert(
            "check".to_string(),
            Step::decision("check", "Check")
                .branch(
                    crate::rule::Condition::from_string("$score_for_tier >= 90"),
                    "gold",
                )
                .default("silver")
                .build(),
        );
        classify_steps.insert(
            "gold".to_string(),
            Step::action(
                "gold",
                "Gold",
                vec![Action {
                    kind: ActionKind::SetVariable {
                        name: "tier".to_string(),
                        value: Expr::literal("gold"),
                    },
                    description: String::new(),
                }],
                "done",
            ),
        );
        classify_steps.insert(
            "silver".to_string(),
            Step::action(
                "silver",
                "Silver",
                vec![Action {
                    kind: ActionKind::SetVariable {
                        name: "tier".to_string(),
                        value: Expr::literal("silver"),
                    },
                    description: String::new(),
                }],
                "done",
            ),
        );
        classify_steps.insert(
            "done".to_string(),
            Step::terminal("done", "Done", TerminalResult::new("OK")),
        );

        let mut ruleset = RuleSet::new("compiled_sub_rule_roundtrip", "classify");
        ruleset.add_sub_rule(
            "normalize_score",
            SubRuleGraph {
                entry_step: "set_score".to_string(),
                steps: normalize_steps,
            },
        );
        ruleset.add_sub_rule(
            "classify_score",
            SubRuleGraph {
                entry_step: "normalize".to_string(),
                steps: classify_steps,
            },
        );
        ruleset.add_step(Step {
            id: "classify".to_string(),
            name: "Classify".to_string(),
            kind: StepKind::SubRule {
                ref_name: "classify_score".to_string(),
                bindings: vec![("score".to_string(), Expr::field("score"))],
                outputs: vec![("tier".to_string(), "tier".to_string())],
                next_step: "done".to_string(),
            },
        });
        ruleset.add_step(Step::terminal(
            "done",
            "Done",
            TerminalResult::new("OK").with_output("tier", Expr::field("$tier")),
        ));

        ruleset.validate().unwrap();
        let compiled = RuleSetCompiler::compile(&ruleset).unwrap();
        let bytes = compiled.serialize();
        let decoded = CompiledRuleSet::deserialize(&bytes).unwrap();
        let executor = CompiledRuleExecutor::new();

        let input = serde_json::from_str(r#"{"score": 95}"#).unwrap();
        let result = executor.execute(&decoded, input).unwrap();

        assert_eq!(result.code, "OK");
        assert_eq!(result.output.get_path("tier"), Some(&Value::string("gold")));
    }

    #[test]
    fn compiled_sub_rule_bindings_follow_lenient_missing_field_behavior() {
        let mut sub_steps = hashbrown::HashMap::new();
        sub_steps.insert(
            "check_score".to_string(),
            Step::decision("check_score", "Check Score")
                .branch(
                    crate::rule::Condition::from_string("score >= 90"),
                    "tier_gold",
                )
                .default("tier_silver")
                .build(),
        );
        sub_steps.insert(
            "tier_gold".to_string(),
            Step::action(
                "tier_gold",
                "Gold",
                vec![Action {
                    kind: ActionKind::SetVariable {
                        name: "tier".to_string(),
                        value: Expr::literal("gold"),
                    },
                    description: String::new(),
                }],
                "done",
            ),
        );
        sub_steps.insert(
            "tier_silver".to_string(),
            Step::action(
                "tier_silver",
                "Silver",
                vec![Action {
                    kind: ActionKind::SetVariable {
                        name: "tier".to_string(),
                        value: Expr::literal("silver"),
                    },
                    description: String::new(),
                }],
                "done",
            ),
        );
        sub_steps.insert(
            "done".to_string(),
            Step::terminal("done", "Done", TerminalResult::new("OK")),
        );

        let mut ruleset = RuleSet::new("compiled_sub_rule_lenient", "start");
        ruleset.add_sub_rule(
            "classify",
            SubRuleGraph {
                entry_step: "check_score".to_string(),
                steps: sub_steps,
            },
        );
        ruleset.add_step(Step {
            id: "start".to_string(),
            name: "Start".to_string(),
            kind: StepKind::SubRule {
                ref_name: "classify".to_string(),
                bindings: vec![("score".to_string(), Expr::field("score"))],
                outputs: vec![("result_tier".to_string(), "tier".to_string())],
                next_step: "done".to_string(),
            },
        });
        ruleset.add_step(Step::terminal(
            "done",
            "Done",
            TerminalResult::new("DONE").with_output("tier", Expr::field("$result_tier")),
        ));

        ruleset.validate().unwrap();
        let compiled = RuleSetCompiler::compile(&ruleset).unwrap();
        let executor = CompiledRuleExecutor::new();

        let input = serde_json::from_str(r#"{}"#).unwrap();
        let result = executor.execute(&compiled, input).unwrap();

        assert_eq!(result.code, "DONE");
        assert_eq!(
            result.output.get_path("tier"),
            Some(&Value::string("silver"))
        );
    }

    #[test]
    fn compiled_executor_preserves_object_payloads_for_external_calls() {
        let mut ruleset = RuleSet::new("compiled_external_payload_test", "invoke");
        ruleset.add_step(Step::action(
            "invoke",
            "Invoke Capability",
            vec![Action {
                kind: ActionKind::ExternalCall {
                    service: "network.http".to_string(),
                    method: "POST".to_string(),
                    params: vec![
                        (
                            "url".to_string(),
                            Expr::literal("https://example.test/object-payload"),
                        ),
                        (
                            "json_body".to_string(),
                            Expr::Object(vec![
                                ("hello".to_string(), Expr::literal("world")),
                                ("amount".to_string(), Expr::field("amount")),
                            ]),
                        ),
                        ("amount".to_string(), Expr::field("amount")),
                    ],
                    result_variable: Some("http_result".to_string()),
                    timeout_ms: 250,
                },
                description: String::new(),
            }],
            "done",
        ));
        ruleset.add_step(Step::terminal(
            "done",
            "Done",
            TerminalResult::new("OK")
                .with_output("hello", Expr::field("$http_result.payload.json_body.hello"))
                .with_output(
                    "amount",
                    Expr::field("$http_result.payload.json_body.amount"),
                ),
        ));

        let compiled = RuleSetCompiler::compile(&ruleset).unwrap();
        let registry = Arc::new(CapabilityRegistry::new());
        registry.register(Arc::new(EchoCapability));

        let mut executor = CompiledRuleExecutor::new();
        executor.set_capability_invoker(registry);

        let input = serde_json::from_str(r#"{"amount": 42}"#).unwrap();
        let result = executor.execute(&compiled, input).unwrap();

        assert_eq!(result.code, "OK");
        assert_eq!(
            result.output.get_path("hello"),
            Some(&Value::string("world"))
        );
        assert_eq!(result.output.get_path("amount"), Some(&Value::int(42)));
    }
}
