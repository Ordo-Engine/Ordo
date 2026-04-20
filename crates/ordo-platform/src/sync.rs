//! NATS-based platform → server sync publisher.

use async_nats::jetstream::{self, stream::RetentionPolicy};
use serde::{Deserialize, Serialize};
use std::io;
use std::time::Duration;
use tracing::{info, warn};
use url::Url;

const STREAM_NAME: &str = "ordo-rules";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SyncEvent {
    RulePut {
        tenant_id: String,
        name: String,
        ruleset_json: String,
        version: String,
    },
    RuleDeleted {
        tenant_id: String,
        name: String,
    },
    TenantUpsert {
        tenant_id: String,
        name: String,
        enabled: bool,
    },
    TenantDeleted {
        tenant_id: String,
    },
    TenantConfigChanged {
        config_json: String,
    },
    ServerRegistered {
        name: String,
        url: String,
        token: String,
        version: Option<String>,
        org_id: Option<String>,
    },
    ServerHeartbeat {
        token: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncMessage {
    pub instance_id: String,
    pub event: SyncEvent,
    pub timestamp_ms: i64,
}

impl SyncMessage {
    pub fn new(instance_id: String, event: SyncEvent) -> Self {
        Self {
            instance_id,
            event,
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn subject(&self, prefix: &str) -> String {
        match &self.event {
            SyncEvent::RulePut {
                tenant_id, name, ..
            }
            | SyncEvent::RuleDeleted { tenant_id, name } => {
                format!("{}.{}.{}", prefix, tenant_id, name)
            }
            SyncEvent::TenantUpsert { tenant_id, .. } | SyncEvent::TenantDeleted { tenant_id } => {
                format!("{}.tenants.{}", prefix, tenant_id)
            }
            SyncEvent::TenantConfigChanged { .. } => format!("{}.tenants", prefix),
            SyncEvent::ServerRegistered { .. } => format!("{}.control.servers.register", prefix),
            SyncEvent::ServerHeartbeat { .. } => format!("{}.control.servers.heartbeat", prefix),
        }
    }
}

#[derive(Clone)]
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

    pub async fn publish(&self, event: SyncEvent) -> anyhow::Result<()> {
        self.publish_to(&self.subject_prefix.clone(), event).await
    }

    /// Publish to an explicit NATS subject prefix (for multi-environment deployments).
    pub async fn publish_to(&self, prefix: &str, event: SyncEvent) -> anyhow::Result<()> {
        let msg = SyncMessage::new(self.instance_id.clone(), event);
        let subject = msg.subject(prefix);
        let payload = serde_json::to_vec(&msg)?;

        let ack = self
            .jetstream
            .publish(subject.clone(), payload.into())
            .await
            .map_err(|e| anyhow::anyhow!("failed to publish to {}: {}", subject, e))?;

        ack.await.map_err(|e| {
            anyhow::anyhow!("failed to receive JetStream ack for {}: {}", subject, e)
        })?;
        Ok(())
    }
}

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

pub async fn connect(nats_url: &str) -> Result<jetstream::Context, async_nats::Error> {
    let (options, server_addr) = connect_options_and_addr(nats_url)?;
    let client = options.connect(server_addr.as_str()).await?;
    Ok(jetstream::new(client))
}

pub async fn ensure_stream(
    jetstream: &jetstream::Context,
    subject_prefix: &str,
) -> Result<(), async_nats::Error> {
    jetstream
        .get_or_create_stream(jetstream::stream::Config {
            name: STREAM_NAME.to_string(),
            subjects: vec![format!("{}.>", subject_prefix)],
            retention: RetentionPolicy::Limits,
            storage: jetstream::stream::StorageType::File,
            max_age: Duration::from_secs(7 * 24 * 3600),
            ..Default::default()
        })
        .await?;
    Ok(())
}

pub async fn create_control_consumer(
    jetstream: &jetstream::Context,
    instance_id: &str,
    subject_prefix: &str,
) -> anyhow::Result<jetstream::consumer::PullConsumer> {
    let stream = jetstream.get_stream(STREAM_NAME).await?;
    let consumer_name = format!("platform-registry-{}", instance_id);
    let filter = format!("{}.control.servers.>", subject_prefix);

    Ok(stream
        .get_or_create_consumer(
            &consumer_name,
            jetstream::consumer::pull::Config {
                durable_name: Some(consumer_name.clone()),
                ack_policy: jetstream::consumer::AckPolicy::Explicit,
                deliver_policy: jetstream::consumer::DeliverPolicy::All,
                ack_wait: Duration::from_secs(30),
                max_deliver: 20,
                max_ack_pending: 1_000,
                filter_subject: filter,
                ..Default::default()
            },
        )
        .await?)
}

pub fn start_registry_subscriber(
    consumer: jetstream::consumer::PullConsumer,
    store: std::sync::Arc<crate::store::PlatformStore>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        use futures::StreamExt;

        info!("NATS server-registry subscriber started");

        loop {
            let messages = consumer
                .fetch()
                .max_messages(100)
                .expires(Duration::from_secs(5))
                .messages()
                .await;

            let mut messages = match messages {
                Ok(messages) => messages,
                Err(e) => {
                    warn!("NATS registry subscriber fetch error: {}", e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }
            };

            while let Some(msg_result) = messages.next().await {
                let msg = match msg_result {
                    Ok(msg) => msg,
                    Err(e) => {
                        warn!("NATS registry subscriber receive error: {}", e);
                        continue;
                    }
                };

                let sync_msg: SyncMessage = match serde_json::from_slice(&msg.payload) {
                    Ok(sync_msg) => sync_msg,
                    Err(e) => {
                        warn!("Failed to deserialize NATS registry event: {}", e);
                        let _ = msg.ack().await;
                        continue;
                    }
                };

                let result = match sync_msg.event {
                    SyncEvent::ServerRegistered {
                        name,
                        url,
                        token,
                        version,
                        org_id,
                    } => {
                        let existing = store.find_server_by_token(&token).await;
                        match existing {
                            Ok(existing) => {
                                let id = existing
                                    .as_ref()
                                    .map(|s| s.id.clone())
                                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

                                let server = crate::models::ServerNode {
                                    id,
                                    name,
                                    url,
                                    token,
                                    org_id,
                                    labels: serde_json::Value::Object(Default::default()),
                                    version,
                                    status: crate::models::ServerStatus::Online,
                                    last_seen: Some(chrono::Utc::now()),
                                    registered_at: existing
                                        .map(|s| s.registered_at)
                                        .unwrap_or_else(chrono::Utc::now),
                                };
                                store.upsert_server(&server).await
                            }
                            Err(e) => Err(e),
                        }
                    }
                    SyncEvent::ServerHeartbeat { token } => {
                        store.update_server_heartbeat(&token).await.map(|_| ())
                    }
                    _ => Ok(()),
                };

                match result {
                    Ok(()) => {
                        let _ = msg.ack().await;
                    }
                    Err(e) => {
                        warn!("Failed to apply NATS registry event: {}", e);
                        let _ = msg
                            .ack_with(async_nats::jetstream::AckKind::Nak(None))
                            .await;
                    }
                }
            }
        }
    })
}
