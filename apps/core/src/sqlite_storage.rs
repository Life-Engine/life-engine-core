//! SQLite storage adapter — re-exports from `life_engine_storage_sqlite::legacy`.
//!
//! The implementation was migrated to `packages/storage-sqlite/src/legacy.rs`
//! as part of the architecture migration (WP 10.4). This module re-exports
//! all public types for backward compatibility.

pub use life_engine_storage_sqlite::legacy::*;
