//! WebAssembly bindings for Ordo Rule Engine
//!
//! This crate provides WASM-compatible bindings for executing rules in the browser.

use ordo_core::expr::{BinaryOp, Expr};
use ordo_core::prelude::*;
use ordo_core::rule::{ActionKind, CompiledRuleExecutor, Condition, RuleSetCompiler};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Initialize panic hook for better error messages in the browser console
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// Execution result returned to JavaScript
#[derive(Serialize, Deserialize)]
pub struct WasmExecutionResult {
    /// Result code
    pub code: String,
    /// Result message
    pub message: String,
    /// Output data as JSON value
    pub output: serde_json::Value,
    /// Execution duration in microseconds
    pub duration_us: u64,
    /// Execution trace (if enabled)
    pub trace: Option<WasmExecutionTrace>,
}

/// Execution trace
#[derive(Serialize, Deserialize)]
pub struct WasmExecutionTrace {
    /// Path of executed steps
    pub path: String,
    /// Step traces
    pub steps: Vec<WasmStepTrace>,
}

/// Step trace information
#[derive(Serialize, Deserialize)]
pub struct WasmStepTrace {
    /// Step ID
    pub id: String,
    /// Step name
    pub name: String,
    /// Duration in microseconds
    pub duration_us: u64,
    /// Step result (for decision steps)
    pub result: Option<String>,
}

/// Execute a ruleset with given input
///
/// # Arguments
/// * `ruleset_json` - RuleSet definition as JSON string
/// * `input_json` - Input data as JSON string
/// * `include_trace` - Whether to include execution trace
///
/// # Returns
/// JSON string containing the execution result
#[wasm_bindgen]
pub fn execute_ruleset(
    ruleset_json: &str,
    input_json: &str,
    include_trace: bool,
) -> std::result::Result<String, JsValue> {
    // Parse ruleset
    let ruleset: RuleSet = serde_json::from_str(ruleset_json)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse ruleset: {}", e)))?;

    // Debug: log parsed ruleset structure
    web_sys::console::log_1(
        &format!(
            "[WASM DEBUG] Parsed ruleset steps: {:?}",
            ruleset.steps.keys().collect::<Vec<_>>()
        )
        .into(),
    );
    for (step_id, step) in &ruleset.steps {
        web_sys::console::log_1(&format!("[WASM DEBUG] Step {}: {:?}", step_id, step.kind).into());
    }

    // Parse input
    let input: Value = serde_json::from_str(input_json)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse input: {}", e)))?;

    // Create executor with optional trace
    let executor = if include_trace {
        RuleExecutor::with_trace(TraceConfig::full())
    } else {
        RuleExecutor::new()
    };

    // Execute
    let start = now();
    let result = executor
        .execute(&ruleset, input)
        .map_err(|e| JsValue::from_str(&format!("Execution failed: {}", e)))?;
    let duration_us = ((now() - start) * 1000.0) as u64;

    // Convert trace if present
    let trace = result.trace.as_ref().map(|t| WasmExecutionTrace {
        path: t.path_string(),
        steps: t
            .steps
            .iter()
            .map(|s| WasmStepTrace {
                id: s.step_id.clone(),
                name: s.step_name.clone(),
                duration_us: s.duration_us,
                result: None,
            })
            .collect(),
    });

    // Convert output to serde_json::Value
    let output_json: serde_json::Value = serde_json::to_value(&result.output)
        .map_err(|e| JsValue::from_str(&format!("Failed to convert output: {}", e)))?;

    // Build response
    let wasm_result = WasmExecutionResult {
        code: result.code,
        message: result.message,
        output: output_json,
        duration_us,
        trace,
    };

    // Serialize to JSON
    serde_json::to_string(&wasm_result)
        .map_err(|e| JsValue::from_str(&format!("Failed to serialize result: {}", e)))
}

