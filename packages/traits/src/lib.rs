//! Infrastructure contracts for Life Engine.
//!
//! Defines the core traits that all modules implement: `StorageBackend`,
//! `Transport`, `Plugin`, and `EngineError`, plus capability types for
//! plugin access control.

pub mod blob;
pub mod capability;
pub mod error;
pub mod index_hints;
pub mod plugin;
pub mod schema;
pub mod schema_versioning;
pub mod storage;
pub mod storage_context;
pub mod storage_router;
pub mod transport;
pub mod types;

pub use capability::{Capability, CapabilityViolation};
pub use error::{EngineError, Severity};
pub use life_engine_types::{StorageMutation, StorageQuery};
pub use plugin::{Action, Plugin};
pub use storage::StorageBackend;
pub use transport::{TlsConfig, Transport, TransportConfig};

#[cfg(test)]
mod tests;
