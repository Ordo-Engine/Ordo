use anyhow::{Context, Result};
use ordo_core::capability::{
    CapabilityCategory, CapabilityDescriptor, CapabilityProvider, CapabilityRegistry,
    CapabilityRequest, CapabilityResponse,
};
use ordo_core::error::{OrdoError, Result as OrdoResult};
use ordo_core::prelude::{
    CompiledRuleExecutor, CompiledRuleSet, ExecutionOptions, ExecutionResult, RuleExecutor,
    RuleSet, Value,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

pub enum LoadedRule {
    Source(RuleSet),
    Compiled(CompiledRuleSet),
}

pub fn load_rule(path: &str) -> Result<LoadedRule> {
    if path.ends_with(".ordo") {
        let compiled = CompiledRuleSet::load_from_file(path)
            .with_context(|| format!("Failed to load compiled rule: {}", path))?;
        return Ok(LoadedRule::Compiled(compiled));
    }

    let content =
        std::fs::read_to_string(path).with_context(|| format!("Failed to read rule: {}", path))?;
    if path.ends_with(".yaml") || path.ends_with(".yml") {
        RuleSet::from_yaml_compiled(&content)
            .map(LoadedRule::Source)
            .map_err(|e| anyhow::anyhow!("Failed to parse YAML rule: {}", e))
    } else {
        RuleSet::from_json_compiled(&content)
            .map(LoadedRule::Source)
            .map_err(|e| anyhow::anyhow!("Failed to parse JSON rule: {}", e))
    }
}

pub fn execute_loaded_rule(
    rule: &LoadedRule,
    input: Value,
    trace: bool,
) -> Result<ExecutionResult> {
    let capability_invoker = build_cli_capability_invoker();

    match rule {
        LoadedRule::Source(ruleset) => {
            let mut executor = RuleExecutor::new();
            executor.set_capability_invoker(capability_invoker);
            let options = if trace {
                Some(ExecutionOptions::default().trace(true))
            } else {
                None
            };
            executor
                .execute_with_options(ruleset, input, options.as_ref())
                .map_err(|e| anyhow::anyhow!("Execution error: {}", e))
        }
        LoadedRule::Compiled(compiled) => {
            if trace {
                eprintln!(
                    "warning: --trace is not supported for compiled .ordo execution; continuing without trace"
                );
            }
            let mut executor = CompiledRuleExecutor::new();
            executor.set_capability_invoker(capability_invoker);
            executor
                .execute(compiled, input)
                .map_err(|e| anyhow::anyhow!("Execution error: {}", e))
        }
    }
}

fn build_cli_capability_invoker() -> Arc<CapabilityRegistry> {
    let registry = Arc::new(CapabilityRegistry::new());
    registry.register(Arc::new(HttpCapability::new()));
    registry.register(Arc::new(MetricCapability));
    registry.register(Arc::new(AuditLoggerCapability));
    registry
}

struct HttpCapability {
    client: reqwest::blocking::Client,
}

impl HttpCapability {
    fn new() -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        Self { client }
    }
}

impl CapabilityProvider for HttpCapability {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor::new("network.http", CapabilityCategory::Network)
    }

    fn invoke(&self, request: &CapabilityRequest) -> OrdoResult<CapabilityResponse> {
        let payload = expect_object(&request.payload, "network.http")?;
        let url = required_string(payload, "url", "network.http")?;
        let method = request
            .operation
            .parse::<reqwest::Method>()
            .map_err(|error| {
                OrdoError::capability_invocation(
                    "network.http",
                    format!("invalid method '{}': {}", request.operation, error),
                )
            })?;
        let headers = optional_tags(payload, "headers", "network.http")?;
        let json_body = payload
            .get("json_body")
            .map(|body| {
                serde_json::to_value(body).map_err(|error| {
                    OrdoError::capability_invocation(
                        "network.http",
                        format!("failed to serialize request body: {}", error),
                    )
                })
            })
            .transpose()?;

        let mut builder = self.client.request(method, url);
        if let Some(timeout_ms) = request.timeout_ms {
            builder = builder.timeout(Duration::from_millis(timeout_ms));
        }
        for (name, value) in headers {
            builder = builder.header(name, value);
        }
        if let Some(json_body) = json_body {
            builder = builder.json(&json_body);
        }

        let response = builder
            .send()
            .map_err(|error| OrdoError::capability_invocation("network.http", error.to_string()))?;
        let status = response.status().as_u16();
        let body_text = response.text().unwrap_or_default();

        let mut response_payload = HashMap::new();
        response_payload.insert("status".to_string(), Value::int(status as i64));
        response_payload.insert("body".to_string(), Value::string(&body_text));
        if let Ok(json_body) = serde_json::from_str::<serde_json::Value>(&body_text) {
            let json_body = serde_json::from_value(json_body).map_err(|error| {
                OrdoError::capability_invocation(
                    "network.http",
                    format!("failed to convert response body: {}", error),
                )
            })?;
            response_payload.insert("json_body".to_string(), json_body);
        }

        Ok(CapabilityResponse::new(Value::object(response_payload))
            .with_metadata("status", status.to_string()))
    }
}

struct MetricCapability;

