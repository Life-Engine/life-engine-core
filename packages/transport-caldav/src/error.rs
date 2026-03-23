//! CalDAV transport error types.

use life_engine_traits::{EngineError, Severity};
use thiserror::Error;

/// Errors that can occur in the CalDAV transport layer.
#[derive(Debug, Error)]
pub enum CaldavError {
    /// CalDAV request failed.
    #[error("CalDAV request failed: {0}")]
    RequestFailed(String),

    /// Transport failed to bind to the configured address.
    #[error("failed to bind CalDAV transport: {0}")]
    BindFailed(String),

    /// Configuration is invalid.
    #[error("invalid CalDAV transport config: {0}")]
    InvalidConfig(String),
}

impl EngineError for CaldavError {
    fn code(&self) -> &str {
        match self {
            CaldavError::RequestFailed(_) => "TRANSPORT_CALDAV_001",
            CaldavError::BindFailed(_) => "TRANSPORT_CALDAV_002",
            CaldavError::InvalidConfig(_) => "TRANSPORT_CALDAV_003",
        }
    }

    fn severity(&self) -> Severity {
        match self {
            CaldavError::RequestFailed(_) => Severity::Retryable,
            CaldavError::BindFailed(_) => Severity::Fatal,
            CaldavError::InvalidConfig(_) => Severity::Fatal,
        }
    }

    fn source_module(&self) -> &str {
        "transport-caldav"
    }
}
