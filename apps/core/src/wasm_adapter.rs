//! Adapter that wraps a `CorePlugin` to route all host interactions
//! through the `WasmHostBridge`, validating the WASM runtime's capability
//! enforcement and collection scoping against existing native plugin behaviour.
//!
//! This is used during the Phase 4 migration to verify that first-party
//! plugins produce identical results when running through the WASM bridge
//! vs. direct native execution.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use life_engine_plugin_sdk::prelude::*;

use crate::message_bus::MessageBus;
use crate::storage::StorageAdapter;
use crate::wasm_runtime::{HostRequest, WasmHostBridge, WasmPluginConfig};

/// Wraps a native `CorePlugin` and routes its storage/event operations
/// through the `WasmHostBridge` to validate WASM runtime enforcement.
pub struct WasmPluginAdapter {
    /// The wrapped native plugin.
    inner: Box<dyn CorePlugin>,
    /// The host bridge for this plugin.
    bridge: Arc<WasmHostBridge>,
}

impl WasmPluginAdapter {
    /// Create a new adapter wrapping the given native plugin.
    ///
    /// Extracts the plugin's declared capabilities and collections
    /// to configure the host bridge with the same constraints a
    /// WASM plugin would have.
    pub fn new(
        plugin: Box<dyn CorePlugin>,
        storage: Arc<dyn StorageAdapter>,
        message_bus: Arc<MessageBus>,
        declared_collections: Vec<String>,
    ) -> Self {
        let capabilities: HashSet<Capability> =
            plugin.capabilities().into_iter().collect();

        let config = WasmPluginConfig {
            plugin_id: plugin.id().to_string(),
            display_name: plugin.display_name().to_string(),
            version: plugin.version().to_string(),
            capabilities,
            declared_collections,
            memory_limit_bytes: 64 * 1024 * 1024,
            execution_timeout: Duration::from_secs(30),
            rate_limit_per_second: 1000,
            allowed_http_domains: vec![],
        };

        let bridge = Arc::new(WasmHostBridge::new(config, storage, message_bus));

        Self {
            inner: plugin,
            bridge,
        }
    }

    /// Returns a reference to the underlying host bridge.
    pub fn bridge(&self) -> &WasmHostBridge {
        &self.bridge
    }

    /// Store a record through the WASM bridge (validates capability enforcement).
    pub async fn bridge_store_write(
        &self,
        collection: &str,
        data: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let resp = self
            .bridge
            .handle_request(HostRequest::StoreWrite {
                collection: collection.to_string(),
                data,
            })
            .await;

        if resp.success {
            Ok(resp.data.unwrap_or(serde_json::Value::Null))
        } else {
            Err(anyhow::anyhow!(
                resp.error.unwrap_or_else(|| "unknown error".into())
            ))
        }
    }

    /// Read a record through the WASM bridge (validates capability enforcement).
    pub async fn bridge_store_read(
        &self,
        collection: &str,
        id: &str,
    ) -> Result<serde_json::Value> {
        let resp = self
            .bridge
            .handle_request(HostRequest::StoreRead {
                collection: collection.to_string(),
                id: id.to_string(),
            })
            .await;

        if resp.success {
            Ok(resp.data.unwrap_or(serde_json::Value::Null))
        } else {
            Err(anyhow::anyhow!(
                resp.error.unwrap_or_else(|| "unknown error".into())
            ))
        }
    }

    /// Query records through the WASM bridge.
    pub async fn bridge_store_query(
        &self,
        collection: &str,
        limit: Option<u32>,
    ) -> Result<serde_json::Value> {
        let resp = self
            .bridge
            .handle_request(HostRequest::StoreQuery {
                collection: collection.to_string(),
                filters: serde_json::json!({}),
                limit,
                offset: None,
            })
            .await;

        if resp.success {
            Ok(resp.data.unwrap_or(serde_json::Value::Null))
        } else {
            Err(anyhow::anyhow!(
                resp.error.unwrap_or_else(|| "unknown error".into())
            ))
        }
    }

