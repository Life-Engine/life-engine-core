//! Bridge between the SDK's `CredentialAccess` trait and Core's `CredentialStore`.
//!
//! Provides a concrete implementation of `CredentialAccess` that scopes all
//! credential operations to a specific plugin ID by delegating to a
//! `CredentialStore` implementation.

use async_trait::async_trait;
use life_engine_plugin_sdk::credential_store::CredentialStore;
use life_engine_plugin_sdk::types::CredentialAccess;
use std::sync::Arc;

/// Bridge that delegates `CredentialAccess` calls to a `CredentialStore`,
/// scoping all operations to a specific plugin ID.
///
/// This ensures plugins can only access their own credentials without
/// needing to know or supply their plugin ID on each call.
pub struct PluginCredentialBridge {
    /// The underlying credential store.
    store: Arc<dyn CredentialStore>,
    /// The plugin ID all operations are scoped to.
    plugin_id: String,
}

impl PluginCredentialBridge {
    /// Create a new bridge scoped to the given plugin ID.
    pub fn new(store: Arc<dyn CredentialStore>, plugin_id: String) -> Self {
        Self { store, plugin_id }
    }
}

#[async_trait]
impl CredentialAccess for PluginCredentialBridge {
    async fn get_credential(&self, service_name: &str) -> anyhow::Result<Option<String>> {
        self.store.retrieve(&self.plugin_id, service_name).await
    }

    async fn store_credential(&self, service_name: &str, value: &str) -> anyhow::Result<()> {
        self.store
            .store(&self.plugin_id, service_name, value)
            .await
    }

    async fn delete_credential(&self, service_name: &str) -> anyhow::Result<bool> {
        self.store.delete(&self.plugin_id, service_name).await
    }

    async fn list_credential_keys(&self) -> anyhow::Result<Vec<String>> {
        self.store.list_keys(&self.plugin_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tokio::sync::Mutex;

    /// In-memory credential store for testing.
    struct InMemoryCredentialStore {
        data: Mutex<HashMap<(String, String), String>>,
    }

    impl InMemoryCredentialStore {
        fn new() -> Self {
            Self {
                data: Mutex::new(HashMap::new()),
            }
        }
    }

    #[async_trait]
    impl CredentialStore for InMemoryCredentialStore {
        async fn store(
            &self,
            plugin_id: &str,
            key: &str,
            value: &str,
        ) -> anyhow::Result<()> {
            self.data
                .lock()
                .await
                .insert((plugin_id.to_string(), key.to_string()), value.to_string());
            Ok(())
        }

        async fn retrieve(
            &self,
            plugin_id: &str,
            key: &str,
        ) -> anyhow::Result<Option<String>> {
            Ok(self
                .data
                .lock()
                .await
                .get(&(plugin_id.to_string(), key.to_string()))
                .cloned())
        }

        async fn delete(&self, plugin_id: &str, key: &str) -> anyhow::Result<bool> {
            Ok(self
                .data
                .lock()
                .await
                .remove(&(plugin_id.to_string(), key.to_string()))
                .is_some())
        }

        async fn delete_all_for_plugin(&self, plugin_id: &str) -> anyhow::Result<u64> {
            let mut data = self.data.lock().await;
            let keys_to_remove: Vec<_> = data
                .keys()
                .filter(|(pid, _)| pid == plugin_id)
                .cloned()
                .collect();
            let count = keys_to_remove.len() as u64;
            for key in keys_to_remove {
                data.remove(&key);
            }
            Ok(count)
        }

        async fn list_keys(&self, plugin_id: &str) -> anyhow::Result<Vec<String>> {
            let data = self.data.lock().await;
            let mut keys: Vec<String> = data
                .keys()
                .filter(|(pid, _)| pid == plugin_id)
                .map(|(_, k)| k.clone())
                .collect();
            keys.sort();
            Ok(keys)
        }
    }

    #[tokio::test]
    async fn bridge_store_and_retrieve() {
        let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
        let bridge = PluginCredentialBridge::new(Arc::clone(&store), "com.test.plugin".into());

        bridge
            .store_credential("api_key", "secret-123")
            .await
            .expect("store should succeed");

        let value = bridge
            .get_credential("api_key")
            .await
            .expect("get should succeed");
        assert_eq!(value, Some("secret-123".to_string()));
    }

    #[tokio::test]
    async fn bridge_scopes_to_plugin_id() {
        let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
        let bridge_a =
            PluginCredentialBridge::new(Arc::clone(&store), "com.test.plugin-a".into());
        let bridge_b =
            PluginCredentialBridge::new(Arc::clone(&store), "com.test.plugin-b".into());

        bridge_a
            .store_credential("token", "value-a")
            .await
            .expect("store should succeed");

        // Plugin B should not see plugin A's credentials
        let value = bridge_b
            .get_credential("token")
            .await
            .expect("get should succeed");
        assert!(value.is_none());
    }

    #[tokio::test]
    async fn bridge_delete_credential() {
        let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
        let bridge = PluginCredentialBridge::new(Arc::clone(&store), "com.test.plugin".into());

        bridge
            .store_credential("key", "val")
            .await
            .expect("store");

        let deleted = bridge
            .delete_credential("key")
            .await
            .expect("delete should succeed");
        assert!(deleted);

        let value = bridge.get_credential("key").await.expect("get");
        assert!(value.is_none());
    }

    #[tokio::test]
    async fn bridge_list_credential_keys() {
        let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
        let bridge = PluginCredentialBridge::new(Arc::clone(&store), "com.test.plugin".into());

        bridge.store_credential("beta", "v1").await.expect("store");
        bridge
            .store_credential("alpha", "v2")
            .await
            .expect("store");

        let keys = bridge
            .list_credential_keys()
            .await
            .expect("list should succeed");
        assert_eq!(keys, vec!["alpha", "beta"]);
    }

    #[tokio::test]
    async fn bridge_retrieve_nonexistent_returns_none() {
        let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
        let bridge = PluginCredentialBridge::new(Arc::clone(&store), "com.test.plugin".into());

        let value = bridge.get_credential("nonexistent").await.expect("get");
        assert!(value.is_none());
    }

    #[tokio::test]
    async fn bridge_delete_nonexistent_returns_false() {
        let store: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::new());
        let bridge = PluginCredentialBridge::new(Arc::clone(&store), "com.test.plugin".into());

        let deleted = bridge
            .delete_credential("nonexistent")
            .await
            .expect("delete");
        assert!(!deleted);
    }
}
