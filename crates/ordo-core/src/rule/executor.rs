//! Rule executor
//!
//! Executes rule sets against input data

use super::metrics::{MetricSink, NoOpMetricSink};
use super::model::{FieldMissingBehavior, RuleSet};
use super::step::{ActionKind, Condition, LogLevel, Step, StepKind, TerminalResult};
use crate::context::{Context, Value};
use crate::error::{OrdoError, Result};
use crate::expr::{Evaluator, ExprParser};
use crate::trace::{ExecutionTrace, StepTrace, TraceConfig};
use rayon::prelude::*;
use std::sync::Arc;

// Use web_time for WASM, std::time for native
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

#[cfg(target_arch = "wasm32")]
mod wasm_time {
    /// A simple instant implementation for WASM using performance.now()
    #[derive(Clone, Copy)]
    pub struct Instant(f64);

    impl Instant {
        pub fn now() -> Self {
            #[cfg(target_arch = "wasm32")]
            {
                // In WASM, we can't use std::time::Instant
                // Return a dummy value - timing will be done in JS
                Instant(0.0)
            }
        }

        pub fn elapsed(&self) -> std::time::Duration {
            // Return zero duration in WASM - timing is handled by JS
            std::time::Duration::from_micros(0)
        }
    }
}

#[cfg(target_arch = "wasm32")]
use wasm_time::Instant;

/// Rule executor
pub struct RuleExecutor {
    /// Expression evaluator
    evaluator: Evaluator,
    /// Trace configuration
    trace_config: TraceConfig,
    /// Metric sink for recording custom metrics from rule actions
    metric_sink: Arc<dyn MetricSink>,
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
            metric_sink: Arc::new(NoOpMetricSink),
        }
    }

    /// Create executor with trace config
    pub fn with_trace(trace_config: TraceConfig) -> Self {
        Self {
            evaluator: Evaluator::new(),
            trace_config,
            metric_sink: Arc::new(NoOpMetricSink),
        }
    }

    /// Create executor with metric sink
    pub fn with_metric_sink(metric_sink: Arc<dyn MetricSink>) -> Self {
        Self {
            evaluator: Evaluator::new(),
            trace_config: TraceConfig::default(),
            metric_sink,
        }
    }

    /// Create executor with trace config and metric sink
    pub fn with_trace_and_metrics(
        trace_config: TraceConfig,
        metric_sink: Arc<dyn MetricSink>,
    ) -> Self {
        Self {
            evaluator: Evaluator::new(),
            trace_config,
            metric_sink,
        }
    }

    /// Get the metric sink
    pub fn metric_sink(&self) -> &Arc<dyn MetricSink> {
        &self.metric_sink
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
            if ruleset.config.timeout_ms > 0 {
                let elapsed_ms = start_time.elapsed().as_millis() as u64;
                if elapsed_ms >= ruleset.config.timeout_ms {
                    return Err(OrdoError::Timeout {
                        timeout_ms: ruleset.config.timeout_ms,
                    });
                }
            }

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

    /// Execute a rule set against multiple inputs (batch execution)
    ///
    /// This method is more efficient than calling `execute` multiple times because:
    /// - The ruleset is only looked up once
    /// - Inputs can be processed in parallel using rayon
    ///
    /// # Arguments
    /// * `ruleset` - The rule set to execute
    /// * `inputs` - Vector of input values to execute
    /// * `parallel` - Whether to execute in parallel (uses rayon)
    ///
    /// # Returns
    /// A `BatchExecutionResult` containing results for each input
    #[cfg(not(target_arch = "wasm32"))]
    pub fn execute_batch(
        &self,
        ruleset: &RuleSet,
        inputs: Vec<Value>,
        parallel: bool,
    ) -> BatchExecutionResult {
        let start_time = Instant::now();
        let total = inputs.len();

        let results: Vec<SingleExecutionResult> = if parallel && total > 1 {
            // Parallel execution using rayon
            inputs
                .into_par_iter()
                .map(|input| self.execute_single_for_batch(ruleset, input))
                .collect()
        } else {
            // Sequential execution
            inputs
                .into_iter()
                .map(|input| self.execute_single_for_batch(ruleset, input))
                .collect()
        };

        let success = results.iter().filter(|r| r.error.is_none()).count();
        let failed = total - success;
        let total_duration_us = start_time.elapsed().as_micros() as u64;

        BatchExecutionResult {
            results,
            total,
            success,
            failed,
            total_duration_us,
        }
    }

    /// Execute a single input for batch processing
    #[cfg(not(target_arch = "wasm32"))]
    fn execute_single_for_batch(&self, ruleset: &RuleSet, input: Value) -> SingleExecutionResult {
        match self.execute(ruleset, input) {
            Ok(result) => SingleExecutionResult {
                code: result.code,
                message: result.message,
                output: result.output,
                duration_us: result.duration_us,
                trace: result.trace,
                error: None,
            },
            Err(e) => SingleExecutionResult {
                code: "error".to_string(),
                message: e.to_string(),
                output: Value::Null,
                duration_us: 0,
                trace: None,
                error: Some(e.to_string()),
            },
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
                // Convert Value to f64 for metric recording
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
                        tracing::warn!(
                            metric = %name,
                            value = ?val,
                            "Cannot convert value to metric, expected numeric type"
                        );
                        return Ok(());
                    }
                };
                // Record metric via sink
                self.metric_sink.record_gauge(name, metric_value, tags);
                tracing::debug!(metric = %name, value = %metric_value, tags = ?tags, "Metric recorded");
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
        use crate::context::IString;

        let mut output: hashbrown::HashMap<IString, Value> = hashbrown::HashMap::new();

        // Evaluate output expressions
        for (key, expr) in &result.output {
            let value = self.evaluator.eval(expr, ctx)?;
            output.insert(Arc::from(key.as_str()), value);
        }

        // Merge with static data
        if let Value::Object(data) = &result.data {
            for (k, v) in data {
                output.insert(k.clone(), v.clone());
            }
        }

        Ok(Value::object_optimized(output))
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
}

