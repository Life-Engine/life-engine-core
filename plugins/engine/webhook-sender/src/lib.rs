//! Webhook sender plugin for Life Engine Core.
//!
//! Subscribes to Core event bus events and dispatches webhook POST
//! requests to configured external URLs. Supports HMAC-SHA256 payload
//! signing, configurable timeouts, per-URL rate limiting with token
//! bucket, and exponential backoff retry.
//!
//! # Architecture
//!
//! - `models` — Webhook subscription and delivery record types
//! - `retry` — Exponential backoff retry state tracker
//! - `delivery` — Delivery log for tracking send attempts
//! - `config` — Configuration with timeout and retry settings

pub mod config;
pub mod delivery;
pub mod error;
pub mod models;
pub mod steps;
pub mod transform;

#[cfg(test)]
#[path = "tests/mod.rs"]
mod delivery_tests;

/// Re-export the shared retry module from the plugin SDK.
pub use life_engine_plugin_sdk::retry;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use async_trait::async_trait;
use life_engine_plugin_sdk::prelude::*;
use life_engine_plugin_sdk::retry::RetryState;
use life_engine_plugin_sdk::types::Capability;
use tokio::sync::Mutex;

use crate::config::WebhookSenderConfig;
use crate::delivery::DeliveryLog;
use crate::models::{DeliveryRecord, WebhookSubscription};

/// Per-URL token bucket rate limiter.
///
/// Each URL gets its own bucket with a configurable rate (tokens per second)
/// and burst capacity. Tokens are refilled lazily on each `acquire` call.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    buckets: Arc<Mutex<HashMap<String, TokenBucket>>>,
    tokens_per_sec: f64,
    burst: u32,
}

#[derive(Debug, Clone)]
struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
}

impl RateLimiter {
    /// Create a new rate limiter.
    ///
    /// - `tokens_per_sec` — steady-state rate (e.g. 10.0 = 10 requests/sec)
    /// - `burst` — maximum tokens that can accumulate
    pub fn new(tokens_per_sec: f64, burst: u32) -> Self {
        Self {
            buckets: Arc::new(Mutex::new(HashMap::new())),
            tokens_per_sec,
            burst,
        }
    }

    /// Try to acquire a token for the given URL.
    /// Returns `true` if a token was available, `false` if rate-limited.
    pub async fn acquire(&self, url: &str) -> bool {
        let mut buckets = self.buckets.lock().await;
        let now = Instant::now();
        let bucket = buckets.entry(url.to_string()).or_insert(TokenBucket {
            tokens: self.burst as f64,
            last_refill: now,
        });

        // Refill tokens based on elapsed time
        let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * self.tokens_per_sec).min(self.burst as f64);
        bucket.last_refill = now;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Sign a payload with HMAC-SHA256 and return the `sha256=<hex>` signature string.
pub fn sign_payload(secret: &str, body: &[u8]) -> String {
    use hmac::{Hmac, Mac};
    type HmacSha256 = Hmac<sha2::Sha256>;

    let mut mac =
        <HmacSha256 as Mac>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key size");
    mac.update(body);
    format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
}

/// Deliver a webhook payload to a single subscription URL.
///
/// Performs an HTTP POST with optional HMAC-SHA256 signing, respecting
/// the configured connect and request timeouts. Returns the HTTP status
/// code on success or an error string on failure.
pub async fn deliver(
    client: &reqwest::Client,
    sub: &WebhookSubscription,
    body: &[u8],
) -> std::result::Result<u16, String> {
    let mut request = client
        .post(&sub.url)
        .header("Content-Type", "application/json")
        .header("User-Agent", "LifeEngine-Webhook/1.0");

    if let Some(ref secret) = sub.secret {
        let sig = sign_payload(secret, body);
        request = request.header("X-Webhook-Signature", sig);
    }

    match request.body(body.to_vec()).send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            if resp.status().is_success() {
                Ok(status)
            } else {
                Err(format!("HTTP {status}"))
            }
        }
        Err(e) => Err(e.to_string()),
    }
}

/// The webhook sender plugin.
///
/// Manages webhook subscriptions and dispatches events to external
/// URLs when matching events occur on the Core event bus.
pub struct WebhookSenderPlugin {
    /// Active webhook subscriptions.
    subscriptions: Vec<WebhookSubscription>,
    /// Delivery attempt log.
    delivery_log: DeliveryLog,
    /// Retry state per subscription (keyed by index for simplicity).
    retry_states: Vec<RetryState>,
    /// Plugin configuration.
    config: WebhookSenderConfig,
}

