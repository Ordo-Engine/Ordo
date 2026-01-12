//! Prometheus metrics for Ordo Server
//!
//! This module defines all metrics collected by the server for monitoring and observability.

use lazy_static::lazy_static;
use prometheus::{
    register_counter_vec, register_gauge, register_gauge_vec, register_histogram_vec,
    register_int_gauge, CounterVec, Encoder, Gauge, GaugeVec, HistogramVec, IntGauge, TextEncoder,
};
use std::time::Instant;

lazy_static! {
    /// Server start time for uptime calculation
    pub static ref START_TIME: Instant = Instant::now();

    // ==================== Engine Metrics ====================

    /// Server info gauge (always 1, used for version label)
    pub static ref INFO: GaugeVec = register_gauge_vec!(
        "ordo_info",
        "Ordo server info",
        &["version"]
    ).unwrap();

    /// Server uptime in seconds
    pub static ref UPTIME_SECONDS: Gauge = register_gauge!(
        "ordo_uptime_seconds",
        "Server uptime in seconds"
    ).unwrap();

    // ==================== Rule Metrics ====================

    /// Total number of loaded rules
    pub static ref RULES_TOTAL: IntGauge = register_int_gauge!(
        "ordo_rules_total",
        "Total number of loaded rules"
    ).unwrap();

    // ==================== Execution Metrics ====================

    /// Total rule executions counter
    pub static ref EXECUTIONS_TOTAL: CounterVec = register_counter_vec!(
        "ordo_executions_total",
        "Total number of rule executions",
        &["ruleset", "result"]
    ).unwrap();

    /// Rule execution duration histogram
    pub static ref EXECUTION_DURATION: HistogramVec = register_histogram_vec!(
        "ordo_execution_duration_seconds",
        "Rule execution duration in seconds",
        &["ruleset"],
        // Buckets optimized for microsecond-level latency
        vec![0.00001, 0.00005, 0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0]
    ).unwrap();

    // ==================== Expression Evaluation Metrics ====================

    /// Total expression evaluations counter
    pub static ref EVALS_TOTAL: CounterVec = register_counter_vec!(
        "ordo_evals_total",
        "Total number of expression evaluations",
        &["result"]
    ).unwrap();

    /// Expression evaluation duration histogram
    pub static ref EVAL_DURATION: HistogramVec = register_histogram_vec!(
        "ordo_eval_duration_seconds",
        "Expression evaluation duration in seconds",
        &[],
        // Buckets for nanosecond to microsecond level
        vec![0.000001, 0.000005, 0.00001, 0.00005, 0.0001, 0.0005, 0.001, 0.01]
    ).unwrap();

    // ==================== HTTP Request Metrics ====================

    /// Total HTTP requests counter
    pub static ref HTTP_REQUESTS_TOTAL: CounterVec = register_counter_vec!(
        "ordo_http_requests_total",
        "Total number of HTTP requests",
        &["method", "endpoint", "status"]
    ).unwrap();
}

/// Initialize metrics (call once at startup)
pub fn init() {
    // Set version info
    INFO.with_label_values(&[ordo_core::VERSION]).set(1.0);

    // Initialize counters to ensure they appear in /metrics even with 0 value
    RULES_TOTAL.set(0);
}

/// Update uptime metric
pub fn update_uptime() {
    UPTIME_SECONDS.set(START_TIME.elapsed().as_secs_f64());
}

/// Update rules count metric
pub fn set_rules_count(count: i64) {
    RULES_TOTAL.set(count);
}

/// Record a successful rule execution
pub fn record_execution_success(ruleset: &str, duration_secs: f64) {
    EXECUTIONS_TOTAL
        .with_label_values(&[ruleset, "success"])
        .inc();
    EXECUTION_DURATION
        .with_label_values(&[ruleset])
        .observe(duration_secs);
}

/// Record a failed rule execution
pub fn record_execution_error(ruleset: &str, duration_secs: f64) {
    EXECUTIONS_TOTAL
        .with_label_values(&[ruleset, "error"])
        .inc();
    EXECUTION_DURATION
        .with_label_values(&[ruleset])
        .observe(duration_secs);
}

/// Record a successful expression evaluation
pub fn record_eval_success(duration_secs: f64) {
    EVALS_TOTAL.with_label_values(&["success"]).inc();
    EVAL_DURATION.with_label_values(&[]).observe(duration_secs);
}

/// Record a failed expression evaluation
pub fn record_eval_error(duration_secs: f64) {
    EVALS_TOTAL.with_label_values(&["error"]).inc();
    EVAL_DURATION.with_label_values(&[]).observe(duration_secs);
}

/// Encode all metrics to Prometheus text format
pub fn encode_metrics() -> String {
    // Update dynamic metrics before encoding
    update_uptime();

    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_init() {
        init();
        // Verify version info is set
        let output = encode_metrics();
        assert!(output.contains("ordo_info"));
        assert!(output.contains(ordo_core::VERSION));
    }

    #[test]
    fn test_execution_metrics() {
        record_execution_success("test-rule", 0.001);
        record_execution_error("test-rule", 0.002);

        let output = encode_metrics();
        assert!(output.contains("ordo_executions_total"));
        assert!(output.contains("ordo_execution_duration_seconds"));
    }

    #[test]
    fn test_eval_metrics() {
        record_eval_success(0.0001);
        record_eval_error(0.0002);

        let output = encode_metrics();
        assert!(output.contains("ordo_evals_total"));
        assert!(output.contains("ordo_eval_duration_seconds"));
    }
}