/// Validate a ruleset
///
/// # Arguments
/// * `ruleset_json` - RuleSet definition as JSON string
///
/// # Returns
/// JSON string containing validation result: `{"valid": true}` or `{"valid": false, "errors": [...]}`
#[wasm_bindgen]
pub fn validate_ruleset(ruleset_json: &str) -> std::result::Result<String, JsValue> {
    // Parse ruleset
    let ruleset: RuleSet = serde_json::from_str(ruleset_json)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse ruleset: {}", e)))?;

    // Validate
    match ruleset.validate() {
        Ok(_) => Ok(r#"{"valid": true}"#.to_string()),
        Err(errors) => {
            let result = serde_json::json!({
                "valid": false,
                "errors": errors
            });
            Ok(result.to_string())
        }
    }
}

/// Evaluate an expression with given context
///
/// # Arguments
/// * `expression` - Expression string to evaluate
/// * `context_json` - Context data as JSON string
///
/// # Returns
/// JSON string containing the evaluation result and parsed expression
#[wasm_bindgen]
pub fn eval_expression(
    expression: &str,
    context_json: &str,
) -> std::result::Result<String, JsValue> {
    // Parse context
    let context_value: Value = serde_json::from_str(context_json)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse context: {}", e)))?;

    // Create context
    let context = Context::new(context_value);

    // Parse expression
    let expr = ExprParser::parse(expression)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse expression: {}", e)))?;

    // Evaluate
    let evaluator = Evaluator::new();
    let result = evaluator
        .eval(&expr, &context)
        .map_err(|e| JsValue::from_str(&format!("Evaluation failed: {}", e)))?;

    // Build response
    let response = serde_json::json!({
        "result": result,
        "parsed": format!("{:?}", expr)
    });

    Ok(response.to_string())
}

/// Get the current timestamp in milliseconds
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = Date, js_name = now)]
    fn now() -> f64;
}

// ============================================================================
// JIT Compatibility Analysis
// ============================================================================

/// Result of JIT compatibility analysis for a single expression
#[derive(Serialize, Deserialize)]
pub struct JITExprAnalysis {
    /// Whether the expression is JIT-compatible
    pub jit_compatible: bool,
    /// Reason for incompatibility (if not compatible)
    pub reason: Option<String>,
    /// List of fields accessed by the expression
    pub accessed_fields: Vec<String>,
    /// Unsupported features found in the expression
    pub unsupported_features: Vec<String>,
    /// Supported features used in the expression
    pub supported_features: Vec<String>,
}

/// Result of JIT compatibility analysis for a ruleset
#[derive(Serialize, Deserialize)]
pub struct JITRulesetAnalysis {
    /// Overall JIT compatibility (all expressions must be compatible)
    pub overall_compatible: bool,
    /// Number of JIT-compatible expressions
    pub compatible_count: usize,
    /// Number of incompatible expressions
    pub incompatible_count: usize,
    /// Total number of expressions analyzed
    pub total_expressions: usize,
    /// Analysis of individual expressions (keyed by step_id)
    pub expressions: Vec<JITExpressionEntry>,
    /// Estimated performance improvement (1.0 = no improvement)
    pub estimated_speedup: f64,
    /// Summary of required schema fields
    pub required_fields: Vec<RequiredFieldInfo>,
}

/// Entry for a single expression analysis
#[derive(Serialize, Deserialize)]
pub struct JITExpressionEntry {
    /// Step ID containing this expression
    pub step_id: String,
    /// Step name
    pub step_name: String,
    /// Type of expression location (condition, assignment, etc.)
    pub location: String,
    /// The expression string
    pub expression: String,
    /// Analysis result
    pub analysis: JITExprAnalysis,
}

/// Information about a required field for JIT
#[derive(Serialize, Deserialize)]
pub struct RequiredFieldInfo {
    /// Field path (e.g., "user.age")
    pub path: String,
    /// Inferred type from usage
    pub inferred_type: String,
    /// Steps that access this field
    pub used_in_steps: Vec<String>,
}

/// Analyze a single expression for JIT compatibility
///
/// # Arguments
/// * `expression` - Expression string to analyze
///
/// # Returns
/// JSON string containing JITExprAnalysis
#[wasm_bindgen]
pub fn analyze_jit_compatibility(expression: &str) -> std::result::Result<String, JsValue> {
    // Parse expression
    let expr = ExprParser::parse(expression)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse expression: {}", e)))?;

    let analysis = analyze_expr_jit_compatibility(&expr);

    serde_json::to_string(&analysis)
        .map_err(|e| JsValue::from_str(&format!("Failed to serialize result: {}", e)))
}

/// Analyze an entire ruleset for JIT compatibility
///
/// # Arguments
/// * `ruleset_json` - RuleSet definition as JSON string
///
/// # Returns
/// JSON string containing JITRulesetAnalysis
#[wasm_bindgen]
pub fn analyze_ruleset_jit(ruleset_json: &str) -> std::result::Result<String, JsValue> {
    // Parse ruleset
    let ruleset: RuleSet = serde_json::from_str(ruleset_json)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse ruleset: {}", e)))?;

    let analysis = analyze_ruleset_jit_compatibility(&ruleset);

    serde_json::to_string(&analysis)
        .map_err(|e| JsValue::from_str(&format!("Failed to serialize result: {}", e)))
}

