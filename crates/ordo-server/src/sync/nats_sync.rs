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
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch, RwLock};
use tracing::{debug, error, info, warn};
use url::Url;

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
    jetstream: jetstream::Context,
    subject_prefix: String,
    instance_id: String,
    server_id: String,
    store: Arc<RwLock<RuleStore>>,
    tenant_manager: Arc<TenantManager>,
}

impl NatsSubscriber {
    pub fn new(
        consumer: PullConsumer,
        jetstream: jetstream::Context,
        subject_prefix: String,
        instance_id: String,
        server_id: String,
        store: Arc<RwLock<RuleStore>>,
        tenant_manager: Arc<TenantManager>,
    ) -> Self {
        Self {
            consumer,
            jetstream,
            subject_prefix,
            instance_id,
            server_id,
            store,
            tenant_manager,
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

            if self.apply_event(&sync_msg.event).await {
                if let Err(e) = msg.ack().await {
                    warn!("Failed to ack sync message: {}", e);
                }
            } else if let Err(e) = msg
                .ack_with(async_nats::jetstream::AckKind::Nak(None))
                .await
            {
                warn!("Failed to nack sync message: {}", e);
            }
        }

        Ok(())
    }

    async fn apply_event(&self, event: &SyncEvent) -> bool {
        match event {
            SyncEvent::RulePut {
                tenant_id,
                name,
                ruleset_json,
                version,
                release_execution_id,
                target_server_ids,
            } => {
                self.apply_rule_put(
                    tenant_id,
                    name,
                    ruleset_json,
                    version,
                    release_execution_id.as_deref(),
                    target_server_ids.as_deref(),
                )
                .await
            }
            SyncEvent::RuleDeleted { tenant_id, name } => {
                self.apply_rule_deleted(tenant_id, name).await
            }
            SyncEvent::TenantUpsert {
                tenant_id,
                name,
                enabled,
            } => self.apply_tenant_upsert(tenant_id, name, *enabled).await,
            SyncEvent::TenantDeleted { tenant_id } => self.apply_tenant_deleted(tenant_id).await,
            SyncEvent::TenantConfigChanged { config_json } => {
                self.apply_tenant_config_changed(config_json).await
            }
            SyncEvent::ServerRegistered { .. }
            | SyncEvent::ServerHeartbeat { .. }
            | SyncEvent::ReleaseExecutionAck { .. }
            | SyncEvent::ReleaseExecutionFailed { .. } => true,
        }
    }

    async fn apply_rule_put(
        &self,
        tenant_id: &str,
        name: &str,
        ruleset_json: &str,
        version: &str,
        release_execution_id: Option<&str>,
        target_server_ids: Option<&[String]>,
    ) -> bool {
        if let Some(targets) = target_server_ids {
            if !targets.iter().any(|server_id| server_id == &self.server_id) {
                debug!(
                    "Skipping targeted RulePut for '{}' because this server is not in the rollout batch",
                    name
                );
                return true;
            }
        }

        // Idempotency: check if we already have this version.
        {
            let store = self.store.read().await;
            if let Some(existing) = store.get_for_tenant(tenant_id, name) {
                if existing.config.version == *version {
                    if let Some(execution_id) = release_execution_id {
                        if let Err(e) = self
                            .publish_release_feedback(SyncEvent::ReleaseExecutionAck {
                                execution_id: execution_id.to_string(),
                                server_id: self.server_id.clone(),
                                message: Some(
                                    "Ruleset version already present on target server".to_string(),
                                ),
                            })
                            .await
                        {
                            warn!(
                                "Failed to publish release ack for duplicate RulePut '{}' v{}: {}",
                                name, version, e
                            );
                            return false;
                        }
                    }
                    debug!(
                        "Skipping duplicate RulePut for '{}' v{} (already present)",
                        name, version
                    );
                    return true;
                }
            }
        }

        let mut store = self.store.write().await;
        match store.apply_sync_put(tenant_id, ruleset_json) {
            Ok(()) => {
                if let Some(execution_id) = release_execution_id {
                    if let Err(e) = self
                        .publish_release_feedback(SyncEvent::ReleaseExecutionAck {
                            execution_id: execution_id.to_string(),
                            server_id: self.server_id.clone(),
                            message: Some("Ruleset applied successfully".to_string()),
                        })
                        .await
                    {
                        warn!(
                            "Failed to publish release ack for '{}' (tenant '{}'): {}",
                            name, tenant_id, e
                        );
                        return false;
                    }
                }
                info!(
                    "Applied sync RulePut: '{}' (tenant '{}') v{}",
                    name, tenant_id, version
                );
                metrics::record_sync_applied("RulePut");
                true
            }
            Err(e) => {
                if let Some(execution_id) = release_execution_id {
                    if let Err(pub_err) = self
                        .publish_release_feedback(SyncEvent::ReleaseExecutionFailed {
                            execution_id: execution_id.to_string(),
                            server_id: self.server_id.clone(),
                            error: e.to_string(),
                        })
                        .await
                    {
                        warn!(
                            "Failed to publish release failure feedback for '{}' (tenant '{}'): {}",
                            name, tenant_id, pub_err
                        );
                        return false;
                    }
                    error!(
                        "Failed to apply release RulePut for '{}' (tenant '{}'): {}",
                        name, tenant_id, e
                    );
                    metrics::record_sync_failed("RulePut", "apply");
                    return true;
                }
                error!(
                    "Failed to apply sync RulePut for '{}' (tenant '{}'): {}",
                    name, tenant_id, e
                );
                metrics::record_sync_failed("RulePut", "apply");
                false
            }
        }
    }

