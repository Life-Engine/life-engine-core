//! GraphQL transport layer for Life Engine.

pub mod config;
pub mod error;
pub mod handlers;
pub mod types;

use async_trait::async_trait;
use error::GraphqlError;
use life_engine_traits::{EngineError, Transport};
use serde::Deserialize;

#[cfg(test)]
mod tests;

/// Configuration for the GraphQL transport.
#[derive(Debug, Clone, Deserialize)]
pub struct GraphqlTransportConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    4000
}

/// GraphQL transport implementation.
pub struct GraphqlTransport {
    config: GraphqlTransportConfig,
}

impl GraphqlTransport {
    pub fn from_config(config_value: &toml::Value) -> Result<Self, Box<dyn EngineError>> {
        let config: GraphqlTransportConfig =
            config_value.clone().try_into().map_err(|e: toml::de::Error| {
                Box::new(GraphqlError::InvalidConfig(e.to_string())) as Box<dyn EngineError>
            })?;
        Ok(Self { config })
    }

    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.config.host, self.config.port)
    }
}

#[async_trait]
impl Transport for GraphqlTransport {
    async fn start(&self, _config: toml::Value) -> Result<(), Box<dyn EngineError>> {
        tracing::info!(
            transport = "graphql",
            address = %self.bind_address(),
            "Transport graphql started on {}",
            self.bind_address()
        );
        Ok(())
    }

    async fn stop(&self) -> Result<(), Box<dyn EngineError>> {
        tracing::info!(transport = "graphql", "GraphQL transport stopped");
        Ok(())
    }

    fn name(&self) -> &str {
        "graphql"
    }
}
