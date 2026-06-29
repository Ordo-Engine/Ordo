//! Prometheus metrics for the Ordo Platform service.
//!
//! Intentionally minimal: a process-up gauge, database connection-pool gauges,
//! and a release-execution outcome counter. It mirrors the `prometheus` +
//! `lazy_static` pattern used by `ordo-server::metrics` so both services scrape
//! the same way.

use lazy_static::lazy_static;
use prometheus::{
    register_int_counter_vec, register_int_gauge, Encoder, IntCounterVec, IntGauge, TextEncoder,
};

lazy_static! {
    /// Always `1` while the platform process is running.
    pub static ref PROCESS_UP: IntGauge = register_int_gauge!(
        "ordo_platform_up",
        "1 if the platform process is running"
    )
    .unwrap();

    /// Total size of the database connection pool (in use + idle).
    pub static ref DB_POOL_SIZE: IntGauge = register_int_gauge!(
        "ordo_platform_db_pool_size",
        "Total number of connections in the database pool"
    )
    .unwrap();

    /// Connections currently checked out of the database pool.
    pub static ref DB_POOL_IN_USE: IntGauge = register_int_gauge!(
        "ordo_platform_db_pool_in_use",
        "Database connections currently in use"
    )
    .unwrap();

    /// Idle connections available in the database pool.
    pub static ref DB_POOL_IDLE: IntGauge = register_int_gauge!(
        "ordo_platform_db_pool_idle",
        "Idle database connections in the pool"
    )
    .unwrap();

    /// Release executions by lifecycle outcome (`started` / `succeeded` / `failed`).
    pub static ref RELEASE_EXECUTIONS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "ordo_platform_release_executions_total",
        "Release executions by lifecycle outcome",
        &["outcome"]
    )
    .unwrap();

    /// Unix timestamp (seconds) of the release worker's last completed poll
    /// iteration. Used as a liveness heartbeat: if `now - this` exceeds a few
    /// poll intervals, the worker loop has stalled or died.
    pub static ref RELEASE_WORKER_LAST_POLL: IntGauge = register_int_gauge!(
        "ordo_platform_release_worker_last_poll_timestamp_seconds",
        "Unix timestamp of the release worker's last completed poll iteration"
    )
    .unwrap();

    /// Release worker poll iterations by outcome (`ok` / `error` / `panic`).
    pub static ref RELEASE_WORKER_POLLS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "ordo_platform_release_worker_polls_total",
        "Release worker poll iterations by outcome",
        &["outcome"]
    )
    .unwrap();
}

/// Record a completed release worker poll iteration with the given outcome
/// (`ok` / `error` / `panic`) and refresh the liveness heartbeat. Called once
/// per loop tick regardless of outcome, so a stalled loop is detectable by the
/// heartbeat going stale.
pub fn record_release_worker_poll(outcome: &str, now_unix_secs: i64) {
    RELEASE_WORKER_LAST_POLL.set(now_unix_secs);
    RELEASE_WORKER_POLLS_TOTAL
        .with_label_values(&[outcome])
        .inc();
}

/// Seconds since the worker's last completed poll, or `None` if it has never
/// polled (gauge still at 0). Used by the worker's liveness probe.
pub fn release_worker_staleness_secs(now_unix_secs: i64) -> Option<i64> {
    let last = RELEASE_WORKER_LAST_POLL.get();
    if last == 0 {
        None
    } else {
        Some(now_unix_secs - last)
    }
}

/// Initialize metrics so they appear in `/metrics` even before any traffic.
pub fn init() {
    PROCESS_UP.set(1);
    // Touch each outcome so the counter is exported at 0 from the start.
    for outcome in ["started", "succeeded", "failed"] {
        RELEASE_EXECUTIONS_TOTAL.with_label_values(&[outcome]);
    }
    for outcome in ["ok", "error", "panic"] {
        RELEASE_WORKER_POLLS_TOTAL.with_label_values(&[outcome]);
    }
}

/// Record that a release execution was started.
pub fn record_release_execution_started() {
    RELEASE_EXECUTIONS_TOTAL
        .with_label_values(&["started"])
        .inc();
}

/// Record that a release execution finished successfully.
pub fn record_release_execution_succeeded() {
    RELEASE_EXECUTIONS_TOTAL
        .with_label_values(&["succeeded"])
        .inc();
}

/// Record that a release execution failed (or its rollback failed).
pub fn record_release_execution_failed() {
    RELEASE_EXECUTIONS_TOTAL
        .with_label_values(&["failed"])
        .inc();
}

/// Refresh the database connection-pool gauges from the live pool stats.
pub fn set_db_pool_stats(size: u32, idle: usize) {
    let size = size as i64;
    let idle = idle as i64;
    DB_POOL_SIZE.set(size);
    DB_POOL_IDLE.set(idle);
    DB_POOL_IN_USE.set((size - idle).max(0));
}

/// Encode all registered metrics in the Prometheus text exposition format.
pub fn encode() -> String {
    PROCESS_UP.set(1);
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&metric_families, &mut buffer) {
        tracing::error!("failed to encode platform metrics: {}", e);
        return String::new();
    }
    String::from_utf8(buffer).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_init_and_encode() {
        init();
        record_release_execution_started();
        record_release_execution_succeeded();
        record_release_execution_failed();
        set_db_pool_stats(10, 4);

        let output = encode();
        assert!(output.contains("ordo_platform_up"));
        assert!(output.contains("ordo_platform_release_executions_total"));
        assert!(output.contains("ordo_platform_db_pool_in_use"));
        // 10 total - 4 idle = 6 in use
        assert!(output.contains("ordo_platform_db_pool_in_use 6"));
    }

    #[test]
    fn worker_heartbeat_and_staleness() {
        // Before any poll the gauge is 0 -> staleness is unknown (None), so the
        // liveness probe gives the worker startup grace.
        // (RELEASE_WORKER_LAST_POLL starts at 0 unless another test set it; we
        // set it explicitly below to make the assertions deterministic.)
        record_release_worker_poll("ok", 1_000);
        assert_eq!(RELEASE_WORKER_LAST_POLL.get(), 1_000);
        // 1_005 - 1_000 = 5 seconds since last poll.
        assert_eq!(release_worker_staleness_secs(1_005), Some(5));

        record_release_worker_poll("error", 2_000);
        assert_eq!(release_worker_staleness_secs(2_050), Some(50));

        let output = encode();
        assert!(output.contains("ordo_platform_release_worker_last_poll_timestamp_seconds"));
        assert!(output.contains("ordo_platform_release_worker_polls_total"));
    }
}
