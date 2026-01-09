//! WebAssembly bindings for Ordo Rule Engine
//!
//! This crate provides WASM-compatible bindings for executing rules in the browser.

use ordo_core::prelude::*;
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
    web_sys::console::log_1(&format!("[WASM DEBUG] Parsed ruleset steps: {:?}", ruleset.steps.keys().collect::<Vec<_>>()).into());
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
pub fn eval_expression(expression: &str, context_json: &str) -> std::result::Result<String, JsValue> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
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