    async fn publish_release_feedback(&self, event: SyncEvent) -> anyhow::Result<()> {
        publish_immediate(
            &self.jetstream,
            &self.subject_prefix,
            &self.instance_id,
            event,
        )
        .await
    }

    async fn apply_rule_deleted(&self, tenant_id: &str, name: &str) -> bool {
        let mut store = self.store.write().await;
        match store.apply_sync_delete(tenant_id, name) {
            Ok(true) => {
                info!(
                    "Applied sync RuleDeleted: '{}' (tenant '{}')",
                    name, tenant_id
                );
                metrics::record_sync_applied("RuleDeleted");
                true
            }
            Ok(false) => {
                debug!(
                    "Sync RuleDeleted for '{}' (tenant '{}') — already absent",
                    name, tenant_id
                );
                true
            }
            Err(e) => {
                error!(
                    "Failed to apply sync RuleDeleted for '{}' (tenant '{}'): {}",
                    name, tenant_id, e
                );
                metrics::record_sync_failed("RuleDeleted", "apply");
                false
            }
        }
    }

    async fn apply_tenant_upsert(&self, tenant_id: &str, name: &str, enabled: bool) -> bool {
        match self
            .tenant_manager
            .apply_sync_upsert(tenant_id, name, enabled)
            .await
        {
            Ok(()) => {
                info!("Applied sync TenantUpsert: '{}' ({})", tenant_id, name);
                metrics::record_sync_applied("TenantUpsert");
                true
            }
            Err(e) => {
                error!(
                    "Failed to apply sync TenantUpsert for '{}': {}",
                    tenant_id, e
                );
                metrics::record_sync_failed("TenantUpsert", "apply");
                false
            }
        }
    }

    async fn apply_tenant_deleted(&self, tenant_id: &str) -> bool {
        let removed_rules = {
            let mut store = self.store.write().await;
            match store.apply_sync_delete_tenant(tenant_id) {
                Ok(count) => count,
                Err(e) => {
                    error!(
                        "Failed to purge rules for deleted tenant '{}': {}",
                        tenant_id, e
                    );
                    metrics::record_sync_failed("TenantDeleted", "apply");
                    return false;
                }
            }
        };

        match self.tenant_manager.apply_sync_delete(tenant_id).await {
            Ok(true) => {
                info!(
                    "Applied sync TenantDeleted: '{}' (removed {} rulesets)",
                    tenant_id, removed_rules
                );
                metrics::record_sync_applied("TenantDeleted");
                true
            }
            Ok(false) => {
                debug!(
                    "Sync TenantDeleted for '{}' — tenant already absent (removed {} rulesets)",
                    tenant_id, removed_rules
                );
                true
            }
            Err(e) => {
                error!(
                    "Failed to apply sync TenantDeleted for '{}': {}",
                    tenant_id, e
                );
                metrics::record_sync_failed("TenantDeleted", "apply");
                false
            }
        }
    }

