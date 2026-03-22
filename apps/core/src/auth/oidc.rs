//! OIDC authentication provider using Pocket ID.
//!
//! Implements the `AuthProvider` trait by validating JWT tokens issued
//! by an OIDC identity provider. JWKS keys are cached and refreshed
//! periodically or on key miss.

use crate::auth::jwt::{self, JwksCache, JwksResponse, JwtError};
use crate::auth::types::{AuthError, AuthIdentity, TokenInfo, TokenRequest, TokenResponse};
use crate::auth::AuthProvider;

use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

/// Configuration for OIDC authentication via Pocket ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcConfig {
    /// The Pocket ID issuer URL (e.g., "http://localhost:3751").
    pub issuer_url: String,
    /// The client ID registered with Pocket ID.
    pub client_id: String,
    /// The client secret (if using confidential client).
    #[serde(default)]
    pub client_secret: Option<String>,
    /// JWKS endpoint for token validation (derived from issuer_url if not set).
    #[serde(default)]
    pub jwks_uri: Option<String>,
    /// Expected audience claim value.
    #[serde(default)]
    pub audience: Option<String>,
}

impl OidcConfig {
    /// Resolve the JWKS URI, either from explicit config or derived from the issuer URL.
    pub fn resolve_jwks_uri(&self) -> String {
        self.jwks_uri
            .clone()
            .unwrap_or_else(|| {
                format!(
                    "{}/.well-known/jwks.json",
                    self.issuer_url.trim_end_matches('/')
                )
            })
    }

    /// Resolve the token endpoint URL.
    pub fn token_endpoint(&self) -> String {
        format!(
            "{}/api/oidc/token",
            self.issuer_url.trim_end_matches('/')
        )
    }

    /// Resolve the userinfo endpoint URL.
    pub fn userinfo_endpoint(&self) -> String {
        format!(
            "{}/api/oidc/userinfo",
            self.issuer_url.trim_end_matches('/')
        )
    }

    /// Resolve the OIDC discovery document URL.
    pub fn discovery_endpoint(&self) -> String {
        format!(
            "{}/.well-known/openid-configuration",
            self.issuer_url.trim_end_matches('/')
        )
    }

    /// Resolve the Pocket ID registration endpoint URL.
    pub fn registration_endpoint(&self) -> String {
        format!(
            "{}/api/oidc/register",
            self.issuer_url.trim_end_matches('/')
        )
    }
}

/// OIDC authentication provider.
///
/// Validates JWT tokens by verifying their signature against keys
/// fetched from the identity provider's JWKS endpoint. Keys are
/// cached with a configurable TTL.
pub struct OidcProvider {
    /// OIDC configuration.
    config: OidcConfig,
    /// Cached JWKS keys for token validation.
    jwks_cache: Arc<JwksCache>,
    /// HTTP client for fetching JWKS and proxying requests.
    http_client: reqwest::Client,
}

impl OidcProvider {
    /// Create a new OIDC provider with the given configuration.
    pub fn new(config: OidcConfig) -> Self {
        Self {
            config,
            jwks_cache: Arc::new(JwksCache::new()),
            http_client: reqwest::Client::new(),
        }
    }

    /// Create a new OIDC provider with a custom HTTP client and TTL.
    ///
    /// Useful for testing with mock HTTP servers.
    #[allow(dead_code)]
    pub fn with_client_and_ttl(
        config: OidcConfig,
        http_client: reqwest::Client,
        jwks_ttl: Duration,
    ) -> Self {
        Self {
            config,
            jwks_cache: Arc::new(JwksCache::with_ttl(jwks_ttl)),
            http_client,
        }
    }

    /// Fetch JWKS from the identity provider and update the cache.
    async fn fetch_jwks(&self) -> Result<(), AuthError> {
        let jwks_uri = self.config.resolve_jwks_uri();
        tracing::debug!(uri = %jwks_uri, "fetching JWKS");

        let response = self
            .http_client
            .get(&jwks_uri)
            .send()
            .await
            .map_err(|e| AuthError::Internal(format!("JWKS fetch failed: {e}")))?;

        if !response.status().is_success() {
            return Err(AuthError::Internal(format!(
                "JWKS endpoint returned status {}",
                response.status()
            )));
        }

        let jwks: JwksResponse = response
            .json()
            .await
            .map_err(|e| AuthError::Internal(format!("JWKS parse failed: {e}")))?;

        let key_count = jwks.keys.len();
        self.jwks_cache.update(jwks).await;
        tracing::info!(keys = key_count, "JWKS cache refreshed");

        Ok(())
    }