// ==================== Batch Execution Types ====================

/// Single execution result for batch processing
#[derive(Debug, Clone)]
pub struct SingleExecutionResult {
    /// Result code
    pub code: String,
    /// Result message
    pub message: String,
    /// Output data
    pub output: Value,
    /// Execution duration in microseconds
    pub duration_us: u64,
    /// Execution trace (if enabled)
    pub trace: Option<ExecutionTrace>,
    /// Error message (if execution failed)
    pub error: Option<String>,
}

impl SingleExecutionResult {
    /// Check if execution was successful
    pub fn is_success(&self) -> bool {
        self.error.is_none()
    }
}

/// Batch execution result
#[derive(Debug, Clone)]
pub struct BatchExecutionResult {
    /// Results for each input (in order)
    pub results: Vec<SingleExecutionResult>,
    /// Total number of inputs
    pub total: usize,
    /// Number of successful executions
    pub success: usize,
    /// Number of failed executions
    pub failed: usize,
    /// Total execution time in microseconds
    pub total_duration_us: u64,
}

impl BatchExecutionResult {
    /// Check if all executions were successful
    pub fn all_success(&self) -> bool {
        self.failed == 0
    }

    /// Get success rate (0.0 - 1.0)
    pub fn success_rate(&self) -> f64 {
        if self.total == 0 {
            1.0
        } else {
            self.success as f64 / self.total as f64
        }
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
                .branch(Condition::from_string("age >= 18"), "adult_discount")
                .branch(Condition::from_string("age >= 13"), "teen_discount")
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

    #[test]
    fn test_execute_with_metric_sink() {
        use crate::rule::metrics::MetricSink;
        use crate::rule::step::{Action, ActionKind};
        use std::sync::atomic::{AtomicUsize, Ordering};

        // Create a test metric sink that counts calls
        struct TestMetricSink {
            gauge_calls: AtomicUsize,
            counter_calls: AtomicUsize,
        }

        impl MetricSink for TestMetricSink {
            fn record_gauge(&self, _name: &str, _value: f64, _tags: &[(String, String)]) {
                self.gauge_calls.fetch_add(1, Ordering::SeqCst);
            }

            fn record_counter(&self, _name: &str, _value: f64, _tags: &[(String, String)]) {
                self.counter_calls.fetch_add(1, Ordering::SeqCst);
            }
        }

        let sink = Arc::new(TestMetricSink {
            gauge_calls: AtomicUsize::new(0),
            counter_calls: AtomicUsize::new(0),
        });

        let executor = RuleExecutor::with_metric_sink(sink.clone());

        // Create a ruleset with a metric action
        let mut ruleset = RuleSet::new("metric_test", "record_metric");

        // Action step that records a metric
        ruleset.add_step(Step::action(
            "record_metric",
            "Record Metric",
            vec![Action {
                kind: ActionKind::Metric {
                    name: "test_metric".to_string(),
                    value: Expr::literal(42.0f64),
                    tags: vec![("env".to_string(), "test".to_string())],
                },
                description: "Test metric".to_string(),
            }],
            "done",
        ));

        ruleset.add_step(Step::terminal(
            "done",
            "Done",
            TerminalResult::new("OK").with_message("Metric recorded"),
        ));

        let input = serde_json::from_str(r#"{}"#).unwrap();
        let result = executor.execute(&ruleset, input).unwrap();

        assert_eq!(result.code, "OK");
        // Verify that the metric sink was called
        assert_eq!(sink.gauge_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_execute_batch_sequential() {
        let ruleset = create_test_ruleset();
        let executor = RuleExecutor::new();

        let inputs = vec![
            serde_json::from_str(r#"{"age": 25}"#).unwrap(),
            serde_json::from_str(r#"{"age": 15}"#).unwrap(),
            serde_json::from_str(r#"{"age": 10}"#).unwrap(),
        ];

        let result = executor.execute_batch(&ruleset, inputs, false);

        assert_eq!(result.total, 3);
        assert_eq!(result.success, 3);
        assert_eq!(result.failed, 0);
        assert!(result.all_success());
        assert_eq!(result.success_rate(), 1.0);

        assert_eq!(result.results[0].code, "ADULT");
        assert_eq!(result.results[1].code, "TEEN");
        assert_eq!(result.results[2].code, "CHILD");
    }

    #[test]
    fn test_execute_batch_parallel() {
        let ruleset = create_test_ruleset();
        let executor = RuleExecutor::new();

        let inputs = vec![
            serde_json::from_str(r#"{"age": 25}"#).unwrap(),
            serde_json::from_str(r#"{"age": 15}"#).unwrap(),
            serde_json::from_str(r#"{"age": 10}"#).unwrap(),
            serde_json::from_str(r#"{"age": 30}"#).unwrap(),
            serde_json::from_str(r#"{"age": 5}"#).unwrap(),
        ];

        let result = executor.execute_batch(&ruleset, inputs, true);

        assert_eq!(result.total, 5);
        assert_eq!(result.success, 5);
        assert_eq!(result.failed, 0);
        assert!(result.all_success());

        // Results should be in order even with parallel execution
        assert_eq!(result.results[0].code, "ADULT");
        assert_eq!(result.results[1].code, "TEEN");
        assert_eq!(result.results[2].code, "CHILD");
        assert_eq!(result.results[3].code, "ADULT");
        assert_eq!(result.results[4].code, "CHILD");
    }

    #[test]
    fn test_execute_batch_empty() {
        let ruleset = create_test_ruleset();
        let executor = RuleExecutor::new();

        let inputs = vec![];
        let result = executor.execute_batch(&ruleset, inputs, false);

        assert_eq!(result.total, 0);
        assert_eq!(result.success, 0);
        assert_eq!(result.failed, 0);
        assert!(result.all_success());
        assert_eq!(result.success_rate(), 1.0);
    }

    #[test]
    fn test_execute_batch_with_errors() {
        // Create a ruleset that will fail for certain inputs
        let mut ruleset = RuleSet::new("error_test", "check");

        ruleset.add_step(
            Step::decision("check", "Check Value")
                .branch(Condition::from_string("value > 0"), "ok")
                // No default - will error if value <= 0
                .build(),
        );

        ruleset.add_step(Step::terminal(
            "ok",
            "OK",
            TerminalResult::new("SUCCESS").with_message("Value is positive"),
        ));

        let executor = RuleExecutor::new();

        let inputs = vec![
            serde_json::from_str(r#"{"value": 10}"#).unwrap(),
            serde_json::from_str(r#"{"value": -5}"#).unwrap(), // This will fail
            serde_json::from_str(r#"{"value": 20}"#).unwrap(),
        ];

        let result = executor.execute_batch(&ruleset, inputs, false);

        assert_eq!(result.total, 3);
        assert_eq!(result.success, 2);
        assert_eq!(result.failed, 1);
        assert!(!result.all_success());

        assert_eq!(result.results[0].code, "SUCCESS");
        assert_eq!(result.results[1].code, "error");
        assert!(result.results[1].error.is_some());
        assert_eq!(result.results[2].code, "SUCCESS");
    }
}
