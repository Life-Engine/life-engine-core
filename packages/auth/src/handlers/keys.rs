//! API key management handler.

use async_trait::async_trait;
use uuid::Uuid;

use crate::error::AuthError;
use crate::types::AuthIdentity;
use crate::AuthProvider;

/// API key authentication provider.
///
/// Validates API keys by looking up their salted hash in storage.
/// Full CRUD operations are implemented in WP 6.10.
pub struct ApiKeyProvider;

impl ApiKeyProvider {
    /// Create a new API key provider.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl AuthProvider for ApiKeyProvider {
    async fn validate_token(&self, _token: &str) -> Result<AuthIdentity, AuthError> {
        // API key provider does not handle JWT tokens.
        Err(AuthError::TokenInvalid(
            "api-key provider does not support bearer tokens".to_string(),
        ))
    }

    async fn validate_key(&self, _key: &str) -> Result<AuthIdentity, AuthError> {
        // Full key validation implemented in WP 6.10.
        Err(AuthError::KeyInvalid)
    }

    async fn revoke_key(&self, _key_id: Uuid) -> Result<(), AuthError> {
        // Full key revocation implemented in WP 6.10.
        Err(AuthError::KeyInvalid)
    }
}
