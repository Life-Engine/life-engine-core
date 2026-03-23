//! REST transport error types.

use life_engine_traits::{EngineError, Severity};
use thiserror::Error;

/// Errors that can occur in the REST transport layer.
#[derive(Debug, Error)]
pub enum RestError {
    /// Request handling failed.
    #[error("request handling failed: {0}")]
    RequestFailed(String),

    /// Transport failed to bind to the configured address.
    #[error("failed to bind REST transport: {0}")]
    BindFailed(String),

    /// Configuration is invalid.
    #[error("invalid REST transport config: {0}")]
    InvalidConfig(String),
}

impl EngineError for RestError {
    fn code(&self) -> &str {
        match self {
            RestError::RequestFailed(_) => "TRANSPORT_REST_001",
            RestError::BindFailed(_) => "TRANSPORT_REST_002",
            RestError::InvalidConfig(_) => "TRANSPORT_REST_003",
        }
    }

    fn severity(&self) -> Severity {
        match self {
            RestError::RequestFailed(_) => Severity::Retryable,
            RestError::BindFailed(_) => Severity::Fatal,
            RestError::InvalidConfig(_) => Severity::Fatal,
        }
    }

    fn source_module(&self) -> &str {
        "transport-rest"
    }
}
