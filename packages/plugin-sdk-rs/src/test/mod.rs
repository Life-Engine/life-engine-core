//! Test utilities for plugin authors.
//!
//! This module provides mock implementations of storage and message builders
//! for unit-testing plugins without a running Life Engine Core instance.
//!
//! Enable via the `test-utils` feature in your `Cargo.toml`:
//!
//! ```toml
//! [dev-dependencies]
//! life-engine-plugin-sdk = { path = "...", features = ["test-utils"] }
//! ```

mod mock_message;
mod mock_storage;

pub use mock_message::MockMessageBuilder;
pub use mock_storage::MockStorageContext;
