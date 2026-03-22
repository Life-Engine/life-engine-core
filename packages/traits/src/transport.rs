//! Transport trait definition.

use async_trait::async_trait;

/// Trait for transport layer implementations (REST, GraphQL, CalDAV, etc.).
#[async_trait]
pub trait Transport: Send + Sync {
    /// The error type returned by this transport.
    type Error: std::error::Error + Send + Sync + 'static;
}