    /// Get a key from the cache, refreshing if expired or if the key ID is missing.
    async fn get_jwk_key(
        &self,
        kid: &str,
    ) -> Result<jwt::JwkKey, AuthError> {
        // Try reading from cache first.
        if !self.jwks_cache.is_expired().await
            && let Some(key) = self.jwks_cache.get_key(kid).await
        {
            return Ok(key);
        }

        // Cache is expired, empty, or missing the key -- refresh.
        self.fetch_jwks().await?;

        // Try again after refresh.
        self.jwks_cache.get_key(kid).await.ok_or_else(|| {
            AuthError::Internal(format!(
                "key ID '{kid}' not found in JWKS after refresh"
            ))
        })
    }
}

impl std::fmt::Debug for OidcProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OidcProvider")
            .field("config", &self.config)
            .finish()
    }
}

#[async_trait]
impl AuthProvider for OidcProvider {
    /// Validate an OIDC JWT token.
    ///
    /// Decodes the JWT header, fetches the matching JWKS key, verifies
    /// the signature and claims, then returns an `AuthIdentity`.
    async fn validate_token(&self, token: &str) -> Result<AuthIdentity, AuthError> {
        // 1. Decode JWT header to get kid.
        let header = jwt::decode_jwt_header(token).map_err(|e| {
            tracing::debug!(error = %e, "JWT header decode failed");
            AuthError::InvalidCredentials
        })?;

        let kid = header.kid.ok_or(AuthError::InvalidCredentials)?;

        // 2. Get the matching JWK key (with caching).
        let jwk_key = self.get_jwk_key(&kid).await?;

        // 3. Validate the JWT using decode_and_validate_jwt which
        //    builds the decoding key from the JWK internally.
        let token_data = jwt::decode_and_validate_jwt(
            token,
            &jwk_key,
            Some(&self.config.issuer_url),
            self.config.audience.as_deref(),
        )
        .map_err(|e: JwtError| match e {
            JwtError::Expired => AuthError::TokenExpired,
            JwtError::InvalidSignature => AuthError::InvalidCredentials,
            JwtError::IssuerMismatch
            | JwtError::AudienceMismatch
            | JwtError::NotYetValid => {
                tracing::debug!(error = %e, "JWT validation failed");
                AuthError::InvalidCredentials
            }
            _ => {
                tracing::warn!(error = %e, "JWT validation error");
                AuthError::Internal(e.to_string())
            }
        })?;

        // 4. Build AuthIdentity from JWT claims.
        let claims = &token_data.claims;
        let created_at = claims
            .iat
            .and_then(|ts| Utc.timestamp_opt(ts as i64, 0).single())
            .unwrap_or_else(Utc::now);
        let expires_at = claims
            .exp
            .and_then(|ts| Utc.timestamp_opt(ts as i64, 0).single())
            .unwrap_or_else(Utc::now);

        Ok(AuthIdentity {
            token_id: claims.sub.clone(),
            user_id: Some(claims.sub.clone()),
            household_id: None,
            role: None,
            created_at,
            expires_at,
        })
    }

    /// Generate a token via OIDC is not supported directly.
    ///
    /// OIDC tokens are obtained through the login flow, not via passphrase.
    /// This method returns an error for the OIDC provider.
    async fn generate_token(
        &self,
        _credentials: &TokenRequest,
    ) -> Result<TokenResponse, AuthError> {
        Err(AuthError::Internal(
            "OIDC provider does not support direct token generation; use /api/auth/login"
                .into(),
        ))
    }

    /// Revoking OIDC tokens is not supported by this provider.
    ///
    /// Token revocation should be handled by the identity provider.
    async fn revoke_token(&self, _token_id: &str) -> Result<(), AuthError> {
        Err(AuthError::Internal(
            "OIDC provider does not support token revocation".into(),
        ))
    }

