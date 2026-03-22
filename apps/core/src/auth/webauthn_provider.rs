//! WebAuthn authentication provider for passkey registration and authentication.
//!
//! Wraps `webauthn-rs` to provide passkey-based FIDO2/WebAuthn ceremonies.
//! Challenge state is cached in memory with a configurable TTL. After
//! successful authentication, a session token is generated via the local
//! token provider.

use crate::auth::types::{AuthError, AuthIdentity, TokenInfo, TokenRequest, TokenResponse};
use crate::auth::webauthn_store::{StoredPasskey, WebAuthnCredentialStore, WebAuthnStoreError};
use crate::auth::AuthProvider;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use webauthn_rs::prelude::*;

/// WebAuthn configuration for the relying party.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebAuthnConfig {
    /// Relying party name (e.g. "Life Engine").
    pub rp_name: String,
    /// Relying party ID (e.g. "localhost" or "example.com").
    pub rp_id: String,
    /// Relying party origin URL (e.g. "https://example.com" or "http://localhost:3750").
    pub rp_origin: String,
    /// Challenge TTL in seconds (default 300 = 5 minutes).
    #[serde(default = "default_challenge_ttl")]
    pub challenge_ttl_secs: u64,
}

/// Default challenge TTL: 5 minutes.
fn default_challenge_ttl() -> u64 {
    300
}

/// The state of a pending WebAuthn challenge ceremony.
enum ChallengeState {
    /// A pending passkey registration ceremony.
    Registration(PasskeyRegistration),
    /// A pending passkey authentication ceremony.
    Authentication {
        /// The authentication state from webauthn-rs.
        state: PasskeyAuthentication,
        /// The user ID associated with this authentication.
        user_id: String,
    },
}

/// A cached challenge entry with creation timestamp for TTL enforcement.
struct ChallengeEntry {
    /// The ceremony state (registration or authentication).
    state: ChallengeState,
    /// When this challenge was created.
    created_at: Instant,
}

/// WebAuthn authentication provider.
///
/// Manages passkey registration and authentication ceremonies using
/// `webauthn-rs`. Challenge state is cached in memory with automatic
/// expiration. After successful authentication, delegates to the local
/// token provider for session token generation.
pub struct WebAuthnProvider {
    /// The webauthn-rs instance configured for this relying party.
    webauthn: webauthn_rs::Webauthn,
    /// Credential storage backend.
    store: Arc<dyn WebAuthnCredentialStore>,
    /// In-flight challenge states keyed by challenge ID.
    challenges: RwLock<HashMap<String, ChallengeEntry>>,
    /// How long challenges remain valid.
    challenge_ttl: Duration,
    /// Local token provider for generating session tokens after authentication.
    local_token_provider: Arc<dyn AuthProvider>,
}

impl WebAuthnProvider {
    /// Create a new WebAuthn provider from the given configuration.
    ///
    /// Returns `AuthError` if the configuration contains an invalid origin URL
    /// or if the WebAuthn builder fails.
    pub fn new(
        config: &WebAuthnConfig,
        store: Arc<dyn WebAuthnCredentialStore>,
        local_token_provider: Arc<dyn AuthProvider>,
    ) -> Result<Self, AuthError> {
        let rp_origin = url::Url::parse(&config.rp_origin).map_err(|e| {
            AuthError::Internal(format!(
                "invalid WebAuthn rp_origin '{}': {e}",
                config.rp_origin
            ))
        })?;

        let builder = WebauthnBuilder::new(&config.rp_id, &rp_origin)
            .map_err(|e| AuthError::Internal(format!("WebAuthn builder error: {e}")))?;

        let builder = builder.rp_name(&config.rp_name);

        let webauthn = builder
            .build()
            .map_err(|e| AuthError::Internal(format!("WebAuthn build error: {e}")))?;

        Ok(Self {
            webauthn,
            store,
            challenges: RwLock::new(HashMap::new()),
            challenge_ttl: Duration::from_secs(config.challenge_ttl_secs),
            local_token_provider,
        })
    }

