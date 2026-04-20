//! Ordo Platform Server.

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
mod i18n;
mod member;
mod middleware;
mod models;
mod notification;
mod org;
mod project;
mod proxy;
mod rbac;
mod release;
mod ruleset_draft;
mod ruleset_history;
mod server_registry;
mod store;
mod sub_org_member;
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

    let pool = sqlx::PgPool::connect(&config.database_url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    let store = Arc::new(PlatformStore::new(pool).await?);

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

        if let Err(e) = store.backfill_project_rulesets_from_history().await {
            tracing::warn!("backfill_project_rulesets_from_history failed: {}", e);
        }

        match store.fail_stuck_queued_deployments().await {
            Ok(n) if n > 0 => tracing::warn!(count = n, "Marked stuck queued deployments as failed on startup"),
            Ok(_) => {}
            Err(e) => tracing::warn!("fail_stuck_queued_deployments: {}", e),
        }

        match store.fail_stuck_active_executions().await {
            Ok(n) if n > 0 => tracing::warn!(count = n, "Marked stuck active release executions as failed on startup"),
            Ok(_) => {}
            Err(e) => tracing::warn!("fail_stuck_active_executions: {}", e),
        }
    }

    {
        let store_bg = store.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                let now = chrono::Utc::now();
                let degraded_threshold = now - chrono::Duration::seconds(90);
                let offline_threshold = now - chrono::Duration::minutes(10);
                let prune_threshold = now - chrono::Duration::minutes(30);

                let _ = store_bg
                    .mark_stale_servers_degraded(degraded_threshold)
                    .await;
                let _ = store_bg.mark_stale_servers_offline(offline_threshold).await;
                let _ = store_bg.delete_stale_offline_servers(prune_threshold).await;
            }
        });
    }

    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

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
        let registry_consumer = sync::create_control_consumer(
            &jetstream,
            &config.resolve_instance_id(),
            &config.nats_subject_prefix,
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create NATS registry consumer: {}", e))?;
        sync::start_registry_subscriber(registry_consumer, store.clone());
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

    if let Some(publisher) = &state.sync_publisher {
        if let Ok(projects) = state.store.list_all_projects().await {
            for project in projects {
                let _ = publisher
                    .publish(sync::SyncEvent::TenantUpsert {
                        tenant_id: project.id.clone(),
                        name: project.name.clone(),
                        enabled: true,
                    })
                    .await;
            }
        }
    }

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
        .route("/api/v1/system/config", get(auth::system_config))
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
        .route("/api/v1/orgs/:id/sub-orgs", get(org::list_sub_orgs))
        // Cross-org sub-org member management (auth based on parent org role)
        .route(
            "/api/v1/orgs/:parent_id/sub-orgs/:sub_id/members",
            get(sub_org_member::list_sub_org_members)
                .post(sub_org_member::add_sub_org_member),
        )
        .route(
            "/api/v1/orgs/:parent_id/sub-orgs/:sub_id/members/:uid",
            axum::routing::delete(sub_org_member::remove_sub_org_member),
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
            get(testing::run_project_tests).post(testing::run_project_tests),
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
        // Release center
        .route(
            "/api/v1/orgs/:oid/projects/:pid/release-policies",
            get(release::list_release_policies).post(release::create_release_policy),
        )
        .route(
            "/api/v1/orgs/:oid/projects/:pid/release-policies/:rid",
            put(release::update_release_policy).delete(release::delete_release_policy),
        )
        .route(
            "/api/v1/orgs/:oid/projects/:pid/releases",
            get(release::list_release_requests).post(release::create_release_request),
        )
        .route(
            "/api/v1/orgs/:oid/projects/:pid/releases/preview",
            get(release::preview_release_target),
        )
        .route(
            "/api/v1/orgs/:oid/projects/:pid/releases/:rid",
            get(release::get_release_request),
        )
        .route(
            "/api/v1/orgs/:oid/projects/:pid/releases/:rid/approve",
            post(release::approve_release_request),
        )
        .route(
            "/api/v1/orgs/:oid/projects/:pid/releases/:rid/reject",
            post(release::reject_release_request),
        )
        .route(
            "/api/v1/orgs/:oid/projects/:pid/releases/:rid/execute",
            post(release::execute_release_request),
        )
        .route(
            "/api/v1/orgs/:oid/projects/:pid/releases/:rid/pause",
            post(release::pause_release_execution),
        )
        .route(
            "/api/v1/orgs/:oid/projects/:pid/releases/:rid/resume",
            post(release::resume_release_execution),
        )
        .route(
            "/api/v1/orgs/:oid/projects/:pid/releases/:rid/rollback",
            post(release::rollback_release_execution),
        )
        .route(
            "/api/v1/orgs/:oid/projects/:pid/releases/:rid/execution",
            get(release::get_release_execution_for_request),
        )
        .route(
            "/api/v1/orgs/:oid/projects/:pid/releases/:rid/executions/:eid/events",
            get(release::list_release_execution_events),
        )
        .route(
            "/api/v1/orgs/:oid/projects/:pid/release-executions/current",
            get(release::get_current_release_execution),
        )
        // Notifications
        .route(
            "/api/v1/orgs/:oid/notifications",
            get(notification::list_notifications),
        )
        .route(
            "/api/v1/orgs/:oid/notifications/count",
            get(notification::get_notification_count),
        )
        .route(
            "/api/v1/orgs/:oid/notifications/read-all",
            post(notification::mark_all_notifications_read),
        )
        .route(
            "/api/v1/orgs/:oid/notifications/:nid/read",
            post(notification::mark_notification_read),
        )
        .route(
            "/api/v1/orgs/:oid/releases/pending-for-me",
            get(notification::list_pending_approvals_for_me),
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
        .route(
            "/api/v1/orgs/:oid/projects/:pid/rulesets/:name/trace",
            post(ruleset_draft::trace_draft),
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
        .layer(axum::middleware::from_fn(i18n::with_request_locale))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .layer(CatchPanicLayer::new())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(config.listen_addr).await?;
    info!("ordo-platform listening on {}", config.listen_addr);
    axum::serve(listener, app).await?;

    Ok(())
}
