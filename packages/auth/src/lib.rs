//! Authentication and authorization module for Life Engine.
//!
//! Provides transport-agnostic authentication supporting two mechanisms:
//! Pocket ID (OIDC) for user sessions and API keys for scripting.
//! The auth module is initialized once during Core startup and shared
//! with all transports via `Arc<dyn AuthProvider>`.

pub mod config;
pub mod error;
pub mod handlers;
pub mod types;

pub use config::AuthConfig;
pub use error::AuthError;
pub use types::{ApiKeyRecord, AuthIdentity, AuthToken};

#[cfg(test)]
mod tests;
