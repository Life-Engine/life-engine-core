//! Storage backend trait definition.

use async_trait::async_trait;

/// Trait for storage backend implementations.
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// The error type returned by this storage backend.
    type Error: std::error::Error + Send + Sync + 'static;
}
