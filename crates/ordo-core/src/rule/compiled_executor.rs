//! Executor for compiled rulesets

use super::compiled::{
    CompiledAction, CompiledCondition, CompiledRuleSet, CompiledStep, FIELD_MISSING_LENIENT,
};
use super::metrics::{MetricSink, NoOpMetricSink};
use super::{ExecutionResult, TerminalResult};
use crate::context::{Context, IString, Value};
use crate::error::{OrdoError, Result};
use crate::expr::BytecodeVM;
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
        }
    }

    pub fn with_metric_sink(metric_sink: Arc<dyn MetricSink>) -> Self {
        Self {
            vm: BytecodeVM::new(),
            metric_sink,
        }
    }

    pub fn execute(&self, ruleset: &CompiledRuleSet, input: Value) -> Result<ExecutionResult> {
        let start_time = Instant::now();
        let mut ctx = Context::new(input);
        let mut current_step = ruleset.entry_step;
        let mut depth = 0usize;

        loop {
            if ruleset.metadata.timeout_ms > 0 {
                let elapsed_ms = start_time.elapsed().as_millis() as u64;
                if elapsed_ms >= ruleset.metadata.timeout_ms {
                    return Err(OrdoError::Timeout {
                        timeout_ms: ruleset.metadata.timeout_ms,
                    });
                }
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
                CompiledStep::Terminal {
                    code,
                    message,
                    outputs,
                    data,
                    ..
                } => {
                    let result = TerminalResult {
                        code: ruleset.get_string(*code)?.to_string(),
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
                ctx.set_variable(name.to_string(), val);
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
                let expr = ruleset
                    .expressions
                    .get(*value as usize)
                    .ok_or_else(|| OrdoError::parse_error("Expression index out of range"))?;
                let val = self.vm.execute(expr, ctx)?;
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
                let name = ruleset.get_string(*name)?.to_string();
                let tags = tags
                    .iter()
                    .map(|(k, v)| {
                        Ok((
                            ruleset.get_string(*k)?.to_string(),
                            ruleset.get_string(*v)?.to_string(),
                        ))
                    })
                    .collect::<Result<Vec<(String, String)>>>()?;
                self.metric_sink.record_gauge(&name, metric_value, &tags);
            }
        }
        Ok(())
    }

    fn build_output(
        &self,
        ruleset: &CompiledRuleSet,
        outputs: &[super::compiled::CompiledOutput],
        result: &TerminalResult,
        ctx: &Context,
    ) -> Result<Value> {
        let mut output: hashbrown::HashMap<IString, Value> = hashbrown::HashMap::new();

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
