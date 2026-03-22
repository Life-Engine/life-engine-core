//! Engine error trait definition.

/// Trait for engine-level errors.
pub trait EngineError: std::error::Error + Send + Sync + 'static {}
