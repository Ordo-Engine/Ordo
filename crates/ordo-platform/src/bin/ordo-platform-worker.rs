//! Background worker for release rollout and rollback execution.
//!
//! The worker is otherwise headless, so it also runs a tiny HTTP server
//! (`worker_health_addr`) exposing `/health/live` and `/metrics`. This lets a
//! hung or stalled poll loop be detected by k8s liveness probes (the heartbeat
//! goes stale) and lets Prometheus scrape the worker's metrics directly.

use std::sync::Arc;
use std::time::Duration;

use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use clap::Parser;
use ordo_platform::{
    bootstrap_platform_store, build_app_state, config::PlatformConfig, connect_platform_store,
    init_tracing, metrics, publish_existing_tenants, release, start_server_registry_maintenance,
};
use tracing::{error, info};

/// Poll interval for the release worker loop.
const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// The worker's last poll must be no older than this for `/health/live` to pass.
/// Generous relative to the 2s poll interval so a single slow poll doesn't flap
/// the liveness probe, while a genuinely stalled loop is still caught quickly.
const LIVENESS_STALENESS_LIMIT_SECS: i64 = 30;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Arc::new(PlatformConfig::parse());
    init_tracing(&config)?;

    if let Err(e) = config.validate() {
        return Err(anyhow::anyhow!("Configuration error: {}", e));
    }

    info!("Starting ordo-platform-worker");
    if config.nats_enabled() {
        info!(
            "NATS sync enabled (url={}, prefix={})",
            config.nats_url.as_deref().unwrap_or(""),
            config.nats_subject_prefix
        );
    }

    metrics::init();

    let store = connect_platform_store(&config).await?;
    bootstrap_platform_store(&store, false).await?;
    start_server_registry_maintenance(store.clone());

    // Spawn the liveness/metrics server before entering the loop.
    let health_addr = config.worker_health_addr;
    tokio::spawn(async move {
        let app = Router::new()
            .route("/health/live", get(liveness))
            .route("/metrics", get(metrics_handler));
        match tokio::net::TcpListener::bind(health_addr).await {
            Ok(listener) => {
                info!("worker health/metrics server listening on {}", health_addr);
                if let Err(e) = axum::serve(listener, app).await {
                    error!("worker health server error: {e}");
                }
            }
            Err(e) => error!("failed to bind worker health server on {health_addr}: {e}"),
        }
    });

    let state = build_app_state(config, store, true).await?;
    publish_existing_tenants(&state).await;

    release::run_release_worker_loop(state, POLL_INTERVAL).await
}

/// Liveness probe: `200` while the poll loop is cycling, `503` once its last
/// completed poll is older than the staleness limit (loop hung or dead). Before
/// the first poll completes the worker is considered live (startup grace).
async fn liveness() -> Response {
    let now = chrono::Utc::now().timestamp();
    match metrics::release_worker_staleness_secs(now) {
        Some(stale) if stale > LIVENESS_STALENESS_LIMIT_SECS => (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("worker stalled: last poll {stale}s ago"),
        )
            .into_response(),
        _ => (StatusCode::OK, "ok").into_response(),
    }
}

/// Prometheus metrics for the headless worker process.
async fn metrics_handler() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        metrics::encode(),
    )
        .into_response()
}
