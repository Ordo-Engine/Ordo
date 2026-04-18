//! NATS-based platform → server sync publisher.

use async_nats::jetstream::{self, stream::RetentionPolicy};
use serde::{Deserialize, Serialize};
use std::time::Duration;

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

pub async fn connect(nats_url: &str) -> Result<jetstream::Context, async_nats::Error> {
    let client = async_nats::connect(nats_url).await?;
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
