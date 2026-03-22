//! Authentication module for the Life Engine Core.
//!
//! Provides the `AuthProvider` trait for swappable auth backends,
//! the `LocalTokenProvider` implementation, OIDC provider, auth
//! middleware, and token management routes.

pub mod jwt;
pub mod local_token;
pub mod middleware;
pub mod oidc;
pub mod routes;
pub mod types;
pub mod webauthn_provider;
pub mod webauthn_store;

use async_trait::async_trait;
pub use types::{AuthError, AuthIdentity, TokenInfo, TokenRequest, TokenResponse};

/// Trait for pluggable authentication providers.
///
/// Implementations handle token generation, validation, revocation,
/// and listing. The active provider is selected by the `auth.provider`
/// configuration field.
#[async_trait]
pub trait AuthProvider: Send + Sync {
    /// Validate a token and return the identity if valid.
    async fn validate_token(&self, token: &str) -> Result<AuthIdentity, AuthError>;

    /// Generate a new token. Implementation-specific (e.g., requires
    /// passphrase for local-token).
    async fn generate_token(&self, credentials: &TokenRequest) -> Result<TokenResponse, AuthError>;

    /// Revoke a token by ID.
    async fn revoke_token(&self, token_id: &str) -> Result<(), AuthError>;

    /// List all active tokens (returns metadata, not raw tokens).
    async fn list_tokens(&self) -> Result<Vec<TokenInfo>, AuthError>;
}

/// Multi-provider authentication that tries each provider in order.
///
/// When validating a token, each provider is tried sequentially.
/// The first successful validation is returned. If all providers fail,
/// the error from the last provider is returned.
pub struct MultiAuthProvider {
    /// Ordered list of auth providers to try.
    providers: Vec<std::sync::Arc<dyn AuthProvider>>,
}

impl MultiAuthProvider {
    /// Create a new multi-provider with the given providers.
    ///
    /// Providers are tried in the order given.
    pub fn new(providers: Vec<std::sync::Arc<dyn AuthProvider>>) -> Self {
        Self { providers }
    }
}

#[async_trait]
impl AuthProvider for MultiAuthProvider {
    async fn validate_token(&self, token: &str) -> Result<AuthIdentity, AuthError> {
        let mut last_error = AuthError::TokenNotFound;
        for provider in &self.providers {
            match provider.validate_token(token).await {
                Ok(identity) => return Ok(identity),
                Err(e) => last_error = e,
            }
        }
        Err(last_error)
    }

    async fn generate_token(&self, credentials: &TokenRequest) -> Result<TokenResponse, AuthError> {
        // Delegate to the first provider that supports token generation.
        let mut last_error = AuthError::Internal("no providers configured".into());
        for provider in &self.providers {
            match provider.generate_token(credentials).await {
                Ok(resp) => return Ok(resp),
                Err(e) => last_error = e,
            }
        }
        Err(last_error)
    }

    async fn revoke_token(&self, token_id: &str) -> Result<(), AuthError> {
        let mut last_error = AuthError::Internal("no providers configured".into());
        for provider in &self.providers {
            match provider.revoke_token(token_id).await {
                Ok(()) => return Ok(()),
                Err(e) => last_error = e,
            }
        }
        Err(last_error)
    }

    async fn list_tokens(&self) -> Result<Vec<TokenInfo>, AuthError> {
        let mut all_tokens = Vec::new();
        for provider in &self.providers {
            match provider.list_tokens().await {
                Ok(tokens) => all_tokens.extend(tokens),
                Err(_) => { /* Skip providers that don't support listing. */ }
            }
        }
        Ok(all_tokens)
    }
}

