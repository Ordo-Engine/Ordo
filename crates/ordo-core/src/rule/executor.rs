//! Rule executor
//!
//! Executes rule sets against input data

use super::metrics::{MetricSink, NoOpMetricSink};
use super::model::{FieldMissingBehavior, RuleSet};
use super::step::{ActionKind, Condition, LogLevel, Step, StepKind, SubRuleGraph, TerminalResult};
use crate::capability::{CapabilityInvoker, CapabilityRequest};
use crate::context::{Context, Value};
use crate::error::{OrdoError, Result};
use crate::expr::{Evaluator, ExprParser};
use crate::trace::{ExecutionTrace, StepTrace, SubRuleCallTrace, SubRuleOutputTrace, TraceConfig};
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

/// Runtime execution options that can override RuleSet config.
///
/// This allows passing execution-specific options without cloning the entire RuleSet.
#[derive(Debug, Clone, Default)]
pub struct ExecutionOptions {
    /// Override timeout in milliseconds (0 = use RuleSet config)
    pub timeout_ms: Option<u64>,
    /// Override trace enabled flag
    pub enable_trace: Option<bool>,
    /// Override max execution depth
    pub max_depth: Option<usize>,
}

impl ExecutionOptions {
    /// Create new execution options with timeout override
    #[inline]
    pub fn with_timeout(timeout_ms: u64) -> Self {
        Self {
            timeout_ms: Some(timeout_ms),
            ..Default::default()
        }
    }

    /// Set timeout override
    #[inline]
    pub fn timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    /// Set trace enabled override
    #[inline]
    pub fn trace(mut self, enabled: bool) -> Self {
        self.enable_trace = Some(enabled);
        self
    }
}

/// Rule executor
pub struct RuleExecutor {
    /// Expression evaluator
    evaluator: Evaluator,
    /// Trace configuration
    trace_config: TraceConfig,
    /// Metric sink for recording custom metrics from rule actions
    metric_sink: Arc<dyn MetricSink>,
    /// Optional resolver for CallRuleSet actions
    resolver: Option<Arc<dyn super::RuleSetResolver>>,
    /// Optional capability invoker for ExternalCall actions
    capability_invoker: Option<Arc<dyn CapabilityInvoker>>,
    /// Maximum nesting depth for CallRuleSet (prevents unbounded recursion)
    max_call_depth: usize,
}

impl Default for RuleExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleExecutor {
    const METRIC_CAPABILITY: &'static str = "metrics.prometheus";
    const METRIC_OPERATION_GAUGE: &'static str = "gauge";

    /// Create a new executor
    pub fn new() -> Self {
        Self {
            evaluator: Evaluator::new(),
            trace_config: TraceConfig::default(),
            metric_sink: Arc::new(NoOpMetricSink),
            resolver: None,
            capability_invoker: None,
            max_call_depth: 10,
        }
    }

    /// Create executor with trace config
    pub fn with_trace(trace_config: TraceConfig) -> Self {
        Self {
            evaluator: Evaluator::new(),
            trace_config,
            metric_sink: Arc::new(NoOpMetricSink),
            resolver: None,
            capability_invoker: None,
            max_call_depth: 10,
        }
    }

    /// Create executor with metric sink
    pub fn with_metric_sink(metric_sink: Arc<dyn MetricSink>) -> Self {
        Self {
            evaluator: Evaluator::new(),
            trace_config: TraceConfig::default(),
            metric_sink,
            resolver: None,
            capability_invoker: None,
            max_call_depth: 10,
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
            resolver: None,
            capability_invoker: None,
            max_call_depth: 10,
        }
    }

    /// Set a resolver for CallRuleSet actions
    pub fn set_resolver(&mut self, resolver: Arc<dyn super::RuleSetResolver>) {
        self.resolver = Some(resolver);
    }

    /// Set an invoker for ExternalCall actions
    pub fn set_capability_invoker(&mut self, capability_invoker: Arc<dyn CapabilityInvoker>) {
        self.capability_invoker = Some(capability_invoker);
    }