    /// Listing OIDC tokens is not supported by this provider.
    async fn list_tokens(&self) -> Result<Vec<TokenInfo>, AuthError> {
        Err(AuthError::Internal(
            "OIDC provider does not support token listing".into(),
        ))
    }
}

/// OIDC login request body.
#[derive(Debug, Deserialize)]
pub struct OidcLoginRequest {
    /// The username (or email) to authenticate.
    pub username: String,
    /// The password to authenticate with.
    pub password: String,
}

/// OIDC login response body.
#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct OidcLoginResponse {
    /// The access token (JWT).
    pub access_token: String,
    /// The token type (always "Bearer").
    pub token_type: String,
    /// When the token expires (seconds from now).
    pub expires_in: u64,
    /// The refresh token, if provided.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    /// The ID token, if provided.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>,
}

/// OIDC refresh token request body.
#[derive(Debug, Deserialize)]
pub struct OidcRefreshRequest {
    /// The refresh token to exchange for a new access token.
    pub refresh_token: String,
}

/// OIDC registration request body.
#[derive(Debug, Deserialize)]
pub struct OidcRegisterRequest {
    /// The username for the new account.
    pub username: String,
    /// The password for the new account.
    pub password: String,
    /// Optional display name for the new account.
    pub display_name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oidc_config_serialization() {
        let config = OidcConfig {
            issuer_url: "http://localhost:3751".into(),
            client_id: "life-engine".into(),
            client_secret: Some("secret-123".into()),
            jwks_uri: None,
            audience: Some("life-engine".into()),
        };

        let json = serde_json::to_string(&config).unwrap();
        let restored: OidcConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.issuer_url, "http://localhost:3751");
        assert_eq!(restored.client_id, "life-engine");
        assert_eq!(restored.client_secret.as_deref(), Some("secret-123"));
        assert_eq!(restored.audience.as_deref(), Some("life-engine"));
    }

    #[test]
    fn oidc_config_deserialization_without_optional_fields() {
        let json = r#"{
            "issuer_url": "http://localhost:3751",
            "client_id": "my-client"
        }"#;
        let config: OidcConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.issuer_url, "http://localhost:3751");
        assert_eq!(config.client_id, "my-client");
        assert!(config.client_secret.is_none());
        assert!(config.jwks_uri.is_none());
        assert!(config.audience.is_none());
    }

    #[test]
    fn oidc_config_resolve_jwks_uri_default() {
        let config = OidcConfig {
            issuer_url: "http://localhost:3751".into(),
            client_id: "test".into(),
            client_secret: None,
            jwks_uri: None,
            audience: None,
        };
        assert_eq!(
            config.resolve_jwks_uri(),
            "http://localhost:3751/.well-known/jwks.json"
        );
    }

    #[test]
    fn oidc_config_resolve_jwks_uri_explicit() {
        let config = OidcConfig {
            issuer_url: "http://localhost:3751".into(),
            client_id: "test".into(),
            client_secret: None,
            jwks_uri: Some("http://custom/jwks".into()),
            audience: None,
        };
        assert_eq!(config.resolve_jwks_uri(), "http://custom/jwks");
    }

    #[test]
    fn oidc_config_resolve_jwks_uri_strips_trailing_slash() {
        let config = OidcConfig {
            issuer_url: "http://localhost:3751/".into(),
            client_id: "test".into(),
            client_secret: None,
            jwks_uri: None,
            audience: None,
        };
        assert_eq!(
            config.resolve_jwks_uri(),
            "http://localhost:3751/.well-known/jwks.json"
        );
    }

    #[test]
    fn oidc_config_endpoints() {
        let config = OidcConfig {
            issuer_url: "http://localhost:3751".into(),
            client_id: "test".into(),
            client_secret: None,
            jwks_uri: None,
            audience: None,
        };
        assert_eq!(
            config.token_endpoint(),
            "http://localhost:3751/api/oidc/token"
        );
        assert_eq!(
            config.userinfo_endpoint(),
            "http://localhost:3751/api/oidc/userinfo"
        );
        assert_eq!(
            config.discovery_endpoint(),
            "http://localhost:3751/.well-known/openid-configuration"
        );
    }

    #[test]
    fn oidc_provider_construction() {
        let config = OidcConfig {
            issuer_url: "http://localhost:3751".into(),
            client_id: "life-engine".into(),
            client_secret: None,
            jwks_uri: None,
            audience: None,
        };

        let provider = OidcProvider::new(config);
        assert_eq!(provider.config.issuer_url, "http://localhost:3751");
        assert_eq!(provider.config.client_id, "life-engine");
    }

    #[test]
    fn oidc_provider_debug_impl() {
        let config = OidcConfig {
            issuer_url: "http://localhost:3751".into(),
            client_id: "test".into(),
            client_secret: None,
            jwks_uri: None,
            audience: None,
        };
        let provider = OidcProvider::new(config);
        let debug = format!("{provider:?}");
        assert!(debug.contains("OidcProvider"));
        assert!(debug.contains("localhost:3751"));
    }

    #[tokio::test]
    async fn oidc_provider_generate_token_returns_error() {
        let config = OidcConfig {
            issuer_url: "http://localhost:3751".into(),
            client_id: "test".into(),
            client_secret: None,
            jwks_uri: None,
            audience: None,
        };
        let provider = OidcProvider::new(config);
        let req = TokenRequest {
            passphrase: "test".into(),
            expires_in_days: None,
        };

        let err = provider.generate_token(&req).await.unwrap_err();
        assert!(matches!(err, AuthError::Internal(_)));
    }

    #[tokio::test]
    async fn oidc_provider_revoke_token_returns_error() {
        let config = OidcConfig {
            issuer_url: "http://localhost:3751".into(),
            client_id: "test".into(),
            client_secret: None,
            jwks_uri: None,
            audience: None,
        };
        let provider = OidcProvider::new(config);

        let err = provider.revoke_token("some-id").await.unwrap_err();
        assert!(matches!(err, AuthError::Internal(_)));
    }

    #[tokio::test]
    async fn oidc_provider_list_tokens_returns_error() {
        let config = OidcConfig {
            issuer_url: "http://localhost:3751".into(),
            client_id: "test".into(),
            client_secret: None,
            jwks_uri: None,
            audience: None,
        };
        let provider = OidcProvider::new(config);

        let err = provider.list_tokens().await.unwrap_err();
        assert!(matches!(err, AuthError::Internal(_)));
    }

    #[test]
    fn oidc_login_request_deserializes() {
        let json = r#"{"username": "alice@example.com", "password": "secret"}"#;
        let req: OidcLoginRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.username, "alice@example.com");
        assert_eq!(req.password, "secret");
    }

    #[test]
    fn oidc_login_response_serializes() {
        let resp = OidcLoginResponse {
            access_token: "jwt-token".into(),
            token_type: "Bearer".into(),
            expires_in: 3600,
            refresh_token: Some("refresh-123".into()),
            id_token: None,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["access_token"], "jwt-token");
        assert_eq!(json["token_type"], "Bearer");
        assert_eq!(json["expires_in"], 3600);
        assert_eq!(json["refresh_token"], "refresh-123");
        assert!(json.get("id_token").is_none());
    }

    #[test]
    fn oidc_refresh_request_deserializes() {
        let json = r#"{"refresh_token": "rt-abc"}"#;
        let req: OidcRefreshRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.refresh_token, "rt-abc");
    }

    #[test]
    fn register_request_deserializes() {
        let json = r#"{"username": "bob", "password": "pass123", "display_name": "Bob Smith"}"#;
        let req: OidcRegisterRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.username, "bob");
        assert_eq!(req.password, "pass123");
        assert_eq!(req.display_name.as_deref(), Some("Bob Smith"));
    }

    #[test]
    fn register_request_deserializes_without_optional() {
        let json = r#"{"username": "alice", "password": "secret"}"#;
        let req: OidcRegisterRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.username, "alice");
        assert_eq!(req.password, "secret");
        assert!(req.display_name.is_none());
    }

    #[test]
    fn registration_endpoint_resolves() {
        let config = OidcConfig {
            issuer_url: "http://localhost:3751".into(),
            client_id: "test".into(),
            client_secret: None,
            jwks_uri: None,
            audience: None,
        };
        assert_eq!(
            config.registration_endpoint(),
            "http://localhost:3751/api/oidc/register"
        );
    }
}
