//! Shared Ordo Platform library used by the HTTP API and background workers.

use std::sync::Arc;
use std::time::Duration;

use config::PlatformConfig;
use store::PlatformStore;
use template::TemplateStore;

pub mod auth;
pub mod catalog;
pub mod config;
pub mod contract;
pub mod environment;
pub mod error;
pub mod github;
pub mod i18n;
pub mod member;
pub mod middleware;
pub mod models;
pub mod notification;
pub mod org;
pub mod project;
pub mod proxy;
pub mod rbac;
pub mod release;
pub mod ruleset_draft;
pub mod ruleset_history;
pub mod server_registry;
pub mod store;
pub mod sub_org_member;
pub mod sub_rules;
pub mod sync;
pub mod template;
pub mod templates_api;
pub mod testing;

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    pub store: Arc<PlatformStore>,
    pub config: Arc<PlatformConfig>,
    pub http_client: reqwest::Client,
    pub templates: Arc<TemplateStore>,
    pub nats_client: Option<async_nats::Client>,
    pub sync_publisher: Option<Arc<sync::NatsPublisher>>,
    pub marketplace_cache: Arc<github::MarketplaceCache>,
}

pub fn init_tracing(config: &PlatformConfig) -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                config
                    .log_level
                    .parse()
                    .unwrap_or_else(|_| "info".parse().unwrap())
            }),
        )
        .try_init()
        .map_err(|err| anyhow::anyhow!("failed to initialize tracing: {}", err))
}

pub fn resolve_templates_dir(configured: &std::path::Path) -> std::path::PathBuf {
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

pub async fn connect_platform_store(config: &PlatformConfig) -> anyhow::Result<Arc<PlatformStore>> {
    let pool = sqlx::PgPool::connect(&config.database_url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(Arc::new(PlatformStore::new(pool).await?))
}

pub async fn bootstrap_platform_store(
    store: &Arc<PlatformStore>,
    fail_active_executions: bool,
) -> anyhow::Result<()> {
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
        Ok(n) if n > 0 => tracing::warn!(
            count = n,
            "Marked stuck queued deployments as failed on startup"
        ),
        Ok(_) => {}
        Err(e) => tracing::warn!("fail_stuck_queued_deployments: {}", e),
    }

    if fail_active_executions {
        match store.fail_stuck_active_executions().await {
            Ok(n) if n > 0 => tracing::warn!(
                count = n,
                "Marked stuck active release executions as failed on startup"
            ),
            Ok(_) => {}
            Err(e) => tracing::warn!("fail_stuck_active_executions: {}", e),
        }
    }

    Ok(())
}

pub fn start_server_registry_maintenance(store: Arc<PlatformStore>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            let now = chrono::Utc::now();
            let degraded_threshold = now - chrono::Duration::seconds(90);
            let offline_threshold = now - chrono::Duration::minutes(10);
            let prune_threshold = now - chrono::Duration::minutes(30);

            let _ = store.mark_stale_servers_degraded(degraded_threshold).await;
            let _ = store.mark_stale_servers_offline(offline_threshold).await;
            let _ = store.delete_stale_offline_servers(prune_threshold).await;
        }
    })
}

pub async fn build_app_state(
    config: Arc<PlatformConfig>,
    store: Arc<PlatformStore>,
    start_control_subscriber: bool,
) -> anyhow::Result<AppState> {
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

    let (nats_client, sync_publisher) = if let Some(nats_url) = config.nats_url.as_deref() {
        let nats_client = sync::connect_client(nats_url)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to connect to NATS at {}: {}", nats_url, e))?;
        let jetstream = async_nats::jetstream::new(nats_client.clone());
        sync::ensure_stream(&jetstream, &config.nats_subject_prefix)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to ensure NATS stream: {}", e))?;

        if start_control_subscriber {
            let consumer_id = format!("{}-worker", config.resolve_instance_id());
            let registry_consumer = sync::create_control_consumer(
                &jetstream,
                &consumer_id,
                &config.nats_subject_prefix,
            )
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create NATS registry consumer: {}", e))?;
            sync::start_registry_subscriber(registry_consumer, store.clone());
        }

        (
            Some(nats_client),
            Some(Arc::new(sync::NatsPublisher::new(
                jetstream,
                config.nats_subject_prefix.clone(),
                config.resolve_instance_id(),
            ))),
        )
    } else {
        (None, None)
    };

    Ok(AppState {
        store,
        config,
        http_client,
        templates,
        nats_client,
        sync_publisher,
        marketplace_cache: github::MarketplaceCache::new(),
    })
}

pub async fn publish_existing_tenants(state: &AppState) {
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
}
