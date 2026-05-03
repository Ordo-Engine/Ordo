use crate::context::Value;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// High-level execution tier for a capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityCategory {
    Network,
    Compute,
    Action,
}

/// Retry policy for a capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub backoff_ms: u64,
}

impl RetryPolicy {
    #[inline]
    pub fn disabled() -> Self {
        Self {
            max_attempts: 1,
            backoff_ms: 0,
        }
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::disabled()
    }
}

/// Consecutive-failure breaker for a capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub reset_timeout_ms: u64,
}

impl CircuitBreakerConfig {
    #[inline]
    pub fn disabled() -> Self {
        Self {
            failure_threshold: 0,
            reset_timeout_ms: 0,
        }
    }

    #[inline]
    pub fn enabled(&self) -> bool {
        self.failure_threshold > 0
    }
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self::disabled()
    }
}

/// Runtime policy for a registered capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityConfig {
    pub category: CapabilityCategory,
    pub timeout_ms: Option<u64>,
    pub retry: RetryPolicy,
    pub circuit_breaker: CircuitBreakerConfig,
}

impl CapabilityConfig {
    #[inline]
    pub fn new(category: CapabilityCategory) -> Self {
        Self {
            category,
            timeout_ms: None,
            retry: RetryPolicy::default(),
            circuit_breaker: CircuitBreakerConfig::default(),
        }
    }

    #[inline]
    pub fn timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    #[inline]
    pub fn retry(mut self, retry: RetryPolicy) -> Self {
        self.retry = retry;
        self
    }

    #[inline]
    pub fn circuit_breaker(mut self, circuit_breaker: CircuitBreakerConfig) -> Self {
        self.circuit_breaker = circuit_breaker;
        self
    }
}

/// Metadata for a registered capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityDescriptor {
    pub name: String,
    pub description: String,
    pub config: CapabilityConfig,
    /// Operations this capability accepts (e.g. ["GET","POST"] or ["counter","gauge"]).
    #[serde(default)]
    pub operations: Vec<String>,
}

impl CapabilityDescriptor {
    #[inline]
    pub fn new(name: impl Into<String>, category: CapabilityCategory) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            config: CapabilityConfig::new(category),
            operations: Vec::new(),
        }
    }

    #[inline]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    #[inline]
    pub fn with_config(mut self, config: CapabilityConfig) -> Self {
        self.config = config;
        self
    }

    #[inline]
    pub fn with_operations(mut self, ops: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.operations = ops.into_iter().map(Into::into).collect();
        self
    }
}

/// Normalized request shape passed to capability providers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityRequest {
    pub capability: String,
    pub operation: String,
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub category: Option<CapabilityCategory>,
}

impl CapabilityRequest {
    #[inline]
    pub fn new(
        capability: impl Into<String>,
        operation: impl Into<String>,
        payload: Value,
    ) -> Self {
        Self {
            capability: capability.into(),
            operation: operation.into(),
            payload,
            metadata: HashMap::new(),
            timeout_ms: None,
            category: None,
        }
    }

    #[inline]
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    #[inline]
    pub fn with_category(mut self, category: CapabilityCategory) -> Self {
        self.category = Some(category);
        self
    }

    #[inline]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Normalized response shape returned by capability providers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityResponse {
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl CapabilityResponse {
    #[inline]
    pub fn new(payload: Value) -> Self {
        Self {
            payload,
            metadata: HashMap::new(),
        }
    }

    #[inline]
    pub fn empty() -> Self {
        Self::new(Value::Null)
    }

    #[inline]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Provider implementation for a named capability.
pub trait CapabilityProvider: Send + Sync {
    fn descriptor(&self) -> CapabilityDescriptor;
    fn invoke(&self, request: &CapabilityRequest) -> Result<CapabilityResponse>;
}

/// Trait used by the rule executor to call a capability registry.
pub trait CapabilityInvoker: Send + Sync {
    fn invoke(&self, request: &CapabilityRequest) -> Result<CapabilityResponse>;

    fn describe(&self, capability: &str) -> Option<CapabilityDescriptor> {
        let _ = capability;
        None
    }

    fn list_capabilities(&self) -> Vec<CapabilityDescriptor> {
        vec![]
    }
}
