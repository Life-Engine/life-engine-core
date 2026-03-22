//! Webhook receiver plugin for Life Engine Core.
//!
//! Accepts incoming webhooks from external services, validates HMAC-SHA256
//! signatures, maps payloads to CDM fields via configurable JSON path mappings,
//! and stores results in Core's storage.
//!
//! # Architecture
//!
//! - `signature` — HMAC-SHA256 signature verification
//! - `mapping` — Configurable JSON path to CDM field mapping
//! - `models` — Webhook endpoint configuration and received event types

pub mod mapping;
pub mod models;
pub mod signature;

use anyhow::Result;
use async_trait::async_trait;
use life_engine_plugin_sdk::prelude::*;
use life_engine_plugin_sdk::types::Capability;

use crate::models::{WebhookEndpoint, WebhookReceivedEvent};
use crate::signature::verify_hmac_sha256;

/// The webhook receiver plugin.
///
/// Manages configurable webhook endpoints that accept incoming HTTP POST
/// requests from external services. Each endpoint can optionally verify
/// HMAC-SHA256 signatures and apply payload mappings to transform incoming
/// data into CDM-compatible records.
pub struct WebhookReceiverPlugin {
    /// Configured webhook endpoints, keyed by endpoint ID.
    endpoints: Vec<WebhookEndpoint>,
}

impl WebhookReceiverPlugin {
    pub fn new() -> Self {
        Self {
            endpoints: Vec::new(),
        }
    }

    /// Register a new webhook endpoint configuration.
    pub fn register_endpoint(&mut self, endpoint: WebhookEndpoint) {
        self.endpoints.push(endpoint);
    }

    /// Returns the configured endpoints.
    pub fn endpoints(&self) -> &[WebhookEndpoint] {
        &self.endpoints
    }

    /// Find an endpoint by its ID.
    pub fn find_endpoint(&self, id: &str) -> Option<&WebhookEndpoint> {
        self.endpoints.iter().find(|e| e.id == id)
    }

    /// Process an incoming webhook request.
    ///
    /// 1. Looks up the endpoint configuration by ID
    /// 2. Verifies the HMAC-SHA256 signature if a secret is configured
    /// 3. Applies payload mapping to extract CDM fields
    /// 4. Returns the processed event
    pub fn process_webhook(
        &self,
        endpoint_id: &str,
        signature_header: Option<&str>,
        raw_body: &[u8],
        body: serde_json::Value,
    ) -> Result<WebhookReceivedEvent> {
        let endpoint = self
            .find_endpoint(endpoint_id)
            .ok_or_else(|| anyhow::anyhow!("unknown webhook endpoint: {}", endpoint_id))?;

        // Verify HMAC signature if secret is configured
        if let Some(secret) = &endpoint.secret {
            let sig = signature_header
                .ok_or_else(|| anyhow::anyhow!("missing signature header"))?;
            verify_hmac_sha256(secret.as_bytes(), raw_body, sig)?;
        }

        // Apply payload mapping
        let mapped_data = if let Some(ref mappings) = endpoint.payload_mappings {
            mapping::apply_mappings(&body, mappings)
        } else {
            body.clone()
        };

        Ok(WebhookReceivedEvent {
            endpoint_id: endpoint_id.to_string(),
            source: endpoint.source_name.clone(),
            collection: endpoint.target_collection.clone(),
            data: mapped_data,
            timestamp: chrono::Utc::now(),
        })
    }
}

impl Default for WebhookReceiverPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CorePlugin for WebhookReceiverPlugin {
    fn id(&self) -> &str {
        "com.life-engine.webhook-receiver"
    }

    fn display_name(&self) -> &str {
        "Webhook Receiver"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![
            Capability::StorageRead,
            Capability::StorageWrite,
            Capability::EventsEmit,
            Capability::Logging,
        ]
    }

    async fn on_load(&mut self, ctx: &PluginContext) -> Result<()> {
        tracing::info!(
            plugin_id = ctx.plugin_id(),
            "webhook receiver plugin loaded"
        );
        Ok(())
    }

    async fn on_unload(&mut self) -> Result<()> {
        self.endpoints.clear();
        tracing::info!("webhook receiver plugin unloaded");
        Ok(())
    }

    fn routes(&self) -> Vec<PluginRoute> {
        vec![
            PluginRoute {
                method: HttpMethod::Post,
                path: "/receive/{id}".into(),
            },
            PluginRoute {
                method: HttpMethod::Post,
                path: "/endpoints".into(),
            },
            PluginRoute {
                method: HttpMethod::Get,
                path: "/endpoints".into(),
            },
            PluginRoute {
                method: HttpMethod::Delete,
                path: "/endpoints/{id}".into(),
            },
        ]
    }

    async fn handle_event(&self, _event: &CoreEvent) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{PayloadMapping, WebhookEndpoint};
    use life_engine_plugin_sdk::types::PluginContext;

    fn test_endpoint() -> WebhookEndpoint {
        WebhookEndpoint {
            id: "github-push".to_string(),
            source_name: "GitHub".to_string(),
            target_collection: "webhooks".to_string(),
            secret: Some("test-secret-key".to_string()),
            payload_mappings: Some(vec![
                PayloadMapping {
                    source_path: "repository.full_name".to_string(),
                    target_field: "repo".to_string(),
                },
                PayloadMapping {
                    source_path: "ref".to_string(),
                    target_field: "branch".to_string(),
                },
            ]),
        }
    }

    fn test_endpoint_no_secret() -> WebhookEndpoint {
        WebhookEndpoint {
            id: "simple-hook".to_string(),
            source_name: "External".to_string(),
            target_collection: "events".to_string(),
            secret: None,
            payload_mappings: None,
        }
    }

