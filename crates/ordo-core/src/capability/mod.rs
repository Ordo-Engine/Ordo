mod provider;
mod registry;

pub use provider::{
    CapabilityCategory, CapabilityConfig, CapabilityDescriptor, CapabilityInvoker,
    CapabilityProvider, CapabilityRequest, CapabilityResponse, CircuitBreakerConfig, RetryPolicy,
};
pub use registry::CapabilityRegistry;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::{Action, ActionKind, Expr, RuleExecutor, RuleSet, Step, TerminalResult};
    use crate::{context::Value, error::Result};
    use std::sync::Arc;

    struct EchoProvider;

    impl CapabilityProvider for EchoProvider {
        fn descriptor(&self) -> CapabilityDescriptor {
            CapabilityDescriptor::new("demo.echo", CapabilityCategory::Compute)
                .with_description("Echo payloads back to the caller")
        }

        fn invoke(&self, request: &CapabilityRequest) -> Result<CapabilityResponse> {
            Ok(CapabilityResponse::new(request.payload.clone()))
        }
    }

    #[test]
    fn executor_external_call_routes_through_capability_registry() {
        let registry = Arc::new(CapabilityRegistry::new());
        registry.register(Arc::new(EchoProvider));

        let mut ruleset = RuleSet::new("capability_demo", "call_echo");
        ruleset.add_step(Step::action(
            "call_echo",
            "Call echo",
            vec![Action {
                kind: ActionKind::ExternalCall {
                    service: "demo.echo".to_string(),
                    method: "echo".to_string(),
                    params: vec![("amount".to_string(), Expr::field("amount"))],
                    timeout_ms: 250,
                    result_variable: Some("capability_result".to_string()),
                },
                description: String::new(),
            }],
            "done",
        ));
        ruleset.add_step(Step::terminal(
            "done",
            "Done",
            TerminalResult::new("OK").with_output(
                "echoed_amount",
                Expr::field("$capability_result.payload.amount"),
            ),
        ));

        let mut executor = RuleExecutor::new();
        executor.set_capability_invoker(registry);

        let input: Value = serde_json::from_str(r#"{"amount": 42}"#).unwrap();
        let result = executor.execute(&ruleset, input).unwrap();

        let amount = result
            .output
            .get_path("echoed_amount")
            .expect("echoed amount missing");
        assert_eq!(amount, &Value::int(42));
    }
}
