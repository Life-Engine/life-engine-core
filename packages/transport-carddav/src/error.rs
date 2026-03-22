//! CardDAV transport error types.

use thiserror::Error;

/// Errors that can occur in the CardDAV transport layer.
#[derive(Debug, Error)]
pub enum CarddavError {
    /// CardDAV request failed.
    #[error("CardDAV request failed: {0}")]
    RequestFailed(String),
}