// ============================================================================
// Internal Analysis Functions
// ============================================================================

/// Analyze a single expression for JIT compatibility
fn analyze_expr_jit_compatibility(expr: &Expr) -> JITExprAnalysis {
    let mut accessed_fields = Vec::new();
    let mut unsupported_features = Vec::new();
    let mut supported_features = Vec::new();
    let mut is_compatible = true;
    let mut reason: Option<String> = None;

    collect_expr_analysis(
        expr,
        &mut accessed_fields,
        &mut unsupported_features,
        &mut supported_features,
    );

    // Check for unsupported features
    if !unsupported_features.is_empty() {
        is_compatible = false;
        reason = Some(format!(
            "Unsupported features: {}",
            unsupported_features.join(", ")
        ));
    }

    JITExprAnalysis {
        jit_compatible: is_compatible,
        reason,
        accessed_fields,
        unsupported_features,
        supported_features,
    }
}

/// Collect analysis information from an expression
fn collect_expr_analysis(
    expr: &Expr,
    accessed_fields: &mut Vec<String>,
    unsupported_features: &mut Vec<String>,
    supported_features: &mut Vec<String>,
) {
    match expr {
        Expr::Literal(v) => match v {
            Value::Null | Value::Bool(_) | Value::Int(_) | Value::Float(_) => {
                if !supported_features.contains(&"numeric_literal".to_string()) {
                    supported_features.push("numeric_literal".to_string());
                }
            }
            Value::String(_) => {
                if !unsupported_features.contains(&"string_literal".to_string()) {
                    unsupported_features.push("string_literal".to_string());
                }
            }
            Value::Array(_) => {
                if !unsupported_features.contains(&"array_literal".to_string()) {
                    unsupported_features.push("array_literal".to_string());
                }
            }
            Value::Object(_) => {
                if !unsupported_features.contains(&"object_literal".to_string()) {
                    unsupported_features.push("object_literal".to_string());
                }
            }
        },
        Expr::Field(name) => {
            if !accessed_fields.contains(name) {
                accessed_fields.push(name.clone());
            }
            if !supported_features.contains(&"field_access".to_string()) {
                supported_features.push("field_access".to_string());
            }
        }
        Expr::Binary { left, right, op } => {
            // Check operator support
            match op {
                BinaryOp::In | BinaryOp::NotIn | BinaryOp::Contains => {
                    let op_name = format!("{:?}_operator", op);
                    if !unsupported_features.contains(&op_name) {
                        unsupported_features.push(op_name);
                    }
                }
                _ => {
                    if !supported_features.contains(&"binary_operations".to_string()) {
                        supported_features.push("binary_operations".to_string());
                    }
                }
            }
            collect_expr_analysis(
                left,
                accessed_fields,
                unsupported_features,
                supported_features,
            );
            collect_expr_analysis(
                right,
                accessed_fields,
                unsupported_features,
                supported_features,
            );
        }
        Expr::Unary { operand, .. } => {
            if !supported_features.contains(&"unary_operations".to_string()) {
                supported_features.push("unary_operations".to_string());
            }
            collect_expr_analysis(
                operand,
                accessed_fields,
                unsupported_features,
                supported_features,
            );
        }
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => {
            if !supported_features.contains(&"conditional".to_string()) {
                supported_features.push("conditional".to_string());
            }
            collect_expr_analysis(
                condition,
                accessed_fields,
                unsupported_features,
                supported_features,
            );
            collect_expr_analysis(
                then_branch,
                accessed_fields,
                unsupported_features,
                supported_features,
            );
            collect_expr_analysis(
                else_branch,
                accessed_fields,
                unsupported_features,
                supported_features,
            );
        }
        Expr::Call { name, args } => {
            // Check if function is supported
            let supported_funcs = ["abs", "min", "max", "floor", "ceil", "round", "sqrt", "pow"];
            if supported_funcs.contains(&name.as_str()) {
                if !supported_features.contains(&"math_functions".to_string()) {
                    supported_features.push("math_functions".to_string());
                }
            } else {
                let feature = format!("function:{}", name);
                if !unsupported_features.contains(&feature) {
                    unsupported_features.push(feature);
                }
            }
            for arg in args {
                collect_expr_analysis(
                    arg,
                    accessed_fields,
                    unsupported_features,
                    supported_features,
                );
            }
        }
        Expr::Array(items) => {
            if !unsupported_features.contains(&"array_construction".to_string()) {
                unsupported_features.push("array_construction".to_string());
            }
            for item in items {
                collect_expr_analysis(
                    item,
                    accessed_fields,
                    unsupported_features,
                    supported_features,
                );
            }
        }
        Expr::Object(pairs) => {
            if !unsupported_features.contains(&"object_construction".to_string()) {
                unsupported_features.push("object_construction".to_string());
            }
            for (_, v) in pairs {
                collect_expr_analysis(v, accessed_fields, unsupported_features, supported_features);
            }
        }
        Expr::Exists(field) => {
            if !unsupported_features.contains(&"exists_check".to_string()) {
                unsupported_features.push("exists_check".to_string());
            }
            if !accessed_fields.contains(field) {
                accessed_fields.push(field.clone());
            }
        }
        Expr::Coalesce(exprs) => {
            if !unsupported_features.contains(&"coalesce".to_string()) {
                unsupported_features.push("coalesce".to_string());
            }
            for e in exprs {
                collect_expr_analysis(e, accessed_fields, unsupported_features, supported_features);
            }
        }
    }
}