    /// Delete a record through the WASM bridge.
    pub async fn bridge_store_delete(
        &self,
        collection: &str,
        id: &str,
    ) -> Result<serde_json::Value> {
        let resp = self
            .bridge
            .handle_request(HostRequest::StoreDelete {
                collection: collection.to_string(),
                id: id.to_string(),
            })
            .await;

        if resp.success {
            Ok(resp.data.unwrap_or(serde_json::Value::Null))
        } else {
            Err(anyhow::anyhow!(
                resp.error.unwrap_or_else(|| "unknown error".into())
            ))
        }
    }

    /// Emit an event through the WASM bridge.
    pub async fn bridge_event_emit(
        &self,
        event_type: &str,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let resp = self
            .bridge
            .handle_request(HostRequest::EventEmit {
                event_type: event_type.to_string(),
                payload,
            })
            .await;

        if resp.success {
            Ok(resp.data.unwrap_or(serde_json::Value::Null))
        } else {
            Err(anyhow::anyhow!(
                resp.error.unwrap_or_else(|| "unknown error".into())
            ))
        }
    }

    /// Log through the WASM bridge.
    pub async fn bridge_log(&self, level: &str, message: &str) -> Result<()> {
        let req = match level {
            "warn" => HostRequest::LogWarn {
                message: message.to_string(),
            },
            "error" => HostRequest::LogError {
                message: message.to_string(),
            },
            _ => HostRequest::LogInfo {
                message: message.to_string(),
            },
        };

        let resp = self.bridge.handle_request(req).await;
        if resp.success {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                resp.error.unwrap_or_else(|| "unknown error".into())
            ))
        }
    }
}

#[async_trait]
impl CorePlugin for WasmPluginAdapter {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn display_name(&self) -> &str {
        self.inner.display_name()
    }

    fn version(&self) -> &str {
        self.inner.version()
    }

    fn capabilities(&self) -> Vec<Capability> {
        self.inner.capabilities()
    }

    async fn on_load(&mut self, ctx: &PluginContext) -> Result<()> {
        self.inner.on_load(ctx).await
    }

    async fn on_unload(&mut self) -> Result<()> {
        self.inner.on_unload().await
    }

    fn routes(&self) -> Vec<PluginRoute> {
        self.inner.routes()
    }

    async fn handle_event(&self, event: &CoreEvent) -> Result<()> {
        self.inner.handle_event(event).await
    }