    async fn apply_tenant_config_changed(&self, config_json: &str) -> bool {
        match self.tenant_manager.apply_sync_config(config_json).await {
            Ok(()) => {
                info!("Applied sync TenantConfigChanged");
                metrics::record_sync_applied("TenantConfigChanged");
                true
            }
            Err(e) => {
                error!("Failed to apply sync TenantConfigChanged: {}", e);
                metrics::record_sync_failed("TenantConfigChanged", "apply");
                false
            }
        }
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
            storage: jetstream::stream::StorageType::File,
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
///
/// `subject_prefix` is used to create a `filter_subjects` filter so this
/// instance only receives messages published to its own environment prefix.
/// This enables multi-environment isolation: each ordo-server consumes only
/// the events targeted at its NATS prefix.
pub async fn create_consumer(
    jetstream: &jetstream::Context,
    instance_id: &str,
    subject_prefix: &str,
) -> Result<PullConsumer, async_nats::Error> {
    let stream = jetstream.get_stream(STREAM_NAME).await?;

    let consumer_name = format!("ordo-{}", instance_id);
    let filter = format!("{}.>", subject_prefix);

    let consumer = stream
        .get_or_create_consumer(
            &consumer_name,
            jetstream::consumer::pull::Config {
                durable_name: Some(consumer_name.clone()),
                ack_policy: jetstream::consumer::AckPolicy::Explicit,
                deliver_policy: jetstream::consumer::DeliverPolicy::All,
                ack_wait: Duration::from_secs(30),
                max_deliver: 20,
                max_ack_pending: 1_000,
                filter_subject: filter.clone(),
                ..Default::default()
            },
        )
        .await?;

    info!(
        "JetStream consumer '{}' ready (filter: {}, replay from last acked)",
        consumer_name, filter
    );
    Ok(consumer)
}

/// Connect to NATS and set up JetStream.
///
/// Returns the JetStream context for creating publishers/subscribers.
fn connect_options_and_addr(
    nats_url: &str,
) -> Result<(async_nats::ConnectOptions, String), async_nats::Error> {
    let mut url = Url::parse(nats_url).map_err(async_nats::Error::from)?;
    let username = if url.username().is_empty() {
        None
    } else {
        Some(url.username().to_string())
    };
    let password = url.password().map(str::to_string);

    url.set_username("")
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid username in NATS url"))?;
    url.set_password(None)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid password in NATS url"))?;

    let options = match (username, password) {
        (Some(user), Some(pass)) => async_nats::ConnectOptions::with_user_and_password(user, pass),
        (Some(token), None) => async_nats::ConnectOptions::with_token(token),
        (None, None) => async_nats::ConnectOptions::new(),
        (None, Some(_)) => async_nats::ConnectOptions::new(),
    };

    Ok((options, url.to_string()))
}

pub async fn connect_client(nats_url: &str) -> Result<async_nats::Client, async_nats::Error> {
    let (options, server_addr) = connect_options_and_addr(nats_url)?;
    let client = options.connect(server_addr.as_str()).await?;
    info!("Connected to NATS at {}", nats_url);
    Ok(client)
}

fn rpc_prefix(subject_prefix: &str) -> String {
    let safe_prefix = subject_prefix
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if safe_prefix.is_empty() {
        "_ORDO_RPC.default".to_string()
    } else {
        format!("_ORDO_RPC.{}", safe_prefix)
    }
}

pub fn server_rpc_wildcard_subject(subject_prefix: &str, server_id: &str) -> String {
    format!("{}.servers.{}.*", rpc_prefix(subject_prefix), server_id)
}

pub async fn publish_immediate(
    jetstream: &jetstream::Context,
    subject_prefix: &str,
    instance_id: &str,
    event: SyncEvent,
) -> anyhow::Result<()> {
    let msg = SyncMessage::new(instance_id.to_string(), event);
    let subject = msg.subject(subject_prefix);
    let payload = serde_json::to_vec(&msg)?;

    let ack = jetstream.publish(subject, payload.into()).await?;
    ack.await?;
    Ok(())
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
                release_execution_id: None,
                target_server_ids: None,
            },
        );
        assert_eq!(msg.subject(DEFAULT_SUBJECT_PREFIX), "ordo.rules.acme.fraud");
    }

    #[test]
    fn test_stream_name_constant() {
        assert_eq!(STREAM_NAME, "ordo-rules");
    }

    #[test]
    fn test_server_rpc_wildcard_uses_non_stream_prefix() {
        assert_eq!(
            server_rpc_wildcard_subject(DEFAULT_SUBJECT_PREFIX, "srv_abc"),
            "_ORDO_RPC.ordo_rules.servers.srv_abc.*"
        );
    }
}
