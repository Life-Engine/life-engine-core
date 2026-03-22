//! SQLite-backed encrypted credential storage.
//!
//! Implements the `CredentialStore` trait from the plugin SDK, storing
//! encrypted credential values in a dedicated SQLite table. Each
//! credential is scoped to a plugin ID and identified by a key.
//!
//! # Encryption
//!
//! Credential values are encrypted using a simple XOR-based key derivation
//! with a per-instance secret and base64 encoding. This provides basic
//! at-rest protection. A future phase will integrate SQLCipher for
//! full database-level encryption.
//!
//! # Security
//!
//! - Credential values are NEVER logged.
//! - Plugins can only access their own credentials (scoped by plugin_id).

use crate::crypto;
use anyhow::{Context, Result};
use async_trait::async_trait;
use base64::Engine as _;
use life_engine_plugin_sdk::credential_store::CredentialStore;
use rusqlite::params;
use std::sync::Arc;
use tokio::sync::Mutex;
/// SQLite-backed credential store with encryption.
pub struct SqliteCredentialStore {
    /// The SQLite connection, protected by a Mutex for async access.
    conn: Arc<Mutex<rusqlite::Connection>>,
    /// The encryption key derived from the master secret.
    encryption_key: Vec<u8>,
}

impl SqliteCredentialStore {
    /// Create a new credential store backed by the given SQLite connection.
    ///
    /// The `master_secret` is used to derive an encryption key for
    /// credential values. It should be unique per Core instance.
    pub fn new(conn: Arc<Mutex<rusqlite::Connection>>, master_secret: &str) -> Result<Self> {
        let encryption_key = crypto::derive_key(master_secret, crypto::DOMAIN_CREDENTIAL_STORE);
        Ok(Self {
            conn,
            encryption_key,
        })
    }

    /// Initialise the credentials table if it does not exist.
    pub async fn init(&self) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS credentials (
                plugin_id TEXT NOT NULL,
                key TEXT NOT NULL,
                encrypted_value TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (plugin_id, key)
            );",
        )
        .context("failed to create credentials table")?;
        tracing::debug!("credential store initialised");
        Ok(())
    }

    /// Encrypt a plaintext value using AES-256-GCM with the instance key.
    fn encrypt(&self, plaintext: &str) -> String {
        let encrypted = crypto::encrypt(plaintext.as_bytes(), &self.encryption_key)
            .expect("AES-256-GCM encryption should not fail with valid key");
        base64::engine::general_purpose::STANDARD.encode(encrypted)
    }

    /// Decrypt an AES-256-GCM encrypted value back to plaintext.
    fn decrypt(&self, ciphertext: &str) -> Result<String> {
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(ciphertext)
            .context("failed to decode base64 credential value")?;
        let decrypted = crypto::decrypt(&decoded, &self.encryption_key)
            .map_err(|_| anyhow::anyhow!("failed to decrypt credential value"))?;
        String::from_utf8(decrypted).context("decrypted credential is not valid UTF-8")
    }
}

