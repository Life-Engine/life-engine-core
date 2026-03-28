//! CalDAV transport layer for Life Engine.
//!
//! Implements an Axum-based CalDAV server supporting PROPFIND, REPORT,
//! GET, PUT, DELETE, and MKCALENDAR methods per RFC 4791.

pub mod config;
pub mod error;
pub mod handlers;
pub mod types;

use async_trait::async_trait;
use axum::routing::{any, get};
use axum::Router;
use error::CaldavError;
use handlers::CaldavState;
use life_engine_traits::{EngineError, Transport};
use serde::Deserialize;
use std::sync::Arc;
use tokio::net::TcpListener;

#[cfg(test)]
mod tests;

/// Configuration for the CalDAV transport.
#[derive(Debug, Clone, Deserialize)]
pub struct CaldavTransportConfig {
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
    5232
}

fn default_base_path() -> String {
    "/caldav".to_string()
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

    /// Build the Axum router with CalDAV routes.
    pub fn build_router(&self, state: Arc<CaldavState>) -> Router {
        let base = &self.config.base_path;

        Router::new()
            // Collection-level PROPFIND (calendar home)
            .route(
                &format!("{base}/"),
                any(handlers::handle_propfind),
            )
            // Collection-level PROPFIND and REPORT for a specific calendar
            .route(
                &format!("{base}/{{collection}}/"),
                any(handlers::handle_propfind),
            )
            // REPORT on a collection (without trailing slash)
            .route(
                &format!("{base}/{{collection}}"),
                any(handlers::handle_report),
            )
            // MKCALENDAR for creating a new collection
            .route(
                &format!("{base}/{{collection}}/mkcalendar"),
                any(handlers::handle_mkcalendar),
            )
            // Resource-level GET, PUT, DELETE
            .route(
                &format!("{base}/{{collection}}/{{resource}}"),
                get(handlers::handle_get)
                    .put(handlers::handle_put)
                    .delete(handlers::handle_delete),
            )
            .with_state(state)
    }
}

#[async_trait]
impl Transport for CaldavTransport {
    async fn start(&self) -> Result<(), Box<dyn EngineError>> {
        let addr = self.bind_address();

        let state = Arc::new(CaldavState {
            base_path: self.config.base_path.clone(),
            principal: "/principals/default".to_string(),
            calendars: Vec::new(),
            resources: std::collections::HashMap::new(),
        });

        let router = self.build_router(state);

        let listener = TcpListener::bind(&addr).await.map_err(|e| {
            Box::new(CaldavError::BindFailed(e.to_string())) as Box<dyn EngineError>
        })?;

        tracing::info!(
            transport = "caldav",
            address = %addr,
            "CalDAV transport listening on {addr}",
        );

        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, router).await {
                tracing::error!(error = %e, "CalDAV transport server error");
            }
        });

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
