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
mod environment;
mod error;
mod github;
mod member;
mod middleware;
mod models;
mod org;
mod project;
mod proxy;
mod rbac;
mod ruleset_draft;
mod ruleset_history;
mod server_registry;
mod store;
mod sync;
mod template;
mod templates_api;
mod testing;

use config::PlatformConfig;
use middleware::require_auth;
use store::PlatformStore;
use template::TemplateStore;

fn resolve_templates_dir(configured: &std::path::Path) -> std::path::PathBuf {
    if configured.exists() {
        return configured.to_path_buf();
    }

    let fallbacks = [
        std::path::PathBuf::from("./crates/ordo-platform/templates"),
        std::path::PathBuf::from("./templates"),
        std::path::PathBuf::from("/app/templates"),
    ];

    if let Some(path) = fallbacks.into_iter().find(|path| path.exists()) {
        tracing::info!(
            configured = %configured.display(),
            resolved = %path.display(),
            "Using fallback templates directory",
        );
        return path;
    }

    configured.to_path_buf()
}

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub store: Arc<PlatformStore>,
    pub config: Arc<PlatformConfig>,
    pub http_client: reqwest::Client,
    pub templates: Arc<TemplateStore>,
    pub sync_publisher: Option<Arc<sync::NatsPublisher>>,
    pub marketplace_cache: Arc<github::MarketplaceCache>,
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
    if config.nats_enabled() {
        info!(
            "NATS sync enabled (url={}, prefix={})",
            config.nats_url.as_deref().unwrap_or(""),
            config.nats_subject_prefix
        );
    }

    // Init database pool and run migrations
    let pool = sqlx::PgPool::connect(&config.database_url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    let store = Arc::new(PlatformStore::new(pool).await?);

    // Startup migrations: seed RBAC system roles for newly created orgs
    {
        let orgs = store.list_all_orgs().await.unwrap_or_default();
        for org in &orgs {
            if let Err(e) = store.seed_system_roles(&org.id).await {
                tracing::warn!("seed_system_roles failed for org {}: {}", org.id, e);
            }
        }

        let all_projects = store.list_all_projects().await.unwrap_or_default();
        for project in all_projects {
            if let Err(e) = store
                .migrate_project_server_to_environment(&project.id, project.server_id.as_deref())
                .await
            {
                tracing::warn!("migrate env failed for project {}: {}", project.id, e);
            }
        }
    }

    // Background task: mark servers offline when they stop heartbeating
    {
        let store_bg = store.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                let threshold = chrono::Utc::now() - chrono::Duration::seconds(90);
                let _ = store_bg.mark_stale_servers_offline(threshold).await;
            }
        });
    }

    // HTTP client for engine proxy
    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    // Load rule templates (best-effort — missing dir just disables the feature)
    let templates_dir = resolve_templates_dir(&config.templates_dir);
    let templates = Arc::new(
        TemplateStore::load_from_dir(&templates_dir).unwrap_or_else(|e| {
            tracing::warn!("Failed to load templates from {:?}: {:#}", templates_dir, e);
            TemplateStore::default()
        }),
    );

    let sync_publisher = if let Some(nats_url) = config.nats_url.as_deref() {
        let jetstream = sync::connect(nats_url)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to connect to NATS at {}: {}", nats_url, e))?;
        sync::ensure_stream(&jetstream, &config.nats_subject_prefix)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to ensure NATS stream: {}", e))?;
        Some(Arc::new(sync::NatsPublisher::new(
            jetstream,
            config.nats_subject_prefix.clone(),
            config.resolve_instance_id(),
        )))
    } else {
        None
    };

    let marketplace_cache = github::MarketplaceCache::new();

    let state = AppState {
        store,
        config: config.clone(),
        http_client,
        templates,
        sync_publisher,
        marketplace_cache,
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
        .route("/api/v1/auth/login", post(auth::login))
        // GitHub OAuth callback (public — GitHub redirects here)
        .route("/api/v1/github/callback", get(github::github_callback))
        // Internal: called by ordo-server (token auth inside handler)
        .route("/api/v1/internal/register", post(server_registry::register_server))
        .route("/api/v1/internal/heartbeat", post(server_registry::server_heartbeat));

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
        // Server registry
        .route("/api/v1/servers", get(server_registry::list_servers))
        .route("/api/v1/servers/:id", get(server_registry::get_server).delete(server_registry::delete_server))
        .route("/api/v1/servers/:id/metrics", get(server_registry::get_server_metrics))
        .route("/api/v1/servers/:id/health", get(server_registry::get_server_health))
        .route(
            "/api/v1/orgs/:oid/projects/:pid/server",
            axum::routing::put(server_registry::bind_project_server),
        )
        // GitHub OAuth (protected)
        .route("/api/v1/github/connect", get(github::get_connect_url))
        .route("/api/v1/github/status", get(github::get_status))
        .route("/api/v1/github/disconnect", axum::routing::delete(github::disconnect))
        // GitHub Marketplace
        .route("/api/v1/marketplace/search", get(github::search_marketplace))
        .route("/api/v1/marketplace/repos/:owner/:repo", get(github::get_marketplace_item))
        .route(
            "/api/v1/marketplace/install/:owner/:repo",
            post(github::install_marketplace_item),
        )
        // RBAC: org roles
        .route(
            "/api/v1/orgs/:oid/roles",
            get(org::list_roles).post(org::create_role),
        )
        .route(
            "/api/v1/orgs/:oid/roles/:rid",
            put(org::update_role).delete(org::delete_role),
        )
        // RBAC: member role assignments
        .route(
            "/api/v1/orgs/:oid/members/:uid/roles",
            get(org::list_member_roles).post(org::assign_member_role),
        )
        .route(
            "/api/v1/orgs/:oid/members/:uid/roles/:rid",
            axum::routing::delete(org::revoke_member_role),
        )
        // Environments
        .route(
            "/api/v1/orgs/:oid/projects/:pid/environments",
            get(environment::list_environments).post(environment::create_environment),
        )
        .route(
            "/api/v1/orgs/:oid/projects/:pid/environments/:eid",
            put(environment::update_environment).delete(environment::delete_environment),
        )
        .route(
            "/api/v1/orgs/:oid/projects/:pid/environments/:eid/canary",
            put(environment::set_canary),
        )
        // Draft rulesets
        .route(
            "/api/v1/orgs/:oid/projects/:pid/rulesets",
            get(ruleset_draft::list_drafts),
        )
        .route(
            "/api/v1/orgs/:oid/projects/:pid/rulesets/:name",
            get(ruleset_draft::get_draft)
                .put(ruleset_draft::save_draft)
                .delete(ruleset_draft::delete_draft),
        )
        .route(
            "/api/v1/orgs/:oid/projects/:pid/rulesets/:name/publish",
            post(ruleset_draft::publish_draft),
        )
        // Deployment history
        .route(
            "/api/v1/orgs/:oid/projects/:pid/deployments",
            get(ruleset_draft::list_project_deployments),
        )
        .route(
            "/api/v1/orgs/:oid/projects/:pid/rulesets/:name/deployments",
            get(ruleset_draft::list_ruleset_deployments),
        )
        .route(
            "/api/v1/orgs/:oid/projects/:pid/rulesets/:name/deployments/:did/redeploy",
            post(ruleset_draft::redeploy),
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