/// Build the appropriate auth provider based on configuration.
///
/// Returns a multi-provider when OIDC is configured (OIDC first,
/// local-token fallback), or a local-token provider otherwise.
///
/// When `db_path` is `Some`, the local-token provider persists tokens
/// and the master passphrase hash in a file-backed SQLite database.
/// When `None`, an in-memory database is used.
///
/// When the provider is `"webauthn"`, returns the `WebAuthnProvider`
/// separately so route handlers can call ceremony methods directly.
pub fn build_auth_provider(
    auth_provider_name: &str,
    oidc_config: Option<oidc::OidcConfig>,
    webauthn_config: Option<webauthn_provider::WebAuthnConfig>,
    db_path: Option<&std::path::Path>,
) -> (
    std::sync::Arc<dyn AuthProvider>,
    Option<std::sync::Arc<webauthn_provider::WebAuthnProvider>>,
) {
    use std::sync::Arc;

    let build_local = || -> Arc<dyn AuthProvider> {
        match db_path {
            Some(path) => match local_token::LocalTokenProvider::open(path) {
                Ok(provider) => {
                    tracing::info!(path = %path.display(), "local-token provider using file-backed database");
                    Arc::new(provider)
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "failed to open file-backed auth database; falling back to in-memory"
                    );
                    Arc::new(local_token::LocalTokenProvider::new())
                }
            },
            None => Arc::new(local_token::LocalTokenProvider::new()),
        }
    };

    match auth_provider_name {
        "oidc" => {
            if let Some(config) = oidc_config {
                let oidc_provider = Arc::new(oidc::OidcProvider::new(config));
                let local_provider = build_local();
                // OIDC first, fall back to local-token.
                (
                    Arc::new(MultiAuthProvider::new(vec![oidc_provider, local_provider])),
                    None,
                )
            } else {
                tracing::warn!(
                    "OIDC auth provider configured but no OIDC config found; falling back to local-token"
                );
                (build_local(), None)
            }
        }
        "webauthn" => {
            let local_provider = build_local();

            if let Some(wn_config) = webauthn_config {
                let store = Arc::new(webauthn_store::InMemoryWebAuthnStore::new());

                match webauthn_provider::WebAuthnProvider::new(
                    &wn_config,
                    store,
                    Arc::clone(&local_provider),
                ) {
                    Ok(wn_provider) => {
                        let wn_arc: Arc<webauthn_provider::WebAuthnProvider> =
                            Arc::new(wn_provider);
                        // Include both local-token and WebAuthn in the multi-provider.
                        // Local-token is tried first for bearer token validation.
                        let wn_as_auth: Arc<dyn AuthProvider> = wn_arc.clone();
                        let multi = Arc::new(MultiAuthProvider::new(vec![
                            Arc::clone(&local_provider),
                            wn_as_auth,
                        ]));
                        (multi, Some(wn_arc))
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "failed to create WebAuthn provider; falling back to local-token"
                        );
                        (local_provider, None)
                    }
                }
            } else {
                tracing::warn!(
                    "WebAuthn auth provider configured but no WebAuthn config found; falling back to local-token"
                );
                (local_provider, None)
            }
        }
        _ => (build_local(), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// Verify the trait is object-safe by constructing a trait object.
    #[test]
    fn auth_provider_is_object_safe() {
        fn _assert_object_safe(_p: &dyn AuthProvider) {}
    }

    #[tokio::test]
    async fn multi_provider_tries_each_in_order() {
        let provider1 = Arc::new(local_token::LocalTokenProvider::new());
        let provider2 = Arc::new(local_token::LocalTokenProvider::new());

        // Generate a token on provider2.
        let req = TokenRequest {
            passphrase: "test".into(),
            expires_in_days: Some(30),
        };
        let resp = provider2.generate_token(&req).await.unwrap();

        // Multi-provider should find it via provider2 fallback.
        let multi = MultiAuthProvider::new(vec![
            provider1 as Arc<dyn AuthProvider>,
            provider2 as Arc<dyn AuthProvider>,
        ]);
        let identity = multi.validate_token(&resp.token).await.unwrap();
        assert_eq!(identity.token_id, resp.token_id);
    }

    #[tokio::test]
    async fn multi_provider_returns_first_match() {
        let provider1 = Arc::new(local_token::LocalTokenProvider::new());
        let provider2 = Arc::new(local_token::LocalTokenProvider::new());

        // Generate tokens on both.
        let req = TokenRequest {
            passphrase: "test".into(),
            expires_in_days: Some(30),
        };
        let resp1 = provider1.generate_token(&req).await.unwrap();
        let _resp2 = provider2
            .generate_token(&TokenRequest {
                passphrase: "test2".into(),
                expires_in_days: Some(30),
            })
            .await
            .unwrap();

        let multi = MultiAuthProvider::new(vec![
            provider1 as Arc<dyn AuthProvider>,
            provider2 as Arc<dyn AuthProvider>,
        ]);

        // Token from provider1 should be found first.
        let identity = multi.validate_token(&resp1.token).await.unwrap();
        assert_eq!(identity.token_id, resp1.token_id);
    }

    #[tokio::test]
    async fn multi_provider_returns_error_when_all_fail() {
        let provider = Arc::new(local_token::LocalTokenProvider::new());
        let multi =
            MultiAuthProvider::new(vec![provider as Arc<dyn AuthProvider>]);
        let err = multi.validate_token("nonexistent").await.unwrap_err();
        assert!(matches!(err, AuthError::TokenNotFound));
    }

    #[tokio::test]
    async fn multi_provider_list_tokens_aggregates() {
        let provider1 = Arc::new(local_token::LocalTokenProvider::new());
        let provider2 = Arc::new(local_token::LocalTokenProvider::new());

        let req = TokenRequest {
            passphrase: "test".into(),
            expires_in_days: Some(30),
        };
        provider1.generate_token(&req).await.unwrap();
        provider2
            .generate_token(&TokenRequest {
                passphrase: "test2".into(),
                expires_in_days: Some(30),
            })
            .await
            .unwrap();

        let multi = MultiAuthProvider::new(vec![
            provider1 as Arc<dyn AuthProvider>,
            provider2 as Arc<dyn AuthProvider>,
        ]);

        let tokens = multi.list_tokens().await.unwrap();
        assert_eq!(tokens.len(), 2);
    }

    #[test]
    fn build_auth_provider_local_token_default() {
        let (provider, wn) = build_auth_provider("local-token", None, None, None);
        // Should return successfully (a LocalTokenProvider).
        let _: &dyn AuthProvider = &*provider;
        assert!(wn.is_none());
    }

    #[test]
    fn build_auth_provider_oidc_without_config_falls_back() {
        let (provider, wn) = build_auth_provider("oidc", None, None, None);
        let _: &dyn AuthProvider = &*provider;
        assert!(wn.is_none());
    }

    #[test]
    fn build_auth_provider_oidc_with_config() {
        let oidc_config = oidc::OidcConfig {
            issuer_url: "http://localhost:3751".into(),
            client_id: "test".into(),
            client_secret: None,
            jwks_uri: None,
            audience: None,
        };
        let (provider, wn) = build_auth_provider("oidc", Some(oidc_config), None, None);
        let _: &dyn AuthProvider = &*provider;
        assert!(wn.is_none());
    }

    #[test]
    fn build_auth_provider_unknown_falls_back_to_local() {
        let (provider, wn) = build_auth_provider("unknown", None, None, None);
        let _: &dyn AuthProvider = &*provider;
        assert!(wn.is_none());
    }

    #[test]
    fn build_auth_provider_webauthn_with_config() {
        let wn_config = webauthn_provider::WebAuthnConfig {
            rp_name: "Life Engine".into(),
            rp_id: "localhost".into(),
            rp_origin: "http://localhost:3750".into(),
            challenge_ttl_secs: 300,
        };
        let (provider, wn) = build_auth_provider("webauthn", None, Some(wn_config), None);
        let _: &dyn AuthProvider = &*provider;
        assert!(wn.is_some());
    }

    #[test]
    fn build_auth_provider_webauthn_without_config_falls_back() {
        let (provider, wn) = build_auth_provider("webauthn", None, None, None);
        let _: &dyn AuthProvider = &*provider;
        assert!(wn.is_none());
    }
}
