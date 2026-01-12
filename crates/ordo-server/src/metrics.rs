//! Prometheus metrics for Ordo Server
//!
//! This module defines all metrics collected by the server for monitoring and observability.
//! It also provides a `PrometheusMetricSink` implementation for recording custom metrics
//! from rule actions.

use lazy_static::lazy_static;
use ordo_core::prelude::MetricSink;
use parking_lot::RwLock;
use prometheus::{
    register_counter_vec, register_gauge, register_gauge_vec, register_histogram_vec,
    register_int_gauge, Counter, CounterVec, Encoder, Gauge, GaugeVec, HistogramVec, IntGauge,
    Opts, Registry, TextEncoder,
};
use std::collections::HashMap;
use std::sync::Arc;
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

// ==================== Custom Rule Metrics (PrometheusMetricSink) ====================

/// Prometheus implementation of MetricSink for recording custom metrics from rule actions.
///
/// This sink dynamically creates Prometheus gauges and counters based on the metric names
/// and tags provided by rule actions.
pub struct PrometheusMetricSink {
    /// Registry for custom rule metrics
    registry: Registry,
    /// Cache of gauge metrics by name
    gauges: RwLock<HashMap<String, GaugeVec>>,
    /// Cache of counter metrics by name
    counters: RwLock<HashMap<String, CounterVec>>,
}

impl PrometheusMetricSink {
    /// Create a new PrometheusMetricSink
    pub fn new() -> Self {
        Self {
            registry: Registry::new(),
            gauges: RwLock::new(HashMap::new()),
            counters: RwLock::new(HashMap::new()),
        }
    }

    /// Get or create a gauge metric
    fn get_or_create_gauge(&self, name: &str, tags: &[(String, String)]) -> Gauge {
        let label_names: Vec<&str> = tags.iter().map(|(k, _)| k.as_str()).collect();
        let label_values: Vec<&str> = tags.iter().map(|(_, v)| v.as_str()).collect();

        // Check if gauge exists
        {
            let gauges = self.gauges.read();
            if let Some(gauge_vec) = gauges.get(name) {
                return gauge_vec.with_label_values(&label_values);
            }
        }

        // Create new gauge
        let mut gauges = self.gauges.write();
        // Double-check after acquiring write lock
        if let Some(gauge_vec) = gauges.get(name) {
            return gauge_vec.with_label_values(&label_values);
        }

        let metric_name = format!("ordo_rule_{}", sanitize_metric_name(name));
        let opts = Opts::new(&metric_name, format!("Custom rule metric: {}", name));
        let gauge_vec = GaugeVec::new(opts, &label_names).unwrap();

        // Register with custom registry
        if let Err(e) = self.registry.register(Box::new(gauge_vec.clone())) {
            tracing::warn!(metric = %name, error = %e, "Failed to register gauge metric");
        }

        gauges.insert(name.to_string(), gauge_vec.clone());
        gauge_vec.with_label_values(&label_values)
    }

    /// Get or create a counter metric
    fn get_or_create_counter(&self, name: &str, tags: &[(String, String)]) -> Counter {
        let label_names: Vec<&str> = tags.iter().map(|(k, _)| k.as_str()).collect();
        let label_values: Vec<&str> = tags.iter().map(|(_, v)| v.as_str()).collect();

        // Check if counter exists
        {
            let counters = self.counters.read();
            if let Some(counter_vec) = counters.get(name) {
                return counter_vec.with_label_values(&label_values);
            }
        }

        // Create new counter
        let mut counters = self.counters.write();
        // Double-check after acquiring write lock
        if let Some(counter_vec) = counters.get(name) {
            return counter_vec.with_label_values(&label_values);
        }

        let metric_name = format!("ordo_rule_{}_total", sanitize_metric_name(name));
        let opts = Opts::new(&metric_name, format!("Custom rule counter: {}", name));
        let counter_vec = CounterVec::new(opts, &label_names).unwrap();

        // Register with custom registry
        if let Err(e) = self.registry.register(Box::new(counter_vec.clone())) {
            tracing::warn!(metric = %name, error = %e, "Failed to register counter metric");
        }

        counters.insert(name.to_string(), counter_vec.clone());
        counter_vec.with_label_values(&label_values)
    }

    /// Encode custom rule metrics to Prometheus format
    pub fn encode_custom_metrics(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap()
    }
}

impl Default for PrometheusMetricSink {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricSink for PrometheusMetricSink {
    fn record_gauge(&self, name: &str, value: f64, tags: &[(String, String)]) {
        let gauge = self.get_or_create_gauge(name, tags);
        gauge.set(value);
        tracing::trace!(metric = %name, value = %value, "Gauge metric recorded");
    }

    fn record_counter(&self, name: &str, value: f64, tags: &[(String, String)]) {
        let counter = self.get_or_create_counter(name, tags);
        counter.inc_by(value);
        tracing::trace!(metric = %name, value = %value, "Counter metric recorded");
    }
}

/// Sanitize metric name for Prometheus compatibility
/// Prometheus metric names must match [a-zA-Z_:][a-zA-Z0-9_:]*
fn sanitize_metric_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == ':' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Create a new PrometheusMetricSink wrapped in Arc
#[allow(dead_code)]
pub fn create_prometheus_sink() -> Arc<PrometheusMetricSink> {
    Arc::new(PrometheusMetricSink::new())
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

    #[test]
    fn test_prometheus_metric_sink_gauge() {
        let sink = PrometheusMetricSink::new();

        // Record a gauge metric
        sink.record_gauge(
            "order_amount",
            150.0,
            &[("status".to_string(), "approved".to_string())],
        );

        let output = sink.encode_custom_metrics();
        assert!(output.contains("ordo_rule_order_amount"));
        assert!(output.contains("150"));
    }

    #[test]
    fn test_prometheus_metric_sink_counter() {
        let sink = PrometheusMetricSink::new();

        // Record a counter metric
        sink.record_counter(
            "orders_processed",
            1.0,
            &[("region".to_string(), "us-west".to_string())],
        );
        sink.record_counter(
            "orders_processed",
            1.0,
            &[("region".to_string(), "us-west".to_string())],
        );

        let output = sink.encode_custom_metrics();
        assert!(output.contains("ordo_rule_orders_processed_total"));
        assert!(output.contains("2")); // Counter should be 2
    }

    #[test]
    fn test_sanitize_metric_name() {
        assert_eq!(sanitize_metric_name("order-amount"), "order_amount");
        assert_eq!(sanitize_metric_name("order.total"), "order_total");
        assert_eq!(sanitize_metric_name("valid_name"), "valid_name");
        assert_eq!(sanitize_metric_name("name:with:colons"), "name:with:colons");
    }
}
