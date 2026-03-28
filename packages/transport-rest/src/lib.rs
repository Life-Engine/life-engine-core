//! REST/HTTP transport layer for Life Engine.

pub mod config;
pub mod error;
pub mod handlers;
pub mod listener;
pub mod middleware;
pub mod router;
pub mod types;

use async_trait::async_trait;
use error::RestError;
use life_engine_traits::{EngineError, Transport};
use serde::Deserialize;

#[cfg(test)]
mod tests;

/// Configuration for the REST transport, parsed from the `[transports.rest]`
/// TOML section.
#[derive(Debug, Clone, Deserialize)]
pub struct RestTransportConfig {
    /// Address to bind to.
    #[serde(default = "default_host")]
    pub host: String,
    /// Port to listen on.
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    3000
}

/// REST transport implementation.
///
/// In the current architecture, the REST transport's axum router is built
/// directly in the Core binary. This struct serves as the Transport trait
/// adapter, allowing the startup orchestrator to manage it uniformly
/// alongside other transports.
pub struct RestTransport {
    config: RestTransportConfig,
}

impl RestTransport {
    /// Create a new REST transport from a TOML config value.
    pub fn from_config(config_value: &toml::Value) -> Result<Self, Box<dyn EngineError>> {
        let config: RestTransportConfig =
            config_value.clone().try_into().map_err(|e: toml::de::Error| {
                Box::new(RestError::InvalidConfig(e.to_string())) as Box<dyn EngineError>
            })?;
        Ok(Self { config })
    }

    /// Returns the bind address string.
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.config.host, self.config.port)
    }

    /// Returns the parsed config.
    pub fn config(&self) -> &RestTransportConfig {
        &self.config
    }
}

#[async_trait]
impl Transport for RestTransport {
    async fn start(&self, _config: toml::Value) -> Result<(), Box<dyn EngineError>> {
        tracing::info!(
            transport = "rest",
            address = %self.bind_address(),
            "Transport rest started on {}",
            self.bind_address()
        );
        Ok(())
    }

    async fn stop(&self) -> Result<(), Box<dyn EngineError>> {
        tracing::info!(transport = "rest", "REST transport stopped");
        Ok(())
    }

    fn name(&self) -> &str {
        "rest"
    }
}
