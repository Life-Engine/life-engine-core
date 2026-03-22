//! CalDAV transport error types.

use thiserror::Error;

/// Errors that can occur in the CalDAV transport layer.
#[derive(Debug, Error)]
pub enum CaldavError {
    /// CalDAV request failed.
    #[error("CalDAV request failed: {0}")]
    RequestFailed(String),
}
