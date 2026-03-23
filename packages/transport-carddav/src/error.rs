//! CardDAV transport error types.

use life_engine_traits::{EngineError, Severity};
use thiserror::Error;

/// Errors that can occur in the CardDAV transport layer.
#[derive(Debug, Error)]
pub enum CarddavError {
    /// CardDAV request failed.
    #[error("CardDAV request failed: {0}")]
    RequestFailed(String),

    /// Transport failed to bind to the configured address.
    #[error("failed to bind CardDAV transport: {0}")]
    BindFailed(String),

    /// Configuration is invalid.
    #[error("invalid CardDAV transport config: {0}")]
    InvalidConfig(String),
}

impl EngineError for CarddavError {
    fn code(&self) -> &str {
        match self {
            CarddavError::RequestFailed(_) => "TRANSPORT_CARDDAV_001",
            CarddavError::BindFailed(_) => "TRANSPORT_CARDDAV_002",
            CarddavError::InvalidConfig(_) => "TRANSPORT_CARDDAV_003",
        }
    }

    fn severity(&self) -> Severity {
        match self {
            CarddavError::RequestFailed(_) => Severity::Retryable,
            CarddavError::BindFailed(_) => Severity::Fatal,
            CarddavError::InvalidConfig(_) => Severity::Fatal,
        }
    }

    fn source_module(&self) -> &str {
        "transport-carddav"
    }
}
