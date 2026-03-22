//! Credential storage trait for securely persisting plugin credentials.
//!
//! The `CredentialStore` trait defines the interface that Core implements
//! for plugins to store and retrieve encrypted credentials. Credentials
//! are scoped per plugin ID -- a plugin can only access its own
//! credentials.
//!
//! SECURITY: Implementations MUST encrypt credential values at rest
//! and MUST NEVER log credential values.

use async_trait::async_trait;

/// A stored credential entry.
///
/// Contains a key-value pair scoped to a specific plugin.
/// The `value` is stored encrypted at rest by the implementation.
#[derive(Debug, Clone)]
pub struct StoredCredential {
    /// The plugin ID this credential belongs to.
    pub plugin_id: String,
    /// The credential key (e.g. "imap_password", "oauth_token").
    pub key: String,
    /// The credential value (plaintext in memory, encrypted at rest).
    pub value: String,
}

/// Trait for encrypted credential storage.
///
/// Core provides an implementation of this trait that plugins use via
/// the `PluginContext`. All credential values are encrypted before
/// writing to the underlying store and decrypted when read.
///
/// Credentials are scoped per plugin -- each plugin can only access
/// credentials stored under its own ID.
#[async_trait]
pub trait CredentialStore: Send + Sync {
    /// Store a credential value for the given plugin and key.
    ///
    /// If a credential with the same plugin_id and key already exists,
    /// it is overwritten.
    async fn store(
        &self,
        plugin_id: &str,
        key: &str,
        value: &str,
    ) -> anyhow::Result<()>;

    /// Retrieve a credential value for the given plugin and key.
    ///
    /// Returns `None` if no credential exists for this plugin/key pair.
    async fn retrieve(
        &self,
        plugin_id: &str,
        key: &str,
    ) -> anyhow::Result<Option<String>>;

    /// Delete a credential for the given plugin and key.
    ///
    /// Returns `Ok(true)` if a credential was deleted, `Ok(false)` if
    /// no matching credential existed.
    async fn delete(
        &self,
        plugin_id: &str,
        key: &str,
    ) -> anyhow::Result<bool>;

    /// Delete all credentials for the given plugin.
    ///
    /// Returns the number of credentials deleted.
    async fn delete_all_for_plugin(
        &self,
        plugin_id: &str,
    ) -> anyhow::Result<u64>;

    /// List all credential keys for the given plugin.
    ///
    /// Returns only the keys, never the values.
    async fn list_keys(
        &self,
        plugin_id: &str,
    ) -> anyhow::Result<Vec<String>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_credential_construction() {
        let cred = StoredCredential {
            plugin_id: "com.test.plugin".into(),
            key: "api_key".into(),
            value: "secret-value".into(),
        };
        assert_eq!(cred.plugin_id, "com.test.plugin");
        assert_eq!(cred.key, "api_key");
        assert_eq!(cred.value, "secret-value");
    }

    #[test]
    fn stored_credential_clone() {
        let cred = StoredCredential {
            plugin_id: "com.test.plugin".into(),
            key: "token".into(),
            value: "abc123".into(),
        };
        let cloned = cred.clone();
        assert_eq!(cloned.plugin_id, cred.plugin_id);
        assert_eq!(cloned.key, cred.key);
        assert_eq!(cloned.value, cred.value);
    }
}