    /// Begin passkey registration for a user.
    ///
    /// Returns a challenge ID and the `CreationChallengeResponse` that should
    /// be sent to the browser. The challenge ID must be passed back when
    /// calling `finish_registration`.
    pub async fn start_registration(
        &self,
        user_id: &str,
        user_name: &str,
    ) -> Result<(String, CreationChallengeResponse), AuthError> {
        // Look up existing credentials to exclude from registration.
        let existing = self
            .store
            .get_passkeys_for_user(user_id)
            .await
            .map_err(|e| AuthError::Internal(format!("failed to query existing passkeys: {e}")))?;

        let exclude_creds: Vec<CredentialID> = existing
            .iter()
            .map(|pk| pk.passkey.cred_id().clone())
            .collect();

        let exclude = if exclude_creds.is_empty() {
            None
        } else {
            Some(exclude_creds)
        };

        // Generate a deterministic UUID from the user_id string by hashing it
        // and using the first 16 bytes as a UUID. This ensures the same user_id
        // always maps to the same UUID for webauthn-rs.
        let user_uuid = {
            use sha2::{Digest, Sha256};
            let hash = Sha256::digest(user_id.as_bytes());
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(&hash[..16]);
            uuid::Uuid::from_bytes(bytes)
        };

        let (ccr, reg_state) = self
            .webauthn
            .start_passkey_registration(user_uuid, user_name, user_name, exclude)
            .map_err(|e| AuthError::Internal(format!("start_passkey_registration failed: {e}")))?;

        let challenge_id = uuid::Uuid::new_v4().to_string();
        let entry = ChallengeEntry {
            state: ChallengeState::Registration(reg_state),
            created_at: Instant::now(),
        };

        self.challenges
            .write()
            .await
            .insert(challenge_id.clone(), entry);

        tracing::info!(
            user_id = %user_id,
            challenge_id = %challenge_id,
            "passkey registration started"
        );

        Ok((challenge_id, ccr))
    }

    /// Complete passkey registration.
    ///
    /// Validates the browser's response against the stored challenge state,
    /// then persists the new passkey in the credential store.
    pub async fn finish_registration(
        &self,
        challenge_id: &str,
        user_id: &str,
        label: &str,
        response: &RegisterPublicKeyCredential,
    ) -> Result<StoredPasskey, AuthError> {
        let entry = self
            .challenges
            .write()
            .await
            .remove(challenge_id)
            .ok_or(AuthError::Internal(
                "challenge not found or expired".into(),
            ))?;

        // Verify TTL.
        if entry.created_at.elapsed() > self.challenge_ttl {
            return Err(AuthError::Internal("challenge expired".into()));
        }

        let reg_state = match entry.state {
            ChallengeState::Registration(s) => s,
            ChallengeState::Authentication { .. } => {
                return Err(AuthError::Internal(
                    "challenge ID belongs to an authentication ceremony, not registration".into(),
                ));
            }
        };

        let passkey = self
            .webauthn
            .finish_passkey_registration(response, &reg_state)
            .map_err(|e| {
                AuthError::Internal(format!("finish_passkey_registration failed: {e}"))
            })?;

        let stored = self
            .store
            .store_passkey(user_id, passkey, label)
            .await
            .map_err(|e| match e {
                WebAuthnStoreError::PasskeyAlreadyExists => {
                    AuthError::Internal("passkey already registered".into())
                }
                other => AuthError::Internal(format!("store_passkey failed: {other}")),
            })?;

        tracing::info!(
            user_id = %user_id,
            passkey_id = %stored.id,
            "passkey registration completed"
        );

        Ok(stored)
    }

    /// Begin passkey authentication for a user.
    ///
    /// Returns a challenge ID and the `RequestChallengeResponse` that should
    /// be sent to the browser. The challenge ID must be passed back when
    /// calling `finish_authentication`.
    pub async fn start_authentication(
        &self,
        user_id: &str,
    ) -> Result<(String, RequestChallengeResponse), AuthError> {
        let passkeys = self
            .store
            .get_passkeys_for_user(user_id)
            .await
            .map_err(|e| AuthError::Internal(format!("failed to query passkeys: {e}")))?;

        if passkeys.is_empty() {
            return Err(AuthError::Internal(
                "no passkeys registered for this user".into(),
            ));
        }

        let creds: Vec<Passkey> = passkeys.iter().map(|pk| pk.passkey.clone()).collect();

        let (rcr, auth_state) = self
            .webauthn
            .start_passkey_authentication(&creds)
            .map_err(|e| {
                AuthError::Internal(format!("start_passkey_authentication failed: {e}"))
            })?;

        let challenge_id = uuid::Uuid::new_v4().to_string();
        let entry = ChallengeEntry {
            state: ChallengeState::Authentication {
                state: auth_state,
                user_id: user_id.to_string(),
            },
            created_at: Instant::now(),
        };

        self.challenges
            .write()
            .await
            .insert(challenge_id.clone(), entry);

        tracing::info!(
            user_id = %user_id,
            challenge_id = %challenge_id,
            "passkey authentication started"
        );

        Ok((challenge_id, rcr))
    }

