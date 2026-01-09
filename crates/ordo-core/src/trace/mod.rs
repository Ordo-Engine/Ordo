//! Execution trace module
//!
//! Provides execution tracing for debugging and monitoring

use crate::context::Value;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Trace configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TraceConfig {
    /// Whether tracing is enabled
    #[serde(default)]
    pub enabled: bool,

    /// Whether to capture input at each step
    #[serde(default)]
    pub capture_input: bool,

    /// Whether to capture variables at each step
    #[serde(default)]
    pub capture_variables: bool,

    /// Maximum number of steps to trace
    #[serde(default = "default_max_steps")]
    pub max_steps: usize,

    /// Sampling rate (0.0 - 1.0)
    #[serde(default = "default_sample_rate")]
    pub sample_rate: f64,
}

fn default_max_steps() -> usize {
    1000
}

fn default_sample_rate() -> f64 {
    1.0
}

impl TraceConfig {
    /// Create a full trace config (captures everything)
    pub fn full() -> Self {
        Self {
            enabled: true,
            capture_input: true,
            capture_variables: true,
            max_steps: default_max_steps(),
            sample_rate: 1.0,
        }
    }

    /// Create a minimal trace config (only step IDs and timing)
    pub fn minimal() -> Self {
        Self {
            enabled: true,
            capture_input: false,
            capture_variables: false,
            max_steps: default_max_steps(),
            sample_rate: 1.0,
        }
    }

    /// Create a disabled trace config
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }
}

/// Complete execution trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTrace {
    /// RuleSet name
    pub ruleset_name: String,

    /// Trace ID (for correlation)
    pub trace_id: String,

    /// Start timestamp (Unix millis)
    pub start_time: i64,

    /// Step traces
    pub steps: Vec<StepTrace>,

    /// Total execution time in microseconds
    #[serde(default)]
    pub total_duration_us: u64,

    /// Final result code
    #[serde(default)]
    pub result_code: String,

    /// Error message if any
    #[serde(default)]
    pub error: Option<String>,
}

impl ExecutionTrace {
    /// Create a new trace
    pub fn new(ruleset_name: &str) -> Self {
        Self {
            ruleset_name: ruleset_name.to_string(),
            trace_id: generate_trace_id(),
            start_time: get_current_timestamp_millis(),
            steps: Vec::new(),
            total_duration_us: 0,
            result_code: String::new(),
            error: None,
        }
    }

    /// Add a step trace
    pub fn add_step(&mut self, step: StepTrace) {
        self.steps.push(step);
    }

    /// Set the result
    pub fn set_result(&mut self, code: &str, duration_us: u64) {
        self.result_code = code.to_string();
        self.total_duration_us = duration_us;
    }

    /// Set an error
    pub fn set_error(&mut self, error: &str) {
        self.error = Some(error.to_string());
    }

    /// Get step count
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Get total duration in milliseconds
    pub fn duration_ms(&self) -> f64 {
        self.total_duration_us as f64 / 1000.0
    }

    /// Convert to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Get step path as string
    pub fn path_string(&self) -> String {
        self.steps
            .iter()
            .map(|s| s.step_id.as_str())
            .collect::<Vec<_>>()
            .join(" -> ")
    }
}

/// Single step trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepTrace {
    /// Step ID
    pub step_id: String,

    /// Step name
    pub step_name: String,

    /// Execution duration in microseconds
    pub duration_us: u64,

    /// Input data snapshot (if captured)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_snapshot: Option<Value>,

    /// Variables snapshot (if captured)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables_snapshot: Option<HashMap<String, Value>>,

    /// Next step ID (if continued)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_step: Option<String>,

    /// Whether this was a terminal step
    #[serde(default)]
    pub is_terminal: bool,
}

impl StepTrace {
    /// Create a minimal step trace
    pub fn minimal(step_id: &str, step_name: &str, duration_us: u64) -> Self {
        Self {
            step_id: step_id.to_string(),
            step_name: step_name.to_string(),
            duration_us,
            input_snapshot: None,
            variables_snapshot: None,
            next_step: None,
            is_terminal: false,
        }
    }

    /// Create a step trace that continues to next step
    pub fn continued(step_id: &str, step_name: &str, duration_us: u64, next_step: &str) -> Self {
        Self {
            step_id: step_id.to_string(),
            step_name: step_name.to_string(),
            duration_us,
            input_snapshot: None,
            variables_snapshot: None,
            next_step: Some(next_step.to_string()),
            is_terminal: false,
        }
    }

    /// Create a terminal step trace
    pub fn terminal(step_id: &str, step_name: &str, duration_us: u64) -> Self {
        Self {
            step_id: step_id.to_string(),
            step_name: step_name.to_string(),
            duration_us,
            input_snapshot: None,
            variables_snapshot: None,
            next_step: None,
            is_terminal: true,
        }
    }
}

/// Get current timestamp in milliseconds
#[cfg(not(target_arch = "wasm32"))]
fn get_current_timestamp_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Get current timestamp in milliseconds (WASM version - returns 0)
#[cfg(target_arch = "wasm32")]
fn get_current_timestamp_millis() -> i64 {
    // In WASM, we can't access system time directly
    // The JS wrapper will provide the actual timestamp
    0
}

/// Generate a unique trace ID
#[cfg(not(target_arch = "wasm32"))]
fn generate_trace_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    let nanos = duration.as_nanos();
    let random: u32 = (nanos % 0xFFFFFFFF) as u32;

    format!("{:016x}{:08x}", nanos as u64, random)
}

/// Generate a unique trace ID (WASM version)
#[cfg(target_arch = "wasm32")]
fn generate_trace_id() -> String {
    // In WASM, generate a simple random-ish ID
    static mut COUNTER: u64 = 0;
    unsafe {
        COUNTER += 1;
        format!("wasm-trace-{:016x}", COUNTER)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_config() {
        let full = TraceConfig::full();
        assert!(full.enabled);
        assert!(full.capture_input);
        assert!(full.capture_variables);

        let minimal = TraceConfig::minimal();
        assert!(minimal.enabled);
        assert!(!minimal.capture_input);
        assert!(!minimal.capture_variables);

        let disabled = TraceConfig::disabled();
        assert!(!disabled.enabled);
    }

    #[test]
    fn test_execution_trace() {
        let mut trace = ExecutionTrace::new("test_ruleset");

        trace.add_step(StepTrace::minimal("step1", "Step 1", 100));
        trace.add_step(StepTrace::minimal("step2", "Step 2", 200));

        assert_eq!(trace.step_count(), 2);
        assert_eq!(trace.path_string(), "step1 -> step2");
    }
}
