//! Sync events for distributed rule propagation.
//!
//! Events are published by the writer instance after successful mutations
//! and consumed by reader instances to update their in-memory caches.

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
    },
    /// A ruleset was deleted.
    RuleDeleted { tenant_id: String, name: String },
    /// Tenant configuration was changed (create/update/delete).
    /// Carries the full tenants map so readers can replace atomically.
    TenantConfigChanged { config_json: String },
    /// Runtime configuration was changed via the admin API.
    /// Carries the full [`RuntimeConfig`] as JSON so every node applies it atomically.
    RuntimeConfigChanged { config_json: String },
}

impl SyncEvent {
    /// Returns a short label for metrics (e.g. "RulePut", "RuleDeleted").
    pub fn event_type(&self) -> &'static str {
        match self {
            SyncEvent::RulePut { .. } => "RulePut",
            SyncEvent::RuleDeleted { .. } => "RuleDeleted",
            SyncEvent::TenantConfigChanged { .. } => "TenantConfigChanged",
            SyncEvent::RuntimeConfigChanged { .. } => "RuntimeConfigChanged",
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
            SyncEvent::TenantConfigChanged { .. } => {
                format!("{}.tenants", prefix)
            }
            SyncEvent::RuntimeConfigChanged { .. } => {
                format!("{}.runtime-config", prefix)
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
            },
        );
        let bytes = serde_json::to_vec(&msg).unwrap();
        let decoded: SyncMessage = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded.instance_id, "node-42");
        assert_eq!(decoded.timestamp_ms, msg.timestamp_ms);
    }
}