    fn collections(&self) -> Vec<CollectionSchema> {
        self.inner.collections()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message_bus::MessageBus;
    use crate::storage::{
        Pagination, QueryFilters, QueryResult, Record, SortOptions, StorageAdapter,
    };
    use chrono::Utc;
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// In-memory mock storage.
    struct MockStorage {
        records: Mutex<HashMap<String, Record>>,
    }

    impl MockStorage {
        fn new() -> Self {
            Self {
                records: Mutex::new(HashMap::new()),
            }
        }

        fn key(plugin_id: &str, collection: &str, id: &str) -> String {
            format!("{plugin_id}:{collection}:{id}")
        }
    }

    #[async_trait]
    impl StorageAdapter for MockStorage {
        async fn get(
            &self,
            plugin_id: &str,
            collection: &str,
            id: &str,
        ) -> Result<Option<Record>> {
            let key = Self::key(plugin_id, collection, id);
            Ok(self.records.lock().unwrap().get(&key).cloned())
        }

        async fn create(
            &self,
            plugin_id: &str,
            collection: &str,
            data: serde_json::Value,
        ) -> Result<Record> {
            let id = uuid::Uuid::new_v4().to_string();
            let now = Utc::now();
            let record = Record {
                id: id.clone(),
                plugin_id: plugin_id.into(),
                collection: collection.into(),
                data,
                version: 1,
                user_id: None,
                household_id: None,
                created_at: now,
                updated_at: now,
            };
            let key = Self::key(plugin_id, collection, &id);
            self.records.lock().unwrap().insert(key, record.clone());
            Ok(record)
        }

        async fn update(
            &self,
            plugin_id: &str,
            collection: &str,
            id: &str,
            data: serde_json::Value,
            version: i64,
        ) -> Result<Record> {
            let key = Self::key(plugin_id, collection, id);
            let mut records = self.records.lock().unwrap();
            let record = records
                .get(&key)
                .ok_or_else(|| anyhow::anyhow!("not found"))?;
            if record.version != version {
                return Err(anyhow::anyhow!("version mismatch"));
            }
            let updated = Record {
                data,
                version: version + 1,
                updated_at: Utc::now(),
                ..record.clone()
            };
            records.insert(key, updated.clone());
            Ok(updated)
        }

        async fn query(
            &self,
            plugin_id: &str,
            collection: &str,
            _filters: QueryFilters,
            _sort: Option<SortOptions>,
            pagination: Pagination,
        ) -> Result<QueryResult> {
            let records = self.records.lock().unwrap();
            let matching: Vec<Record> = records
                .values()
                .filter(|r| r.plugin_id == plugin_id && r.collection == collection)
                .cloned()
                .collect();
            let total = matching.len() as u64;
            let paged = matching
                .into_iter()
                .skip(pagination.offset as usize)
                .take(pagination.limit as usize)
                .collect();
            Ok(QueryResult {
                records: paged,
                total,
                limit: pagination.limit,
                offset: pagination.offset,
            })
        }

        async fn delete(
            &self,
            plugin_id: &str,
            collection: &str,
            id: &str,
        ) -> Result<bool> {
            let key = Self::key(plugin_id, collection, id);
            Ok(self.records.lock().unwrap().remove(&key).is_some())
        }

        async fn list(
            &self,
            plugin_id: &str,
            collection: &str,
            sort: Option<SortOptions>,
            pagination: Pagination,
        ) -> Result<QueryResult> {
            self.query(
                plugin_id,
                collection,
                QueryFilters::default(),
                sort,
                pagination,
            )
            .await
        }
    }

    /// A simple test plugin that mimics a connector plugin.
    struct TestConnectorPlugin;

    #[async_trait]
    impl CorePlugin for TestConnectorPlugin {
        fn id(&self) -> &str {
            "com.life-engine.connector-email"
        }
        fn display_name(&self) -> &str {
            "Email Connector"
        }
        fn version(&self) -> &str {
            "0.1.0"
        }
        fn capabilities(&self) -> Vec<Capability> {
            vec![
                Capability::StorageRead,
                Capability::StorageWrite,
                Capability::EventsEmit,
                Capability::Logging,
            ]
        }
        async fn on_load(&mut self, _ctx: &PluginContext) -> Result<()> {
            Ok(())
        }
        async fn on_unload(&mut self) -> Result<()> {
            Ok(())
        }
        fn routes(&self) -> Vec<PluginRoute> {
            vec![PluginRoute {
                method: HttpMethod::Post,
                path: "/sync".into(),
            }]
        }
        async fn handle_event(&self, _event: &CoreEvent) -> Result<()> {
            Ok(())
        }
    }

    fn make_adapter(
        plugin: Box<dyn CorePlugin>,
        collections: Vec<&str>,
    ) -> WasmPluginAdapter {
        let storage: Arc<dyn StorageAdapter> = Arc::new(MockStorage::new());
        let bus = Arc::new(MessageBus::new());
        WasmPluginAdapter::new(
            plugin,
            storage,
            bus,
            collections.into_iter().map(|s| s.to_string()).collect(),
        )
    }

    #[test]
    fn adapter_preserves_plugin_metadata() {
        let adapter = make_adapter(Box::new(TestConnectorPlugin), vec!["emails"]);
        assert_eq!(adapter.id(), "com.life-engine.connector-email");
        assert_eq!(adapter.display_name(), "Email Connector");
        assert_eq!(adapter.version(), "0.1.0");
        assert_eq!(adapter.capabilities().len(), 4);
        assert_eq!(adapter.routes().len(), 1);
    }

    #[tokio::test]
    async fn adapter_lifecycle_works() {
        let mut adapter = make_adapter(Box::new(TestConnectorPlugin), vec!["emails"]);
        let ctx = PluginContext::new(adapter.id());
        adapter.on_load(&ctx).await.unwrap();
        adapter.on_unload().await.unwrap();
    }

    #[tokio::test]
    async fn adapter_bridge_write_and_read() {
        let adapter = make_adapter(Box::new(TestConnectorPlugin), vec!["emails"]);

        // Write through bridge
        let write_result = adapter
            .bridge_store_write("emails", json!({"subject": "Test email", "from": "a@b.com"}))
            .await
            .unwrap();

        let record: Record = serde_json::from_value(write_result).unwrap();
        assert_eq!(record.data["subject"], "Test email");

        // Read through bridge
        let read_result = adapter
            .bridge_store_read("emails", &record.id)
            .await
            .unwrap();

        let read_record: Record = serde_json::from_value(read_result).unwrap();
        assert_eq!(read_record.id, record.id);
        assert_eq!(read_record.data["subject"], "Test email");
    }

    #[tokio::test]
    async fn adapter_bridge_enforces_collection_scoping() {
        let adapter = make_adapter(Box::new(TestConnectorPlugin), vec!["emails"]);

        // Writing to undeclared collection should fail
        let result = adapter
            .bridge_store_write("contacts", json!({"name": "Evil"}))
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not declared in plugin manifest"));
    }

    #[tokio::test]
    async fn adapter_bridge_query_works() {
        let adapter = make_adapter(Box::new(TestConnectorPlugin), vec!["emails"]);

        // Write some records
        for i in 0..3 {
            adapter
                .bridge_store_write("emails", json!({"subject": format!("Email {}", i)}))
                .await
                .unwrap();
        }

        // Query
        let result = adapter.bridge_store_query("emails", Some(10)).await.unwrap();
        let query_result: crate::storage::QueryResult =
            serde_json::from_value(result).unwrap();
        assert_eq!(query_result.total, 3);
    }

    #[tokio::test]
    async fn adapter_bridge_delete_works() {
        let adapter = make_adapter(Box::new(TestConnectorPlugin), vec!["emails"]);

        // Write a record
        let write_result = adapter
            .bridge_store_write("emails", json!({"subject": "To delete"}))
            .await
            .unwrap();
        let record: Record = serde_json::from_value(write_result).unwrap();

        // Delete it
        let delete_result = adapter
            .bridge_store_delete("emails", &record.id)
            .await
            .unwrap();
        assert_eq!(delete_result["deleted"], true);
    }

    #[tokio::test]
    async fn adapter_bridge_event_emit_works() {
        let adapter = make_adapter(Box::new(TestConnectorPlugin), vec!["emails"]);

        let result = adapter
            .bridge_event_emit("email.synced", json!({"count": 5}))
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn adapter_bridge_log_works() {
        let adapter = make_adapter(Box::new(TestConnectorPlugin), vec!["emails"]);

        assert!(adapter.bridge_log("info", "test info log").await.is_ok());
        assert!(adapter.bridge_log("warn", "test warn log").await.is_ok());
        assert!(adapter.bridge_log("error", "test error log").await.is_ok());
    }

    #[tokio::test]
    async fn migrated_email_connector_produces_identical_output() {
        // This test validates that a native plugin wrapped in the WASM adapter
        // produces identical storage results to what a direct storage call would.
        let storage: Arc<dyn StorageAdapter> = Arc::new(MockStorage::new());
        let bus = Arc::new(MessageBus::new());

        // --- Direct native path ---
        let email_data = json!({
            "subject": "Welcome to Life Engine",
            "from": "noreply@life-engine.org",
            "to": "user@example.com",
            "date": "2026-03-22T10:00:00Z",
            "body": "Welcome!"
        });

        let native_record = storage
            .create(
                "com.life-engine.connector-email",
                "emails",
                email_data.clone(),
            )
            .await
            .unwrap();

        // --- WASM bridge path ---
        let adapter = WasmPluginAdapter::new(
            Box::new(TestConnectorPlugin),
            Arc::clone(&storage),
            bus,
            vec!["emails".to_string()],
        );

        let bridge_result = adapter
            .bridge_store_write("emails", email_data.clone())
            .await
            .unwrap();

        let bridge_record: Record = serde_json::from_value(bridge_result).unwrap();

        // Verify identical structure (IDs will differ but data must match)
        assert_eq!(native_record.plugin_id, bridge_record.plugin_id);
        assert_eq!(native_record.collection, bridge_record.collection);
        assert_eq!(native_record.data, bridge_record.data);
        assert_eq!(native_record.version, bridge_record.version);

        // Verify the bridge record can be read back
        let read_back = adapter
            .bridge_store_read("emails", &bridge_record.id)
            .await
            .unwrap();
        let read_record: Record = serde_json::from_value(read_back).unwrap();
        assert_eq!(read_record.data, email_data);
    }
}
