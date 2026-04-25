//! Background worker for release rollout and rollback execution.

use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use ordo_platform::{
    bootstrap_platform_store, build_app_state, config::PlatformConfig, connect_platform_store,
    init_tracing, publish_existing_tenants, release, start_server_registry_maintenance,
};
use tracing::info;

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

    let store = connect_platform_store(&config).await?;
    bootstrap_platform_store(&store, false).await?;
    start_server_registry_maintenance(store.clone());

    let state = build_app_state(config, store, true).await?;
    publish_existing_tenants(&state).await;

    release::run_release_worker_loop(state, Duration::from_secs(2)).await
}
