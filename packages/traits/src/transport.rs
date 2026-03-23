//! Transport trait definition.
//!
//! Defines the `Transport` trait for protocol-specific entry points
//! (REST, GraphQL, CalDAV, etc.) and the `TransportConfig` struct
//! for common transport configuration.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::EngineError;

/// Common configuration shared by all transports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Path to the TLS certificate file.
    pub cert_path: String,
    /// Path to the TLS private key file.
    pub key_path: String,
}

/// Common configuration shared by all transports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    /// Address to bind the transport listener to.
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
    /// Port number for the transport listener.
    pub port: u16,
    /// Optional TLS configuration.
    pub tls: Option<TlsConfig>,
}

fn default_bind_address() -> String {
    "127.0.0.1".to_string()
}

/// Trait for transport layer implementations (REST, GraphQL, CalDAV, etc.).
///
/// Transports are protocol-specific entry points that receive requests,
/// route them through the workflow engine, and validate authentication.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Bind and begin serving requests.
    async fn start(&self, config: toml::Value) -> Result<(), Box<dyn EngineError>>;

    /// Gracefully shut down the transport.
    async fn stop(&self) -> Result<(), Box<dyn EngineError>>;

    /// Returns the transport identifier (e.g., "rest", "graphql", "caldav").
    fn name(&self) -> &str;
}
