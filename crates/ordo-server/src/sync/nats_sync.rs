//! NATS JetStream-based sync for distributed Ordo deployments.
//!
//! **Writer** instances publish [`SyncEvent`]s to a JetStream stream after
//! successful mutations.  **Reader** instances create a durable pull consumer
//! and apply events to their local [`RuleStore`] / [`TenantManager`].
//!
//! Key design points:
//! - Publish failures are logged but never block the write path (graceful degradation).
//! - Readers use a durable consumer so they can resume from the last acked sequence
//!   after a restart or network blip.
//! - Echo suppression: each message carries an `instance_id`; the subscriber skips
//!   messages from its own instance.

use crate::metrics;
use crate::store::RuleStore;
use crate::sync::event::{SyncEvent, SyncMessage};
use crate::tenant::TenantManager;
use async_nats::jetstream::{self, consumer::PullConsumer, stream::RetentionPolicy};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch, RwLock};
use tracing::{debug, error, info, warn};

/// Default NATS subject prefix for rule sync events.
#[cfg(test)]
pub const DEFAULT_SUBJECT_PREFIX: &str = "ordo.rules";

/// Default JetStream stream name.
const STREAM_NAME: &str = "ordo-rules";

/// How many messages to fetch in a single pull batch.
const PULL_BATCH_SIZE: usize = 100;

/// Max wait time for a pull batch before returning what's available.
const PULL_BATCH_TIMEOUT: Duration = Duration::from_secs(5);

// ─── Publisher (Writer side) ──────────────────────────────────────────────────

/// Publisher that sends sync events to NATS JetStream.
///
/// Runs as a background task, reading from an internal mpsc channel.
/// The channel sender is handed to `RuleStore` so it can enqueue events
/// without doing async I/O in the sync `put_for_tenant` / `delete_for_tenant` paths.
pub struct NatsPublisher {
    jetstream: jetstream::Context,
    subject_prefix: String,
    instance_id: String,
}

impl NatsPublisher {
    pub fn new(jetstream: jetstream::Context, subject_prefix: String, instance_id: String) -> Self {
        Self {
            jetstream,
            subject_prefix,
            instance_id,
        }
    }

    /// Start the publisher loop.  Returns the sender for enqueuing events.
    ///
    /// The loop runs until the channel is closed (all senders dropped) or
    /// `shutdown_rx` fires.
    pub fn start(self, mut shutdown_rx: watch::Receiver<bool>) -> mpsc::UnboundedSender<SyncEvent> {
        let (tx, mut rx) = mpsc::unbounded_channel::<SyncEvent>();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => {
                        info!("NATS publisher: shutdown signal received");
                        break;
                    }
                    event = rx.recv() => {
                        match event {
                            Some(evt) => self.publish(evt).await,
                            None => {
                                info!("NATS publisher: channel closed");
                                break;
                            }
                        }
                    }
                }
            }
            // Drain remaining events
            while let Ok(evt) = rx.try_recv() {
                self.publish(evt).await;
            }
            info!("NATS publisher stopped");
        });

        tx
    }

    async fn publish(&self, event: SyncEvent) {
        let event_type = event.event_type();
        let msg = SyncMessage::new(self.instance_id.clone(), event);
        let subject = msg.subject(&self.subject_prefix);

        let payload = match serde_json::to_vec(&msg) {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to serialize sync event: {}", e);
                metrics::record_sync_failed(event_type, "publish");
                return;
            }
        };

        match self
            .jetstream
            .publish(subject.clone(), payload.into())
            .await
        {
            Ok(ack_future) => {
                // Wait for JetStream ack (ensures durable storage).
                match ack_future.await {
                    Ok(_ack) => {
                        debug!("Published sync event to {}", subject);
                        metrics::record_sync_published(event_type);
                    }
                    Err(e) => {
                        warn!("NATS JetStream ack failed for {}: {}", subject, e);
                        metrics::record_sync_failed(event_type, "publish");
                    }
                }
            }
            Err(e) => {
                warn!("Failed to publish sync event to {}: {}", subject, e);
                metrics::record_sync_failed(event_type, "publish");
            }
        }
    }
}

// ─── Subscriber (Reader side) ─────────────────────────────────────────────────

/// Subscriber that applies sync events from NATS JetStream to the local store.
pub struct NatsSubscriber {
    consumer: PullConsumer,
    instance_id: String,
    store: Arc<RwLock<RuleStore>>,
    tenant_manager: Arc<TenantManager>,
    runtime_config: crate::runtime_config::SharedRuntimeConfig,
}