    /// Get the configured capability invoker
    pub fn capability_invoker(&self) -> Option<Arc<dyn CapabilityInvoker>> {
        self.capability_invoker.clone()
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
    #[inline]
    pub fn execute(&self, ruleset: &RuleSet, input: Value) -> Result<ExecutionResult> {
        self.execute_with_options(ruleset, input, None)
    }

    /// Execute a rule set with runtime options override.
    ///
    /// This method allows overriding RuleSet config at runtime without cloning the RuleSet,
    /// which is more efficient for batch execution with tenant-specific timeouts.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let executor = RuleExecutor::new();
    /// let options = ExecutionOptions::with_timeout(5000); // 5 second timeout
    /// let result = executor.execute_with_options(&ruleset, input, Some(&options))?;
    /// ```
    pub fn execute_with_options(
        &self,
        ruleset: &RuleSet,
        input: Value,
        options: Option<&ExecutionOptions>,
    ) -> Result<ExecutionResult> {
        // Resolve effective config values (options override ruleset config)
        let timeout_ms = options
            .and_then(|o| o.timeout_ms)
            .filter(|&t| t > 0)
            .unwrap_or(ruleset.config.timeout_ms);
        let max_depth = options
            .and_then(|o| o.max_depth)
            .unwrap_or(ruleset.config.max_depth);
        let enable_trace = options
            .and_then(|o| o.enable_trace)
            .unwrap_or(ruleset.config.enable_trace);

        self.execute_internal(
            ruleset,
            input,
            timeout_ms,
            max_depth,
            enable_trace,
            self.max_call_depth,
        )
    }

    /// Internal execute implementation with explicit config parameters
    fn execute_internal(
        &self,
        ruleset: &RuleSet,
        input: Value,
        timeout_ms: u64,
        max_depth: usize,
        enable_trace: bool,
        remaining_call_depth: usize,
    ) -> Result<ExecutionResult> {
        let start_time = Instant::now();
        let mut ctx = Context::new(input);
        let tracing = self.trace_config.enabled || enable_trace;
        let mut trace = if tracing {
            Some(ExecutionTrace::new(&ruleset.config.name))
        } else {
            None
        };

        let mut current_step_id = ruleset.config.entry_step.as_str();
        let mut depth: usize = 0;

        loop {
            // Amortized timeout: skip the first 16 steps entirely, then check every 16 steps.
            // Rationale: 16 steps at ~100ns each = ~1.6µs worst-case detection delay,
            // negligible vs a 5000ms timeout. This eliminates syscall overhead for short rules
            // (most production rules have <10 steps) while still catching runaway execution.
            if timeout_ms > 0
                && depth >= 16
                && depth & 15 == 0
                && start_time.elapsed().as_millis() as u64 >= timeout_ms
            {
                return Err(OrdoError::Timeout { timeout_ms });
            }

            // Check depth limit
            if depth >= max_depth {
                return Err(OrdoError::MaxDepthExceeded { max_depth });
            }

            // Get current step
            let step =
                ruleset
                    .get_step(current_step_id)
                    .ok_or_else(|| OrdoError::StepNotFound {
                        step_id: current_step_id.to_string(),
                    })?;

            // Execute step — branch on tracing to avoid Instant syscalls in the hot path.
            // When tracing is off (default), zero Instant calls per step.
            let (step_result, step_duration, sub_frames, sub_rule_call) =
                if let StepKind::SubRule {
                    ref_name,
                    bindings,
                    outputs,
                    next_step,
                } = &step.kind
                {
                    // SubRule: execute inline sub-graph, then map outputs back to parent context
                    if remaining_call_depth == 0 {
                        return Err(OrdoError::eval_error(format!(
                            "SubRule max nesting depth ({}) exceeded calling '{}'",
                            self.max_call_depth, ref_name
                        )));
                    }
                    let graph = ruleset.sub_rules.get(ref_name.as_str()).ok_or_else(|| {
                        OrdoError::eval_error(format!("Sub-rule '{}' not found", ref_name))
                    })?;
                    let mut child_data = hashbrown::HashMap::new();
                    for (field, expr) in bindings {
                        child_data.insert(
                            std::sync::Arc::from(field.as_str()),
                            self.evaluator.eval(expr, &ctx)?,
                        );
                    }
                    let child_input = Value::object_optimized(child_data);
                    let traced_child_input = if tracing {
                        Some(child_input.clone())
                    } else {
                        None
                    };
                    let step_start = if tracing { Some(Instant::now()) } else { None };
                    let (child_ctx, sub_trace) = self.execute_sub_graph(
                        &ruleset.sub_rules,
                        graph,
                        child_input,
                        &ruleset.config.field_missing,
                        tracing,
                        remaining_call_depth - 1,
                    )?;
                    let dur = step_start
                        .map(|t| t.elapsed().as_micros() as u64)
                        .unwrap_or(0);
                    let mut output_trace = Vec::new();
                    for (parent_var, child_var) in outputs {
                        let value = child_ctx.variables().get(child_var.as_str()).cloned();
                        if let Some(val) = &value {
                            ctx.set_variable(parent_var.clone(), val.clone());
                        }
                        if tracing {
                            output_trace.push(SubRuleOutputTrace {
                                parent_var: parent_var.clone(),
                                child_var: child_var.clone(),
                                missing: value.is_none(),
                                value,
                            });
                        }
                    }
                    let frames = if tracing { Some(sub_trace) } else { None };
                    let call_trace = traced_child_input.map(|input| SubRuleCallTrace {
                        ref_name: ref_name.clone(),
                        input,
                        outputs: output_trace,
                    });
                    (
                        StepResult::Continue {
                            next_step: next_step.as_str(),
                        },
                        dur,
                        frames,
                        call_trace,
                    )
                } else if tracing {
                    let step_start = Instant::now();
                    let result = self.execute_step(
                        step,
                        &mut ctx,
                        &ruleset.config.field_missing,
                        remaining_call_depth,
                    )?;
                    (result, step_start.elapsed().as_micros() as u64, None, None)
                } else {
                    let result = self.execute_step(
                        step,
                        &mut ctx,
                        &ruleset.config.field_missing,
                        remaining_call_depth,
                    )?;
                    (result, 0, None, None)
                };

            // Record trace (only when enabled — zero overhead otherwise)
            if let Some(ref mut trace) = trace {
                let mut step_trace = match &step_result {
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
                if let Some(frames) = sub_frames {
                    step_trace.sub_rule_frames = Some(frames);
                }
                if let Some(call) = sub_rule_call {
                    step_trace.sub_rule_call = Some(call);
                }
                trace.add_step(step_trace);
            }

            // Handle step result
            match step_result {
                StepResult::Continue { next_step } => {
                    current_step_id = next_step;
                    depth += 1;
                }
                StepResult::Terminal { result } => {
                    let output = self.build_output(result, &ctx)?;
                    return Ok(ExecutionResult {
                        code: result.code.clone(),
                        message: result.message.clone(),
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
    fn execute_step<'a>(
        &self,
        step: &'a Step,
        ctx: &mut Context,
        field_missing: &FieldMissingBehavior,
        remaining_call_depth: usize,
    ) -> Result<StepResult<'a>> {
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
                            self.execute_action(action, ctx, remaining_call_depth)?;
                        }
                        return Ok(StepResult::Continue {
                            next_step: branch.next_step.as_str(),
                        });
                    }
                }

                // No branch matched, use default
                if let Some(default) = default_next {
                    Ok(StepResult::Continue {
                        next_step: default.as_str(),
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
                    self.execute_action(action, ctx, remaining_call_depth)?;
                }
                Ok(StepResult::Continue {
                    next_step: next_step.as_str(),
                })
            }

            StepKind::Terminal { result } => Ok(StepResult::Terminal { result }),

            // Handled at the execute_internal loop level before reaching execute_step
            StepKind::SubRule { .. } => {
                unreachable!("SubRule steps are dispatched in execute_internal")
            }
        }
    }

    /// Execute a sub-rule graph and return the resulting context and optional trace frames.
    fn execute_sub_graph(
        &self,
        sub_rules: &hashbrown::HashMap<String, SubRuleGraph>,
        graph: &SubRuleGraph,
        input: Value,
        field_missing: &FieldMissingBehavior,
        tracing: bool,
        remaining_call_depth: usize,
    ) -> Result<(Context, Vec<StepTrace>)> {
        let mut ctx = Context::new(input);
        let mut frames: Vec<StepTrace> = Vec::new();
        let mut current = graph.entry_step.clone();
        let mut depth: usize = 0;

        loop {
            if depth >= 1000 {
                return Err(OrdoError::MaxDepthExceeded { max_depth: 1000 });
            }

            let step =
                graph
                    .steps
                    .get(current.as_str())
                    .ok_or_else(|| OrdoError::StepNotFound {
                        step_id: current.clone(),
                    })?;

            let (result, dur, sub_frames, sub_rule_call) = if let StepKind::SubRule {
                ref_name,
                bindings,
                outputs,
                next_step,
            } = &step.kind
            {
                if remaining_call_depth == 0 {
                    return Err(OrdoError::eval_error(format!(
                        "SubRule max nesting depth ({}) exceeded calling '{}'",
                        self.max_call_depth, ref_name
                    )));
                }
                let graph = sub_rules.get(ref_name.as_str()).ok_or_else(|| {
                    OrdoError::eval_error(format!("Sub-rule '{}' not found", ref_name))
                })?;
                let mut child_data = hashbrown::HashMap::new();
                for (field, expr) in bindings {
                    child_data.insert(
                        std::sync::Arc::from(field.as_str()),
                        self.evaluator.eval(expr, &ctx)?,
                    );
                }
                let child_input = Value::object_optimized(child_data);
                let traced_child_input = if tracing {
                    Some(child_input.clone())
                } else {
                    None
                };
                let step_start = if tracing { Some(Instant::now()) } else { None };
                let (child_ctx, child_frames) = self.execute_sub_graph(
                    sub_rules,
                    graph,
                    child_input,
                    field_missing,
                    tracing,
                    remaining_call_depth - 1,
                )?;
                let dur = step_start
                    .map(|t| t.elapsed().as_micros() as u64)
                    .unwrap_or(0);
                let mut output_trace = Vec::new();
                for (parent_var, child_var) in outputs {
                    let value = child_ctx.variables().get(child_var.as_str()).cloned();
                    if let Some(val) = &value {
                        ctx.set_variable(parent_var.clone(), val.clone());
                    }
                    if tracing {
                        output_trace.push(SubRuleOutputTrace {
                            parent_var: parent_var.clone(),
                            child_var: child_var.clone(),
                            missing: value.is_none(),
                            value,
                        });
                    }
                }
                let call_trace = traced_child_input.map(|input| SubRuleCallTrace {
                    ref_name: ref_name.clone(),
                    input,
                    outputs: output_trace,
                });
                (
                    StepResult::Continue {
                        next_step: next_step.as_str(),
                    },
                    dur,
                    if tracing { Some(child_frames) } else { None },
                    call_trace,
                )
            } else if tracing {
                let t = Instant::now();
                let r = self.execute_step(step, &mut ctx, field_missing, remaining_call_depth)?;
                (r, t.elapsed().as_micros() as u64, None, None)
            } else {
                (
                    self.execute_step(step, &mut ctx, field_missing, remaining_call_depth)?,
                    0,
                    None,
                    None,
                )
            };

            if tracing {
                let mut st = match &result {
                    StepResult::Continue { next_step } => {
                        StepTrace::continued(&step.id, &step.name, dur, next_step)
                    }
                    StepResult::Terminal { .. } => StepTrace::terminal(&step.id, &step.name, dur),
                };
                if self.trace_config.capture_input {
                    st.input_snapshot = Some(ctx.data().clone());
                }
                if self.trace_config.capture_variables {
                    st.variables_snapshot = Some(ctx.variables().clone());
                }
                if let Some(frames) = sub_frames {
                    st.sub_rule_frames = Some(frames);
                }
                if let Some(call) = sub_rule_call {
                    st.sub_rule_call = Some(call);
                }
                frames.push(st);
            }

            match result {
                StepResult::Continue { next_step } => {
                    current = next_step.to_string();
                    depth += 1;
                }
                StepResult::Terminal { .. } => {
                    return Ok((ctx, frames));
                }
            }
        }
    }

    /// Evaluate a condition
    ///
    /// NOTE: For best performance, call `RuleSet::compile()` after loading to pre-compile
    /// all expression strings. If not compiled, expressions will be parsed on each evaluation.
    fn evaluate_condition(
        &self,
        condition: &Condition,
        ctx: &Context,
        field_missing: &FieldMissingBehavior,
    ) -> Result<bool> {
        match condition {
            Condition::Always => Ok(true),

            Condition::Expression(expr) => {
                self.eval_expr_with_field_missing(expr, ctx, field_missing)
            }

            Condition::ExpressionString(s) => {
                // Parse and evaluate - consider calling RuleSet::compile() for better performance
                let expr = ExprParser::parse(s)?;
                self.eval_expr_with_field_missing(&expr, ctx, field_missing)
            }
        }
    }

    /// Helper to evaluate expression with field missing behavior
    #[inline]
    fn eval_expr_with_field_missing(
        &self,
        expr: &crate::expr::Expr,
        ctx: &Context,
        field_missing: &FieldMissingBehavior,
    ) -> Result<bool> {
        match self.evaluator.eval(expr, ctx) {
            Ok(value) => Ok(value.is_truthy()),
            Err(OrdoError::FieldNotFound { .. })
                if *field_missing == FieldMissingBehavior::Lenient =>
            {
                Ok(false)
            }
            Err(e) => Err(e),
        }
    }

    /// Execute an action
    fn execute_action(
        &self,
        action: &super::step::Action,
        ctx: &mut Context,
        remaining_call_depth: usize,
    ) -> Result<()> {
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
                self.record_metric(name, metric_value, tags)?;
                tracing::debug!(metric = %name, value = %metric_value, tags = ?tags, "Metric recorded");
            }

            ActionKind::CallRuleSet {
                ruleset_name,
                input_mapping,
                result_variable,
            } => {
                if remaining_call_depth == 0 {
                    return Err(OrdoError::eval_error(format!(
                        "CallRuleSet max nesting depth ({}) exceeded calling '{}'",
                        self.max_call_depth, ruleset_name
                    )));
                }

                let resolver = self.resolver.as_ref().ok_or_else(|| {
                    OrdoError::eval_error("CallRuleSet requires a resolver to be configured")
                })?;
                let target =
                    resolver
                        .resolve(ruleset_name)
                        .ok_or_else(|| OrdoError::RuleSetNotFound {
                            name: ruleset_name.clone(),
                        })?;

                // Build input for the sub-ruleset
                let sub_input = if let Some(mapping) = input_mapping {
                    self.evaluator.eval(mapping, ctx)?
                } else {
                    ctx.data().clone()
                };

                // Execute sub-ruleset with decremented call depth
                let sub_result = self.execute_internal(
                    &target,
                    sub_input,
                    target.config.timeout_ms,
                    target.config.max_depth,
                    false,
                    remaining_call_depth - 1,
                )?;

                // Store result as a variable
                let result_obj = Value::object({
                    let mut m = std::collections::HashMap::new();
                    m.insert("code".to_string(), Value::string(&sub_result.code));
                    m.insert("message".to_string(), Value::string(&sub_result.message));
                    m.insert("output".to_string(), sub_result.output);
                    m
                });
                ctx.set_variable(result_variable, result_obj);
            }

            ActionKind::ExternalCall {
                service,
                method,
                params,
                result_variable,
                timeout_ms,
            } => {
                let capability_invoker = self.capability_invoker.as_ref().ok_or_else(|| {
                    OrdoError::eval_error("ExternalCall requires a capability invoker")
                })?;

                let mut payload = std::collections::HashMap::with_capacity(params.len());
                for (name, expr) in params {
                    payload.insert(name.clone(), self.evaluator.eval(expr, ctx)?);
                }

                let mut request =
                    CapabilityRequest::new(service.clone(), method.clone(), Value::object(payload));
                if *timeout_ms > 0 {
                    request = request.with_timeout(*timeout_ms);
                }

                let response = capability_invoker.invoke(&request)?;
                if let Some(result_variable) = result_variable {
                    let response_obj = Value::object({
                        let mut m = std::collections::HashMap::new();
                        m.insert("capability".to_string(), Value::string(service));
                        m.insert("operation".to_string(), Value::string(method));
                        m.insert("payload".to_string(), response.payload);
                        let metadata = Value::object(
                            response
                                .metadata
                                .into_iter()
                                .map(|(key, value)| (key, Value::string(value)))
                                .collect(),
                        );
                        m.insert("metadata".to_string(), metadata);
                        m
                    });
                    ctx.set_variable(result_variable, response_obj);
                }
            }
        }
        Ok(())
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

            let request = CapabilityRequest::new(
                Self::METRIC_CAPABILITY,
                Self::METRIC_OPERATION_GAUGE,
                Value::object(payload),
            );

            match capability_invoker.invoke(&request) {
                Ok(_) => return Ok(()),
                Err(OrdoError::CapabilityNotFound { .. }) => {}
                Err(error) => return Err(error),
            }
        }

        self.metric_sink.record_gauge(name, value, tags);
        Ok(())
    }