    // --- Plugin metadata tests ---

    #[test]
    fn plugin_id() {
        let plugin = WebhookReceiverPlugin::new();
        assert_eq!(plugin.id(), "com.life-engine.webhook-receiver");
    }

    #[test]
    fn plugin_display_name() {
        let plugin = WebhookReceiverPlugin::new();
        assert_eq!(plugin.display_name(), "Webhook Receiver");
    }

    #[test]
    fn plugin_version() {
        let plugin = WebhookReceiverPlugin::new();
        assert_eq!(plugin.version(), "0.1.0");
    }

    #[test]
    fn plugin_capabilities() {
        use life_engine_test_utils::assert_plugin_capabilities;
        let plugin = WebhookReceiverPlugin::new();
        assert_plugin_capabilities!(plugin, [
            Capability::StorageRead,
            Capability::StorageWrite,
            Capability::EventsEmit,
            Capability::Logging,
        ]);
    }

    #[test]
    fn plugin_routes() {
        use life_engine_test_utils::assert_plugin_routes;
        let plugin = WebhookReceiverPlugin::new();
        assert_plugin_routes!(plugin, [
            "/receive/{id}",
            "/endpoints",
            "/endpoints",
            "/endpoints/{id}",
        ]);
    }

    #[tokio::test]
    async fn plugin_lifecycle() {
        let mut plugin = WebhookReceiverPlugin::new();
        plugin.register_endpoint(test_endpoint());
        assert_eq!(plugin.endpoints().len(), 1);

        let ctx = PluginContext::new(plugin.id());
        plugin.on_load(&ctx).await.expect("on_load should succeed");
        plugin.on_unload().await.expect("on_unload should succeed");
        assert!(plugin.endpoints().is_empty());
    }

    #[tokio::test]
    async fn handle_event_returns_ok() {
        let plugin = WebhookReceiverPlugin::new();
        life_engine_test_utils::plugin_test_helpers::test_handle_event_ok(&plugin).await;
    }

    #[test]
    fn default_impl() {
        let plugin = WebhookReceiverPlugin::default();
        assert_eq!(plugin.id(), "com.life-engine.webhook-receiver");
    }

    // --- Webhook receiver accepts and maps payload ---

    #[test]
    fn process_webhook_accepts_valid_payload_without_secret() {
        let mut plugin = WebhookReceiverPlugin::new();
        plugin.register_endpoint(test_endpoint_no_secret());

        let body = serde_json::json!({
            "action": "triggered",
            "data": { "key": "value" }
        });

        let result = plugin.process_webhook(
            "simple-hook",
            None,
            b"{}",
            body.clone(),
        );

        assert!(result.is_ok());
        let event = result.unwrap();
        assert_eq!(event.endpoint_id, "simple-hook");
        assert_eq!(event.source, "External");
        assert_eq!(event.collection, "events");
        assert_eq!(event.data, body);
    }

    #[test]
    fn process_webhook_applies_payload_mapping() {
        let mut plugin = WebhookReceiverPlugin::new();
        plugin.register_endpoint(test_endpoint());

        let body = serde_json::json!({
            "repository": {
                "full_name": "life-engine-org/life-engine"
            },
            "ref": "refs/heads/main",
            "sender": { "login": "user1" }
        });
        let raw = serde_json::to_vec(&body).unwrap();

        // Generate valid HMAC signature
        let sig = crate::signature::compute_hmac_sha256(
            b"test-secret-key",
            &raw,
        );

        let result = plugin.process_webhook(
            "github-push",
            Some(&sig),
            &raw,
            body,
        );

        assert!(result.is_ok());
        let event = result.unwrap();
        assert_eq!(event.data["repo"], "life-engine-org/life-engine");
        assert_eq!(event.data["branch"], "refs/heads/main");
    }

    #[test]
    fn process_webhook_rejects_unknown_endpoint() {
        let plugin = WebhookReceiverPlugin::new();
        let result = plugin.process_webhook(
            "nonexistent",
            None,
            b"{}",
            serde_json::json!({}),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unknown webhook endpoint"));
    }

    #[test]
    fn process_webhook_rejects_missing_signature() {
        let mut plugin = WebhookReceiverPlugin::new();
        plugin.register_endpoint(test_endpoint());

        let result = plugin.process_webhook(
            "github-push",
            None, // Missing signature
            b"{}",
            serde_json::json!({}),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing signature header"));
    }

    #[test]
    fn process_webhook_rejects_invalid_signature() {
        let mut plugin = WebhookReceiverPlugin::new();
        plugin.register_endpoint(test_endpoint());

        let result = plugin.process_webhook(
            "github-push",
            Some("sha256=invalid_signature"),
            b"some body",
            serde_json::json!({}),
        );
        assert!(result.is_err());
    }

    #[test]
    fn register_and_find_endpoint() {
        let mut plugin = WebhookReceiverPlugin::new();
        assert!(plugin.find_endpoint("github-push").is_none());

        plugin.register_endpoint(test_endpoint());
        let found = plugin.find_endpoint("github-push");
        assert!(found.is_some());
        assert_eq!(found.unwrap().source_name, "GitHub");
    }

    #[test]
    fn process_webhook_preserves_full_body_without_mappings() {
        let mut plugin = WebhookReceiverPlugin::new();
        plugin.register_endpoint(test_endpoint_no_secret());

        let body = serde_json::json!({
            "deeply": { "nested": { "value": 42 } },
            "array": [1, 2, 3]
        });

        let result = plugin.process_webhook("simple-hook", None, b"{}", body.clone());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().data, body);
    }
}
