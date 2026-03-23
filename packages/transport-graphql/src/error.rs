//! GraphQL transport error types.

use life_engine_traits::{EngineError, Severity};
use thiserror::Error;

/// Errors that can occur in the GraphQL transport layer.
#[derive(Debug, Error)]
pub enum GraphqlError {
    /// Query execution failed.
    #[error("query execution failed: {0}")]
    QueryFailed(String),

    /// Transport failed to bind to the configured address.
    #[error("failed to bind GraphQL transport: {0}")]
    BindFailed(String),

    /// Configuration is invalid.
    #[error("invalid GraphQL transport config: {0}")]
    InvalidConfig(String),
}

impl EngineError for GraphqlError {
    fn code(&self) -> &str {
        match self {
            GraphqlError::QueryFailed(_) => "TRANSPORT_GRAPHQL_001",
            GraphqlError::BindFailed(_) => "TRANSPORT_GRAPHQL_002",
            GraphqlError::InvalidConfig(_) => "TRANSPORT_GRAPHQL_003",
        }
    }

    fn severity(&self) -> Severity {
        match self {
            GraphqlError::QueryFailed(_) => Severity::Retryable,
            GraphqlError::BindFailed(_) => Severity::Fatal,
            GraphqlError::InvalidConfig(_) => Severity::Fatal,
        }
    }

    fn source_module(&self) -> &str {
        "transport-graphql"
    }
}
