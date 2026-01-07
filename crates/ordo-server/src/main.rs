//! Ordo Rule Engine Server
//!
//! Provides HTTP and gRPC APIs for rule execution

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use tokio::sync::RwLock;
use tower_http::trace::TraceLayer;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

mod api;
mod error;
mod store;

use store::RuleStore;

/// Application state
#[derive(Clone)]
pub struct AppState {
    store: Arc<RwLock<RuleStore>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Initialize state
    let state = AppState {
        store: Arc::new(RwLock::new(RuleStore::new())),
    };

    // Build router
    let app = Router::new()
        // Health check
        .route("/health", get(health_check))
        // Rule management
        .route("/api/v1/rulesets", get(api::list_rulesets).post(api::create_ruleset))
        .route("/api/v1/rulesets/:name", get(api::get_ruleset).delete(api::delete_ruleset))
        // Rule execution
        .route("/api/v1/execute/:name", post(api::execute_ruleset))
        // Expression evaluation (debug)
        .route("/api/v1/eval", post(api::eval_expression))
        // Metrics
        .route("/metrics", get(metrics))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    info!("Starting Ordo server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

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
