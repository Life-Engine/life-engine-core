//! CardDAV transport layer for Life Engine.

pub mod config;
pub mod error;
pub mod handlers;
pub mod types;

use async_trait::async_trait;
use error::CarddavError;
use life_engine_traits::{EngineError, Transport};
use serde::Deserialize;

#[cfg(test)]
mod tests;

/// Configuration for the CardDAV transport.
#[derive(Debug, Clone, Deserialize)]
pub struct CarddavTransportConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    5233
}

/// CardDAV transport implementation.
pub struct CarddavTransport {
    config: CarddavTransportConfig,
}

impl CarddavTransport {
    pub fn from_config(config_value: &toml::Value) -> Result<Self, Box<dyn EngineError>> {
        let config: CarddavTransportConfig =
            config_value.clone().try_into().map_err(|e: toml::de::Error| {
                Box::new(CarddavError::InvalidConfig(e.to_string())) as Box<dyn EngineError>
            })?;
        Ok(Self { config })
    }

    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.config.host, self.config.port)
    }
}

#[async_trait]
impl Transport for CarddavTransport {
    async fn start(&self, _config: toml::Value) -> Result<(), Box<dyn EngineError>> {
        tracing::info!(
            transport = "carddav",
            address = %self.bind_address(),
            "Transport carddav started on {}",
            self.bind_address()
        );
        Ok(())
    }

    async fn stop(&self) -> Result<(), Box<dyn EngineError>> {
        tracing::info!(transport = "carddav", "CardDAV transport stopped");
        Ok(())
    }

    fn name(&self) -> &str {
        "carddav"
    }
}