impl NatsSubscriber {
    pub fn new(
        consumer: PullConsumer,
        instance_id: String,
        store: Arc<RwLock<RuleStore>>,
        tenant_manager: Arc<TenantManager>,
        runtime_config: crate::runtime_config::SharedRuntimeConfig,
    ) -> Self {
        Self {
            consumer,
            instance_id,
            store,
            tenant_manager,
            runtime_config,
        }
    }

    /// Start the subscriber loop as a background task.
    ///
    /// Returns a `JoinHandle` that runs until shutdown.
    pub fn start(self, mut shutdown_rx: watch::Receiver<bool>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            info!(
                "NATS subscriber started (consumer for instance '{}')",
                self.instance_id
            );

            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => {
                        info!("NATS subscriber: shutdown signal received");
                        break;
                    }
                    result = self.pull_batch() => {
                        if let Err(e) = result {
                            warn!("NATS subscriber pull error: {} — retrying in 1s", e);
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                    }
                }
            }

            info!("NATS subscriber stopped");
        })
    }

    async fn pull_batch(&self) -> Result<(), async_nats::Error> {
        use futures::StreamExt;

        let mut messages = self
            .consumer
            .fetch()
            .max_messages(PULL_BATCH_SIZE)
            .expires(PULL_BATCH_TIMEOUT)
            .messages()
            .await?;

        while let Some(msg_result) = messages.next().await {
            let msg = match msg_result {
                Ok(m) => m,
                Err(e) => {
                    warn!("NATS message receive error: {}", e);
                    continue;
                }
            };

            let sync_msg: SyncMessage = match serde_json::from_slice(&msg.payload) {
                Ok(m) => m,
                Err(e) => {
                    error!("Failed to deserialize sync message: {}", e);
                    // Ack to avoid redelivery of permanently bad messages.
                    if let Err(e) = msg.ack().await {
                        warn!("Failed to ack bad message: {}", e);
                    }
                    continue;
                }
            };

            // Echo suppression — skip our own events.
            if sync_msg.instance_id == self.instance_id {
                debug!("Skipping own sync event");
                if let Err(e) = msg.ack().await {
                    warn!("Failed to ack own message: {}", e);
                }
                continue;
            }

            self.apply_event(&sync_msg.event).await;

            if let Err(e) = msg.ack().await {
                warn!("Failed to ack sync message: {}", e);
            }
        }

        Ok(())
    }

    async fn apply_event(&self, event: &SyncEvent) {
        match event {
            SyncEvent::RulePut {
                tenant_id,
                name,
                ruleset_json,
                version,
            } => {
                self.apply_rule_put(tenant_id, name, ruleset_json, version)
                    .await;
            }
            SyncEvent::RuleDeleted { tenant_id, name } => {
                self.apply_rule_deleted(tenant_id, name).await;
            }
            SyncEvent::TenantConfigChanged { config_json } => {
                self.apply_tenant_config_changed(config_json).await;
            }
            SyncEvent::RuntimeConfigChanged { config_json } => {
                self.apply_runtime_config_changed(config_json).await;
            }
        }
    }

    async fn apply_rule_put(&self, tenant_id: &str, name: &str, ruleset_json: &str, version: &str) {
        // Idempotency: check if we already have this version.
        {
            let store = self.store.read().await;
            if let Some(existing) = store.get_for_tenant(tenant_id, name) {
                if existing.config.version == *version {
                    debug!(
                        "Skipping duplicate RulePut for '{}' v{} (already present)",
                        name, version
                    );
                    return;
                }
            }
        }

        let mut store = self.store.write().await;
        match store.apply_sync_put(tenant_id, ruleset_json) {
            Ok(()) => {
                info!(
                    "Applied sync RulePut: '{}' (tenant '{}') v{}",
                    name, tenant_id, version
                );
                metrics::record_sync_applied("RulePut");
            }
            Err(e) => {
                error!(
                    "Failed to apply sync RulePut for '{}' (tenant '{}'): {}",
                    name, tenant_id, e
                );
                metrics::record_sync_failed("RulePut", "apply");
            }
        }
    }

    async fn apply_rule_deleted(&self, tenant_id: &str, name: &str) {
        let mut store = self.store.write().await;
        if store.delete_for_tenant(tenant_id, name) {
            info!(
                "Applied sync RuleDeleted: '{}' (tenant '{}')",
                name, tenant_id
            );
            metrics::record_sync_applied("RuleDeleted");
        } else {
            debug!(
                "Sync RuleDeleted for '{}' (tenant '{}') — already absent",
                name, tenant_id
            );
        }
    }

    async fn apply_tenant_config_changed(&self, config_json: &str) {
        match self.tenant_manager.apply_sync_config(config_json).await {
            Ok(()) => {
                info!("Applied sync TenantConfigChanged");
                metrics::record_sync_applied("TenantConfigChanged");
            }
            Err(e) => {
                error!("Failed to apply sync TenantConfigChanged: {}", e);
                metrics::record_sync_failed("TenantConfigChanged", "apply");
            }
        }
    }

    async fn apply_runtime_config_changed(&self, config_json: &str) {
        let new_cfg: crate::runtime_config::RuntimeConfig = match serde_json::from_str(config_json)
        {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to deserialize RuntimeConfigChanged: {}", e);
                metrics::record_sync_failed("RuntimeConfigChanged", "deserialize");
                return;
            }
        };

        // Apply to all mutable components on this node.
        {
            let mut guard = self.runtime_config.write().await;
            *guard = new_cfg.clone();
        }
        self.tenant_manager
            .update_defaults(crate::tenant::TenantDefaults {
                default_qps_limit: new_cfg.default_tenant_qps,
                default_burst_limit: new_cfg.default_tenant_burst,
                default_timeout_ms: new_cfg.default_tenant_timeout_ms,
            });
        {
            let mut store = self.store.write().await;
            store.set_resource_limits(new_cfg.max_rules_per_tenant, new_cfg.max_total_rules);
        }

        info!("Applied sync RuntimeConfigChanged");
        metrics::record_sync_applied("RuntimeConfigChanged");
    }
}

