use super::provider::{
    CapabilityDescriptor, CapabilityInvoker, CapabilityProvider, CapabilityRequest,
    CapabilityResponse,
};
use crate::error::OrdoError;
use crate::error::Result;
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Default)]
struct CircuitState {
    consecutive_failures: u32,
    opened_at: Option<Instant>,
}

struct RegisteredCapability {
    descriptor: CapabilityDescriptor,
    provider: Arc<dyn CapabilityProvider>,
    state: Mutex<CircuitState>,
}

/// In-memory capability registry used by the executor and tests.
#[derive(Default)]
pub struct CapabilityRegistry {
    providers: RwLock<HashMap<String, Arc<RegisteredCapability>>>,
}

impl CapabilityRegistry {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &self,
        provider: Arc<dyn CapabilityProvider>,
    ) -> Option<Arc<dyn CapabilityProvider>> {
        let descriptor = provider.descriptor();
        let name = descriptor.name.clone();
        let registered = Arc::new(RegisteredCapability {
            descriptor,
            provider,
            state: Mutex::new(CircuitState::default()),
        });

        self.providers
            .write()
            .insert(name, registered)
            .map(|previous| previous.provider.clone())
    }

    #[inline]
    pub fn contains(&self, capability: &str) -> bool {
        self.providers.read().contains_key(capability)
    }

    pub fn list(&self) -> Vec<CapabilityDescriptor> {
        self.providers
            .read()
            .values()
            .map(|entry| entry.descriptor.clone())
            .collect()
    }

    fn lookup(&self, capability: &str) -> Option<Arc<RegisteredCapability>> {
        self.providers.read().get(capability).cloned()
    }

    fn ensure_circuit_closed(entry: &RegisteredCapability) -> Result<()> {
        let config = &entry.descriptor.config.circuit_breaker;
        if !config.enabled() {
            return Ok(());
        }

        let mut state = entry.state.lock();
        if let Some(opened_at) = state.opened_at {
            let elapsed = opened_at.elapsed();
            if elapsed >= Duration::from_millis(config.reset_timeout_ms) {
                state.consecutive_failures = 0;
                state.opened_at = None;
                return Ok(());
            }

            let remaining = config
                .reset_timeout_ms
                .saturating_sub(elapsed.as_millis().min(u128::from(u64::MAX)) as u64);
            return Err(OrdoError::CircuitOpen {
                capability: entry.descriptor.name.clone(),
                retry_after_ms: Some(remaining),
            });
        }

        Ok(())
    }

    fn mark_success(entry: &RegisteredCapability) {
        let mut state = entry.state.lock();
        state.consecutive_failures = 0;
        state.opened_at = None;
    }

    fn mark_failure(entry: &RegisteredCapability) {
        let config = &entry.descriptor.config.circuit_breaker;
        if !config.enabled() {
            return;
        }

        let mut state = entry.state.lock();
        state.consecutive_failures = state.consecutive_failures.saturating_add(1);
        if state.consecutive_failures >= config.failure_threshold {
            state.opened_at = Some(Instant::now());
        }
    }

    fn is_retryable_error(error: &OrdoError) -> bool {
        matches!(
            error,
            OrdoError::Timeout { .. } | OrdoError::CapabilityInvocation { .. }
        )
    }
}

impl CapabilityInvoker for CapabilityRegistry {
    fn invoke(&self, request: &CapabilityRequest) -> Result<CapabilityResponse> {
        let entry =
            self.lookup(&request.capability)
                .ok_or_else(|| OrdoError::CapabilityNotFound {
                    capability: request.capability.clone(),
                })?;

        Self::ensure_circuit_closed(&entry)?;

        let attempts = entry.descriptor.config.retry.max_attempts.max(1);
        let timeout_ms = request.timeout_ms.or(entry.descriptor.config.timeout_ms);

        for attempt in 0..attempts {
            let start = Instant::now();
            let response = entry.provider.invoke(request);
            let response = match (timeout_ms, response) {
                (Some(limit), Ok(_)) if start.elapsed().as_millis() as u64 > limit => {
                    Err(OrdoError::Timeout { timeout_ms: limit })
                }
                (_, other) => other,
            };

            match response {
                Ok(response) => {
                    Self::mark_success(&entry);
                    return Ok(response);
                }
                Err(error) => {
                    Self::mark_failure(&entry);
                    let should_retry = attempt + 1 < attempts && Self::is_retryable_error(&error);
                    if should_retry {
                        #[cfg(not(target_arch = "wasm32"))]
                        if entry.descriptor.config.retry.backoff_ms > 0 {
                            std::thread::sleep(Duration::from_millis(
                                entry.descriptor.config.retry.backoff_ms,
                            ));
                        }
                        continue;
                    }
                    return Err(error);
                }
            }
        }

        Err(OrdoError::internal_error_static(
            "capability registry retry loop exited unexpectedly",
        ))
    }

