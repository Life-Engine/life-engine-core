//! GraphQL transport error types.

use thiserror::Error;

/// Errors that can occur in the GraphQL transport layer.
#[derive(Debug, Error)]
pub enum GraphqlError {
    /// Query execution failed.
    #[error("query execution failed: {0}")]
    QueryFailed(String),
}
