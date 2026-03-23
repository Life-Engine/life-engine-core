//! Token validation handler and Pocket ID provider.

use async_trait::async_trait;
use uuid::Uuid;

use crate::config::AuthConfig;
use crate::error::AuthError;
use crate::types::AuthIdentity;
use crate::AuthProvider;

/// Pocket ID (OIDC) authentication provider.
///
/// Validates JWT bearer tokens against the configured OIDC issuer.
/// JWKS public keys are cached and periodically refreshed.
pub struct PocketIdProvider {
    /// OIDC issuer URL.
    issuer: String,
    /// Expected JWT audience claim (if configured).
    audience: Option<String>,
    /// Seconds between JWKS key refreshes.
    jwks_refresh_interval: u64,
}

impl PocketIdProvider {
    /// Create a new Pocket ID provider from the auth configuration.
    pub fn new(config: AuthConfig) -> Result<Self, AuthError> {
        let issuer = config
            .issuer
            .ok_or_else(|| AuthError::ConfigInvalid(
                "issuer is required for pocket-id provider".to_string(),
            ))?;

        Ok(Self {
            issuer,
            audience: config.audience,
            jwks_refresh_interval: config.jwks_refresh_interval,
        })
    }
}

#[async_trait]
impl AuthProvider for PocketIdProvider {
    async fn validate_token(&self, _token: &str) -> Result<AuthIdentity, AuthError> {
        // Full JWT validation implemented in WP 6.5.
        Err(AuthError::TokenInvalid("not yet implemented".to_string()))
    }

    async fn validate_key(&self, _key: &str) -> Result<AuthIdentity, AuthError> {
        // Pocket ID provider delegates key validation to the API key provider.
        Err(AuthError::KeyInvalid)
    }

    async fn revoke_key(&self, _key_id: Uuid) -> Result<(), AuthError> {
        // Pocket ID provider delegates key management to the API key provider.
        Err(AuthError::KeyInvalid)
    }
}
