//! REST transport error types.

use thiserror::Error;

/// Errors that can occur in the REST transport layer.
#[derive(Debug, Error)]
pub enum RestError {
    /// Request handling failed.
    #[error("request handling failed: {0}")]
    RequestFailed(String),
}
