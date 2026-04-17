//! Runtime-mutable server configuration.
//!
//! A subset of [`ServerConfig`] that can be changed at runtime via the admin API
//! without restarting the server. Changes are applied immediately to all dependent
//! components and, when NATS sync is active, broadcast to every node in the cluster.
//!
//! # Mutable fields
//!
//! | Field | Effect |
//! |-------|--------|
//! | `audit_sample_rate` | Fraction of executions written to the audit log |
//! | `default_tenant_qps` | Token-bucket refill rate for tenants with no explicit QPS limit |
//! | `default_tenant_burst` | Token-bucket capacity for tenants with no explicit burst limit |
//! | `default_tenant_timeout_ms` | Execution deadline for tenants with no explicit timeout |
//! | `max_rules_per_tenant` | Cap on rulesets per tenant (`null` = unlimited) |
//! | `max_total_rules` | Cap on total rulesets across all tenants (`null` = unlimited) |

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::ServerConfig;

/// The runtime-mutable portion of server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Audit-log sampling rate (0 = none, 100 = all executions).
    pub audit_sample_rate: u8,

    /// Default QPS limit applied to tenants that have no explicit `qps_limit`.
    /// `null` means unlimited.
    pub default_tenant_qps: Option<u32>,

    /// Default burst limit applied to tenants that have no explicit `burst_limit`.
    /// `null` means unlimited.
    pub default_tenant_burst: Option<u32>,

    /// Default execution timeout (ms) applied to tenants with no explicit timeout.
    pub default_tenant_timeout_ms: u64,

    /// Maximum number of rulesets a single tenant may own.
    /// `null` means unlimited.
    pub max_rules_per_tenant: Option<usize>,

    /// Maximum total rulesets across all tenants combined.
    /// `null` means unlimited.
    pub max_total_rules: Option<usize>,
}

impl RuntimeConfig {
    /// Initialise from the startup [`ServerConfig`].
    pub fn from_server_config(config: &ServerConfig) -> Self {
        Self {
            audit_sample_rate: config.audit_sample_rate,
            default_tenant_qps: config.default_tenant_qps,
            default_tenant_burst: config.default_tenant_burst,
            default_tenant_timeout_ms: config.default_tenant_timeout_ms,
            max_rules_per_tenant: config.max_rules_per_tenant,
            max_total_rules: config.max_total_rules,
        }
    }

    /// Validate the update request.
    pub fn validate(&self) -> Result<(), String> {
        if self.audit_sample_rate > 100 {
            return Err(format!(
                "audit_sample_rate must be 0–100, got {}",
                self.audit_sample_rate
            ));
        }
        Ok(())
    }
}

/// Shared handle to the live [`RuntimeConfig`].
pub type SharedRuntimeConfig = Arc<RwLock<RuntimeConfig>>;

/// Create a new shared config initialised from `server_config`.
pub fn new_shared(server_config: &ServerConfig) -> SharedRuntimeConfig {
    Arc::new(RwLock::new(RuntimeConfig::from_server_config(
        server_config,
    )))
}
