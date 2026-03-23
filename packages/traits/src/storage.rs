//! Storage backend trait definition.
//!
//! Defines the `StorageBackend` trait that all storage implementations
//! must implement to provide query and mutation capabilities.

use async_trait::async_trait;
use life_engine_types::{PipelineMessage, StorageMutation, StorageQuery};

use crate::EngineError;

/// Trait for storage backend implementations.
///
/// Storage backends translate `StorageQuery` and `StorageMutation`
/// operations into native queries for their underlying data store.
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Execute a read query and return matching records.
    async fn execute(
        &self,
        query: StorageQuery,
    ) -> Result<Vec<PipelineMessage>, Box<dyn EngineError>>;

    /// Execute a write mutation (insert, update, or delete).
    async fn mutate(&self, op: StorageMutation) -> Result<(), Box<dyn EngineError>>;

    /// Initialize the storage backend from configuration.
    ///
    /// The `key` parameter is the 32-byte encryption key derived from
    /// the user's passphrase, used for at-rest encryption.
    async fn init(config: toml::Value, key: [u8; 32]) -> Result<Self, Box<dyn EngineError>>
    where
        Self: Sized;
}
