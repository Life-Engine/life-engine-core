//! Infrastructure contracts for Life Engine.
//!
//! Defines the core traits that all modules implement: `StorageBackend`,
//! `Transport`, `Plugin`, and `EngineError`, plus capability types for
//! plugin access control.

pub mod capability;
pub mod error;
pub mod plugin;
pub mod storage;
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