impl WebhookSenderPlugin {
    pub fn new() -> Self {
        Self {
            subscriptions: Vec::new(),
            delivery_log: DeliveryLog::new(),
            retry_states: Vec::new(),
            config: WebhookSenderConfig::default(),
        }
    }

    /// Create a plugin with custom configuration.
    pub fn with_config(config: WebhookSenderConfig) -> Self {
        Self {
            subscriptions: Vec::new(),
            delivery_log: DeliveryLog::new(),
            retry_states: Vec::new(),
            config,
        }
    }

    /// Add a new webhook subscription.
    pub fn subscribe(&mut self, subscription: WebhookSubscription) {
        self.subscriptions.push(subscription);
        self.retry_states.push(RetryState::new());
    }

    /// Remove a subscription by ID.
    pub fn unsubscribe(&mut self, id: &str) -> bool {
        if let Some(pos) = self.subscriptions.iter().position(|s| s.id == id) {
            self.subscriptions.remove(pos);
            self.retry_states.remove(pos);
            true
        } else {
            false
        }
    }

    /// Returns the active subscriptions.
    pub fn subscriptions(&self) -> &[WebhookSubscription] {
        &self.subscriptions
    }

    /// Find a subscription by ID.
    pub fn find_subscription(&self, id: &str) -> Option<&WebhookSubscription> {
        self.subscriptions.iter().find(|s| s.id == id)
    }

    /// Returns the delivery log.
    pub fn delivery_log(&self) -> &DeliveryLog {
        &self.delivery_log
    }

    /// Check if an event type matches any active subscriptions.
    pub fn matching_subscriptions(&self, event_type: &str) -> Vec<&WebhookSubscription> {
        self.subscriptions
            .iter()
            .filter(|s| s.active && s.event_types.iter().any(|t| t == event_type))
            .collect()
    }

    /// Record a successful delivery.
    pub fn record_delivery_success(
        &mut self,
        subscription_id: &str,
        event_type: &str,
        payload: &serde_json::Value,
        status_code: u16,
        attempt: u32,
    ) {
        let record = DeliveryRecord::success(
            uuid::Uuid::new_v4().to_string(),
            subscription_id.to_string(),
            event_type.to_string(),
            payload,
            status_code,
            attempt,
        );
        self.delivery_log.record(record);

        if let Some(pos) = self.subscriptions.iter().position(|s| s.id == subscription_id) {
            self.retry_states[pos].record_success();
        }
    }

    /// Record a failed delivery and return whether retry is allowed.
    pub fn record_delivery_failure(
        &mut self,
        subscription_id: &str,
        event_type: &str,
        payload: &serde_json::Value,
        status_code: u16,
        attempt: u32,
        error: &str,
    ) -> bool {
        let record = DeliveryRecord::failure(
            uuid::Uuid::new_v4().to_string(),
            subscription_id.to_string(),
            event_type.to_string(),
            payload,
            status_code,
            attempt,
            error.to_string(),
        );
        self.delivery_log.record(record);

        if let Some(pos) = self.subscriptions.iter().position(|s| s.id == subscription_id) {
            self.retry_states[pos].record_failure();
            self.retry_states[pos].can_retry()
        } else {
            false
        }
    }

    /// Get the retry state for a subscription.
    pub fn retry_state(&self, subscription_id: &str) -> Option<&RetryState> {
        self.subscriptions
            .iter()
            .position(|s| s.id == subscription_id)
            .map(|pos| &self.retry_states[pos])
    }
}

impl Default for WebhookSenderPlugin {
    fn default() -> Self {
        Self::new()
    }
}

/// Return a plugin config suitable for tests: very short timeouts, no retries.
#[cfg(test)]
fn test_config() -> WebhookSenderConfig {
    WebhookSenderConfig {
        max_retries: 0,
        max_delivery_log_size: 100,
        connect_timeout_secs: 1,
        request_timeout_secs: 1,
        total_timeout_secs: 2,
    }
}

impl Plugin for WebhookSenderPlugin {
    fn id(&self) -> &str {
        "com.life-engine.webhook-sender"
    }

