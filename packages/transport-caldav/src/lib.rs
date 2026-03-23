//! CalDAV transport layer for Life Engine.

pub mod config;
pub mod error;
pub mod handlers;
pub mod types;

use async_trait::async_trait;
use error::CaldavError;
use life_engine_traits::{EngineError, Transport};
use serde::Deserialize;

#[cfg(test)]
mod tests;

/// Configuration for the CalDAV transport.
#[derive(Debug, Clone, Deserialize)]
pub struct CaldavTransportConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    5232
}

/// CalDAV transport implementation.
pub struct CaldavTransport {
    config: CaldavTransportConfig,
}

impl CaldavTransport {
    pub fn from_config(config_value: &toml::Value) -> Result<Self, Box<dyn EngineError>> {
        let config: CaldavTransportConfig =
            config_value.clone().try_into().map_err(|e: toml::de::Error| {
                Box::new(CaldavError::InvalidConfig(e.to_string())) as Box<dyn EngineError>
            })?;
        Ok(Self { config })
    }

    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.config.host, self.config.port)
    }
}

#[async_trait]
impl Transport for CaldavTransport {
    async fn start(&self, _config: toml::Value) -> Result<(), Box<dyn EngineError>> {
        tracing::info!(
            transport = "caldav",
            address = %self.bind_address(),
            "Transport caldav started on {}",
            self.bind_address()
        );
        Ok(())
    }

    async fn stop(&self) -> Result<(), Box<dyn EngineError>> {
        tracing::info!(transport = "caldav", "CalDAV transport stopped");
        Ok(())
    }

    fn name(&self) -> &str {
        "caldav"
    }
}
