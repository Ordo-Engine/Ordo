//! Rule executor
//!
//! Executes rule sets against input data

use super::model::{FieldMissingBehavior, RuleSet};
use super::step::{ActionKind, Condition, LogLevel, Step, StepKind, TerminalResult};
use crate::context::{Context, Value};
use crate::error::{OrdoError, Result};
use crate::expr::{Evaluator, ExprParser};
use crate::trace::{ExecutionTrace, StepTrace, TraceConfig};
use std::collections::HashMap;
use std::time::Instant;

/// Rule executor
pub struct RuleExecutor {
    /// Expression evaluator
    evaluator: Evaluator,
    /// Trace configuration
    trace_config: TraceConfig,
}

impl Default for RuleExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleExecutor {
    /// Create a new executor
    pub fn new() -> Self {
        Self {
            evaluator: Evaluator::new(),
            trace_config: TraceConfig::default(),
        }
    }

    /// Create executor with trace config
    pub fn with_trace(trace_config: TraceConfig) -> Self {
        Self {
            evaluator: Evaluator::new(),
            trace_config,
        }
    }

    /// Get evaluator for customization
    pub fn evaluator_mut(&mut self) -> &mut Evaluator {
        &mut self.evaluator
    }

    /// Execute a rule set
    pub fn execute(&self, ruleset: &RuleSet, input: Value) -> Result<ExecutionResult> {
        let start_time = Instant::now();
        let mut ctx = Context::new(input);
        let mut trace = if self.trace_config.enabled || ruleset.config.enable_trace {
            Some(ExecutionTrace::new(&ruleset.config.name))
        } else {
            None
        };

        let mut current_step_id = ruleset.config.entry_step.clone();
        let mut depth = 0;

        loop {
            // Check depth limit
            if depth >= ruleset.config.max_depth {
                return Err(OrdoError::MaxDepthExceeded {
                    max_depth: ruleset.config.max_depth,
                });
            }

            // Get current step
            let step =
                ruleset
                    .get_step(&current_step_id)
                    .ok_or_else(|| OrdoError::StepNotFound {
                        step_id: current_step_id.clone(),
                    })?;

            let step_start = Instant::now();

            // Execute step
            let step_result = self.execute_step(step, &mut ctx, &ruleset.config.field_missing)?;
            let step_duration = step_start.elapsed().as_micros() as u64;

            // Record trace
            if let Some(ref mut trace) = trace {
                let step_trace = match &step_result {
                    StepResult::Continue { next_step } => {
                        let mut st =
                            StepTrace::continued(&step.id, &step.name, step_duration, next_step);
                        if self.trace_config.capture_input {
                            st.input_snapshot = Some(ctx.data().clone());
                        }
                        if self.trace_config.capture_variables {
                            st.variables_snapshot = Some(ctx.variables().clone());
                        }
                        st
                    }
                    StepResult::Terminal { .. } => {
                        let mut st = StepTrace::terminal(&step.id, &step.name, step_duration);
                        if self.trace_config.capture_input {
                            st.input_snapshot = Some(ctx.data().clone());
                        }
                        if self.trace_config.capture_variables {
                            st.variables_snapshot = Some(ctx.variables().clone());
                        }
                        st
                    }
                };
                trace.add_step(step_trace);
            }

            // Handle step result
            match step_result {
                StepResult::Continue { next_step } => {
                    current_step_id = next_step;
                    depth += 1;
                }
                StepResult::Terminal { result } => {
                    let output = self.build_output(&result, &ctx)?;
                    return Ok(ExecutionResult {
                        code: result.code,
                        message: result.message,
                        output,
                        trace,
                        duration_us: start_time.elapsed().as_micros() as u64,
                    });
                }
            }
        }
    }

    /// Execute a single step
    fn execute_step(
        &self,
        step: &Step,
        ctx: &mut Context,
        field_missing: &FieldMissingBehavior,
    ) -> Result<StepResult> {
        match &step.kind {
            StepKind::Decision {
                branches,
                default_next,
            } => {
                // Evaluate branches in order
                for branch in branches {
                    let condition_result =
                        self.evaluate_condition(&branch.condition, ctx, field_missing)?;

                    if condition_result {
                        // Execute branch actions
                        for action in &branch.actions {
                            self.execute_action(action, ctx)?;
                        }
                        return Ok(StepResult::Continue {
                            next_step: branch.next_step.clone(),
                        });
                    }
                }

                // No branch matched, use default
                if let Some(default) = default_next {
                    Ok(StepResult::Continue {
                        next_step: default.clone(),
                    })
                } else {
                    Err(OrdoError::eval_error(format!(
                        "No matching branch in step '{}' and no default",
                        step.id
                    )))
                }
            }

            StepKind::Action { actions, next_step } => {
                // Execute all actions
                for action in actions {
                    self.execute_action(action, ctx)?;
                }
                Ok(StepResult::Continue {
                    next_step: next_step.clone(),
                })
            }

            StepKind::Terminal { result } => Ok(StepResult::Terminal {
                result: result.clone(),
            }),
        }
    }

    /// Evaluate a condition
    fn evaluate_condition(
        &self,
        condition: &Condition,
        ctx: &Context,
        field_missing: &FieldMissingBehavior,
    ) -> Result<bool> {
        match condition {
            Condition::Always => Ok(true),

            Condition::Expression(expr) => match self.evaluator.eval(expr, ctx) {
                Ok(value) => Ok(value.is_truthy()),
                Err(OrdoError::FieldNotFound { .. })
                    if *field_missing == FieldMissingBehavior::Lenient =>
                {
                    Ok(false)
                }
                Err(e) => Err(e),
            },

            Condition::ExpressionString(s) => {
                let expr = ExprParser::parse(s)?;
                match self.evaluator.eval(&expr, ctx) {
                    Ok(value) => Ok(value.is_truthy()),
                    Err(OrdoError::FieldNotFound { .. })
                        if *field_missing == FieldMissingBehavior::Lenient =>
                    {
                        Ok(false)
                    }
                    Err(e) => Err(e),
                }
            }
        }
    }

