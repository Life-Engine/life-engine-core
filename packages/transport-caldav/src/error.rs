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

    /// XML parsing failed on the request body.
    #[error("XML parse error: {0}")]
    XmlParse(String),

    /// CalDAV protocol violation (e.g. invalid Depth header, missing required elements).
    #[error("CalDAV protocol error: {0}")]
    ProtocolViolation(String),

    /// Authentication or authorization failure.
    #[error("CalDAV auth error: {0}")]
    AuthFailed(String),

    /// Requested resource was not found.
    #[error("resource not found: {0}")]
    NotFound(String),

    /// Conflict (e.g. ETag mismatch on PUT, calendar already exists).
    #[error("conflict: {0}")]
    Conflict(String),
}

impl EngineError for CaldavError {
    fn code(&self) -> &str {
        match self {
            CaldavError::RequestFailed(_) => "TRANSPORT_CALDAV_001",
            CaldavError::BindFailed(_) => "TRANSPORT_CALDAV_002",
            CaldavError::InvalidConfig(_) => "TRANSPORT_CALDAV_003",
            CaldavError::XmlParse(_) => "TRANSPORT_CALDAV_004",
            CaldavError::ProtocolViolation(_) => "TRANSPORT_CALDAV_005",
            CaldavError::AuthFailed(_) => "TRANSPORT_CALDAV_006",
            CaldavError::NotFound(_) => "TRANSPORT_CALDAV_007",
            CaldavError::Conflict(_) => "TRANSPORT_CALDAV_008",
        }
    }

    fn severity(&self) -> Severity {
        match self {
            CaldavError::RequestFailed(_) => Severity::Retryable,
            CaldavError::BindFailed(_) => Severity::Fatal,
            CaldavError::InvalidConfig(_) => Severity::Fatal,
            CaldavError::XmlParse(_) => Severity::Fatal,
            CaldavError::ProtocolViolation(_) => Severity::Fatal,
            CaldavError::AuthFailed(_) => Severity::Fatal,
            CaldavError::NotFound(_) => Severity::Fatal,
            CaldavError::Conflict(_) => Severity::Fatal,
        }
    }

    fn source_module(&self) -> &str {
        "transport-caldav"
    }
}