/// Analyze an entire ruleset for JIT compatibility
fn analyze_ruleset_jit_compatibility(ruleset: &RuleSet) -> JITRulesetAnalysis {
    let mut expressions = Vec::new();
    let mut compatible_count = 0;
    let mut incompatible_count = 0;
    let mut all_fields: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    // Analyze each step
    for (step_id, step) in &ruleset.steps {
        match &step.kind {
            StepKind::Decision { branches, .. } => {
                for (branch_idx, branch) in branches.iter().enumerate() {
                    // Analyze the condition
                    let (expr_opt, expr_str) = match &branch.condition {
                        Condition::Always => (None, "true".to_string()),
                        Condition::Expression(expr) => (Some(expr.clone()), format!("{:?}", expr)),
                        Condition::ExpressionString(s) => match ExprParser::parse(s) {
                            Ok(expr) => (Some(expr), s.clone()),
                            Err(_) => (None, s.clone()),
                        },
                    };

                    if let Some(expr) = expr_opt {
                        let analysis = analyze_expr_jit_compatibility(&expr);

                        // Track fields
                        for field in &analysis.accessed_fields {
                            all_fields
                                .entry(field.clone())
                                .or_default()
                                .push(step_id.clone());
                        }

                        if analysis.jit_compatible {
                            compatible_count += 1;
                        } else {
                            incompatible_count += 1;
                        }

                        expressions.push(JITExpressionEntry {
                            step_id: step_id.clone(),
                            step_name: step.name.clone(),
                            location: format!("branch:{}", branch_idx),
                            expression: expr_str,
                            analysis,
                        });
                    }
                }
            }
            StepKind::Action { actions, .. } => {
                for action in actions {
                    if let ActionKind::SetVariable { name: _, value } = &action.kind {
                        // value is an Expr, analyze it directly
                        let analysis = analyze_expr_jit_compatibility(value);

                        for field in &analysis.accessed_fields {
                            all_fields
                                .entry(field.clone())
                                .or_default()
                                .push(step_id.clone());
                        }

                        if analysis.jit_compatible {
                            compatible_count += 1;
                        } else {
                            incompatible_count += 1;
                        }

                        expressions.push(JITExpressionEntry {
                            step_id: step_id.clone(),
                            step_name: step.name.clone(),
                            location: "assignment".to_string(),
                            expression: format!("{:?}", value),
                            analysis,
                        });
                    }
                }
            }
            StepKind::Terminal { .. } => {
                // Terminal steps typically don't have complex expressions to analyze
            }
        }
    }

    let total = compatible_count + incompatible_count;
    let overall_compatible = incompatible_count == 0 && total > 0;

    // Estimate speedup based on compatibility ratio
    let estimated_speedup = if overall_compatible && total > 0 {
        // JIT typically provides 20-30x speedup for numeric expressions
        20.0
    } else if total > 0 {
        let ratio = compatible_count as f64 / total as f64;
        1.0 + (ratio * 19.0) // Linear interpolation from 1x to 20x
    } else {
        1.0
    };

    // Build required fields info
    let required_fields: Vec<RequiredFieldInfo> = all_fields
        .into_iter()
        .map(|(path, used_in)| RequiredFieldInfo {
            path,
            inferred_type: "numeric".to_string(), // JIT requires numeric types
            used_in_steps: used_in,
        })
        .collect();

    JITRulesetAnalysis {
        overall_compatible,
        compatible_count,
        incompatible_count,
        total_expressions: total,
        expressions,
        estimated_speedup,
        required_fields,
    }
}

