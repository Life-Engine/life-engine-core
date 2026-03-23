//! Webhook transport layer for Life Engine.

pub mod config;
pub mod error;
pub mod handlers;
pub mod types;

use async_trait::async_trait;
use error::WebhookError;
use life_engine_traits::{EngineError, Transport};
use serde::Deserialize;

#[cfg(test)]
mod tests;

/// Configuration for the webhook transport.
#[derive(Debug, Clone, Deserialize)]
pub struct WebhookTransportConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    3001
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
}

#[async_trait]
impl Transport for WebhookTransport {
    async fn start(&self, _config: toml::Value) -> Result<(), Box<dyn EngineError>> {
        tracing::info!(
            transport = "webhook",
            address = %self.bind_address(),
            "Transport webhook started on {}",
            self.bind_address()
        );
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
