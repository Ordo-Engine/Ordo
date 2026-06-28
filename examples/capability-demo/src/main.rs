use ordo_core::prelude::*;
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

fn main() {
    let registry = Arc::new(CapabilityRegistry::new());
    registry.register(Arc::new(EchoProvider));

    let mut ruleset = RuleSet::new("capability_demo", "call_echo");
    ruleset.add_step(Step::action(
        "call_echo",
        "Call capability",
        vec![Action {
            kind: ActionKind::ExternalCall {
                service: "demo.echo".to_string(),
                method: "echo".to_string(),
                params: vec![("amount".to_string(), Expr::field("amount"))],
                result_variable: Some("result".to_string()),
                timeout_ms: 250,
            },
            description: "Capability call demo".to_string(),
        }],
        "done",
    ));
    ruleset.add_step(Step::terminal(
        "done",
        "Done",
        TerminalResult::new("OK")
            .with_output("echoed_amount", Expr::field("$result.payload.amount")),
    ));

    let mut executor = RuleExecutor::new();
    executor.set_capability_invoker(registry);

    let input: Value = serde_json::from_str(r#"{"amount": 42}"#).unwrap();
    let result = executor.execute(&ruleset, input).unwrap();

    println!("{:?}", result.output);
}
