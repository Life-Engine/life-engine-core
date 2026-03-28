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

    /// XML parsing failed on the request body.
    #[error("XML parse error: {0}")]
    XmlParse(String),

    /// CardDAV protocol violation.
    #[error("CardDAV protocol error: {0}")]
    ProtocolViolation(String),

    /// Authentication or authorization failure.
    #[error("CardDAV auth error: {0}")]
    AuthFailed(String),

    /// Requested resource was not found.
    #[error("resource not found: {0}")]
    NotFound(String),

    /// Conflict (e.g. ETag mismatch, addressbook already exists).
    #[error("conflict: {0}")]
    Conflict(String),
}

impl EngineError for CarddavError {
    fn code(&self) -> &str {
        match self {
            CarddavError::RequestFailed(_) => "TRANSPORT_CARDDAV_001",
            CarddavError::BindFailed(_) => "TRANSPORT_CARDDAV_002",
            CarddavError::InvalidConfig(_) => "TRANSPORT_CARDDAV_003",
            CarddavError::XmlParse(_) => "TRANSPORT_CARDDAV_004",
            CarddavError::ProtocolViolation(_) => "TRANSPORT_CARDDAV_005",
            CarddavError::AuthFailed(_) => "TRANSPORT_CARDDAV_006",
            CarddavError::NotFound(_) => "TRANSPORT_CARDDAV_007",
            CarddavError::Conflict(_) => "TRANSPORT_CARDDAV_008",
        }
    }

    fn severity(&self) -> Severity {
        match self {
            CarddavError::RequestFailed(_) => Severity::Retryable,
            CarddavError::BindFailed(_) => Severity::Fatal,
            CarddavError::InvalidConfig(_) => Severity::Fatal,
            CarddavError::XmlParse(_) => Severity::Fatal,
            CarddavError::ProtocolViolation(_) => Severity::Fatal,
            CarddavError::AuthFailed(_) => Severity::Fatal,
            CarddavError::NotFound(_) => Severity::Fatal,
            CarddavError::Conflict(_) => Severity::Fatal,
        }
    }

    fn source_module(&self) -> &str {
        "transport-carddav"
    }
}
