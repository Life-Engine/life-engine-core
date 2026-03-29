//! WebAuthn credential storage for passkey registration and authentication.
//!
//! Provides the `WebAuthnCredentialStore` trait for persisting passkey
//! credentials and an `InMemoryWebAuthnStore` implementation backed by
//! a `tokio::sync::RwLock<HashMap>` for development and testing.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;
use webauthn_rs::prelude::{AuthenticationResult, Passkey};

use crate::legacy::types::AuthError;

/// A stored passkey credential with user-facing metadata.
///
/// Wraps the `webauthn_rs::prelude::Passkey` with a human-readable label,
/// creation timestamp, and last-used timestamp for display and management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredPasskey {
    /// Unique identifier for this stored passkey entry.
    pub id: uuid::Uuid,
    /// The user ID that owns this passkey.
    pub user_id: String,
    /// The serialised WebAuthn passkey credential.
    pub passkey: Passkey,
    /// A user-friendly label for this passkey (e.g. "MacBook Pro Touch ID").
    pub label: String,
    /// When this passkey was registered.
    pub created_at: DateTime<Utc>,
    /// When this passkey was last used for authentication. `None` if never used.
    pub last_used_at: Option<DateTime<Utc>>,
}

/// Errors specific to WebAuthn credential storage operations.
///
/// Wraps `AuthError` variants and adds passkey-specific error cases.
#[derive(Debug, thiserror::Error)]
pub enum WebAuthnStoreError {
    /// The requested passkey was not found.
    #[error("passkey not found")]
    PasskeyNotFound,
    /// A passkey with the same credential ID already exists for this user.
    #[error("passkey already exists")]
    PasskeyAlreadyExists,
    /// An underlying auth error occurred.
    #[error(transparent)]
    Auth(#[from] AuthError),
    /// An internal storage error occurred.
    #[error("webauthn store error: {0}")]
    #[allow(dead_code)]
    Internal(String),
}

/// Trait for pluggable WebAuthn credential storage backends.
///
/// Implementations persist `StoredPasskey` entries keyed by user ID.
/// Each user can have multiple passkeys. Methods are async to support
/// both in-memory and database-backed implementations.
#[async_trait]
pub trait WebAuthnCredentialStore: Send + Sync {
    /// Store a new passkey for a user.
    ///
    /// Returns `WebAuthnStoreError::PasskeyAlreadyExists` if a passkey with
    /// the same credential ID already exists for the given user.
    async fn store_passkey(
        &self,
        user_id: &str,
        passkey: Passkey,
        label: &str,
    ) -> Result<StoredPasskey, WebAuthnStoreError>;

    /// Retrieve all passkeys for a given user.
    ///
    /// Returns an empty `Vec` if the user has no registered passkeys.
    async fn get_passkeys_for_user(
        &self,
        user_id: &str,
    ) -> Result<Vec<StoredPasskey>, WebAuthnStoreError>;

    /// Retrieve a specific passkey by its stored entry ID.
    ///
    /// Returns `WebAuthnStoreError::PasskeyNotFound` if no passkey with
    /// the given ID exists.
    #[allow(dead_code)]
    async fn get_passkey_by_id(
        &self,
        passkey_id: &uuid::Uuid,
    ) -> Result<StoredPasskey, WebAuthnStoreError>;

    /// Update the passkey credential from an authentication result.
    ///
    /// Applies counter, backup-state, and backup-eligibility changes from
    /// the `AuthenticationResult` to the stored passkey and updates the
    /// last-used timestamp. Called after each successful authentication.
    ///
    /// Returns `WebAuthnStoreError::PasskeyNotFound` if the passkey does
    /// not exist.
    async fn update_passkey_counter(
        &self,
        passkey_id: &uuid::Uuid,
        auth_result: &AuthenticationResult,
    ) -> Result<(), WebAuthnStoreError>;

