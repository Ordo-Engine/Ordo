//! Webhook notification system.
//!
//! Fires HTTP POST callbacks when rule engine events occur.
//! Webhooks are delivered asynchronously via a background task
//! to avoid blocking the main request path.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// Events that can trigger webhooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookEvent {
    RuleCreated,
    RuleUpdated,
    RuleDeleted,
    RuleRollback,
    RuleExecuted,
}

impl WebhookEvent {
    pub fn all() -> Vec<Self> {
        vec![
            Self::RuleCreated,
            Self::RuleUpdated,
            Self::RuleDeleted,
            Self::RuleRollback,
            Self::RuleExecuted,
        ]
    }
}

/// A registered webhook endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    /// Unique webhook ID (auto-generated if not provided).
    pub id: String,
    /// Target URL to POST to.
    pub url: String,
    /// Events to listen for (empty = all events).
    pub events: Vec<WebhookEvent>,
    /// Optional secret for HMAC-SHA256 signing (sent as `X-Ordo-Signature`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
    /// Whether the webhook is active.
    #[serde(default = "default_true")]
    pub active: bool,
    /// Maximum retry attempts on failure.
    #[serde(default = "default_max_retries")]
    pub max_retries: u8,
    /// Optional description.
    #[serde(default)]
    pub description: String,
}

fn default_true() -> bool {
    true
}
fn default_max_retries() -> u8 {
    3
}

/// Payload delivered to webhook endpoints.
#[derive(Debug, Clone, Serialize)]
pub struct WebhookPayload {
    /// Event type
    pub event: WebhookEvent,
    /// When the event occurred
    pub timestamp: DateTime<Utc>,
    /// Tenant ID (if multi-tenancy)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    /// Event-specific data
    pub data: serde_json::Value,
}

/// Internal message sent to the background delivery task.
struct DeliveryJob {
    webhook: WebhookConfig,
    payload: WebhookPayload,
}

/// Manages webhook registrations and async delivery.
pub struct WebhookManager {
    /// Registered webhooks by ID.
    webhooks: RwLock<HashMap<String, WebhookConfig>>,
    /// Channel to send delivery jobs to the background task.
    tx: mpsc::Sender<DeliveryJob>,
    /// HTTP client (used by delivery loop, kept here for potential direct-send).
    #[allow(dead_code)]
    client: reqwest::Client,
}

impl WebhookManager {
    /// Create a new WebhookManager and spawn the background delivery task.
    pub fn new(shutdown_rx: tokio::sync::watch::Receiver<bool>) -> Arc<Self> {
        let (tx, rx) = mpsc::channel::<DeliveryJob>(1024);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .expect("failed to build reqwest client");

        let manager = Arc::new(Self {
            webhooks: RwLock::new(HashMap::new()),
            tx,
            client: client.clone(),
        });

        // Spawn background delivery task
        tokio::spawn(delivery_loop(rx, client, shutdown_rx));

        manager
    }

    /// Register a webhook. Returns the assigned ID.
    pub async fn register(&self, mut config: WebhookConfig) -> String {
        if config.id.is_empty() {
            config.id = generate_id();
        }
        let id = config.id.clone();
        self.webhooks.write().await.insert(id.clone(), config);
        info!(webhook_id = %id, "Webhook registered");
        id
    }

    /// Unregister a webhook by ID. Returns true if it existed.
    pub async fn unregister(&self, id: &str) -> bool {
        let removed = self.webhooks.write().await.remove(id).is_some();
        if removed {
            info!(webhook_id = %id, "Webhook unregistered");
        }
        removed
    }

    /// Get a webhook by ID.
    pub async fn get(&self, id: &str) -> Option<WebhookConfig> {
        self.webhooks.read().await.get(id).cloned()
    }

    /// List all webhooks.
    pub async fn list(&self) -> Vec<WebhookConfig> {
        self.webhooks.read().await.values().cloned().collect()
    }

    /// Update a webhook. Returns true if it existed.
    pub async fn update(&self, config: WebhookConfig) -> bool {
        let mut hooks = self.webhooks.write().await;
        if hooks.contains_key(&config.id) {
            hooks.insert(config.id.clone(), config);
            true
        } else {
            false
        }
    }