// ─── Setup helpers ────────────────────────────────────────────────────────────

/// Ensure the JetStream stream exists with the right subjects.
pub async fn ensure_stream(
    jetstream: &jetstream::Context,
    subject_prefix: &str,
) -> Result<(), async_nats::Error> {
    let subjects = vec![format!("{}.>", subject_prefix)];

    jetstream
        .get_or_create_stream(jetstream::stream::Config {
            name: STREAM_NAME.to_string(),
            subjects,
            retention: RetentionPolicy::Limits,
            max_age: Duration::from_secs(7 * 24 * 3600), // 7 days
            ..Default::default()
        })
        .await?;

    info!(
        "JetStream stream '{}' ready (subjects: {}.>)",
        STREAM_NAME, subject_prefix
    );
    Ok(())
}

/// Create a durable pull consumer for a reader instance.
pub async fn create_consumer(
    jetstream: &jetstream::Context,
    instance_id: &str,
) -> Result<PullConsumer, async_nats::Error> {
    let stream = jetstream.get_stream(STREAM_NAME).await?;

    let consumer_name = format!("ordo-{}", instance_id);

    let consumer = stream
        .get_or_create_consumer(
            &consumer_name,
            jetstream::consumer::pull::Config {
                durable_name: Some(consumer_name.clone()),
                ack_policy: jetstream::consumer::AckPolicy::Explicit,
                deliver_policy: jetstream::consumer::DeliverPolicy::All,
                ..Default::default()
            },
        )
        .await?;

    info!(
        "JetStream consumer '{}' ready (replay from last acked)",
        consumer_name
    );
    Ok(consumer)
}

/// Connect to NATS and set up JetStream.
///
/// Returns the JetStream context for creating publishers/subscribers.
pub async fn connect(nats_url: &str) -> Result<jetstream::Context, async_nats::Error> {
    let client = async_nats::connect(nats_url).await?;
    info!("Connected to NATS at {}", nats_url);
    Ok(jetstream::new(client))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::event::SyncEvent;

    #[test]
    fn test_default_subject_prefix() {
        assert_eq!(DEFAULT_SUBJECT_PREFIX, "ordo.rules");
    }

    #[test]
    fn test_sync_message_subject_routing() {
        let msg = SyncMessage::new(
            "inst-1".into(),
            SyncEvent::RulePut {
                tenant_id: "acme".into(),
                name: "fraud".into(),
                ruleset_json: "{}".into(),
                version: "1".into(),
            },
        );
        assert_eq!(msg.subject(DEFAULT_SUBJECT_PREFIX), "ordo.rules.acme.fraud");
    }

    #[test]
    fn test_stream_name_constant() {
        assert_eq!(STREAM_NAME, "ordo-rules");
    }
}