    /// Remove a passkey by its stored entry ID.
    ///
    /// Returns `WebAuthnStoreError::PasskeyNotFound` if the passkey does
    /// not exist.
    async fn remove_passkey(
        &self,
        passkey_id: &uuid::Uuid,
    ) -> Result<(), WebAuthnStoreError>;

    /// List all stored passkeys across all users.
    ///
    /// Intended for administrative use. Returns an empty `Vec` if no
    /// passkeys are stored.
    #[allow(dead_code)]
    async fn list_passkeys(&self) -> Result<Vec<StoredPasskey>, WebAuthnStoreError>;
}

/// In-memory WebAuthn credential store.
///
/// Uses a `tokio::sync::RwLock<HashMap<String, Vec<StoredPasskey>>>` for
/// concurrent access. Suitable for development and testing. Data is lost
/// when the store is dropped.
#[derive(Debug)]
pub struct InMemoryWebAuthnStore {
    /// Passkeys keyed by user ID, each user can have multiple passkeys.
    store: RwLock<HashMap<String, Vec<StoredPasskey>>>,
}

impl InMemoryWebAuthnStore {
    /// Create a new empty in-memory WebAuthn credential store.
    pub fn new() -> Self {
        Self {
            store: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryWebAuthnStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl WebAuthnCredentialStore for InMemoryWebAuthnStore {
    async fn store_passkey(
        &self,
        user_id: &str,
        passkey: Passkey,
        label: &str,
    ) -> Result<StoredPasskey, WebAuthnStoreError> {
        let mut store = self.store.write().await;
        let user_passkeys = store.entry(user_id.to_string()).or_default();

        // Check for duplicate credential ID.
        let new_cred_id = passkey.cred_id().clone();
        for existing in user_passkeys.iter() {
            if *existing.passkey.cred_id() == new_cred_id {
                return Err(WebAuthnStoreError::PasskeyAlreadyExists);
            }
        }

        let stored = StoredPasskey {
            id: uuid::Uuid::new_v4(),
            user_id: user_id.to_string(),
            passkey,
            label: label.to_string(),
            created_at: Utc::now(),
            last_used_at: None,
        };

        user_passkeys.push(stored.clone());
        Ok(stored)
    }

    async fn get_passkeys_for_user(
        &self,
        user_id: &str,
    ) -> Result<Vec<StoredPasskey>, WebAuthnStoreError> {
        let store = self.store.read().await;
        Ok(store.get(user_id).cloned().unwrap_or_default())
    }

    async fn get_passkey_by_id(
        &self,
        passkey_id: &uuid::Uuid,
    ) -> Result<StoredPasskey, WebAuthnStoreError> {
        let store = self.store.read().await;
        for passkeys in store.values() {
            for pk in passkeys {
                if pk.id == *passkey_id {
                    return Ok(pk.clone());
                }
            }
        }
        Err(WebAuthnStoreError::PasskeyNotFound)
    }

    async fn update_passkey_counter(
        &self,
        passkey_id: &uuid::Uuid,
        auth_result: &AuthenticationResult,
    ) -> Result<(), WebAuthnStoreError> {
        let mut store = self.store.write().await;
        for passkeys in store.values_mut() {
            for pk in passkeys.iter_mut() {
                if pk.id == *passkey_id {
                    pk.passkey.update_credential(auth_result);
                    pk.last_used_at = Some(Utc::now());
                    return Ok(());
                }
            }
        }
        Err(WebAuthnStoreError::PasskeyNotFound)
    }

    async fn remove_passkey(
        &self,
        passkey_id: &uuid::Uuid,
    ) -> Result<(), WebAuthnStoreError> {
        let mut store = self.store.write().await;
        for passkeys in store.values_mut() {
            let before = passkeys.len();
            passkeys.retain(|pk| pk.id != *passkey_id);
            if passkeys.len() < before {
                return Ok(());
            }
        }
        Err(WebAuthnStoreError::PasskeyNotFound)
    }

    async fn list_passkeys(&self) -> Result<Vec<StoredPasskey>, WebAuthnStoreError> {
        let store = self.store.read().await;
        let all: Vec<StoredPasskey> = store
            .values()
            .flat_map(|v| v.iter().cloned())
            .collect();
        Ok(all)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use webauthn_rs::prelude::AuthenticationResult;

    /// Build a `Webauthn` instance for testing.
    fn test_webauthn() -> webauthn_rs::Webauthn {
        let rp_id = "example.com";
        let rp_origin = url::Url::parse("https://example.com").expect("valid URL");
        let builder = webauthn_rs::WebauthnBuilder::new(rp_id, &rp_origin)
            .expect("valid WebAuthn builder");
        builder.build().expect("valid Webauthn instance")
    }

    /// Helper to base64url-encode bytes (no padding).
    fn base64_urlsafe_encode(data: &[u8]) -> String {
        use base64::Engine;
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
    }

    /// Create a fake `Passkey` for testing via serde deserialisation.
    ///
    /// The `danger-allow-state-serialisation` feature enables
    /// `Serialize`/`Deserialize` on `Passkey`, allowing us to construct
    /// test credentials without running a real WebAuthn ceremony.
    fn make_test_passkey(cred_id_bytes: &[u8]) -> Passkey {
        let cred_id = base64_urlsafe_encode(cred_id_bytes);
        let json = serde_json::json!({
            "cred": {
                "cred_id": cred_id,
                "cred": {
                    "type_": "ES256",
                    "key": {
                        "EC_EC2": {
                            "curve": "SECP256R1",
                            "x": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                            "y": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
                        }
                    }
                },
                "counter": 0,
                "transports": null,
                "user_verified": true,
                "backup_eligible": false,
                "backup_state": false,
                "registration_policy": "preferred",
                "extensions": {
                    "cred_protect": "NotRequested",
                    "hmac_create_secret": "NotRequested",
                    "appid": "NotRequested",
                    "cred_props": "NotRequested"
                },
                "attestation": {
                    "data": "None",
                    "metadata": "None"
                },
                "attestation_format": "None"
            }
        });
        serde_json::from_value(json).expect("valid test Passkey")
    }

    /// Create a fake `AuthenticationResult` for testing via serde deserialisation.
    ///
    /// The `danger-allow-state-serialisation` feature enables
    /// `Deserialize` on `AuthenticationResult`.
    fn make_test_auth_result(cred_id_bytes: &[u8], counter: u32) -> AuthenticationResult {
        let cred_id = base64_urlsafe_encode(cred_id_bytes);
        let json = serde_json::json!({
            "cred_id": cred_id,
            "needs_update": true,
            "user_verified": true,
            "backup_state": false,
            "backup_eligible": false,
            "counter": counter,
            "extensions": {
                "appid": "NotRequested",
                "cred_props": "NotRequested"
            }
        });
        serde_json::from_value(json).expect("valid test AuthenticationResult")
    }

    #[tokio::test]
    async fn store_and_retrieve_passkey() {
        let store = InMemoryWebAuthnStore::new();
        let passkey = make_test_passkey(b"cred-001");

        let stored = store
            .store_passkey("user-1", passkey, "My YubiKey")
            .await
            .unwrap();

        assert_eq!(stored.user_id, "user-1");
        assert_eq!(stored.label, "My YubiKey");
        assert!(stored.last_used_at.is_none());
    }

    #[tokio::test]
    async fn get_passkeys_for_user_returns_all() {
        let store = InMemoryWebAuthnStore::new();

        let pk1 = make_test_passkey(b"cred-a01");
        let pk2 = make_test_passkey(b"cred-a02");

        store
            .store_passkey("user-1", pk1, "Key 1")
            .await
            .unwrap();
        store
            .store_passkey("user-1", pk2, "Key 2")
            .await
            .unwrap();

        let passkeys = store.get_passkeys_for_user("user-1").await.unwrap();
        assert_eq!(passkeys.len(), 2);
    }

    #[tokio::test]
    async fn get_passkeys_for_unknown_user_returns_empty() {
        let store = InMemoryWebAuthnStore::new();
        let passkeys = store
            .get_passkeys_for_user("no-such-user")
            .await
            .unwrap();
        assert!(passkeys.is_empty());
    }

    #[tokio::test]
    async fn get_passkey_by_id_found() {
        let store = InMemoryWebAuthnStore::new();
        let passkey = make_test_passkey(b"cred-b01");

        let stored = store
            .store_passkey("user-2", passkey, "Touch ID")
            .await
            .unwrap();

        let retrieved = store.get_passkey_by_id(&stored.id).await.unwrap();
        assert_eq!(retrieved.id, stored.id);
        assert_eq!(retrieved.label, "Touch ID");
    }

    #[tokio::test]
    async fn get_passkey_by_id_not_found() {
        let store = InMemoryWebAuthnStore::new();
        let fake_id = uuid::Uuid::new_v4();
        let err = store.get_passkey_by_id(&fake_id).await.unwrap_err();
        assert!(matches!(err, WebAuthnStoreError::PasskeyNotFound));
    }

    #[tokio::test]
    async fn update_passkey_counter_succeeds() {
        let store = InMemoryWebAuthnStore::new();
        let cred_id_bytes = b"cred-c01";
        let passkey = make_test_passkey(cred_id_bytes);

        let stored = store
            .store_passkey("user-3", passkey, "Security Key")
            .await
            .unwrap();

        // Counter starts at 0, update to 5 via an AuthenticationResult.
        let auth_result = make_test_auth_result(cred_id_bytes, 5);
        store
            .update_passkey_counter(&stored.id, &auth_result)
            .await
            .unwrap();

        let updated = store.get_passkey_by_id(&stored.id).await.unwrap();
        assert!(updated.last_used_at.is_some());
    }

    #[tokio::test]
    async fn update_passkey_counter_not_found() {
        let store = InMemoryWebAuthnStore::new();
        let fake_id = uuid::Uuid::new_v4();
        let auth_result = make_test_auth_result(b"fake-cred", 1);
        let err = store
            .update_passkey_counter(&fake_id, &auth_result)
            .await
            .unwrap_err();
        assert!(matches!(err, WebAuthnStoreError::PasskeyNotFound));
    }

    #[tokio::test]
    async fn remove_passkey_succeeds() {
        let store = InMemoryWebAuthnStore::new();
        let passkey = make_test_passkey(b"cred-d01");

        let stored = store
            .store_passkey("user-4", passkey, "Old Key")
            .await
            .unwrap();

        store.remove_passkey(&stored.id).await.unwrap();

        let err = store.get_passkey_by_id(&stored.id).await.unwrap_err();
        assert!(matches!(err, WebAuthnStoreError::PasskeyNotFound));
    }

    #[tokio::test]
    async fn remove_passkey_not_found() {
        let store = InMemoryWebAuthnStore::new();
        let fake_id = uuid::Uuid::new_v4();
        let err = store.remove_passkey(&fake_id).await.unwrap_err();
        assert!(matches!(err, WebAuthnStoreError::PasskeyNotFound));
    }

    #[tokio::test]
    async fn list_passkeys_returns_all_users() {
        let store = InMemoryWebAuthnStore::new();

        let pk1 = make_test_passkey(b"cred-e01");
        let pk2 = make_test_passkey(b"cred-e02");
        let pk3 = make_test_passkey(b"cred-e03");

        store
            .store_passkey("user-a", pk1, "Key A")
            .await
            .unwrap();
        store
            .store_passkey("user-a", pk2, "Key B")
            .await
            .unwrap();
        store
            .store_passkey("user-b", pk3, "Key C")
            .await
            .unwrap();

        let all = store.list_passkeys().await.unwrap();
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn list_passkeys_empty_store() {
        let store = InMemoryWebAuthnStore::new();
        let all = store.list_passkeys().await.unwrap();
        assert!(all.is_empty());
    }

    #[tokio::test]
    async fn duplicate_credential_id_rejected() {
        let store = InMemoryWebAuthnStore::new();
        let passkey1 = make_test_passkey(b"same-cred-id");
        let passkey2 = make_test_passkey(b"same-cred-id");

        store
            .store_passkey("user-x", passkey1, "First")
            .await
            .unwrap();

        let err = store
            .store_passkey("user-x", passkey2, "Duplicate")
            .await
            .unwrap_err();
        assert!(matches!(err, WebAuthnStoreError::PasskeyAlreadyExists));
    }

    #[tokio::test]
    async fn same_credential_id_different_users_allowed() {
        let store = InMemoryWebAuthnStore::new();
        let passkey1 = make_test_passkey(b"shared-cred");
        let passkey2 = make_test_passkey(b"shared-cred");

        store
            .store_passkey("user-1", passkey1, "User 1 Key")
            .await
            .unwrap();
        store
            .store_passkey("user-2", passkey2, "User 2 Key")
            .await
            .unwrap();

        let u1 = store.get_passkeys_for_user("user-1").await.unwrap();
        let u2 = store.get_passkeys_for_user("user-2").await.unwrap();
        assert_eq!(u1.len(), 1);
        assert_eq!(u2.len(), 1);
    }

    #[tokio::test]
    async fn remove_one_passkey_leaves_others() {
        let store = InMemoryWebAuthnStore::new();
        let pk1 = make_test_passkey(b"cred-f01");
        let pk2 = make_test_passkey(b"cred-f02");

        let stored1 = store
            .store_passkey("user-y", pk1, "Key 1")
            .await
            .unwrap();
        let _stored2 = store
            .store_passkey("user-y", pk2, "Key 2")
            .await
            .unwrap();

        store.remove_passkey(&stored1.id).await.unwrap();

        let remaining = store.get_passkeys_for_user("user-y").await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].label, "Key 2");
    }

    #[tokio::test]
    async fn stored_passkey_serialization_roundtrip() {
        let store = InMemoryWebAuthnStore::new();
        let passkey = make_test_passkey(b"cred-g01");

        let stored = store
            .store_passkey("user-s", passkey, "Serializable")
            .await
            .unwrap();

        let json = serde_json::to_string(&stored).expect("serialize StoredPasskey");
        let deserialized: StoredPasskey =
            serde_json::from_str(&json).expect("deserialize StoredPasskey");

        assert_eq!(deserialized.id, stored.id);
        assert_eq!(deserialized.user_id, stored.user_id);
        assert_eq!(deserialized.label, stored.label);
    }

    #[test]
    fn default_store_is_empty() {
        let store = InMemoryWebAuthnStore::default();
        // Cannot call async here, just verify it constructs.
        let _store = store;
    }

    #[test]
    fn webauthn_store_error_display() {
        let err = WebAuthnStoreError::PasskeyNotFound;
        assert_eq!(err.to_string(), "passkey not found");

        let err = WebAuthnStoreError::PasskeyAlreadyExists;
        assert_eq!(err.to_string(), "passkey already exists");

        let err = WebAuthnStoreError::Internal("disk full".into());
        assert_eq!(err.to_string(), "webauthn store error: disk full");
    }

    #[test]
    fn webauthn_store_error_from_auth_error() {
        let auth_err = AuthError::Internal("test".into());
        let store_err: WebAuthnStoreError = auth_err.into();
        assert!(matches!(store_err, WebAuthnStoreError::Auth(_)));
    }

    /// Verify the trait is object-safe by constructing a trait object.
    #[test]
    fn webauthn_credential_store_is_object_safe() {
        fn _assert_object_safe(_s: &dyn WebAuthnCredentialStore) {}
    }

    /// Verify the `Webauthn` builder works (sanity check for the dependency).
    #[test]
    fn webauthn_builder_sanity_check() {
        let _wn = test_webauthn();
    }
}