    fn display_name(&self) -> &str {
        "Webhook Sender"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn actions(&self) -> Vec<Action> {
        vec![
            Action::new("subscribe", "Create a new webhook subscription"),
            Action::new("unsubscribe", "Remove a webhook subscription"),
            Action::new("subscriptions", "List active webhook subscriptions"),
            Action::new("deliveries", "List webhook delivery history"),
        ]
    }

    fn execute(
        &self,
        action: &str,
        input: PipelineMessage,
    ) -> std::result::Result<PipelineMessage, Box<dyn EngineError>> {
        match action {
            "subscribe" | "unsubscribe" | "subscriptions" | "deliveries" => Ok(input),
            other => Err(Box::new(
                crate::error::WebhookSenderError::UnknownAction(other.to_string()),
            )),
        }
    }
}

life_engine_plugin_sdk::register_plugin!(WebhookSenderPlugin);

#[async_trait]
impl CorePlugin for WebhookSenderPlugin {
    fn id(&self) -> &str {
        "com.life-engine.webhook-sender"
    }

    fn display_name(&self) -> &str {
        "Webhook Sender"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![
            Capability::StorageRead,
            Capability::StorageWrite,
            Capability::HttpOutbound,
            Capability::EventsSubscribe,
            Capability::Logging,
        ]
    }

    async fn on_load(&mut self, ctx: &PluginContext) -> Result<()> {
        tracing::info!(
            plugin_id = ctx.plugin_id(),
            "webhook sender plugin loaded"
        );
        Ok(())
    }

    async fn on_unload(&mut self) -> Result<()> {
        self.subscriptions.clear();
        self.retry_states.clear();
        tracing::info!("webhook sender plugin unloaded");
        Ok(())
    }

    fn routes(&self) -> Vec<PluginRoute> {
        vec![
            PluginRoute {
                method: HttpMethod::Post,
                path: "/subscribe".into(),
            },
            PluginRoute {
                method: HttpMethod::Delete,
                path: "/subscribe/{id}".into(),
            },
            PluginRoute {
                method: HttpMethod::Get,
                path: "/subscriptions".into(),
            },
            PluginRoute {
                method: HttpMethod::Get,
                path: "/deliveries".into(),
            },
        ]
    }

    async fn handle_event(&self, event: &CoreEvent) -> Result<()> {
        let matching = self.matching_subscriptions(&event.event_type);
        if matching.is_empty() {
            return Ok(());
        }

        tracing::info!(
            event_type = %event.event_type,
            subscription_count = matching.len(),
            "webhook sender matched event to subscriptions"
        );

        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(self.config.connect_timeout_secs))
            .timeout(Duration::from_secs(self.config.request_timeout_secs))
            .build()
            .unwrap_or_default();