    /// Execute an action
    fn execute_action(&self, action: &super::step::Action, ctx: &mut Context) -> Result<()> {
        match &action.kind {
            ActionKind::SetVariable { name, value } => {
                let val = self.evaluator.eval(value, ctx)?;
                ctx.set_variable(name, val);
            }

            ActionKind::Log { message, level } => {
                // Use tracing for logging
                match level {
                    LogLevel::Debug => tracing::debug!(message = %message, "Rule action"),
                    LogLevel::Info => tracing::info!(message = %message, "Rule action"),
                    LogLevel::Warn => tracing::warn!(message = %message, "Rule action"),
                    LogLevel::Error => tracing::error!(message = %message, "Rule action"),
                }
            }

            ActionKind::Metric { name, value, tags } => {
                let val = self.evaluator.eval(value, ctx)?;
                // TODO: Integrate with metrics system
                tracing::debug!(metric = %name, value = ?val, tags = ?tags, "Metric recorded");
            }

            ActionKind::ExternalCall { .. } => {
                // TODO: Implement external calls
                tracing::warn!("External calls not yet implemented");
            }
        }
        Ok(())
    }

    /// Build output from terminal result
    fn build_output(&self, result: &TerminalResult, ctx: &Context) -> Result<Value> {
        let mut output = HashMap::new();

        // Evaluate output expressions
        for (key, expr) in &result.output {
            let value = self.evaluator.eval(expr, ctx)?;
            output.insert(key.clone(), value);
        }

        // Merge with static data
        if let Value::Object(data) = &result.data {
            for (k, v) in data {
                output.insert(k.clone(), v.clone());
            }
        }

        Ok(Value::object(output))
    }
}

/// Step execution result
#[derive(Debug, Clone)]
pub enum StepResult {
    /// Continue to next step
    Continue { next_step: String },
    /// Terminal - execution complete
    Terminal { result: TerminalResult },
}

/// Complete execution result
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Result code
    pub code: String,
    /// Result message
    pub message: String,
    /// Output data
    pub output: Value,
    /// Execution trace (if enabled)
    pub trace: Option<ExecutionTrace>,
    /// Total duration in microseconds
    pub duration_us: u64,
}

impl ExecutionResult {
    /// Check if execution was successful
    pub fn is_success(&self) -> bool {
        self.code == "SUCCESS" || self.code.starts_with("OK")
    }

    /// Convert to JSON
    pub fn to_json(&self) -> Result<String> {
        let mut map = HashMap::new();
        map.insert("code".to_string(), Value::string(&self.code));
        map.insert("message".to_string(), Value::string(&self.message));
        map.insert("output".to_string(), self.output.clone());
        map.insert(
            "duration_us".to_string(),
            Value::int(self.duration_us as i64),
        );

        serde_json::to_string_pretty(&Value::object(map)).map_err(|e| OrdoError::InternalError {
            message: e.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::Expr;

    fn create_test_ruleset() -> RuleSet {
        let mut ruleset = RuleSet::new("test", "check_age");

        ruleset.add_step(
            Step::decision("check_age", "Check Age")
                .branch(Condition::from_str("age >= 18"), "adult_discount")
                .branch(Condition::from_str("age >= 13"), "teen_discount")
                .default("child_discount")
                .build(),
        );

        ruleset.add_step(Step::terminal(
            "adult_discount",
            "Adult Discount",
            TerminalResult::new("ADULT")
                .with_message("Adult discount applied")
                .with_output("discount", Expr::literal(0.1f64)),
        ));

        ruleset.add_step(Step::terminal(
            "teen_discount",
            "Teen Discount",
            TerminalResult::new("TEEN")
                .with_message("Teen discount applied")
                .with_output("discount", Expr::literal(0.15f64)),
        ));

        ruleset.add_step(Step::terminal(
            "child_discount",
            "Child Discount",
            TerminalResult::new("CHILD")
                .with_message("Child discount applied")
                .with_output("discount", Expr::literal(0.2f64)),
        ));

        ruleset
    }

    #[test]
    fn test_execute_adult() {
        let ruleset = create_test_ruleset();
        let executor = RuleExecutor::new();

        let input = serde_json::from_str(r#"{"age": 25}"#).unwrap();
        let result = executor.execute(&ruleset, input).unwrap();

        assert_eq!(result.code, "ADULT");
        assert_eq!(result.output.get_path("discount"), Some(&Value::float(0.1)));
    }

    #[test]
    fn test_execute_teen() {
        let ruleset = create_test_ruleset();
        let executor = RuleExecutor::new();

        let input = serde_json::from_str(r#"{"age": 15}"#).unwrap();
        let result = executor.execute(&ruleset, input).unwrap();

        assert_eq!(result.code, "TEEN");
        assert_eq!(
            result.output.get_path("discount"),
            Some(&Value::float(0.15))
        );
    }

    #[test]
    fn test_execute_child() {
        let ruleset = create_test_ruleset();
        let executor = RuleExecutor::new();

        let input = serde_json::from_str(r#"{"age": 10}"#).unwrap();
        let result = executor.execute(&ruleset, input).unwrap();

        assert_eq!(result.code, "CHILD");
        assert_eq!(result.output.get_path("discount"), Some(&Value::float(0.2)));
    }
}
