//! Sync events for distributed rule propagation.
//!
//! Events are published by the writer instance after successful mutations
//! and consumed by reader instances to update their in-memory caches.

use ordo_core::prelude::CapabilityDescriptor;
use serde::{Deserialize, Serialize};

/// A sync event describing a mutation that occurred on the writer instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SyncEvent {
    /// A ruleset was created or updated.
    RulePut {
        tenant_id: String,
        name: String,
        /// Full JSON-serialized ruleset — readers deserialize and compile locally.
        ruleset_json: String,
        /// RuleSet config version string, used for idempotent dedup on readers.
        version: String,
        /// Associated release execution when the message comes from rollout.
        release_execution_id: Option<String>,
        /// Optional target server id allow-list for batched rollout.
        target_server_ids: Option<Vec<String>>,
    },
    /// A ruleset was deleted.
    RuleDeleted { tenant_id: String, name: String },
    /// A tenant should be created or updated using server-local defaults.
    TenantUpsert {
        tenant_id: String,
        name: String,
        enabled: bool,
    },
    /// A tenant should be deleted.
    TenantDeleted { tenant_id: String },
    /// Tenant configuration was changed (create/update/delete).
    /// Carries the full tenants map so readers can replace atomically.
    TenantConfigChanged { config_json: String },
    /// A server instance announced or refreshed its registry metadata.
    ServerRegistered {
        #[serde(default)]
        server_id: String,
        name: String,
        url: String,
        token: String,
        version: Option<String>,
        org_id: Option<String>,
        #[serde(default)]
        capabilities: Vec<CapabilityDescriptor>,
    },
    /// A server instance sent a heartbeat.
    ServerHeartbeat {
        #[serde(default)]
        server_id: String,
    },
    /// A server acknowledged successful release application.
    ReleaseExecutionAck {
        execution_id: String,
        server_id: String,
        message: Option<String>,
    },
    /// A server reported a release application failure.
    ReleaseExecutionFailed {
        execution_id: String,
        server_id: String,
        error: String,
    },
}

impl SyncEvent {
    /// Returns a short label for metrics (e.g. "RulePut", "RuleDeleted").
    pub fn event_type(&self) -> &'static str {
        match self {
            SyncEvent::RulePut { .. } => "RulePut",
            SyncEvent::RuleDeleted { .. } => "RuleDeleted",
            SyncEvent::TenantUpsert { .. } => "TenantUpsert",
            SyncEvent::TenantDeleted { .. } => "TenantDeleted",
            SyncEvent::TenantConfigChanged { .. } => "TenantConfigChanged",
            SyncEvent::ServerRegistered { .. } => "ServerRegistered",
            SyncEvent::ServerHeartbeat { .. } => "ServerHeartbeat",
            SyncEvent::ReleaseExecutionAck { .. } => "ReleaseExecutionAck",
            SyncEvent::ReleaseExecutionFailed { .. } => "ReleaseExecutionFailed",
        }
    }
}

/// Envelope wrapping a [`SyncEvent`] with metadata for echo suppression and ordering.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(not(feature = "nats-sync"), allow(dead_code))]
pub struct SyncMessage {
    /// Unique identifier of the originating instance (prevents processing our own events).
    pub instance_id: String,
    /// The actual event payload.
    pub event: SyncEvent,
    /// Unix timestamp (milliseconds) when the event was created.
    pub timestamp_ms: i64,
}

#[cfg_attr(not(feature = "nats-sync"), allow(dead_code))]
impl SyncMessage {
    pub fn new(instance_id: String, event: SyncEvent) -> Self {
        let timestamp_ms = chrono::Utc::now().timestamp_millis();
        Self {
            instance_id,
            event,
            timestamp_ms,
        }
    }