        let payload = serde_json::json!({
            "event_type": event.event_type,
            "payload": event.payload,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        let body = serde_json::to_vec(&payload)?;

        let rate_limiter = RateLimiter::new(10.0, 20);
        let total_deadline = Instant::now() + Duration::from_secs(self.config.total_timeout_secs);

        for sub in &matching {
            // Rate limit check
            if !rate_limiter.acquire(&sub.url).await {
                tracing::warn!(
                    subscription_id = %sub.id,
                    url = %sub.url,
                    "webhook delivery rate-limited, skipping"
                );
                continue;
            }

            let mut attempt = 0u32;
            let mut retry_state = RetryState::with_config(
                self.config.max_retries,
                1, // 1-second minimum backoff for webhook delivery
                60, // 60-second maximum backoff
            );

            loop {
                attempt += 1;

                if Instant::now() >= total_deadline {
                    tracing::error!(
                        subscription_id = %sub.id,
                        url = %sub.url,
                        "webhook delivery exceeded total timeout"
                    );
                    break;
                }

                match deliver(&client, sub, &body).await {
                    Ok(status) => {
                        tracing::info!(
                            subscription_id = %sub.id,
                            url = %sub.url,
                            status = status,
                            attempt = attempt,
                            "webhook delivered successfully"
                        );
                        break;
                    }
                    Err(err) => {
                        let backoff = retry_state.record_failure();
                        if retry_state.can_retry() {
                            tracing::warn!(
                                subscription_id = %sub.id,
                                url = %sub.url,
                                attempt = attempt,
                                backoff_ms = backoff.as_millis() as u64,
                                error = %err,
                                "webhook delivery failed, will retry"
                            );
                            tokio::time::sleep(backoff).await;
                        } else {
                            tracing::error!(
                                subscription_id = %sub.id,
                                url = %sub.url,
                                attempt = attempt,
                                error = %err,
                                "webhook delivery failed, retries exhausted"
                            );
                            break;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use life_engine_plugin_sdk::types::PluginContext;

    fn test_subscription() -> WebhookSubscription {
        WebhookSubscription {
            id: "sub-1".to_string(),
            url: "https://example.com/webhook".to_string(),
            event_types: vec![
                "record.created".to_string(),
                "sync.complete".to_string(),
            ],
            secret: Some("webhook-secret".to_string()),
            active: true,
        }
    }

    fn test_subscription_2() -> WebhookSubscription {
        WebhookSubscription {
            id: "sub-2".to_string(),
            url: "https://other.com/hook".to_string(),
            event_types: vec!["record.created".to_string()],
            secret: None,
            active: true,
        }
    }

    fn inactive_subscription() -> WebhookSubscription {
        WebhookSubscription {
            id: "sub-inactive".to_string(),
            url: "https://inactive.com/hook".to_string(),
            event_types: vec!["record.created".to_string()],
            secret: None,
            active: false,
        }
    }

    // --- Plugin metadata tests ---

    #[test]
    fn plugin_id() {
        let plugin = WebhookSenderPlugin::new();
        assert_eq!(CorePlugin::id(&plugin), "com.life-engine.webhook-sender");
    }

    #[test]
    fn plugin_display_name() {
        let plugin = WebhookSenderPlugin::new();
        assert_eq!(CorePlugin::display_name(&plugin), "Webhook Sender");
    }

    #[test]
    fn plugin_version() {
        let plugin = WebhookSenderPlugin::new();
        assert_eq!(CorePlugin::version(&plugin), "0.1.0");
    }

    #[test]
    fn plugin_capabilities() {
        use life_engine_test_utils::assert_plugin_capabilities;
        let plugin = WebhookSenderPlugin::new();
        assert_plugin_capabilities!(plugin, [
            Capability::StorageRead,
            Capability::StorageWrite,
            Capability::HttpOutbound,
            Capability::EventsSubscribe,
            Capability::Logging,
        ]);
    }

    #[test]
    fn plugin_routes() {
        use life_engine_test_utils::assert_plugin_routes;
        let plugin = WebhookSenderPlugin::new();
        assert_plugin_routes!(plugin, [
            "/subscribe",
            "/subscribe/{id}",
            "/subscriptions",
            "/deliveries",
        ]);
    }

    #[tokio::test]
    async fn plugin_lifecycle() {
        let mut plugin = WebhookSenderPlugin::new();
        plugin.subscribe(test_subscription());
        assert_eq!(plugin.subscriptions().len(), 1);

        let ctx = PluginContext::new(CorePlugin::id(&plugin));
        plugin.on_load(&ctx).await.expect("on_load should succeed");
        plugin.on_unload().await.expect("on_unload should succeed");
        assert!(plugin.subscriptions().is_empty());
    }

    #[tokio::test]
    async fn handle_event_returns_ok() {
        let plugin = WebhookSenderPlugin::new();
        life_engine_test_utils::plugin_test_helpers::test_handle_event_ok(&plugin).await;
    }

    #[test]
    fn default_impl() {
        let plugin = WebhookSenderPlugin::default();
        assert_eq!(CorePlugin::id(&plugin), "com.life-engine.webhook-sender");
    }

    // --- Subscription management tests ---

    #[test]
    fn subscribe_and_find() {
        let mut plugin = WebhookSenderPlugin::new();
        assert!(plugin.find_subscription("sub-1").is_none());

        plugin.subscribe(test_subscription());
        let found = plugin.find_subscription("sub-1");
        assert!(found.is_some());
        assert_eq!(found.unwrap().url, "https://example.com/webhook");
    }

    #[test]
    fn unsubscribe() {
        let mut plugin = WebhookSenderPlugin::new();
        plugin.subscribe(test_subscription());
        assert_eq!(plugin.subscriptions().len(), 1);

        assert!(plugin.unsubscribe("sub-1"));
        assert!(plugin.subscriptions().is_empty());

        // Unsubscribing non-existent returns false
        assert!(!plugin.unsubscribe("sub-1"));
    }

    // --- Event matching tests ---

    #[test]
    fn matching_subscriptions_finds_matches() {
        let mut plugin = WebhookSenderPlugin::new();
        plugin.subscribe(test_subscription());
        plugin.subscribe(test_subscription_2());

        let matches = plugin.matching_subscriptions("record.created");
        assert_eq!(matches.len(), 2);

        let matches = plugin.matching_subscriptions("sync.complete");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, "sub-1");
    }

    #[test]
    fn matching_subscriptions_excludes_inactive() {
        let mut plugin = WebhookSenderPlugin::new();
        plugin.subscribe(test_subscription());
        plugin.subscribe(inactive_subscription());

        let matches = plugin.matching_subscriptions("record.created");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, "sub-1");
    }

    #[test]
    fn matching_subscriptions_returns_empty_for_no_match() {
        let mut plugin = WebhookSenderPlugin::new();
        plugin.subscribe(test_subscription());

        let matches = plugin.matching_subscriptions("unknown.event");
        assert!(matches.is_empty());
    }

    // --- Delivery tracking tests ---

    #[test]
    fn delivery_success_recorded() {
        let mut plugin = WebhookSenderPlugin::new();
        plugin.subscribe(test_subscription());

        plugin.record_delivery_success(
            "sub-1",
            "record.created",
            &serde_json::json!({"id": "123"}),
            200,
            1,
        );

        assert_eq!(plugin.delivery_log().len(), 1);
        assert_eq!(plugin.delivery_log().success_count(), 1);
        assert_eq!(plugin.delivery_log().failure_count(), 0);

        let records = plugin.delivery_log().all();
        assert_eq!(records[0].status_code, 200);
        assert!(records[0].success);
    }

    #[test]
    fn delivery_failure_recorded_with_status_code() {
        let mut plugin = WebhookSenderPlugin::new();
        plugin.subscribe(test_subscription());

        let can_retry = plugin.record_delivery_failure(
            "sub-1",
            "record.created",
            &serde_json::json!({"id": "456"}),
            500,
            1,
            "Internal Server Error",
        );

        assert!(can_retry);
        assert_eq!(plugin.delivery_log().len(), 1);
        assert_eq!(plugin.delivery_log().failure_count(), 1);

        let records = plugin.delivery_log().all();
        assert_eq!(records[0].status_code, 500);
        assert!(!records[0].success);
        assert_eq!(records[0].error.as_deref(), Some("Internal Server Error"));
    }

    #[test]
    fn delivery_retry_exhaustion() {
        let mut plugin = WebhookSenderPlugin::new();
        plugin.subscribe(test_subscription());

        // Exhaust all 5 retries
        for attempt in 1..=5 {
            let can_retry = plugin.record_delivery_failure(
                "sub-1",
                "record.created",
                &serde_json::json!({}),
                503,
                attempt,
                "Service Unavailable",
            );

            if attempt < 5 {
                assert!(can_retry, "should allow retry on attempt {}", attempt);
            } else {
                assert!(!can_retry, "should not allow retry on attempt {}", attempt);
            }
        }

        assert_eq!(plugin.delivery_log().len(), 5);
        assert_eq!(plugin.delivery_log().failure_count(), 5);

        let retry = plugin.retry_state("sub-1").unwrap();
        assert!(retry.exhausted());
    }

    #[test]
    fn delivery_success_resets_retry_state() {
        let mut plugin = WebhookSenderPlugin::new();
        plugin.subscribe(test_subscription());

        // Fail twice
        plugin.record_delivery_failure("sub-1", "record.created", &serde_json::json!({}), 500, 1, "err");
        plugin.record_delivery_failure("sub-1", "record.created", &serde_json::json!({}), 500, 2, "err");

        let retry = plugin.retry_state("sub-1").unwrap();
        assert_eq!(retry.failure_count(), 2);

        // Succeed
        plugin.record_delivery_success("sub-1", "record.created", &serde_json::json!({}), 200, 3);

        let retry = plugin.retry_state("sub-1").unwrap();
        assert_eq!(retry.failure_count(), 0);
    }

    #[test]
    fn delivery_log_tracks_multiple_status_codes() {
        let mut plugin = WebhookSenderPlugin::new();
        plugin.subscribe(test_subscription());

        plugin.record_delivery_success("sub-1", "e", &serde_json::json!({}), 200, 1);
        plugin.record_delivery_failure("sub-1", "e", &serde_json::json!({}), 500, 1, "err");
        plugin.record_delivery_failure("sub-1", "e", &serde_json::json!({}), 502, 2, "err");
        plugin.record_delivery_success("sub-1", "e", &serde_json::json!({}), 201, 1);

        let records = plugin.delivery_log().all();
        let codes: Vec<u16> = records.iter().map(|r| r.status_code).collect();
        assert_eq!(codes, vec![200, 500, 502, 201]);
    }

    // --- handle_event with subscriptions ---

    #[tokio::test]
    async fn handle_event_with_matching_subscription() {
        let mut plugin = WebhookSenderPlugin::with_config(test_config());
        plugin.subscribe(test_subscription());

        let event = CoreEvent {
            event_type: "record.created".to_string(),
            payload: serde_json::json!({"id": "rec-1"}),
            source_plugin: "com.life-engine.connector-email".to_string(),
            timestamp: chrono::Utc::now(),
        };

        // Delivery will fail (no real server) but handle_event should still return Ok
        let result = plugin.handle_event(&event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn handle_event_with_no_matching_subscription() {
        let mut plugin = WebhookSenderPlugin::new();
        plugin.subscribe(test_subscription());

        let event = CoreEvent {
            event_type: "unknown.event".to_string(),
            payload: serde_json::json!({}),
            source_plugin: "com.test".to_string(),
            timestamp: chrono::Utc::now(),
        };

        let result = plugin.handle_event(&event).await;
        assert!(result.is_ok());
    }

    // --- WASM Plugin trait tests ---

    #[test]
    fn wasm_plugin_id_matches_core() {
        let plugin = WebhookSenderPlugin::new();
        assert_eq!(Plugin::id(&plugin), CorePlugin::id(&plugin));
    }

    #[test]
    fn wasm_plugin_actions() {
        let plugin = WebhookSenderPlugin::new();
        let actions = Plugin::actions(&plugin);
        let names: Vec<&str> = actions.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, vec!["subscribe", "unsubscribe", "subscriptions", "deliveries"]);
    }

    #[test]
    fn wasm_plugin_execute_known_action() {
        let plugin = WebhookSenderPlugin::new();
        let msg = PipelineMessage {
            metadata: MessageMetadata {
                correlation_id: uuid::Uuid::new_v4(),
                source: "test".into(),
                timestamp: chrono::Utc::now(),
                auth_context: None,
                warnings: vec![],
            },
            payload: TypedPayload::Cdm(Box::new(CdmType::Task(life_engine_plugin_sdk::Task {
                    id: uuid::Uuid::new_v4(),
                    title: "test".into(),
                    description: None,
                    status: life_engine_plugin_sdk::TaskStatus::Pending,
                    priority: life_engine_plugin_sdk::TaskPriority::Medium,
                    due_date: None,
                    completed_at: None,
                    tags: vec![],
                    assignee: None,
                    parent_id: None,
                    source: "test".into(),
                    source_id: "t-1".into(),
                    extensions: None,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                }))),
        };
        let result = Plugin::execute(&plugin, "subscribe", msg);
        assert!(result.is_ok());
    }

    #[test]
    fn wasm_plugin_execute_unknown_action() {
        let plugin = WebhookSenderPlugin::new();
        let msg = PipelineMessage {
            metadata: MessageMetadata {
                correlation_id: uuid::Uuid::new_v4(),
                source: "test".into(),
                timestamp: chrono::Utc::now(),
                auth_context: None,
                warnings: vec![],
            },
            payload: TypedPayload::Cdm(Box::new(CdmType::Task(life_engine_plugin_sdk::Task {
                    id: uuid::Uuid::new_v4(),
                    title: "test".into(),
                    description: None,
                    status: life_engine_plugin_sdk::TaskStatus::Pending,
                    priority: life_engine_plugin_sdk::TaskPriority::Medium,
                    due_date: None,
                    completed_at: None,
                    tags: vec![],
                    assignee: None,
                    parent_id: None,
                    source: "test".into(),
                    source_id: "t-1".into(),
                    extensions: None,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                }))),
        };
        let result = Plugin::execute(&plugin, "nonexistent", msg);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), "WEBHOOK_004");
    }
}