    fn describe(&self, capability: &str) -> Option<CapabilityDescriptor> {
        self.lookup(capability)
            .map(|entry| entry.descriptor.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::{
        CapabilityCategory, CapabilityConfig, CapabilityDescriptor, CircuitBreakerConfig,
        RetryPolicy,
    };
    use crate::context::Value;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct SequenceProvider {
        descriptor: CapabilityDescriptor,
        calls: AtomicUsize,
        responses: Mutex<VecDeque<Result<CapabilityResponse>>>,
    }

    impl SequenceProvider {
        fn new(
            descriptor: CapabilityDescriptor,
            responses: Vec<Result<CapabilityResponse>>,
        ) -> Self {
            Self {
                descriptor,
                calls: AtomicUsize::new(0),
                responses: Mutex::new(responses.into()),
            }
        }
    }

    impl CapabilityProvider for SequenceProvider {
        fn descriptor(&self) -> CapabilityDescriptor {
            self.descriptor.clone()
        }

        fn invoke(&self, _request: &CapabilityRequest) -> Result<CapabilityResponse> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.responses
                .lock()
                .pop_front()
                .unwrap_or_else(|| Ok(CapabilityResponse::new(Value::string("default"))))
        }
    }

    #[test]
    fn registers_and_invokes_provider() {
        let registry = CapabilityRegistry::new();
        registry.register(Arc::new(SequenceProvider::new(
            CapabilityDescriptor::new("demo.echo", CapabilityCategory::Compute),
            vec![Ok(CapabilityResponse::new(Value::string("ok")))],
        )));

        let response = registry
            .invoke(&CapabilityRequest::new(
                "demo.echo",
                "run",
                Value::object(std::collections::HashMap::new()),
            ))
            .unwrap();

        assert_eq!(response.payload, Value::string("ok"));
        assert!(registry.contains("demo.echo"));
        assert_eq!(registry.list().len(), 1);
    }

    #[test]
    fn retries_transient_errors() {
        let registry = CapabilityRegistry::new();
        let descriptor = CapabilityDescriptor::new("demo.retry", CapabilityCategory::Network)
            .with_config(
                CapabilityConfig::new(CapabilityCategory::Network).retry(RetryPolicy {
                    max_attempts: 2,
                    backoff_ms: 0,
                }),
            );
        let provider = Arc::new(SequenceProvider::new(
            descriptor,
            vec![
                Err(OrdoError::CapabilityInvocation {
                    capability: "demo.retry".to_string(),
                    message: "temporary".into(),
                }),
                Ok(CapabilityResponse::new(Value::string("recovered"))),
            ],
        ));
        let provider_ref = provider.clone();
        registry.register(provider);

        let response = registry
            .invoke(&CapabilityRequest::new("demo.retry", "fetch", Value::Null))
            .unwrap();

        assert_eq!(response.payload, Value::string("recovered"));
        assert_eq!(provider_ref.calls.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn opens_circuit_after_repeated_failures() {
        let registry = CapabilityRegistry::new();
        registry.register(Arc::new(SequenceProvider::new(
            CapabilityDescriptor::new("demo.breaker", CapabilityCategory::Action).with_config(
                CapabilityConfig::new(CapabilityCategory::Action).circuit_breaker(
                    CircuitBreakerConfig {
                        failure_threshold: 2,
                        reset_timeout_ms: 1000,
                    },
                ),
            ),
            vec![
                Err(OrdoError::CapabilityInvocation {
                    capability: "demo.breaker".to_string(),
                    message: "boom".into(),
                }),
                Err(OrdoError::CapabilityInvocation {
                    capability: "demo.breaker".to_string(),
                    message: "boom".into(),
                }),
            ],
        )));

        assert!(registry
            .invoke(&CapabilityRequest::new("demo.breaker", "emit", Value::Null))
            .is_err());
        assert!(registry
            .invoke(&CapabilityRequest::new("demo.breaker", "emit", Value::Null))
            .is_err());

        let error = registry
            .invoke(&CapabilityRequest::new("demo.breaker", "emit", Value::Null))
            .unwrap_err();
        assert!(matches!(error, OrdoError::CircuitOpen { .. }));
    }
}
