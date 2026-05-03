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
        release_execution_id: Option<String>,
        target_server_ids: Option<Vec<String>>,
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
        #[serde(default)]
        server_id: String,
        name: String,
        url: String,
        token: String,
        version: Option<String>,
        org_id: Option<String>,
        #[serde(default)]
        capabilities: Vec<serde_json::Value>,
    },
    ServerHeartbeat {
        #[serde(default)]
        server_id: String,
    },
    ReleaseExecutionAck {
        execution_id: String,
        server_id: String,
        message: Option<String>,
    },
    ReleaseExecutionFailed {
        execution_id: String,
        server_id: String,
        error: String,
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
            SyncEvent::ReleaseExecutionAck {
                execution_id,
                server_id,
                ..
            }
            | SyncEvent::ReleaseExecutionFailed {
                execution_id,
                server_id,
                ..
            } => format!("{}.control.releases.{}.{}", prefix, execution_id, server_id),
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

pub async fn connect_client(nats_url: &str) -> Result<async_nats::Client, async_nats::Error> {
    let (options, server_addr) = connect_options_and_addr(nats_url)?;
    Ok(options.connect(server_addr.as_str()).await?)
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

pub fn server_rpc_subject(subject_prefix: &str, server_id: &str, endpoint: &str) -> String {
    format!(
        "{}.servers.{}.{}",
        rpc_prefix(subject_prefix),
        server_id,
        endpoint
    )
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
    let filter = format!("{}.control.>", subject_prefix);

    if let Ok(info) = stream.consumer_info(&consumer_name).await {
        if info.config.filter_subject != filter {
            info!(
                consumer = %consumer_name,
                old_filter = %info.config.filter_subject,
                new_filter = %filter,
                "Recreating NATS control consumer with updated filter"
            );
            stream.delete_consumer(&consumer_name).await?;
        }
    }

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
                        server_id,
                        name,
                        url,
                        token,
                        version,
                        org_id,
                        capabilities,
                    } => match crate::models::derive_server_id(&url) {
                        Ok(derived_server_id) => {
                            let effective_server_id = if server_id.is_empty() {
                                Ok(derived_server_id)
                            } else if derived_server_id != server_id {
                                Err(anyhow::anyhow!(
                                    "server_id {} does not match normalized url {}",
                                    server_id,
                                    url
                                ))
                            } else {
                                Ok(server_id)
                            };
                            match effective_server_id {
                                Ok(effective_server_id) => {
                                    let existing = store.get_server(&effective_server_id).await;
                                    match existing {
                                        Ok(existing_opt) => {
                                            let preserved_token = resolve_server_registry_token(
                                                token,
                                                existing_opt.as_ref().map(|s| s.token.as_str()),
                                                &effective_server_id,
                                            );
                                            // Reject if a different server already owns this token
                                            if !preserved_token.is_empty() {
                                                match store
                                                    .find_server_by_token(&preserved_token)
                                                    .await
                                                {
                                                    Ok(Some(ref owner))
                                                        if owner.id != effective_server_id =>
                                                    {
                                                        let _ = msg.ack().await;
                                                        continue;
                                                    }
                                                    _ => {}
                                                }
                                            }
                                            let caps_json = serde_json::Value::Array(capabilities);
                                            let server = crate::models::ServerNode {
                                                id: effective_server_id,
                                                name,
                                                url,
                                                token: preserved_token,
                                                org_id,
                                                labels: serde_json::Value::Object(
                                                    Default::default(),
                                                ),
                                                version,
                                                status: crate::models::ServerStatus::Online,
                                                last_seen: Some(chrono::Utc::now()),
                                                registered_at: existing_opt
                                                    .map(|s| s.registered_at)
                                                    .unwrap_or_else(chrono::Utc::now),
                                                capabilities: caps_json,
                                            };
                                            store.upsert_server(&server).await
                                        }
                                        Err(e) => Err(e),
                                    }
                                }
                                Err(e) => Err(e),
                            }
                        }
                        Err(e) => Err(e),
                    },
                    SyncEvent::ServerHeartbeat { server_id } => {
                        if server_id.is_empty() {
                            tracing::debug!("Ignoring legacy server heartbeat without server_id");
                            Ok(())
                        } else {
                            store.update_server_heartbeat(&server_id).await.map(|_| ())
                        }
                    }
                    SyncEvent::ReleaseExecutionAck {
                        execution_id,
                        server_id,
                        message,
                    } => match store
                        .find_release_execution_instance_by_target(&execution_id, &server_id)
                        .await
                    {
                        Ok(Some(instance)) => {
                            match store.get_release_execution(&execution_id).await {
                                Ok(Some(execution)) => {
                                    let ack_status = if execution.status
                                        == crate::models::ReleaseExecutionStatus::RollbackInProgress
                                    {
                                        crate::models::ReleaseInstanceStatus::RolledBack
                                    } else {
                                        crate::models::ReleaseInstanceStatus::Success
                                    };
                                    let history_action = if ack_status
                                        == crate::models::ReleaseInstanceStatus::RolledBack
                                    {
                                        "instance_rolled_back"
                                    } else {
                                        "instance_acknowledged"
                                    };
                                    let event_type = if ack_status
                                        == crate::models::ReleaseInstanceStatus::RolledBack
                                    {
                                        "rollback_ack"
                                    } else {
                                        "release_ack"
                                    };
                                    if let Err(err) =
                                        crate::release::validate_release_instance_transition(
                                            &instance.status,
                                            &ack_status,
                                        )
                                    {
                                        Err(err)
                                    } else {
                                        let update_result = store
                                            .update_release_execution_instance(
                                                &instance.id,
                                                ack_status.clone(),
                                                message
                                                    .as_deref()
                                                    .or(Some("Server applied release payload")),
                                                Some(event_type),
                                            )
                                            .await;
                                        if let Err(e) = update_result {
                                            Err(e)
                                        } else {
                                            let actor = crate::release::server_history_actor(
                                                &server_id,
                                                Some(&instance.instance_name),
                                            );
                                            let from_status = instance.status.to_string();
                                            let to_status = ack_status.to_string();
                                            let _ = store
                                                .create_release_request_history(
                                                    &uuid::Uuid::new_v4().to_string(),
                                                    &execution.request_id,
                                                    Some(&execution_id),
                                                    Some(&instance.id),
                                                    crate::models::ReleaseHistoryScope::Instance,
                                                    history_action,
                                                    &actor,
                                                    Some(from_status.as_str()),
                                                    Some(to_status.as_str()),
                                                    serde_json::json!({
                                                        "instance_name": instance.instance_name,
                                                        "target_instance_id": instance.instance_id,
                                                        "server_id": server_id,
                                                        "message": message,
                                                    }),
                                                )
                                                .await;
                                            let _ = store
                                                .create_release_execution_event(
                                                    &uuid::Uuid::new_v4().to_string(),
                                                    &execution_id,
                                                    Some(&instance.id),
                                                    event_type,
                                                    serde_json::json!({
                                                        "server_id": server_id,
                                                        "message": message,
                                                    }),
                                                )
                                                .await;
                                            Ok(())
                                        }
                                    }
                                }
                                Ok(None) => Err(anyhow::anyhow!("release execution not found")),
                                Err(e) => Err(e),
                            }
                        }
                        Ok(None) => Err(anyhow::anyhow!(
                            "release execution instance not found for execution {} server {}",
                            execution_id,
                            server_id
                        )),
                        Err(e) => Err(e),
                    },
                    SyncEvent::ReleaseExecutionFailed {
                        execution_id,
                        server_id,
                        error,
                    } => match store
                        .find_release_execution_instance_by_target(&execution_id, &server_id)
                        .await
                    {
                        Ok(Some(instance)) => {
                            match store.get_release_execution(&execution_id).await {
                                Ok(Some(execution)) => {
                                    if let Err(err) =
                                        crate::release::validate_release_instance_transition(
                                            &instance.status,
                                            &crate::models::ReleaseInstanceStatus::Failed,
                                        )
                                    {
                                        Err(err)
                                    } else {
                                        let update_result = store
                                            .update_release_execution_instance(
                                                &instance.id,
                                                crate::models::ReleaseInstanceStatus::Failed,
                                                Some(&error),
                                                Some("release_failed"),
                                            )
                                            .await;
                                        if let Err(e) = update_result {
                                            Err(e)
                                        } else {
                                            let actor = crate::release::server_history_actor(
                                                &server_id,
                                                Some(&instance.instance_name),
                                            );
                                            let from_status = instance.status.to_string();
                                            let to_status =
                                                crate::models::ReleaseInstanceStatus::Failed
                                                    .to_string();
                                            let _ = store
                                                .create_release_request_history(
                                                    &uuid::Uuid::new_v4().to_string(),
                                                    &execution.request_id,
                                                    Some(&execution_id),
                                                    Some(&instance.id),
                                                    crate::models::ReleaseHistoryScope::Instance,
                                                    "instance_failed",
                                                    &actor,
                                                    Some(from_status.as_str()),
                                                    Some(to_status.as_str()),
                                                    serde_json::json!({
                                                        "instance_name": instance.instance_name,
                                                        "target_instance_id": instance.instance_id,
                                                        "server_id": server_id,
                                                        "error": error,
                                                    }),
                                                )
                                                .await;
                                            let _ = store
                                                .create_release_execution_event(
                                                    &uuid::Uuid::new_v4().to_string(),
                                                    &execution_id,
                                                    Some(&instance.id),
                                                    "release_failed",
                                                    serde_json::json!({
                                                        "server_id": server_id,
                                                        "error": error,
                                                    }),
                                                )
                                                .await;
                                            Ok(())
                                        }
                                    }
                                }
                                Ok(None) => Err(anyhow::anyhow!("release execution not found")),
                                Err(e) => Err(e),
                            }
                        }
                        Ok(None) => Err(anyhow::anyhow!(
                            "release execution instance not found for execution {} server {}",
                            execution_id,
                            server_id
                        )),
                        Err(e) => Err(e),
                    },
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

fn resolve_server_registry_token(
    incoming_token: String,
    existing_token: Option<&str>,
    server_id: &str,
) -> String {
    if !incoming_token.is_empty() {
        incoming_token
    } else if let Some(existing_token) = existing_token.filter(|token| !token.is_empty()) {
        existing_token.to_string()
    } else {
        format!("nats:{}", server_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_rpc_subject_uses_non_stream_prefix() {
        assert_eq!(
            server_rpc_subject("ordo.rules", "srv_abc", "health"),
            "_ORDO_RPC.ordo_rules.servers.srv_abc.health"
        );
    }

    #[test]
    fn nats_server_registration_gets_stable_synthetic_token() {
        assert_eq!(
            resolve_server_registry_token(String::new(), None, "srv_abc"),
            "nats:srv_abc"
        );
        assert_eq!(
            resolve_server_registry_token(String::new(), Some("existing"), "srv_abc"),
            "existing"
        );
        assert_eq!(
            resolve_server_registry_token("incoming".to_string(), Some("existing"), "srv_abc"),
            "incoming"
        );
    }
}
