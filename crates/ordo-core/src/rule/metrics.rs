//! Metric sink trait for recording custom metrics from rule actions
//!
//! This module provides a trait-based abstraction for metrics recording,
//! allowing the core library to remain independent of specific metrics
//! implementations (e.g., Prometheus).

use std::sync::Arc;

/// Metric type for rule actions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricType {
    /// Gauge - a value that can go up and down
    Gauge,
    /// Counter - a value that only increases
    Counter,
}

/// Trait for recording custom metrics from rule actions
///
/// Implementations can connect to various metrics backends like
/// Prometheus, StatsD, or custom systems.
pub trait MetricSink: Send + Sync {
    /// Record a gauge metric (can increase or decrease)
    fn record_gauge(&self, name: &str, value: f64, tags: &[(String, String)]);

    /// Record a counter increment
    fn record_counter(&self, name: &str, value: f64, tags: &[(String, String)]);

    /// Record a metric with explicit type
    fn record(&self, metric_type: MetricType, name: &str, value: f64, tags: &[(String, String)]) {
        match metric_type {
            MetricType::Gauge => self.record_gauge(name, value, tags),
            MetricType::Counter => self.record_counter(name, value, tags),
        }
    }
}

/// No-op implementation of MetricSink
///
/// This is the default implementation that does nothing,
/// useful when metrics collection is not needed.
#[derive(Debug, Clone, Default)]
pub struct NoOpMetricSink;

impl MetricSink for NoOpMetricSink {
    fn record_gauge(&self, _name: &str, _value: f64, _tags: &[(String, String)]) {
        // No-op
    }

    fn record_counter(&self, _name: &str, _value: f64, _tags: &[(String, String)]) {
        // No-op
    }
}

/// Logging implementation of MetricSink
///
/// Records metrics via tracing logs, useful for debugging.
#[derive(Debug, Clone, Default)]
pub struct LoggingMetricSink;

impl MetricSink for LoggingMetricSink {
    fn record_gauge(&self, name: &str, value: f64, tags: &[(String, String)]) {
        tracing::info!(
            metric_type = "gauge",
            metric_name = %name,
            metric_value = %value,
            tags = ?tags,
            "Rule metric recorded"
        );
    }

    fn record_counter(&self, name: &str, value: f64, tags: &[(String, String)]) {
        tracing::info!(
            metric_type = "counter",
            metric_name = %name,
            metric_value = %value,
            tags = ?tags,
            "Rule metric recorded"
        );
    }
}

/// Create a default no-op metric sink
#[allow(dead_code)]
pub fn no_op_sink() -> Arc<dyn MetricSink> {
    Arc::new(NoOpMetricSink)
}

/// Create a logging metric sink
#[allow(dead_code)]
pub fn logging_sink() -> Arc<dyn MetricSink> {
    Arc::new(LoggingMetricSink)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_op_sink() {
        let sink = NoOpMetricSink;
        // Should not panic
        sink.record_gauge("test", 1.0, &[]);
        sink.record_counter("test", 1.0, &[("key".to_string(), "value".to_string())]);
    }

    #[test]
    fn test_metric_type_record() {
        let sink = NoOpMetricSink;
        sink.record(MetricType::Gauge, "test_gauge", 42.0, &[]);
        sink.record(MetricType::Counter, "test_counter", 1.0, &[]);
    }
}
