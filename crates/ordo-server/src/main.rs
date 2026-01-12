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
//! ```

use std::sync::Arc;

use axum::{
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
mod config;
mod error;
mod grpc;
mod store;
#[cfg(unix)]
mod uds;

use config::ServerConfig;
use grpc::OrdoGrpcService;
use store::RuleStore;

/// Application state shared between HTTP handlers
#[derive(Clone)]
pub struct AppState {
    store: Arc<RwLock<RuleStore>>,
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

    // Initialize shared store (with or without persistence)
    let store = if let Some(ref rules_dir) = config.rules_dir {
        info!("Initializing store with persistence at {:?}", rules_dir);
        let mut store = RuleStore::new_with_persistence(rules_dir.clone());

        // Load existing rules from directory
        match store.load_from_dir() {
            Ok(count) => {
                if count > 0 {
                    info!("Loaded {} rules from {:?}", count, rules_dir);
                } else {
                    info!("No rules found in {:?}, starting with empty store", rules_dir);
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
        Arc::new(RwLock::new(RuleStore::new()))
    };

    // Create tasks for each enabled protocol
    let mut tasks = Vec::new();

    // HTTP Server
    if config.http_enabled() {
        let http_store = store.clone();
        let http_addr = config.http_addr;
        tasks.push(tokio::spawn(async move {
            start_http_server(http_addr, http_store).await
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
) -> anyhow::Result<()> {
    let state = AppState { store };

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
        // Rule execution
        .route("/api/v1/execute/:name", post(api::execute_ruleset))
        // Expression evaluation (debug)
        .route("/api/v1/eval", post(api::eval_expression))
        // Metrics
        .route("/metrics", get(metrics))
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

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "version": ordo_core::VERSION,
    }))
}

/// Metrics endpoint (placeholder)
async fn metrics() -> impl IntoResponse {
    // TODO: Integrate with prometheus
    "# Ordo metrics\n"
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