// ============================================================================
// Compiled RuleSet Functions
// ============================================================================

/// Compile a ruleset to binary format (.ordo)
///
/// # Arguments
/// * `ruleset_json` - RuleSet definition as JSON string
///
/// # Returns
/// Binary data as Uint8Array
#[wasm_bindgen]
pub fn compile_ruleset(ruleset_json: &str) -> std::result::Result<Vec<u8>, JsValue> {
    // Parse ruleset
    let ruleset: RuleSet = serde_json::from_str(ruleset_json)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse ruleset: {}", e)))?;

    // Compile to binary format
    let compiled = RuleSetCompiler::compile(&ruleset)
        .map_err(|e| JsValue::from_str(&format!("Failed to compile ruleset: {}", e)))?;

    // Serialize to bytes
    let bytes = compiled.serialize();

    Ok(bytes)
}

/// Execute a compiled ruleset (binary format)
///
/// # Arguments
/// * `compiled_bytes` - Compiled ruleset binary data
/// * `input_json` - Input data as JSON string
/// * `include_trace` - Whether to include execution trace (not supported for compiled, ignored)
///
/// # Returns
/// JSON string containing the execution result
#[wasm_bindgen]
pub fn execute_compiled_ruleset(
    compiled_bytes: &[u8],
    input_json: &str,
) -> std::result::Result<String, JsValue> {
    use ordo_core::rule::CompiledRuleSet;

    // Deserialize compiled ruleset
    let compiled = CompiledRuleSet::deserialize(compiled_bytes).map_err(|e| {
        JsValue::from_str(&format!("Failed to deserialize compiled ruleset: {}", e))
    })?;

    // Parse input
    let input: Value = serde_json::from_str(input_json)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse input: {}", e)))?;

    // Create executor
    let executor = CompiledRuleExecutor::new();

    // Execute
    let start = now();
    let result = executor
        .execute(&compiled, input)
        .map_err(|e| JsValue::from_str(&format!("Execution failed: {}", e)))?;
    let duration_us = ((now() - start) * 1000.0) as u64;

    // Convert output to serde_json::Value
    let output_json: serde_json::Value = serde_json::to_value(&result.output)
        .map_err(|e| JsValue::from_str(&format!("Failed to convert output: {}", e)))?;

    // Build response (no trace for compiled execution)
    let wasm_result = WasmExecutionResult {
        code: result.code,
        message: result.message,
        output: output_json,
        duration_us,
        trace: None,
    };

    // Serialize to JSON
    serde_json::to_string(&wasm_result)
        .map_err(|e| JsValue::from_str(&format!("Failed to serialize result: {}", e)))
}

/// Get compiled ruleset info (metadata)
///
/// # Arguments
/// * `compiled_bytes` - Compiled ruleset binary data
///
/// # Returns
/// JSON string containing metadata
#[wasm_bindgen]
pub fn get_compiled_ruleset_info(compiled_bytes: &[u8]) -> std::result::Result<String, JsValue> {
    use ordo_core::rule::CompiledRuleSet;

    // Deserialize compiled ruleset
    let compiled = CompiledRuleSet::deserialize(compiled_bytes).map_err(|e| {
        JsValue::from_str(&format!("Failed to deserialize compiled ruleset: {}", e))
    })?;

    // Build info response
    let info = serde_json::json!({
        "name": compiled.get_string(compiled.metadata.name).unwrap_or_default(),
        "version": compiled.get_string(compiled.metadata.version).unwrap_or_default(),
        "description": compiled.get_string(compiled.metadata.description).unwrap_or_default(),
        "steps_count": compiled.steps.len(),
        "expressions_count": compiled.expressions.len(),
        "string_pool_size": compiled.string_pool.len(),
    });

    Ok(info.to_string())
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_execute_simple_ruleset() {
        let ruleset_json = r#"{
            "config": {
                "name": "test",
                "version": "1.0.0",
                "entry_step": "start",
                "description": ""
            },
            "steps": {
                "start": {
                    "id": "start",
                    "name": "Start",
                    "kind": {
                        "Terminal": {
                            "code": "SUCCESS",
                            "message": null,
                            "output": []
                        }
                    }
                }
            }
        }"#;

        let input_json = r#"{}"#;

        let result = execute_ruleset(ruleset_json, input_json, false);
        assert!(result.is_ok());

        let result_str = result.unwrap();
        let result_obj: WasmExecutionResult = serde_json::from_str(&result_str).unwrap();
        assert_eq!(result_obj.code, "SUCCESS");
    }
}