#[async_trait]
impl CredentialStore for SqliteCredentialStore {
    async fn store(
        &self,
        plugin_id: &str,
        key: &str,
        value: &str,
    ) -> Result<()> {
        let encrypted = self.encrypt(value);
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO credentials (plugin_id, key, encrypted_value, updated_at)
             VALUES (?1, ?2, ?3, datetime('now'))
             ON CONFLICT(plugin_id, key) DO UPDATE SET
                encrypted_value = ?3,
                updated_at = datetime('now')",
            params![plugin_id, key, encrypted],
        )
        .context("failed to store credential")?;
        tracing::debug!(
            plugin_id = plugin_id,
            key = key,
            "credential stored (value redacted)"
        );
        Ok(())
    }

    async fn retrieve(
        &self,
        plugin_id: &str,
        key: &str,
    ) -> Result<Option<String>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare(
                "SELECT encrypted_value FROM credentials
                 WHERE plugin_id = ?1 AND key = ?2",
            )
            .context("failed to prepare credential query")?;

        let result: Option<String> = stmt
            .query_row(params![plugin_id, key], |row| row.get(0))
            .optional()
            .context("failed to query credential")?;

        match result {
            Some(encrypted) => {
                let decrypted = self.decrypt(&encrypted)?;
                Ok(Some(decrypted))
            }
            None => Ok(None),
        }
    }

    async fn delete(
        &self,
        plugin_id: &str,
        key: &str,
    ) -> Result<bool> {
        let conn = self.conn.lock().await;
        let deleted = conn
            .execute(
                "DELETE FROM credentials WHERE plugin_id = ?1 AND key = ?2",
                params![plugin_id, key],
            )
            .context("failed to delete credential")?;
        tracing::debug!(
            plugin_id = plugin_id,
            key = key,
            deleted = deleted > 0,
            "credential deleted"
        );
        Ok(deleted > 0)
    }

    async fn delete_all_for_plugin(
        &self,
        plugin_id: &str,
    ) -> Result<u64> {
        let conn = self.conn.lock().await;
        let deleted = conn
            .execute(
                "DELETE FROM credentials WHERE plugin_id = ?1",
                params![plugin_id],
            )
            .context("failed to delete credentials for plugin")?;
        tracing::debug!(
            plugin_id = plugin_id,
            count = deleted,
            "all plugin credentials deleted"
        );
        Ok(deleted as u64)
    }

    async fn list_keys(
        &self,
        plugin_id: &str,
    ) -> Result<Vec<String>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare("SELECT key FROM credentials WHERE plugin_id = ?1 ORDER BY key")
            .context("failed to prepare key listing query")?;

        let keys: Vec<String> = stmt
            .query_map(params![plugin_id], |row| row.get(0))
            .context("failed to list credential keys")?
            .filter_map(|r| r.ok())
            .collect();

        Ok(keys)
    }
}

/// Extension trait for optional rusqlite query results.
trait OptionalExt<T> {
    /// Convert a `rusqlite::Error::QueryReturnedNoRows` into `None`.
    fn optional(self) -> Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalExt<T> for std::result::Result<T, rusqlite::Error> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_store() -> SqliteCredentialStore {
        let conn = rusqlite::Connection::open_in_memory()
            .expect("in-memory SQLite should open");
        let conn = Arc::new(Mutex::new(conn));
        let store = SqliteCredentialStore::new(conn, "test-secret-key")
            .expect("store should create");
        store.init().await.expect("init should succeed");
        store
    }

    #[tokio::test]
    async fn store_and_retrieve_credential() {
        let store = setup_store().await;

        store
            .store("com.test.plugin", "api_key", "my-secret-value")
            .await
            .expect("store should succeed");

        let value = store
            .retrieve("com.test.plugin", "api_key")
            .await
            .expect("retrieve should succeed");

        assert_eq!(value, Some("my-secret-value".to_string()));
    }

    #[tokio::test]
    async fn retrieve_nonexistent_returns_none() {
        let store = setup_store().await;

        let value = store
            .retrieve("com.test.plugin", "nonexistent")
            .await
            .expect("retrieve should succeed");

        assert!(value.is_none());
    }

    #[tokio::test]
    async fn store_overwrites_existing() {
        let store = setup_store().await;

        store
            .store("com.test.plugin", "token", "old-value")
            .await
            .expect("first store should succeed");

        store
            .store("com.test.plugin", "token", "new-value")
            .await
            .expect("second store should succeed");

        let value = store
            .retrieve("com.test.plugin", "token")
            .await
            .expect("retrieve should succeed");

        assert_eq!(value, Some("new-value".to_string()));
    }

    #[tokio::test]
    async fn delete_existing_credential() {
        let store = setup_store().await;

        store
            .store("com.test.plugin", "secret", "value")
            .await
            .expect("store should succeed");

        let deleted = store
            .delete("com.test.plugin", "secret")
            .await
            .expect("delete should succeed");

        assert!(deleted, "should return true when credential existed");

        let value = store
            .retrieve("com.test.plugin", "secret")
            .await
            .expect("retrieve should succeed");

        assert!(value.is_none());
    }

    #[tokio::test]
    async fn delete_nonexistent_returns_false() {
        let store = setup_store().await;

        let deleted = store
            .delete("com.test.plugin", "nonexistent")
            .await
            .expect("delete should succeed");

        assert!(!deleted);
    }

