//! Authentication and authorization module for Life Engine.
//!
//! Provides transport-agnostic authentication supporting two mechanisms:
//! Pocket ID (OIDC) for user sessions and API keys for scripting.
//! The auth module is initialized once during Core startup and shared
//! with all transports via `Arc<dyn AuthProvider>`.

use async_trait::async_trait;
use uuid::Uuid;

pub mod config;
pub mod error;
pub mod handlers;
pub mod types;

pub use config::AuthConfig;
pub use error::AuthError;
pub use handlers::rate_limit::RateLimiter;
pub use handlers::validate::validate_request;
pub use types::{ApiKeyRecord, AuthIdentity, AuthToken};

/// Transport-agnostic authentication provider.
///
/// Implementations handle token validation (JWT via Pocket ID) and
/// API key validation. A single provider instance is created at startup
/// and shared across all transports via `Arc<dyn AuthProvider>`.
#[async_trait]
pub trait AuthProvider: Send + Sync {
    /// Validate a JWT bearer token and return the authenticated identity.
    async fn validate_token(&self, token: &str) -> Result<AuthIdentity, AuthError>;

    /// Validate an API key and return the authenticated identity.
    async fn validate_key(&self, key: &str) -> Result<AuthIdentity, AuthError>;

    /// Revoke an API key by its unique identifier.
    async fn revoke_key(&self, key_id: Uuid) -> Result<(), AuthError>;
}

/// Create an auth provider from the given configuration.
///
/// This factory is called once during Core startup. The returned provider
/// should be wrapped in `Arc` for sharing across transport tasks.
pub async fn create_auth_provider(
    config: AuthConfig,
) -> Result<Box<dyn AuthProvider>, AuthError> {
    config.validate().map_err(AuthError::ConfigInvalid)?;

    match config.provider.as_str() {
        "pocket-id" => {
            let provider = handlers::validate::PocketIdProvider::new(config)?;
            Ok(Box::new(provider))
        }
        "api-key" => {
            let provider = handlers::keys::ApiKeyProvider::new();
            Ok(Box::new(provider))
        }
        other => Err(AuthError::ConfigInvalid(format!(
            "unknown auth provider: {other}"
        ))),
    }
}

#[cfg(test)]
mod tests;
