//! Ordo Platform Server
//!
//! Standalone HTTP service providing:
//! - User authentication (register, login, JWT)
//! - Organization and member management
//! - Project (decision domain) management
//! - Authenticated proxy to ordo-server engine API
//!
//! Designed to run alongside ordo-server without modifying it.
//! ordo-server remains a pure rule engine with zero platform code.

use std::sync::Arc;
use std::time::Duration;

use axum::{
    routing::{any, get, post, put},
    Router,
};
use clap::Parser;
use tower_http::{
    catch_panic::CatchPanicLayer,
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::info;

mod auth;
mod catalog;
mod config;
mod contract;
mod error;
mod member;
mod middleware;
mod models;
mod org;
mod project;
mod proxy;
mod ruleset_history;
mod store;
mod template;
mod templates_api;
mod testing;

use config::PlatformConfig;
use middleware::require_auth;
use store::PlatformStore;
use template::TemplateStore;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub store: Arc<PlatformStore>,
    pub config: Arc<PlatformConfig>,
    pub http_client: reqwest::Client,
    pub templates: Arc<TemplateStore>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Arc::new(PlatformConfig::parse());

    // Init logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                config
                    .log_level
                    .parse()
                    .unwrap_or_else(|_| "info".parse().unwrap())
            }),
        )
        .init();

    // Validate config
    if let Err(e) = config.validate() {
        return Err(anyhow::anyhow!("Configuration error: {}", e));
    }

    info!("Starting ordo-platform on {}", config.listen_addr);
    info!("Engine URL: {}", config.engine_url);
    info!("Platform dir: {:?}", config.platform_dir);

    // Init store
    let store = Arc::new(PlatformStore::new(config.platform_dir.clone()).await?);

    // HTTP client for engine proxy
    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    // Load rule templates (best-effort — missing dir just disables the feature)
    let templates = Arc::new(
        TemplateStore::load_from_dir(&config.templates_dir).unwrap_or_else(|e| {
            tracing::warn!(
                "Failed to load templates from {:?}: {:#}",
                config.templates_dir,
                e
            );
            TemplateStore::default()
        }),
    );

    let state = AppState {
        store,
        config: config.clone(),
        http_client,
        templates,
    };

    // CORS
    let cors = {
        let origins = &config.cors_allowed_origins;
        if origins.iter().any(|o| o == "*") {
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
        } else {
            let parsed: Vec<axum::http::HeaderValue> =
                origins.iter().filter_map(|o| o.parse().ok()).collect();
            CorsLayer::new()
                .allow_origin(parsed)
                .allow_methods(Any)
                .allow_headers(Any)
        }
    };

    // Routes that don't require authentication
    let public_routes = Router::new()
        .route("/api/v1/auth/register", post(auth::register))
        .route("/api/v1/auth/login", post(auth::login));

    // Routes that require authentication
    let protected_routes = Router::new()
        // Auth
        .route("/api/v1/auth/me", get(auth::me).put(auth::update_profile))
        .route("/api/v1/auth/refresh", post(auth::refresh))
        .route("/api/v1/auth/change-password", post(auth::change_password))
        // Organizations
        .route("/api/v1/orgs", post(org::create_org).get(org::list_orgs))
        .route(
            "/api/v1/orgs/:id",
            get(org::get_org).put(org::update_org).delete(org::delete_org),
        )
        // Members
        .route(
            "/api/v1/orgs/:id/members",
            get(member::list_members).post(member::invite_member),
        )
        .route(
            "/api/v1/orgs/:id/members/:uid",
            put(member::update_member_role).delete(member::remove_member),
        )
        // Projects (decision domains)
        .route(
            "/api/v1/orgs/:oid/projects",
            post(project::create_project).get(project::list_projects),
        )
        .route(
            "/api/v1/orgs/:oid/projects/:pid",
            get(project::get_project)
                .put(project::update_project)
                .delete(project::delete_project),
        )
        // Rule templates (M1.1)
        .route("/api/v1/templates", get(templates_api::list_templates))
        .route("/api/v1/templates/:id", get(templates_api::get_template))
        .route(
            "/api/v1/orgs/:oid/projects/from-template",
            post(templates_api::create_from_template),
        )
        // Fact Catalog (project-scoped)
        .route(
            "/api/v1/projects/:pid/facts",
            get(catalog::list_facts).post(catalog::upsert_fact),
        )
        .route(
            "/api/v1/projects/:pid/facts/:name",
            axum::routing::delete(catalog::delete_fact),
        )
        // Concept Registry (project-scoped)
        .route(
            "/api/v1/projects/:pid/concepts",
            get(catalog::list_concepts).post(catalog::upsert_concept),
        )
        .route(
            "/api/v1/projects/:pid/concepts/:name",
            axum::routing::delete(catalog::delete_concept),
        )
        // Decision Contracts (project-scoped)
        .route(
            "/api/v1/projects/:pid/contracts",
            get(contract::list_contracts),
        )
        .route(
            "/api/v1/projects/:pid/contracts/:name",
            put(contract::upsert_contract).delete(contract::delete_contract),
        )
        // Ruleset history (project-scoped)
        .route(
            "/api/v1/projects/:pid/rulesets/:name/history",
            get(ruleset_history::list_ruleset_history).post(ruleset_history::append_ruleset_history),
        )
        // Test cases (M1.2)
        .route(
            "/api/v1/projects/:pid/rulesets/:name/tests",
            get(testing::list_tests).post(testing::create_test),
        )
        .route(
            "/api/v1/projects/:pid/rulesets/:name/tests/run",
            post(testing::run_ruleset_tests),
        )
        .route(
            "/api/v1/projects/:pid/rulesets/:name/tests/export",
            get(testing::export_tests),
        )
        .route(
            "/api/v1/projects/:pid/rulesets/:name/tests/:tid",
            put(testing::update_test).delete(testing::delete_test),
        )
        .route(
            "/api/v1/projects/:pid/rulesets/:name/tests/:tid/run",
            post(testing::run_one_test),
        )
        .route(
            "/api/v1/projects/:pid/tests/run",
            get(testing::run_project_tests),
        )
        // Engine proxy: /api/v1/engine/:project_id/*path → ordo-server
        .route("/api/v1/engine/:project_id/*path", any(proxy::proxy_engine))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_auth,
        ));

    // Health check
    let health = Router::new().route("/health", get(|| async { "ok" }));

    let app = public_routes
        .merge(protected_routes)
        .merge(health)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .layer(CatchPanicLayer::new())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(config.listen_addr).await?;
    info!("ordo-platform listening on {}", config.listen_addr);
    axum::serve(listener, app).await?;

    Ok(())
}