    /// Fire a webhook event. This is non-blocking — the actual HTTP call
    /// happens in the background delivery task.
    pub async fn fire(
        &self,
        event: WebhookEvent,
        tenant_id: Option<String>,
        data: serde_json::Value,
    ) {
        let payload = WebhookPayload {
            event,
            timestamp: Utc::now(),
            tenant_id,
            data,
        };

        let hooks = self.webhooks.read().await;
        for webhook in hooks.values() {
            if !webhook.active {
                continue;
            }
            // Check event filter (empty = accept all)
            if !webhook.events.is_empty() && !webhook.events.contains(&event) {
                continue;
            }
            let job = DeliveryJob {
                webhook: webhook.clone(),
                payload: payload.clone(),
            };
            if let Err(e) = self.tx.try_send(job) {
                warn!(webhook_id = %webhook.id, "Webhook delivery queue full, dropping: {}", e);
            }
        }
    }
}

/// Background loop that delivers webhooks with retry.
async fn delivery_loop(
    mut rx: mpsc::Receiver<DeliveryJob>,
    client: reqwest::Client,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            job = rx.recv() => {
                match job {
                    Some(job) => deliver_with_retry(&client, job).await,
                    None => break, // Channel closed
                }
            }
            _ = shutdown_rx.changed() => {
                info!("Webhook delivery task shutting down");
                // Drain remaining jobs
                while let Ok(job) = rx.try_recv() {
                    deliver_with_retry(&client, job).await;
                }
                break;
            }
        }
    }
    debug!("Webhook delivery loop exited");
}

/// Deliver a single webhook with exponential backoff retry.
async fn deliver_with_retry(client: &reqwest::Client, job: DeliveryJob) {
    let webhook_id = &job.webhook.id;
    let max_retries = job.webhook.max_retries as u32;

    for attempt in 0..=max_retries {
        let mut request = client
            .post(&job.webhook.url)
            .header("Content-Type", "application/json")
            .header("User-Agent", format!("ordo-webhook/{}", ordo_core::VERSION))
            .header(
                "X-Ordo-Event",
                format!("{:?}", job.payload.event).to_lowercase(),
            )
            .header("X-Ordo-Webhook-ID", webhook_id.as_str())
            .header("X-Ordo-Delivery-Attempt", (attempt + 1).to_string());

        // HMAC signature if secret is set
        if let Some(ref secret) = job.webhook.secret {
            if let Ok(body_bytes) = serde_json::to_vec(&job.payload) {
                let signature = hmac_sha256(secret.as_bytes(), &body_bytes);
                request = request.header("X-Ordo-Signature", format!("sha256={}", signature));
            }
        }

        match request.json(&job.payload).send().await {
            Ok(resp) if resp.status().is_success() => {
                debug!(
                    webhook_id = %webhook_id,
                    status = %resp.status(),
                    attempt = attempt + 1,
                    "Webhook delivered"
                );
                return;
            }
            Ok(resp) => {
                warn!(
                    webhook_id = %webhook_id,
                    status = %resp.status(),
                    attempt = attempt + 1,
                    "Webhook delivery failed"
                );
            }
            Err(e) => {
                warn!(
                    webhook_id = %webhook_id,
                    error = %e,
                    attempt = attempt + 1,
                    "Webhook delivery error"
                );
            }
        }

        if attempt < max_retries {
            let delay = Duration::from_millis(500 * 2u64.pow(attempt));
            tokio::time::sleep(delay).await;
        }
    }

    error!(
        webhook_id = %webhook_id,
        url = %job.webhook.url,
        "Webhook delivery exhausted all retries"
    );
}

/// Compute a real HMAC-SHA256 signature for webhook payload verification.
fn hmac_sha256(key: &[u8], data: &[u8]) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data);
    let result = mac.finalize();
    hex::encode(result.into_bytes())
}

