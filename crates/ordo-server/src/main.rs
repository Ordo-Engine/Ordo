//! Ordo Rule Engine Server
//!
//! Provides HTTP, gRPC, and Unix Domain Socket APIs for rule execution.
//!
//! # Usage
//!
//! ```bash
//! # Start with default settings (HTTP on 8080, gRPC on 50051)
//! ordo-server
//!
//! # Start with UDS support
//! ordo-server --uds-path /tmp/ordo.sock
//!
//! # Disable HTTP, only gRPC
//! ordo-server --disable-http
//!
//! # Custom ports
//! ordo-server --http-addr 0.0.0.0:9090 --grpc-addr 0.0.0.0:9091
//!
//! # Enable audit logging with 10% sampling
//! ordo-server --audit-dir ./audit --audit-sample-rate 10
//! ```

use std::sync::Arc;

use axum::{
    extract::State,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use clap::Parser;
use ordo_proto::ordo_service_server::OrdoServiceServer;
use tokio::sync::RwLock;
use tonic::transport::Server as TonicServer;
use tower_http::trace::TraceLayer;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

mod api;
mod audit;
mod config;
mod error;
mod grpc;
mod metrics;
mod store;
#[cfg(unix)]
mod uds;

use audit::AuditLogger;
use config::ServerConfig;
use grpc::OrdoGrpcService;
use metrics::PrometheusMetricSink;
use store::RuleStore;

/// Application state shared between HTTP handlers
#[derive(Clone)]
pub struct AppState {
    store: Arc<RwLock<RuleStore>>,
    audit_logger: Arc<AuditLogger>,
    metric_sink: Arc<PrometheusMetricSink>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse command line arguments
    let config = ServerConfig::parse();

    // Initialize logging
    let log_level = match config.log_level.as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };
    let subscriber = FmtSubscriber::builder().with_max_level(log_level).finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Initialize Prometheus metric sink for custom rule metrics
    let metric_sink = Arc::new(PrometheusMetricSink::new());
    info!("Initialized Prometheus metric sink for custom rule metrics");

    // Initialize shared store (with or without persistence)
    let store = if let Some(ref rules_dir) = config.rules_dir {
        info!(
            "Initializing store with persistence at {:?} (max {} versions)",
            rules_dir, config.max_versions
        );
        let mut store = RuleStore::new_with_persistence_and_metrics(
            rules_dir.clone(),
            config.max_versions,
            metric_sink.clone(),
        );

        // Load existing rules from directory
        match store.load_from_dir() {
            Ok(count) => {
                if count > 0 {
                    info!("Loaded {} rules from {:?}", count, rules_dir);
                } else {
                    info!(
                        "No rules found in {:?}, starting with empty store",
                        rules_dir
                    );
                }
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to load rules from {:?}: {}",
                    rules_dir,
                    e
                ));
            }
        }

        Arc::new(RwLock::new(store))
    } else {
        info!("Initializing in-memory store (no persistence)");
        Arc::new(RwLock::new(RuleStore::new_with_metrics(
            metric_sink.clone(),
        )))
    };

    // Initialize metrics
    metrics::init();
    {
        let store_guard = store.read().await;
        metrics::set_rules_count(store_guard.len() as i64);
    }

    // Initialize audit logger
    let audit_logger = Arc::new(AuditLogger::new(
        config.audit_dir.clone(),
        config.audit_sample_rate,
    ));

    // Log audit configuration
    if config.audit_dir.is_some() {
        info!(
            "Audit logging enabled: dir={:?}, sample_rate={}%",
            config.audit_dir, config.audit_sample_rate
        );
    } else {
        info!(
            "Audit logging to stdout only, sample_rate={}%",
            config.audit_sample_rate
        );
    }

    // Log server started event
    {
        let store_guard = store.read().await;
        audit_logger.log_server_started(ordo_core::VERSION, store_guard.len());
    }

    // Create tasks for each enabled protocol
    let mut tasks = Vec::new();

    // HTTP Server
    if config.http_enabled() {
        let http_store = store.clone();
        let http_audit_logger = audit_logger.clone();
        let http_metric_sink = metric_sink.clone();
        let http_addr = config.http_addr;
        tasks.push(tokio::spawn(async move {
            start_http_server(http_addr, http_store, http_audit_logger, http_metric_sink).await
        }));
    }

    // gRPC Server
    if config.grpc_enabled() {
        let grpc_store = store.clone();
        let grpc_addr = config.grpc_addr;
        tasks.push(tokio::spawn(async move {
            start_grpc_server(grpc_addr, grpc_store).await
        }));
    }

    // UDS Server (Unix only)
    #[cfg(unix)]
    if config.uds_enabled() {
        let uds_store = store.clone();
        let uds_path = config.uds_path.clone().unwrap();
        tasks.push(tokio::spawn(async move {
            uds::start_uds_server(&uds_path, uds_store)
                .await
                .map_err(|e| anyhow::anyhow!("UDS server error: {}", e))
        }));
    }

    // Setup shutdown signal handler
    let shutdown_audit_logger = audit_logger.clone();
    tokio::spawn(async move {
        // Wait for Ctrl+C or SIGTERM
        let ctrl_c = tokio::signal::ctrl_c();
        #[cfg(unix)]
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();

        #[cfg(unix)]
        tokio::select! {
            _ = ctrl_c => {},
            _ = sigterm.recv() => {},
        }

        #[cfg(not(unix))]
        ctrl_c.await.ok();

        // Log server stopped event
        let uptime = metrics::START_TIME.elapsed().as_secs();
        shutdown_audit_logger.log_server_stopped(uptime);
        info!("Server shutting down after {} seconds", uptime);
    });

    // Wait for any server to finish (usually due to error)
    if !tasks.is_empty() {
        let (result, _, _) = futures::future::select_all(tasks).await;
        result??;
    } else {
        info!("No servers enabled. Exiting.");
    }

    Ok(())
}