    /// Build output from terminal result
    fn build_output(&self, result: &TerminalResult, ctx: &Context) -> Result<Value> {
        use crate::context::IString;

        // Pre-allocate capacity: output expressions + static data fields
        let data_len = match &result.data {
            Value::Object(map) => map.len(),
            _ => 0,
        };
        let mut output: hashbrown::HashMap<IString, Value> =
            hashbrown::HashMap::with_capacity(result.output.len() + data_len);

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
pub enum StepResult<'a> {
    /// Continue to next step
    Continue { next_step: &'a str },
    /// Terminal - execution complete (borrows TerminalResult to avoid clone)
    Terminal { result: &'a TerminalResult },
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
    fn test_execute_metric_via_capability_invoker() {
        use crate::capability::{
            CapabilityCategory, CapabilityDescriptor, CapabilityProvider, CapabilityRegistry,
            CapabilityRequest, CapabilityResponse,
        };
        use crate::rule::metrics::MetricSink;
        use crate::rule::step::{Action, ActionKind};
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

        let sink = Arc::new(TestMetricSink {
            gauge_calls: AtomicUsize::new(0),
        });
        let mut executor = RuleExecutor::with_metric_sink(sink.clone());
        let registry = Arc::new(CapabilityRegistry::new());
        let capability = Arc::new(TestMetricCapability {
            calls: AtomicUsize::new(0),
        });
        let capability_ref = capability.clone();
        registry.register(capability);
        executor.set_capability_invoker(registry);

        let mut ruleset = RuleSet::new("metric_capability_test", "record_metric");
        ruleset.add_step(Step::action(
            "record_metric",
            "Record Metric",
            vec![Action {
                kind: ActionKind::Metric {
                    name: "cap_metric".to_string(),
                    value: Expr::literal(7.0f64),
                    tags: vec![("env".to_string(), "test".to_string())],
                },
                description: String::new(),
            }],
            "done",
        ));
        ruleset.add_step(Step::terminal("done", "Done", TerminalResult::new("OK")));

        let input = serde_json::from_str(r#"{}"#).unwrap();
        let result = executor.execute(&ruleset, input).unwrap();

        assert_eq!(result.code, "OK");
        assert_eq!(capability_ref.calls.load(Ordering::SeqCst), 1);
        assert_eq!(sink.gauge_calls.load(Ordering::SeqCst), 0);
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

    #[test]
    fn test_call_ruleset() {
        use crate::rule::step::{Action, ActionKind};
        use crate::rule::RuleSetResolver;
        use std::collections::HashMap;

        // Create a sub-ruleset that returns a score
        let mut score_ruleset = RuleSet::new("score", "compute");
        score_ruleset.add_step(Step::terminal(
            "compute",
            "Compute Score",
            TerminalResult::new("SCORED")
                .with_message("Score computed")
                .with_output("score", Expr::literal(95)),
        ));

        // Create a resolver with the sub-ruleset
        struct TestResolver {
            rulesets: HashMap<String, Arc<RuleSet>>,
        }
        impl RuleSetResolver for TestResolver {
            fn resolve(&self, name: &str) -> Option<Arc<RuleSet>> {
                self.rulesets.get(name).cloned()
            }
        }

        let mut resolver_map = HashMap::new();
        resolver_map.insert("score".to_string(), Arc::new(score_ruleset));
        let resolver = Arc::new(TestResolver {
            rulesets: resolver_map,
        });

        // Create main ruleset that calls the sub-ruleset
        let mut main_ruleset = RuleSet::new("main", "call_score");
        main_ruleset.add_step(Step::action(
            "call_score",
            "Call Score",
            vec![Action {
                kind: ActionKind::CallRuleSet {
                    ruleset_name: "score".to_string(),
                    input_mapping: None,
                    result_variable: "score_result".to_string(),
                },
                description: String::new(),
            }],
            "done",
        ));
        main_ruleset.add_step(Step::terminal(
            "done",
            "Done",
            TerminalResult::new("OK")
                .with_message("Done")
                .with_output("sub_result", Expr::field("$score_result")),
        ));

        let mut executor = RuleExecutor::new();
        executor.set_resolver(resolver);

        let input = serde_json::from_str(r#"{"x": 1}"#).unwrap();
        let result = executor.execute(&main_ruleset, input).unwrap();

        assert_eq!(result.code, "OK");
        // Verify the sub-ruleset result is stored as a variable
        let sub_result = result.output.get_path("sub_result").unwrap();
        assert_eq!(sub_result.get_path("code"), Some(&Value::string("SCORED")));
        assert_eq!(
            sub_result.get_path("message"),
            Some(&Value::string("Score computed"))
        );
    }

    #[test]
    fn test_call_ruleset_not_found() {
        use crate::rule::step::{Action, ActionKind};
        use crate::rule::RuleSetResolver;

        struct EmptyResolver;
        impl RuleSetResolver for EmptyResolver {
            fn resolve(&self, _name: &str) -> Option<Arc<RuleSet>> {
                None
            }
        }

        let mut main = RuleSet::new("main", "call");
        main.add_step(Step::action(
            "call",
            "Call",
            vec![Action {
                kind: ActionKind::CallRuleSet {
                    ruleset_name: "nonexistent".to_string(),
                    input_mapping: None,
                    result_variable: "result".to_string(),
                },
                description: String::new(),
            }],
            "done",
        ));
        main.add_step(Step::terminal("done", "Done", TerminalResult::new("OK")));

        let mut executor = RuleExecutor::new();
        executor.set_resolver(Arc::new(EmptyResolver));

        let input = serde_json::from_str(r#"{}"#).unwrap();
        let result = executor.execute(&main, input);
        assert!(result.is_err());
    }

    #[test]
    fn test_call_ruleset_no_resolver() {
        use crate::rule::step::{Action, ActionKind};

        let mut main = RuleSet::new("main", "call");
        main.add_step(Step::action(
            "call",
            "Call",
            vec![Action {
                kind: ActionKind::CallRuleSet {
                    ruleset_name: "any".to_string(),
                    input_mapping: None,
                    result_variable: "result".to_string(),
                },
                description: String::new(),
            }],
            "done",
        ));
        main.add_step(Step::terminal("done", "Done", TerminalResult::new("OK")));

        let executor = RuleExecutor::new(); // No resolver set
        let input = serde_json::from_str(r#"{}"#).unwrap();
        let result = executor.execute(&main, input);
        assert!(result.is_err());
    }

    #[test]
    fn test_sub_rule_basic() {
        use crate::rule::step::{Action, ActionKind, SubRuleGraph};

        // Sub-rule: checks score and sets a "tier" variable
        let mut sub_steps = hashbrown::HashMap::new();
        sub_steps.insert(
            "check_score".to_string(),
            Step::decision("check_score", "Check Score")
                .branch(Condition::from_string("score >= 90"), "tier_gold")
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
                        value: Expr::literal(Value::string("gold")),
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
                        value: Expr::literal(Value::string("silver")),
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

        let graph = SubRuleGraph {
            entry_step: "check_score".to_string(),
            steps: sub_steps,
        };

        let mut ruleset = RuleSet::new("main", "start");
        ruleset.add_sub_rule("classify", graph);

        // Main: SubRule step → terminal
        ruleset.add_step(Step {
            id: "start".to_string(),
            name: "Start".to_string(),
            kind: StepKind::SubRule {
                ref_name: "classify".to_string(),
                bindings: vec![("score".to_string(), Expr::field("score"))],
                outputs: vec![("result_tier".to_string(), "tier".to_string())],
                next_step: "end".to_string(),
            },
        });
        ruleset.add_step(Step::terminal(
            "end",
            "End",
            TerminalResult::new("DONE").with_output("tier", Expr::field("$result_tier")),
        ));

        let executor = RuleExecutor::new();

        // Test with score >= 90 → gold
        let input: Value = serde_json::from_str(r#"{"score": 95}"#).unwrap();
        let result = executor.execute(&ruleset, input).unwrap();
        assert_eq!(result.code, "DONE");
        assert_eq!(result.output.get_path("tier"), Some(&Value::string("gold")));

        // Test with score < 90 → silver
        let input: Value = serde_json::from_str(r#"{"score": 70}"#).unwrap();
        let result = executor.execute(&ruleset, input).unwrap();
        assert_eq!(result.code, "DONE");
        assert_eq!(
            result.output.get_path("tier"),
            Some(&Value::string("silver"))
        );
    }

    #[test]
    fn test_nested_sub_rule_executes_and_traces_frames() {
        use crate::rule::step::{Action, ActionKind, SubRuleGraph};
        use crate::trace::TraceConfig;

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
                .branch(Condition::from_string("$score_for_tier >= 90"), "gold")
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

        let mut ruleset = RuleSet::new("main", "classify");
        ruleset.config.enable_trace = true;
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
                next_step: "end".to_string(),
            },
        });
        ruleset.add_step(Step::terminal(
            "end",
            "End",
            TerminalResult::new("DONE").with_output("tier", Expr::field("$tier")),
        ));

        ruleset.validate().unwrap();
        let executor = RuleExecutor::with_trace(TraceConfig::minimal());
        let input: Value = serde_json::from_str(r#"{"score": 95}"#).unwrap();
        let result = executor.execute(&ruleset, input).unwrap();

        assert_eq!(result.output.get_path("tier"), Some(&Value::string("gold")));
        let trace = result.trace.unwrap();
        let call = trace.steps[0].sub_rule_call.as_ref().unwrap();
        assert_eq!(call.ref_name, "classify_score");
        assert_eq!(call.input.get_path("score"), Some(&Value::int(95)));
        assert_eq!(call.outputs[0].parent_var, "tier");
        assert_eq!(call.outputs[0].child_var, "tier");
        assert_eq!(call.outputs[0].value, Some(Value::string("gold")));
        let top_frames = trace.steps[0].sub_rule_frames.as_ref().unwrap();
        assert_eq!(top_frames[0].step_id, "normalize");
        let nested_call = top_frames[0].sub_rule_call.as_ref().unwrap();
        assert_eq!(nested_call.ref_name, "normalize_score");
        assert_eq!(
            nested_call.input.get_path("raw_score"),
            Some(&Value::int(95))
        );
        assert!(top_frames[0].sub_rule_frames.is_some());
    }

    #[test]
    fn test_sub_rule_validation_cycle() {
        use crate::rule::step::SubRuleGraph;

        // Create a sub-rule that calls itself — should be detected as a cycle
        let mut sub_steps = hashbrown::HashMap::new();
        sub_steps.insert(
            "a".to_string(),
            Step {
                id: "a".to_string(),
                name: "A".to_string(),
                kind: StepKind::SubRule {
                    ref_name: "loop_sub".to_string(),
                    bindings: vec![],
                    outputs: vec![],
                    next_step: "term".to_string(),
                },
            },
        );
        sub_steps.insert(
            "term".to_string(),
            Step::terminal("term", "Term", TerminalResult::new("OK")),
        );

        let graph = SubRuleGraph {
            entry_step: "a".to_string(),
            steps: sub_steps,
        };

        let mut ruleset = RuleSet::new("main", "start");
        ruleset.add_sub_rule("loop_sub", graph);
        ruleset.add_step(Step {
            id: "start".to_string(),
            name: "Start".to_string(),
            kind: StepKind::SubRule {
                ref_name: "loop_sub".to_string(),
                bindings: vec![],
                outputs: vec![],
                next_step: "end".to_string(),
            },
        });
        ruleset.add_step(Step::terminal("end", "End", TerminalResult::new("OK")));

        let errors = ruleset.validate().unwrap_err();
        assert!(
            errors.iter().any(|e| e.contains("Cycle")),
            "Expected cycle error, got: {:?}",
            errors
        );
    }
}