    /// Complete passkey authentication and return a session token.
    ///
    /// Validates the browser's response against the stored challenge state,
    /// updates the passkey counter, and generates a session token via the
    /// local token provider.
    pub async fn finish_authentication(
        &self,
        challenge_id: &str,
        response: &PublicKeyCredential,
    ) -> Result<TokenResponse, AuthError> {
        let entry = self
            .challenges
            .write()
            .await
            .remove(challenge_id)
            .ok_or(AuthError::Internal(
                "challenge not found or expired".into(),
            ))?;

        // Verify TTL.
        if entry.created_at.elapsed() > self.challenge_ttl {
            return Err(AuthError::Internal("challenge expired".into()));
        }

        let (auth_state, user_id) = match entry.state {
            ChallengeState::Authentication { state, user_id } => (state, user_id),
            ChallengeState::Registration(_) => {
                return Err(AuthError::Internal(
                    "challenge ID belongs to a registration ceremony, not authentication".into(),
                ));
            }
        };

        let auth_result = self
            .webauthn
            .finish_passkey_authentication(response, &auth_state)
            .map_err(|e| {
                AuthError::Internal(format!("finish_passkey_authentication failed: {e}"))
            })?;

        // Update the passkey counter in the store.
        // Find which passkey was used by matching the credential ID from the auth result.
        let passkeys = self
            .store
            .get_passkeys_for_user(&user_id)
            .await
            .map_err(|e| {
                AuthError::Internal(format!(
                    "failed to query passkeys for counter update: {e}"
                ))
            })?;

        for pk in &passkeys {
            if *pk.passkey.cred_id() == *auth_result.cred_id() {
                if let Err(e) = self
                    .store
                    .update_passkey_counter(&pk.id, &auth_result)
                    .await
                {
                    tracing::warn!(
                        passkey_id = %pk.id,
                        error = %e,
                        "failed to update passkey counter (non-fatal)"
                    );
                }
                break;
            }
        }

        // Generate a session token via the local token provider.
        let token_request = TokenRequest {
            passphrase: format!("webauthn:{user_id}"),
            expires_in_days: Some(30),
        };

        let token_response = self
            .local_token_provider
            .generate_token(&token_request)
            .await
            .map_err(|e| AuthError::Internal(format!("failed to generate session token: {e}")))?;

        tracing::info!(
            user_id = %user_id,
            token_id = %token_response.token_id,
            "passkey authentication completed, session token issued"
        );

        Ok(token_response)
    }

    /// Clean up expired challenges from the in-memory cache.
    ///
    /// Call this periodically to prevent unbounded memory growth from
    /// abandoned ceremony flows.
    pub async fn cleanup_expired_challenges(&self) {
        let mut challenges = self.challenges.write().await;
        let before = challenges.len();
        challenges.retain(|_, entry| entry.created_at.elapsed() <= self.challenge_ttl);
        let removed = before - challenges.len();
        if removed > 0 {
            tracing::debug!(removed = removed, "expired WebAuthn challenges cleaned up");
        }
    }

    /// List passkeys for a user.
    ///
    /// Returns all registered passkeys with their metadata.
    pub async fn list_passkeys(&self, user_id: &str) -> Result<Vec<StoredPasskey>, AuthError> {
        self.store
            .get_passkeys_for_user(user_id)
            .await
            .map_err(|e| AuthError::Internal(format!("failed to list passkeys: {e}")))
    }

    /// Remove a passkey by its stored entry ID.
    ///
    /// Returns an error if the passkey does not exist.
    pub async fn remove_passkey(&self, passkey_id: &uuid::Uuid) -> Result<(), AuthError> {
        self.store
            .remove_passkey(passkey_id)
            .await
            .map_err(|e| match e {
                WebAuthnStoreError::PasskeyNotFound => {
                    AuthError::Internal("passkey not found".into())
                }
                other => AuthError::Internal(format!("remove_passkey failed: {other}")),
            })
    }

    /// Return the number of pending (in-flight) challenges.
    ///
    /// Useful for monitoring and tests.
    #[allow(dead_code)]
    pub async fn pending_challenge_count(&self) -> usize {
        self.challenges.read().await.len()
    }
}

impl std::fmt::Debug for WebAuthnProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebAuthnProvider")
            .field("challenge_ttl", &self.challenge_ttl)
            .finish()
    }
}