/// Start the HTTP server
async fn start_http_server(
    addr: std::net::SocketAddr,
    store: Arc<RwLock<RuleStore>>,
    audit_logger: Arc<AuditLogger>,
    metric_sink: Arc<PrometheusMetricSink>,
) -> anyhow::Result<()> {
    let state = AppState {
        store,
        audit_logger,
        metric_sink,
    };

    // Build router
    let app = Router::new()
        // Health check
        .route("/health", get(health_check))
        // Rule management (Admin API)
        .route(
            "/api/v1/rulesets",
            get(api::list_rulesets).post(api::create_ruleset),
        )
        .route(
            "/api/v1/rulesets/:name",
            get(api::get_ruleset).delete(api::delete_ruleset),
        )
        // Version management
        .route(
            "/api/v1/rulesets/:name/versions",
            get(api::list_versions),
        )
        .route(
            "/api/v1/rulesets/:name/rollback",
            post(api::rollback_ruleset),
        )
        // Rule execution
        .route("/api/v1/execute/:name", post(api::execute_ruleset))
        // Expression evaluation (debug)
        .route("/api/v1/eval", post(api::eval_expression))
        // Audit configuration
        .route(
            "/api/v1/config/audit-sample-rate",
            get(api::get_audit_sample_rate).put(api::set_audit_sample_rate),
        )
        // Metrics
        .route("/metrics", get(prometheus_metrics))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    info!("HTTP server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Start the gRPC server
async fn start_grpc_server(
    addr: std::net::SocketAddr,
    store: Arc<RwLock<RuleStore>>,
) -> anyhow::Result<()> {
    let grpc_service = OrdoGrpcService::new(store);

    info!("gRPC server listening on {}", addr);

    TonicServer::builder()
        .add_service(OrdoServiceServer::new(grpc_service))
        .serve(addr)
        .await?;

    Ok(())
}

/// Health check endpoint with detailed status
async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    let store = state.store.read().await;

    // Determine storage mode and info
    let storage_info = if store.persistence_enabled() {
        serde_json::json!({
            "mode": "persistent",
            "rules_dir": store.rules_dir().map(|p| p.display().to_string()),
            "rules_count": store.len()
        })
    } else {
        serde_json::json!({
            "mode": "memory",
            "rules_count": store.len()
        })
    };

    // Update metrics
    metrics::set_rules_count(store.len() as i64);

    Json(serde_json::json!({
        "status": "healthy",
        "version": ordo_core::VERSION,
        "uptime_seconds": metrics::START_TIME.elapsed().as_secs(),
        "storage": storage_info
    }))
}

/// Prometheus metrics endpoint
async fn prometheus_metrics(State(state): State<AppState>) -> impl IntoResponse {
    // Update rules count before encoding
    let store = state.store.read().await;
    metrics::set_rules_count(store.len() as i64);
    drop(store);

    // Combine standard metrics with custom rule metrics
    let standard_metrics = metrics::encode_metrics();
    let custom_metrics = state.metric_sink.encode_custom_metrics();

    // Return Prometheus text format
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        format!("{}\n{}", standard_metrics, custom_metrics),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ServerConfig::default();
        assert!(config.http_enabled());
        assert!(config.grpc_enabled());
        assert!(!config.uds_enabled());
    }
}
