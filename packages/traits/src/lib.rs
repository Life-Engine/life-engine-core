//! Infrastructure contracts for Life Engine.
//!
//! Defines the core traits that all modules implement: `StorageBackend`,
//! `Transport`, `Plugin`, and `EngineError`.

pub mod error;
pub mod plugin;
pub mod storage;
pub mod transport;
pub mod types;

#[cfg(test)]
mod tests;