    /// NATS subject for this event.
    ///
    /// Layout: `{prefix}.{tenant_id}.{name}` for rule events,
    ///         `{prefix}.tenants` for tenant config events.
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
            SyncEvent::TenantConfigChanged { .. } => {
                format!("{}.tenants", prefix)
            }
            SyncEvent::ServerRegistered { .. } => {
                format!("{}.control.servers.register", prefix)
            }
            SyncEvent::ServerHeartbeat { .. } => {
                format!("{}.control.servers.heartbeat", prefix)
            }
            SyncEvent::ReleaseExecutionAck {
                execution_id,
                server_id,
                ..
            }
            | SyncEvent::ReleaseExecutionFailed {
                execution_id,
                server_id,
                ..
            } => {
                format!("{}.control.releases.{}.{}", prefix, execution_id, server_id)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_event_roundtrip_rule_put() {
        let event = SyncEvent::RulePut {
            tenant_id: "default".into(),
            name: "payment-check".into(),
            ruleset_json: r#"{"config":{"name":"payment-check"}}"#.into(),
            version: "1.0.0".into(),
            release_execution_id: None,
            target_server_ids: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        let decoded: SyncEvent = serde_json::from_str(&json).unwrap();
        match decoded {
            SyncEvent::RulePut {
                tenant_id, name, ..
            } => {
                assert_eq!(tenant_id, "default");
                assert_eq!(name, "payment-check");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_sync_event_roundtrip_rule_deleted() {
        let event = SyncEvent::RuleDeleted {
            tenant_id: "tenant-a".into(),
            name: "old-rule".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let decoded: SyncEvent = serde_json::from_str(&json).unwrap();
        match decoded {
            SyncEvent::RuleDeleted { tenant_id, name } => {
                assert_eq!(tenant_id, "tenant-a");
                assert_eq!(name, "old-rule");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_sync_event_roundtrip_tenant_config() {
        let event = SyncEvent::TenantConfigChanged {
            config_json: r#"{"default":{"id":"default"}}"#.into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let decoded: SyncEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(decoded, SyncEvent::TenantConfigChanged { .. }));
    }

    #[test]
    fn test_sync_event_roundtrip_tenant_upsert() {
        let event = SyncEvent::TenantUpsert {
            tenant_id: "acme".into(),
            name: "Acme".into(),
            enabled: true,
        };
        let json = serde_json::to_string(&event).unwrap();
        let decoded: SyncEvent = serde_json::from_str(&json).unwrap();
        match decoded {
            SyncEvent::TenantUpsert {
                tenant_id,
                name,
                enabled,
            } => {
                assert_eq!(tenant_id, "acme");
                assert_eq!(name, "Acme");
                assert!(enabled);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_sync_message_new() {
        let msg = SyncMessage::new(
            "instance-1".into(),
            SyncEvent::RuleDeleted {
                tenant_id: "t".into(),
                name: "n".into(),
            },
        );
        assert_eq!(msg.instance_id, "instance-1");
        assert!(msg.timestamp_ms > 0);
    }

    #[test]
    fn test_sync_message_subject() {
        let msg = SyncMessage::new(
            "i1".into(),
            SyncEvent::RulePut {
                tenant_id: "acme".into(),
                name: "fraud".into(),
                ruleset_json: "{}".into(),
                version: "1".into(),
                release_execution_id: None,
                target_server_ids: None,
            },
        );
        assert_eq!(msg.subject("ordo.rules"), "ordo.rules.acme.fraud");

        let msg2 = SyncMessage::new(
            "i1".into(),
            SyncEvent::TenantConfigChanged {
                config_json: "{}".into(),
            },
        );
        assert_eq!(msg2.subject("ordo.rules"), "ordo.rules.tenants");

        let msg3 = SyncMessage::new(
            "i1".into(),
            SyncEvent::TenantUpsert {
                tenant_id: "acme".into(),
                name: "Acme".into(),
                enabled: true,
            },
        );
        assert_eq!(msg3.subject("ordo.rules"), "ordo.rules.tenants.acme");
    }

    #[test]
    fn test_sync_message_roundtrip() {
        let msg = SyncMessage::new(
            "node-42".into(),
            SyncEvent::RulePut {
                tenant_id: "default".into(),
                name: "test".into(),
                ruleset_json: r#"{"config":{"name":"test"}}"#.into(),
                version: "2.0".into(),
                release_execution_id: None,
                target_server_ids: None,
            },
        );
        let bytes = serde_json::to_vec(&msg).unwrap();
        let decoded: SyncMessage = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded.instance_id, "node-42");
        assert_eq!(decoded.timestamp_ms, msg.timestamp_ms);
    }
}