impl CapabilityProvider for MetricCapability {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor::new("metrics.prometheus", CapabilityCategory::Action)
    }

    fn invoke(&self, request: &CapabilityRequest) -> OrdoResult<CapabilityResponse> {
        Ok(CapabilityResponse::empty().with_metadata("operation", request.operation.clone()))
    }
}

struct AuditLoggerCapability;

impl CapabilityProvider for AuditLoggerCapability {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor::new("audit.logger", CapabilityCategory::Action)
    }

    fn invoke(&self, request: &CapabilityRequest) -> OrdoResult<CapabilityResponse> {
        Ok(CapabilityResponse::empty()
            .with_metadata("event", request.operation.clone())
            .with_metadata("capability", "audit.logger"))
    }
}

fn expect_object<'a>(
    value: &'a Value,
    capability: &str,
) -> OrdoResult<&'a hashbrown::HashMap<ordo_core::context::IString, Value>> {
    value.as_object().ok_or_else(|| {
        OrdoError::capability_invocation(
            capability,
            format!("expected object payload, got {}", value.type_name()),
        )
    })
}

fn required_string<'a>(
    payload: &'a hashbrown::HashMap<ordo_core::context::IString, Value>,
    field: &str,
    capability: &str,
) -> OrdoResult<&'a str> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| OrdoError::capability_invocation(capability, format!("missing {}", field)))
}

fn optional_tags(
    payload: &hashbrown::HashMap<ordo_core::context::IString, Value>,
    field: &str,
    capability: &str,
) -> OrdoResult<Vec<(String, String)>> {
    let Some(value) = payload.get(field) else {
        return Ok(Vec::new());
    };

    let object = value.as_object().ok_or_else(|| {
        OrdoError::capability_invocation(
            capability,
            format!("expected object for '{}', got {}", field, value.type_name()),
        )
    })?;

    object
        .iter()
        .map(|(key, value)| {
            value
                .as_str()
                .map(|value| (key.to_string(), value.to_string()))
                .ok_or_else(|| {
                    OrdoError::capability_invocation(
                        capability,
                        format!("header '{}' must be a string", key),
                    )
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ordo_core::expr::Expr;
    use ordo_core::rule::{
        Action, ActionKind, Condition, RuleSetCompiler, Step, StepKind, SubRuleGraph,
        TerminalResult,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_ordo_path(prefix: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{nonce}.ordo"))
    }

    #[test]
    fn compiled_ordo_rules_execute_through_cli_runtime() {
        let mut ruleset = RuleSet::new("cli_compiled_runtime", "invoke");
        ruleset.add_step(Step::action(
            "invoke",
            "Invoke Audit Capability",
            vec![Action {
                kind: ActionKind::ExternalCall {
                    service: "audit.logger".to_string(),
                    method: "emit".to_string(),
                    params: vec![("message".to_string(), Expr::literal("hello"))],
                    result_variable: Some("audit_result".to_string()),
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
                .with_output("event", Expr::field("$audit_result.metadata.event"))
                .with_output(
                    "capability",
                    Expr::field("$audit_result.metadata.capability"),
                ),
        ));

        let compiled = RuleSetCompiler::compile(&ruleset).unwrap();
        let path = unique_temp_ordo_path("ordo-cli-runtime");
        compiled.save_to_file(&path).unwrap();

        let loaded = load_rule(path.to_str().unwrap()).unwrap();
        let result = execute_loaded_rule(&loaded, Value::object(HashMap::new()), false).unwrap();

        assert_eq!(result.code, "OK");
        assert_eq!(
            result.output.get_path("event"),
            Some(&Value::string("emit"))
        );
        assert_eq!(
            result.output.get_path("capability"),
            Some(&Value::string("audit.logger"))
        );

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn compiled_ordo_sub_rule_executes_through_cli_runtime() {
        let mut sub_steps = hashbrown::HashMap::new();
        sub_steps.insert(
            "classify".to_string(),
            Step::decision("classify", "Classify")
                .branch(Condition::from_string("score >= 90"), "gold")
                .default("silver")
                .build(),
        );
        sub_steps.insert(
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
        sub_steps.insert(
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
        sub_steps.insert(
            "done".to_string(),
            Step::terminal("done", "Done", TerminalResult::new("OK")),
        );

        let mut ruleset = RuleSet::new("cli_compiled_sub_rule", "start");
        ruleset.add_sub_rule(
            "tiering",
            SubRuleGraph {
                entry_step: "classify".to_string(),
                steps: sub_steps,
            },
        );
        ruleset.add_step(Step {
            id: "start".to_string(),
            name: "Start".to_string(),
            kind: StepKind::SubRule {
                ref_name: "tiering".to_string(),
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
        let path = unique_temp_ordo_path("ordo-cli-sub-rule-runtime");
        compiled.save_to_file(&path).unwrap();

        let loaded = load_rule(path.to_str().unwrap()).unwrap();
        let input: Value = serde_json::from_str(r#"{"score":95}"#).unwrap();
        let result = execute_loaded_rule(&loaded, input, false).unwrap();

        assert_eq!(result.code, "OK");
        assert_eq!(result.output.get_path("tier"), Some(&Value::string("gold")));

        std::fs::remove_file(path).unwrap();
    }
}
