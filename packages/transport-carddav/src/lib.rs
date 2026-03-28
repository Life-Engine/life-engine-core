//! CardDAV transport layer for Life Engine.
//!
//! Implements an Axum-based CardDAV server supporting PROPFIND, REPORT,
//! GET, PUT, DELETE, and MKCOL methods per RFC 6352.

pub mod config;
pub mod error;
pub mod handlers;
pub mod types;

use async_trait::async_trait;
use axum::routing::{any, get};
use axum::Router;
use error::CarddavError;
use handlers::CarddavState;
use life_engine_traits::{EngineError, Transport};
use serde::Deserialize;
use std::sync::Arc;
use tokio::net::TcpListener;

#[cfg(test)]
mod tests;

/// Configuration for the CardDAV transport.
#[derive(Debug, Clone, Deserialize)]
pub struct CarddavTransportConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_base_path")]
    pub base_path: String,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    5233
}

fn default_base_path() -> String {
    "/carddav".to_string()
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

    /// Build the Axum router with CardDAV routes.
    pub fn build_router(&self, state: Arc<CarddavState>) -> Router {
        let base = &self.config.base_path;

        Router::new()
            .route(
                &format!("{base}/"),
                any(handlers::handle_propfind),
            )
            .route(
                &format!("{base}/{{addressbook}}/"),
                any(handlers::handle_propfind),
            )
            .route(
                &format!("{base}/{{addressbook}}"),
                any(handlers::handle_report),
            )
            .route(
                &format!("{base}/{{addressbook}}/mkcol"),
                any(handlers::handle_mkcol),
            )
            .route(
                &format!("{base}/{{addressbook}}/{{resource}}"),
                get(handlers::handle_get)
                    .put(handlers::handle_put)
                    .delete(handlers::handle_delete),
            )
            .with_state(state)
    }
}

#[async_trait]
impl Transport for CarddavTransport {
    async fn start(&self) -> Result<(), Box<dyn EngineError>> {
        let addr = self.bind_address();

        let state = Arc::new(CarddavState {
            base_path: self.config.base_path.clone(),
            principal: "/principals/default".to_string(),
            addressbooks: Vec::new(),
            resources: std::collections::HashMap::new(),
        });

        let router = self.build_router(state);

        let listener = TcpListener::bind(&addr).await.map_err(|e| {
            Box::new(CarddavError::BindFailed(e.to_string())) as Box<dyn EngineError>
        })?;

        tracing::info!(
            transport = "carddav",
            address = %addr,
            "CardDAV transport listening on {addr}",
        );

        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, router).await {
                tracing::error!(error = %e, "CardDAV transport server error");
            }
        });

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
