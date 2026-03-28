//! Webhook transport layer for Life Engine.
//!
//! Implements an Axum-based inbound webhook receiver with HMAC signature
//! verification, timestamp replay protection, idempotency key deduplication,
//! and content-type validation.

pub mod config;
pub mod error;
pub mod handlers;
pub mod types;

use async_trait::async_trait;
use axum::routing::post;
use axum::Router;
use error::WebhookError;
use handlers::WebhookState;
use life_engine_traits::{EngineError, Transport};
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

#[cfg(test)]
mod tests;

/// Configuration for the webhook transport.
#[derive(Debug, Clone, Deserialize)]
pub struct WebhookTransportConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_base_path")]
    pub base_path: String,
    /// Shared secret for HMAC-SHA256 signature verification.
    /// If absent, signature verification is skipped.
    pub secret: Option<String>,
    /// Maximum idempotency keys to track (default 10000).
    #[serde(default = "default_max_seen_keys")]
    pub max_seen_keys: usize,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    3001
}

fn default_base_path() -> String {
    "/webhooks".to_string()
}

fn default_max_seen_keys() -> usize {
    10_000
}

/// Webhook transport implementation.
pub struct WebhookTransport {
    config: WebhookTransportConfig,
}

impl WebhookTransport {
    pub fn from_config(config_value: &toml::Value) -> Result<Self, Box<dyn EngineError>> {
        let config: WebhookTransportConfig =
            config_value.clone().try_into().map_err(|e: toml::de::Error| {
                Box::new(WebhookError::InvalidConfig(e.to_string())) as Box<dyn EngineError>
            })?;
        Ok(Self { config })
    }

    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.config.host, self.config.port)
    }

    /// Build the Axum router with webhook routes.
    pub fn build_router(&self, state: Arc<WebhookState>) -> Router {
        let base = &self.config.base_path;

        Router::new()
            .route(
                &format!("{base}/inbound"),
                post(handlers::handle_webhook),
            )
            .with_state(state)
    }

    /// Create a WebhookState from this transport's config.
    pub fn create_state(&self) -> Arc<WebhookState> {
        Arc::new(WebhookState {
            secret: self.config.secret.clone(),
            seen_keys: Mutex::new(HashSet::new()),
            max_seen_keys: self.config.max_seen_keys,
        })
    }
}

#[async_trait]
impl Transport for WebhookTransport {
    async fn start(&self, _config: toml::Value) -> Result<(), Box<dyn EngineError>> {
        let addr = self.bind_address();
        let state = self.create_state();
        let router = self.build_router(state);

        let listener = TcpListener::bind(&addr).await.map_err(|e| {
            Box::new(WebhookError::BindFailed(e.to_string())) as Box<dyn EngineError>
        })?;

        tracing::info!(
            transport = "webhook",
            address = %addr,
            "Webhook transport listening on {addr}",
        );

        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, router).await {
                tracing::error!(error = %e, "Webhook transport server error");
            }
        });

        Ok(())
    }

    async fn stop(&self) -> Result<(), Box<dyn EngineError>> {
        tracing::info!(transport = "webhook", "Webhook transport stopped");
        Ok(())
    }

    fn name(&self) -> &str {
        "webhook"
    }
}