    #[tokio::test]
    async fn delete_all_for_plugin() {
        let store = setup_store().await;

        store
            .store("com.test.a", "key1", "val1")
            .await
            .expect("store");
        store
            .store("com.test.a", "key2", "val2")
            .await
            .expect("store");
        store
            .store("com.test.b", "key1", "val1")
            .await
            .expect("store");

        let deleted = store
            .delete_all_for_plugin("com.test.a")
            .await
            .expect("delete_all should succeed");

        assert_eq!(deleted, 2);

        // Plugin B credentials should be untouched.
        let value = store
            .retrieve("com.test.b", "key1")
            .await
            .expect("retrieve");
        assert_eq!(value, Some("val1".to_string()));

        // Plugin A credentials should be gone.
        let value = store
            .retrieve("com.test.a", "key1")
            .await
            .expect("retrieve");
        assert!(value.is_none());
    }

    #[tokio::test]
    async fn list_keys_for_plugin() {
        let store = setup_store().await;

        store
            .store("com.test.plugin", "beta_key", "v1")
            .await
            .expect("store");
        store
            .store("com.test.plugin", "alpha_key", "v2")
            .await
            .expect("store");
        store
            .store("com.test.other", "gamma_key", "v3")
            .await
            .expect("store");

        let keys = store
            .list_keys("com.test.plugin")
            .await
            .expect("list_keys should succeed");

        assert_eq!(keys, vec!["alpha_key", "beta_key"]);
    }

    #[tokio::test]
    async fn plugin_scoping_enforced() {
        let store = setup_store().await;

        store
            .store("com.test.a", "shared_name", "value-a")
            .await
            .expect("store");
        store
            .store("com.test.b", "shared_name", "value-b")
            .await
            .expect("store");

        let val_a = store
            .retrieve("com.test.a", "shared_name")
            .await
            .expect("retrieve")
            .expect("should exist");
        let val_b = store
            .retrieve("com.test.b", "shared_name")
            .await
            .expect("retrieve")
            .expect("should exist");

        assert_eq!(val_a, "value-a");
        assert_eq!(val_b, "value-b");
    }

    #[tokio::test]
    async fn encrypted_value_differs_from_plaintext() {
        let store = setup_store().await;
        let plaintext = "super-secret-password";

        store
            .store("com.test.plugin", "pass", plaintext)
            .await
            .expect("store should succeed");

        // Read the raw encrypted value directly from SQLite.
        let conn = store.conn.lock().await;
        let encrypted: String = conn
            .query_row(
                "SELECT encrypted_value FROM credentials WHERE plugin_id = ?1 AND key = ?2",
                params!["com.test.plugin", "pass"],
                |row| row.get(0),
            )
            .expect("should find row");

        assert_ne!(
            encrypted, plaintext,
            "stored value must be encrypted, not plaintext"
        );
    }

    #[tokio::test]
    async fn roundtrip_special_characters() {
        let store = setup_store().await;
        let value = "p@$$w0rd!#%^&*()_+={}\"|;:'<>,.?/~`";

        store
            .store("com.test.plugin", "special", value)
            .await
            .expect("store");

        let retrieved = store
            .retrieve("com.test.plugin", "special")
            .await
            .expect("retrieve")
            .expect("should exist");

        assert_eq!(retrieved, value);
    }

    #[tokio::test]
    async fn roundtrip_unicode() {
        let store = setup_store().await;
        let value = "password-with-unicode-characters";

        store
            .store("com.test.plugin", "unicode", value)
            .await
            .expect("store");

        let retrieved = store
            .retrieve("com.test.plugin", "unicode")
            .await
            .expect("retrieve")
            .expect("should exist");

        assert_eq!(retrieved, value);
    }

    #[test]
    fn derive_key_is_deterministic() {
        let key1 = crypto::derive_key("secret", crypto::DOMAIN_CREDENTIAL_STORE);
        let key2 = crypto::derive_key("secret", crypto::DOMAIN_CREDENTIAL_STORE);
        assert_eq!(key1, key2);
    }

    #[test]
    fn derive_key_different_secrets_differ() {
        let key1 = crypto::derive_key("secret-a", crypto::DOMAIN_CREDENTIAL_STORE);
        let key2 = crypto::derive_key("secret-b", crypto::DOMAIN_CREDENTIAL_STORE);
        assert_ne!(key1, key2);
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = crypto::derive_key("test-key", crypto::DOMAIN_CREDENTIAL_STORE);
        let plaintext = b"hello world";
        let encrypted = crypto::encrypt(plaintext, &key).unwrap();
        let decrypted = crypto::decrypt(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }
}