#[async_trait]
impl AuthProvider for WebAuthnProvider {
    /// WebAuthn does not validate bearer tokens directly.
    ///
    /// Token validation is handled by the local token provider which is
    /// included alongside WebAuthn in the `MultiAuthProvider`.
    async fn validate_token(&self, _token: &str) -> Result<AuthIdentity, AuthError> {
        Err(AuthError::Internal(
            "WebAuthn does not validate bearer tokens directly; use the local-token provider"
                .into(),
        ))
    }

    /// WebAuthn uses ceremony endpoints, not `generate_token`.
    async fn generate_token(
        &self,
        _credentials: &TokenRequest,
    ) -> Result<TokenResponse, AuthError> {
        Err(AuthError::Internal(
            "WebAuthn uses ceremony endpoints, not generate_token".into(),
        ))
    }

    /// WebAuthn does not manage bearer tokens.
    async fn revoke_token(&self, _token_id: &str) -> Result<(), AuthError> {
        Err(AuthError::Internal(
            "WebAuthn does not manage bearer tokens".into(),
        ))
    }

    /// Return an empty list; passkeys are managed via dedicated endpoints.
    async fn list_tokens(&self) -> Result<Vec<TokenInfo>, AuthError> {
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::local_token::LocalTokenProvider;
    use crate::auth::webauthn_store::InMemoryWebAuthnStore;

    /// Build a valid `WebAuthnConfig` for testing.
    fn test_config() -> WebAuthnConfig {
        WebAuthnConfig {
            rp_name: "Life Engine Test".into(),
            rp_id: "localhost".into(),
            rp_origin: "http://localhost:3750".into(),
            challenge_ttl_secs: 300,
        }
    }

    /// Build a `WebAuthnConfig` with a very short TTL for expiry tests.
    fn test_config_short_ttl() -> WebAuthnConfig {
        WebAuthnConfig {
            rp_name: "Life Engine Test".into(),
            rp_id: "localhost".into(),
            rp_origin: "http://localhost:3750".into(),
            challenge_ttl_secs: 0, // expires immediately
        }
    }

    /// Build the standard test dependencies and `WebAuthnProvider`.
    fn build_provider(config: &WebAuthnConfig) -> WebAuthnProvider {
        let store = Arc::new(InMemoryWebAuthnStore::new());
        let local_token = Arc::new(LocalTokenProvider::new());
        WebAuthnProvider::new(config, store, local_token).expect("valid provider")
    }

    #[test]
    fn new_provider_with_valid_config() {
        let config = test_config();
        let provider = build_provider(&config);
        assert_eq!(provider.challenge_ttl, Duration::from_secs(300));
    }

    #[test]
    fn new_provider_rejects_invalid_origin() {
        let config = WebAuthnConfig {
            rp_name: "Test".into(),
            rp_id: "localhost".into(),
            rp_origin: "not-a-valid-url".into(),
            challenge_ttl_secs: 300,
        };
        let store = Arc::new(InMemoryWebAuthnStore::new());
        let local_token = Arc::new(LocalTokenProvider::new());
        let err = WebAuthnProvider::new(&config, store, local_token).unwrap_err();
        assert!(matches!(err, AuthError::Internal(_)));
        assert!(err.to_string().contains("invalid WebAuthn rp_origin"));
    }

    #[tokio::test]
    async fn start_registration_returns_challenge() {
        let config = test_config();
        let provider = build_provider(&config);

        let (challenge_id, ccr) = provider
            .start_registration("user-1", "alice")
            .await
            .expect("start_registration should succeed");

        assert!(!challenge_id.is_empty());
        // The CreationChallengeResponse is serializable.
        let json = serde_json::to_value(&ccr).expect("ccr should serialize");
        assert!(json.get("publicKey").is_some());
    }

    #[tokio::test]
    async fn start_registration_stores_challenge() {
        let config = test_config();
        let provider = build_provider(&config);

        assert_eq!(provider.pending_challenge_count().await, 0);

        let (_challenge_id, _ccr) = provider
            .start_registration("user-1", "alice")
            .await
            .expect("start_registration should succeed");

        assert_eq!(provider.pending_challenge_count().await, 1);
    }

    #[tokio::test]
    async fn challenge_expires_after_ttl() {
        let config = test_config_short_ttl();
        let provider = build_provider(&config);

        let (challenge_id, _ccr) = provider
            .start_registration("user-1", "alice")
            .await
            .expect("start_registration should succeed");

        // With TTL=0, the challenge expires immediately.
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        provider.cleanup_expired_challenges().await;
        assert_eq!(provider.pending_challenge_count().await, 0);

        // Verify the challenge_id is gone.
        let challenges = provider.challenges.read().await;
        assert!(!challenges.contains_key(&challenge_id));
    }

    #[tokio::test]
    async fn cleanup_expired_challenges_removes_old_entries() {
        let config = test_config_short_ttl();
        let provider = build_provider(&config);

        // Create multiple challenges.
        provider
            .start_registration("user-1", "alice")
            .await
            .expect("start 1");
        provider
            .start_registration("user-2", "bob")
            .await
            .expect("start 2");

        assert_eq!(provider.pending_challenge_count().await, 2);

        // Wait for them to expire.
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        provider.cleanup_expired_challenges().await;
        assert_eq!(provider.pending_challenge_count().await, 0);
    }

    #[tokio::test]
    async fn cleanup_preserves_unexpired_challenges() {
        let config = test_config(); // 300s TTL
        let provider = build_provider(&config);

        provider
            .start_registration("user-1", "alice")
            .await
            .expect("start");

        provider.cleanup_expired_challenges().await;
        assert_eq!(provider.pending_challenge_count().await, 1);
    }

    #[tokio::test]
    async fn list_passkeys_returns_empty_for_unknown_user() {
        let config = test_config();
        let provider = build_provider(&config);

        let passkeys = provider
            .list_passkeys("no-such-user")
            .await
            .expect("list should succeed");
        assert!(passkeys.is_empty());
    }

    #[tokio::test]
    async fn remove_passkey_for_nonexistent_returns_error() {
        let config = test_config();
        let provider = build_provider(&config);

        let fake_id = uuid::Uuid::new_v4();
        let err = provider.remove_passkey(&fake_id).await.unwrap_err();
        assert!(matches!(err, AuthError::Internal(_)));
        assert!(err.to_string().contains("passkey not found"));
    }

    #[tokio::test]
    async fn start_authentication_with_no_passkeys_fails() {
        let config = test_config();
        let provider = build_provider(&config);

        let err = provider
            .start_authentication("no-passkeys-user")
            .await
            .unwrap_err();
        assert!(matches!(err, AuthError::Internal(_)));
        assert!(err.to_string().contains("no passkeys registered"));
    }

    #[tokio::test]
    async fn auth_provider_validate_token_returns_error() {
        let config = test_config();
        let provider = build_provider(&config);

        let err = provider.validate_token("some-token").await.unwrap_err();
        assert!(matches!(err, AuthError::Internal(_)));
    }

    #[tokio::test]
    async fn auth_provider_generate_token_returns_error() {
        let config = test_config();
        let provider = build_provider(&config);

        let req = TokenRequest {
            passphrase: "test".into(),
            expires_in_days: Some(30),
        };
        let err = provider.generate_token(&req).await.unwrap_err();
        assert!(matches!(err, AuthError::Internal(_)));
    }

    #[tokio::test]
    async fn auth_provider_revoke_token_returns_error() {
        let config = test_config();
        let provider = build_provider(&config);

        let err = provider.revoke_token("some-id").await.unwrap_err();
        assert!(matches!(err, AuthError::Internal(_)));
    }

    #[tokio::test]
    async fn auth_provider_list_tokens_returns_empty() {
        let config = test_config();
        let provider = build_provider(&config);

        let tokens = provider
            .list_tokens()
            .await
            .expect("list_tokens should succeed");
        assert!(tokens.is_empty());
    }

    #[test]
    fn debug_impl_works() {
        let config = test_config();
        let provider = build_provider(&config);
        let debug = format!("{provider:?}");
        assert!(debug.contains("WebAuthnProvider"));
        assert!(debug.contains("challenge_ttl"));
    }

    #[test]
    fn webauthn_config_serialization_roundtrip() {
        let config = test_config();
        let json = serde_json::to_string(&config).expect("serialize");
        let restored: WebAuthnConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.rp_name, config.rp_name);
        assert_eq!(restored.rp_id, config.rp_id);
        assert_eq!(restored.rp_origin, config.rp_origin);
        assert_eq!(restored.challenge_ttl_secs, config.challenge_ttl_secs);
    }

    #[test]
    fn webauthn_config_default_ttl() {
        let json = r#"{
            "rp_name": "Test",
            "rp_id": "localhost",
            "rp_origin": "http://localhost:3750"
        }"#;
        let config: WebAuthnConfig = serde_json::from_str(json).expect("deserialize");
        assert_eq!(config.challenge_ttl_secs, 300);
    }

    #[test]
    fn webauthn_provider_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<WebAuthnProvider>();
    }
}