/// Generate a short random ID for webhooks.
fn generate_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let id: u64 = rng.gen();
    format!("wh_{:012x}", id & 0xFFFF_FFFF_FFFF)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_id() {
        let id = generate_id();
        assert!(id.starts_with("wh_"));
        assert_eq!(id.len(), 15); // "wh_" + 12 hex chars
    }

    #[test]
    fn test_webhook_event_serialization() {
        let event = WebhookEvent::RuleCreated;
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, "\"rule_created\"");

        let event: WebhookEvent = serde_json::from_str("\"rule_updated\"").unwrap();
        assert_eq!(event, WebhookEvent::RuleUpdated);
    }

    #[test]
    fn test_webhook_config_defaults() {
        let json = r#"{"id":"test","url":"http://localhost:9999/hook","events":[]}"#;
        let config: WebhookConfig = serde_json::from_str(json).unwrap();
        assert!(config.active);
        assert_eq!(config.max_retries, 3);
        assert!(config.secret.is_none());
    }

    #[test]
    fn test_webhook_payload_serialization() {
        let payload = WebhookPayload {
            event: WebhookEvent::RuleCreated,
            timestamp: Utc::now(),
            tenant_id: Some("tenant-1".to_string()),
            data: serde_json::json!({ "name": "my-rule", "version": "1.0.0" }),
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"event\":\"rule_created\""));
        assert!(json.contains("\"tenant_id\":\"tenant-1\""));
    }

    #[tokio::test]
    async fn test_webhook_manager_crud() {
        let (_tx, rx) = tokio::sync::watch::channel(false);
        let manager = WebhookManager::new(rx);

        // Register
        let id = manager
            .register(WebhookConfig {
                id: String::new(),
                url: "http://localhost:9999/hook".to_string(),
                events: vec![WebhookEvent::RuleCreated],
                secret: None,
                active: true,
                max_retries: 3,
                description: "test".to_string(),
            })
            .await;
        assert!(id.starts_with("wh_"));

        // List
        let hooks = manager.list().await;
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0].url, "http://localhost:9999/hook");

        // Get
        let hook = manager.get(&id).await;
        assert!(hook.is_some());
        assert_eq!(hook.unwrap().url, "http://localhost:9999/hook");

        // Update
        let updated = manager
            .update(WebhookConfig {
                id: id.clone(),
                url: "http://localhost:8888/hook2".to_string(),
                events: vec![],
                secret: Some("my-secret".to_string()),
                active: true,
                max_retries: 5,
                description: "updated".to_string(),
            })
            .await;
        assert!(updated);

        let hook = manager.get(&id).await.unwrap();
        assert_eq!(hook.url, "http://localhost:8888/hook2");
        assert_eq!(hook.max_retries, 5);

        // Unregister
        assert!(manager.unregister(&id).await);
        assert!(manager.list().await.is_empty());

        // Unregister non-existent
        assert!(!manager.unregister("ghost").await);
    }

    #[tokio::test]
    async fn test_fire_filters_inactive_and_events() {
        let (_tx, rx) = tokio::sync::watch::channel(false);
        let manager = WebhookManager::new(rx);

        // Register inactive hook
        manager
            .register(WebhookConfig {
                id: "inactive".to_string(),
                url: "http://localhost:1/nope".to_string(),
                events: vec![],
                secret: None,
                active: false,
                max_retries: 0,
                description: String::new(),
            })
            .await;

        // Register hook that only listens for RuleDeleted
        manager
            .register(WebhookConfig {
                id: "delete-only".to_string(),
                url: "http://localhost:1/nope".to_string(),
                events: vec![WebhookEvent::RuleDeleted],
                secret: None,
                active: true,
                max_retries: 0,
                description: String::new(),
            })
            .await;

        // Fire RuleCreated — neither hook should match:
        // "inactive" is disabled, "delete-only" only listens for RuleDeleted.
        // This should not panic or error (delivery will fail silently).
        manager
            .fire(
                WebhookEvent::RuleCreated,
                None,
                serde_json::json!({"name": "test"}),
            )
            .await;
    }

    #[test]
    fn test_hmac_sha256_deterministic() {
        let sig1 = hmac_sha256(b"secret", b"data");
        let sig2 = hmac_sha256(b"secret", b"data");
        assert_eq!(sig1, sig2);
        // Real HMAC-SHA256 produces 64 hex chars (256 bits)
        assert_eq!(sig1.len(), 64);

        let sig3 = hmac_sha256(b"other", b"data");
        assert_ne!(sig1, sig3);
    }

    #[test]
    fn test_hmac_sha256_matches_known_vector() {
        // Known HMAC-SHA256("key", "The quick brown fox jumps over the lazy dog")
        let sig = hmac_sha256(b"key", b"The quick brown fox jumps over the lazy dog");
        assert_eq!(
            sig,
            "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
        );
    }
}
