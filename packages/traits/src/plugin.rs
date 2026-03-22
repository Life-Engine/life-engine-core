//! Plugin trait definition.

use async_trait::async_trait;

/// Trait for plugin implementations.
#[async_trait]
pub trait Plugin: Send + Sync {
    /// The error type returned by this plugin.
    type Error: std::error::Error + Send + Sync + 'static;
}
