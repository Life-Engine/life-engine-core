//! WASM plugin runtime using Extism for sandboxed plugin execution.
//!
//! Provides capability-scoped host functions, resource limits (memory,
//! timeout, rate limiting), and a bridge between WASM plugins and Core
//! subsystems (storage, events, config, logging, HTTP).

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use life_engine_plugin_sdk::types::Capability;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::message_bus::MessageBus;
use crate::storage::StorageAdapter;

/// Configuration for a WASM plugin instance.
#[derive(Debug, Clone)]
pub struct WasmPluginConfig {
    /// Unique plugin identifier.
    pub plugin_id: String,
    /// Human-readable display name.
    pub display_name: String,
    /// Plugin version.
    pub version: String,
    /// Capabilities declared by this plugin.
    pub capabilities: HashSet<Capability>,
    /// Collections this plugin is allowed to access.
    pub declared_collections: Vec<String>,
    /// Maximum memory in bytes (default: 64 MB).
    pub memory_limit_bytes: u64,
    /// Maximum execution time per request (default: 30 seconds).
    pub execution_timeout: Duration,
    /// Maximum host function calls per second (default: 1000).
    pub rate_limit_per_second: u32,
    /// Allowed outbound HTTP domains (only relevant if HttpOutbound is declared).
    pub allowed_http_domains: Vec<String>,
}

impl Default for WasmPluginConfig {
    fn default() -> Self {
        Self {
            plugin_id: String::new(),
            display_name: String::new(),
            version: String::new(),
            capabilities: HashSet::new(),
            declared_collections: Vec::new(),
            memory_limit_bytes: 64 * 1024 * 1024, // 64 MB
            execution_timeout: Duration::from_secs(30),
            rate_limit_per_second: 1000,
            allowed_http_domains: Vec::new(),
        }
    }
}

/// Tracks rate limiting state for host function calls.
#[derive(Debug)]
pub struct RateLimitState {
    /// Maximum calls per second.
    limit: u32,
    /// Number of calls in the current window.
    count: u32,
    /// Start of the current window.
    window_start: std::time::Instant,
}

impl RateLimitState {
    pub fn new(limit: u32) -> Self {
        Self {
            limit,
            count: 0,
            window_start: std::time::Instant::now(),
        }
    }

    /// Check if a call is allowed. Returns Ok(()) if under limit, Err if exceeded.
    pub fn check_and_increment(&mut self) -> Result<()> {
        let now = std::time::Instant::now();
        if now.duration_since(self.window_start) >= Duration::from_secs(1) {
            self.count = 0;
            self.window_start = now;
        }
        if self.count >= self.limit {
            return Err(anyhow::anyhow!(
                "rate limit exceeded: {} calls/sec",
                self.limit
            ));
        }
        self.count += 1;
        Ok(())
    }
}

/// Request types for host function calls from WASM plugins.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum HostRequest {
    /// Read a record by ID from a collection.
    StoreRead {
        collection: String,
        id: String,
    },
    /// Write (create) a record to a collection.
    StoreWrite {
        collection: String,
        data: serde_json::Value,
    },
    /// Query records in a collection.
    StoreQuery {
        collection: String,
        filters: serde_json::Value,
        limit: Option<u32>,
        offset: Option<u32>,
    },
    /// Delete a record from a collection.
    StoreDelete {
        collection: String,
        id: String,
    },
    /// Read a config value.
    ConfigGet {
        key: String,
    },
    /// Subscribe to an event type.
    EventSubscribe {
        event_type: String,
    },
    /// Emit an event.
    EventEmit {
        event_type: String,
        payload: serde_json::Value,
    },
    /// Log a message at info level.
    LogInfo {
        message: String,
    },
    /// Log a message at warn level.
    LogWarn {
        message: String,
    },
    /// Log a message at error level.
    LogError {
        message: String,
    },
    /// Make an outbound HTTP request.
    HttpRequest {
        url: String,
        method: String,
        headers: Option<serde_json::Value>,
        body: Option<String>,
    },
}

/// Response from a host function call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostResponse {
    /// Whether the call succeeded.
    pub success: bool,
    /// The result data (if success).
    pub data: Option<serde_json::Value>,
    /// Error message (if failure).
    pub error: Option<String>,
}

impl HostResponse {
    pub fn ok(data: serde_json::Value) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
        }
    }
}

/// The host-side handler that processes requests from WASM plugins.
///
/// Each `WasmHostBridge` is scoped to a single plugin and enforces
/// capability checks, collection scoping, and rate limits.
pub struct WasmHostBridge {
    config: WasmPluginConfig,
    storage: Arc<dyn StorageAdapter>,
    message_bus: Arc<MessageBus>,
    rate_limiter: Mutex<RateLimitState>,
}

impl WasmHostBridge {
    pub fn new(
        config: WasmPluginConfig,
        storage: Arc<dyn StorageAdapter>,
        message_bus: Arc<MessageBus>,
    ) -> Self {
        let rate_limiter = Mutex::new(RateLimitState::new(config.rate_limit_per_second));
        Self {
            config,
            storage,
            message_bus,
            rate_limiter,
        }
    }

    /// Process a host function request from the WASM plugin.
    ///
    /// Enforces capability checks, collection scoping, and rate limiting.
    pub async fn handle_request(&self, request: HostRequest) -> HostResponse {
        // Rate limit check
        {
            let mut limiter = self.rate_limiter.lock().await;
            if let Err(e) = limiter.check_and_increment() {
                return HostResponse::err(e.to_string());
            }
        }

        match request {
            HostRequest::StoreRead { collection, id } => {
                self.handle_store_read(&collection, &id).await
            }
            HostRequest::StoreWrite { collection, data } => {
                self.handle_store_write(&collection, data).await
            }
            HostRequest::StoreQuery {
                collection,
                filters,
                limit,
                offset,
            } => {
                self.handle_store_query(&collection, filters, limit, offset)
                    .await
            }
            HostRequest::StoreDelete { collection, id } => {
                self.handle_store_delete(&collection, &id).await
            }
            HostRequest::ConfigGet { key } => self.handle_config_get(&key).await,
            HostRequest::EventSubscribe { event_type } => {
                self.handle_event_subscribe(&event_type).await
            }
            HostRequest::EventEmit {
                event_type,
                payload,
            } => self.handle_event_emit(&event_type, payload).await,
            HostRequest::LogInfo { message } => self.handle_log("info", &message),
            HostRequest::LogWarn { message } => self.handle_log("warn", &message),
            HostRequest::LogError { message } => self.handle_log("error", &message),
            HostRequest::HttpRequest {
                url,
                method,
                headers,
                body,
            } => self.handle_http_request(&url, &method, headers, body).await,
        }
    }

    /// Check if the plugin has a specific capability.
    fn has_capability(&self, cap: Capability) -> bool {
        self.config.capabilities.contains(&cap)
    }

    /// Check if a collection is in the plugin's declared collections.
    fn is_collection_allowed(&self, collection: &str) -> bool {
        self.config.declared_collections.iter().any(|c| c == collection)
    }

    async fn handle_store_read(&self, collection: &str, id: &str) -> HostResponse {
        if !self.has_capability(Capability::StorageRead) {
            return HostResponse::err("capability not granted: StorageRead");
        }
        if !self.is_collection_allowed(collection) {
            return HostResponse::err(format!(
                "collection '{}' not declared in plugin manifest",
                collection
            ));
        }

        match self
            .storage
            .get(&self.config.plugin_id, collection, id)
            .await
        {
            Ok(Some(record)) => HostResponse::ok(serde_json::to_value(&record).unwrap()),
            Ok(None) => HostResponse::ok(serde_json::Value::Null),
            Err(e) => HostResponse::err(e.to_string()),
        }
    }

    async fn handle_store_write(
        &self,
        collection: &str,
        data: serde_json::Value,
    ) -> HostResponse {
        if !self.has_capability(Capability::StorageWrite) {
            return HostResponse::err("capability not granted: StorageWrite");
        }
        if !self.is_collection_allowed(collection) {
            return HostResponse::err(format!(
                "collection '{}' not declared in plugin manifest",
                collection
            ));
        }

        match self
            .storage
            .create(&self.config.plugin_id, collection, data)
            .await
        {
            Ok(record) => HostResponse::ok(serde_json::to_value(&record).unwrap()),
            Err(e) => HostResponse::err(e.to_string()),
        }
    }

    async fn handle_store_query(
        &self,
        collection: &str,
        _filters: serde_json::Value,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> HostResponse {
        if !self.has_capability(Capability::StorageRead) {
            return HostResponse::err("capability not granted: StorageRead");
        }
        if !self.is_collection_allowed(collection) {
            return HostResponse::err(format!(
                "collection '{}' not declared in plugin manifest",
                collection
            ));
        }

        let pagination = crate::storage::Pagination {
            limit: limit.unwrap_or(50),
            offset: offset.unwrap_or(0),
        };

        match self
            .storage
            .list(
                &self.config.plugin_id,
                collection,
                None,
                pagination.clamped(),
            )
            .await
        {
            Ok(result) => HostResponse::ok(serde_json::to_value(&result).unwrap()),
            Err(e) => HostResponse::err(e.to_string()),
        }
    }

    async fn handle_store_delete(&self, collection: &str, id: &str) -> HostResponse {
        if !self.has_capability(Capability::StorageWrite) {
            return HostResponse::err("capability not granted: StorageWrite");
        }
        if !self.is_collection_allowed(collection) {
            return HostResponse::err(format!(
                "collection '{}' not declared in plugin manifest",
                collection
            ));
        }

        match self
            .storage
            .delete(&self.config.plugin_id, collection, id)
            .await
        {
            Ok(deleted) => HostResponse::ok(serde_json::json!({ "deleted": deleted })),
            Err(e) => HostResponse::err(e.to_string()),
        }
    }

    async fn handle_config_get(&self, key: &str) -> HostResponse {
        if !self.has_capability(Capability::ConfigRead) {
            return HostResponse::err("capability not granted: ConfigRead");
        }
        // Config values are plugin-scoped; for now return null (config store TBD)
        HostResponse::ok(serde_json::json!({ "key": key, "value": null }))
    }

    async fn handle_event_subscribe(&self, event_type: &str) -> HostResponse {
        if !self.has_capability(Capability::EventsSubscribe) {
            return HostResponse::err("capability not granted: EventsSubscribe");
        }
        // Subscription registration is handled at the runtime level
        HostResponse::ok(serde_json::json!({ "subscribed": event_type }))
    }

    async fn handle_event_emit(
        &self,
        event_type: &str,
        payload: serde_json::Value,
    ) -> HostResponse {
        if !self.has_capability(Capability::EventsEmit) {
            return HostResponse::err("capability not granted: EventsEmit");
        }

        self.message_bus
            .publish(crate::message_bus::BusEvent::NewRecords {
                collection: format!("event:{}", event_type),
                count: 1,
            });

        HostResponse::ok(serde_json::json!({
            "emitted": event_type,
            "payload": payload
        }))
    }

    fn handle_log(&self, level: &str, message: &str) -> HostResponse {
        if !self.has_capability(Capability::Logging) {
            return HostResponse::err("capability not granted: Logging");
        }

        match level {
            "info" => tracing::info!(plugin_id = %self.config.plugin_id, "{}", message),
            "warn" => tracing::warn!(plugin_id = %self.config.plugin_id, "{}", message),
            "error" => tracing::error!(plugin_id = %self.config.plugin_id, "{}", message),
            _ => tracing::info!(plugin_id = %self.config.plugin_id, "{}", message),
        }

        HostResponse::ok(serde_json::json!({ "logged": true }))
    }

    async fn handle_http_request(
        &self,
        url: &str,
        method: &str,
        headers: Option<serde_json::Value>,
        body: Option<String>,
    ) -> HostResponse {
        if !self.has_capability(Capability::HttpOutbound) {
            return HostResponse::err("capability not granted: HttpOutbound");
        }

        // Check URL against allowed domains.
        // Default-deny: if the allowlist is empty, block all outbound HTTP.
        if let Ok(parsed) = url::Url::parse(url) {
            if let Some(host) = parsed.host_str() {
                if self.config.allowed_http_domains.is_empty() {
                    return HostResponse::err(format!(
                        "HTTP request to '{}' not allowed: no domains in allowlist",
                        host
                    ));
                }
                // Require exact match or dot-prefixed subdomain match to
                // prevent suffix bypass (e.g. evilexample.com matching example.com).
                let allowed = self.config.allowed_http_domains.iter().any(|d| {
                    host == d.as_str() || host.ends_with(&format!(".{}", d))
                });
                if !allowed {
                    return HostResponse::err(format!(
                        "HTTP request to '{}' not allowed: domain not in declared list",
                        host
                    ));
                }
            }
        } else {
            return HostResponse::err(format!("invalid URL: {}", url));
        }

        // Execute the HTTP request
        let client = reqwest::Client::new();
        let mut req = match method.to_uppercase().as_str() {
            "GET" => client.get(url),
            "POST" => client.post(url),
            "PUT" => client.put(url),
            "DELETE" => client.delete(url),
            "PATCH" => client.patch(url),
            _ => return HostResponse::err(format!("unsupported HTTP method: {}", method)),
        };

        // Apply headers from the plugin request.
        if let Some(hdrs) = headers {
            if let Some(obj) = hdrs.as_object() {
                for (key, value) in obj {
                    if let Some(val_str) = value.as_str() {
                        req = req.header(key.as_str(), val_str);
                    }
                }
            }
        }

        // Apply request body.
        if let Some(b) = body {
            req = req.body(b);
        }

        match tokio::time::timeout(self.config.execution_timeout, req.send()).await {
            Ok(Ok(resp)) => {
                let status = resp.status().as_u16();
                let body = resp.text().await.unwrap_or_default();
                HostResponse::ok(serde_json::json!({
                    "status": status,
                    "body": body
                }))
            }
            Ok(Err(e)) => HostResponse::err(format!("HTTP request failed: {}", e)),
            Err(_) => HostResponse::err("HTTP request timed out"),
        }
    }
}

/// The WASM plugin runtime that manages plugin instances via Extism.
pub struct WasmRuntime {
    storage: Arc<dyn StorageAdapter>,
    message_bus: Arc<MessageBus>,
    bridges: std::collections::HashMap<String, Arc<WasmHostBridge>>,
}

impl WasmRuntime {
    pub fn new(storage: Arc<dyn StorageAdapter>, message_bus: Arc<MessageBus>) -> Self {
        Self {
            storage,
            message_bus,
            bridges: std::collections::HashMap::new(),
        }
    }

    /// Register a WASM plugin with its configuration.
    ///
    /// Creates a host bridge scoped to this plugin's capabilities and
    /// collections. Undeclared capabilities result in the host function
    /// being unavailable (returning an error on call).
    pub fn register_plugin(&mut self, config: WasmPluginConfig) -> Result<()> {
        let plugin_id = config.plugin_id.clone();
        if self.bridges.contains_key(&plugin_id) {
            return Err(anyhow::anyhow!(
                "WASM plugin '{}' already registered",
                plugin_id
            ));
        }

        let bridge = Arc::new(WasmHostBridge::new(
            config,
            Arc::clone(&self.storage),
            Arc::clone(&self.message_bus),
        ));

        self.bridges.insert(plugin_id, bridge);
        Ok(())
    }

    /// Get the host bridge for a registered plugin.
    pub fn get_bridge(&self, plugin_id: &str) -> Option<Arc<WasmHostBridge>> {
        self.bridges.get(plugin_id).cloned()
    }

    /// Unregister a WASM plugin.
    pub fn unregister_plugin(&mut self, plugin_id: &str) -> bool {
        self.bridges.remove(plugin_id).is_some()
    }

    /// Returns the number of registered WASM plugins.
    pub fn plugin_count(&self) -> usize {
        self.bridges.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{
        Pagination, QueryFilters, QueryResult, Record, SortOptions, StorageAdapter,
    };
    use async_trait::async_trait;
    use chrono::Utc;
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Mutex as StdMutex;

    /// In-memory mock storage for WASM runtime tests.
    struct MockStorage {
        records: StdMutex<HashMap<String, Record>>,
    }

    impl MockStorage {
        fn new() -> Self {
            Self {
                records: StdMutex::new(HashMap::new()),
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
            self.query(plugin_id, collection, QueryFilters::default(), sort, pagination)
                .await
        }
    }

    fn make_bridge(capabilities: HashSet<Capability>, collections: Vec<&str>) -> WasmHostBridge {
        let storage = Arc::new(MockStorage::new());
        let bus = Arc::new(MessageBus::new());
        let config = WasmPluginConfig {
            plugin_id: "com.test.wasm-plugin".into(),
            display_name: "Test WASM Plugin".into(),
            version: "1.0.0".into(),
            capabilities,
            declared_collections: collections.into_iter().map(|s| s.to_string()).collect(),
            memory_limit_bytes: 64 * 1024 * 1024,
            execution_timeout: Duration::from_secs(30),
            rate_limit_per_second: 1000,
            allowed_http_domains: vec!["example.com".into()],
        };
        WasmHostBridge::new(config, storage, bus)
    }

    fn caps(list: &[Capability]) -> HashSet<Capability> {
        list.iter().copied().collect()
    }

    // ── Test: WASM plugin can read declared collections ──

    #[tokio::test]
    async fn store_read_succeeds_for_declared_collection() {
        let bridge = make_bridge(
            caps(&[Capability::StorageRead, Capability::StorageWrite]),
            vec!["tasks"],
        );

        // First write a record
        let write_resp = bridge
            .handle_request(HostRequest::StoreWrite {
                collection: "tasks".into(),
                data: json!({"title": "Test task"}),
            })
            .await;
        assert!(write_resp.success);

        let record: Record =
            serde_json::from_value(write_resp.data.unwrap()).unwrap();

        // Now read it back
        let read_resp = bridge
            .handle_request(HostRequest::StoreRead {
                collection: "tasks".into(),
                id: record.id.clone(),
            })
            .await;
        assert!(read_resp.success);
        assert!(read_resp.data.is_some());

        let read_record: Record =
            serde_json::from_value(read_resp.data.unwrap()).unwrap();
        assert_eq!(read_record.id, record.id);
        assert_eq!(read_record.data["title"], "Test task");
    }

    // ── Test: WASM plugin cannot access undeclared collections ──

    #[tokio::test]
    async fn store_read_rejected_for_undeclared_collection() {
        let bridge = make_bridge(
            caps(&[Capability::StorageRead]),
            vec!["tasks"], // only "tasks" declared
        );

        let resp = bridge
            .handle_request(HostRequest::StoreRead {
                collection: "secrets".into(), // not declared
                id: "some-id".into(),
            })
            .await;

        assert!(!resp.success);
        assert!(resp
            .error
            .unwrap()
            .contains("not declared in plugin manifest"));
    }

    #[tokio::test]
    async fn store_write_rejected_for_undeclared_collection() {
        let bridge = make_bridge(
            caps(&[Capability::StorageWrite]),
            vec!["tasks"],
        );

        let resp = bridge
            .handle_request(HostRequest::StoreWrite {
                collection: "other_data".into(),
                data: json!({"bad": true}),
            })
            .await;

        assert!(!resp.success);
        assert!(resp
            .error
            .unwrap()
            .contains("not declared in plugin manifest"));
    }

    #[tokio::test]
    async fn store_query_rejected_for_undeclared_collection() {
        let bridge = make_bridge(
            caps(&[Capability::StorageRead]),
            vec!["tasks"],
        );

        let resp = bridge
            .handle_request(HostRequest::StoreQuery {
                collection: "forbidden".into(),
                filters: json!({}),
                limit: None,
                offset: None,
            })
            .await;

        assert!(!resp.success);
        assert!(resp
            .error
            .unwrap()
            .contains("not declared in plugin manifest"));
    }

    #[tokio::test]
    async fn store_delete_rejected_for_undeclared_collection() {
        let bridge = make_bridge(
            caps(&[Capability::StorageWrite]),
            vec!["tasks"],
        );

        let resp = bridge
            .handle_request(HostRequest::StoreDelete {
                collection: "forbidden".into(),
                id: "some-id".into(),
            })
            .await;

        assert!(!resp.success);
        assert!(resp
            .error
            .unwrap()
            .contains("not declared in plugin manifest"));
    }

    // ── Test: Capability enforcement — undeclared = rejected ──

    #[tokio::test]
    async fn store_read_without_storage_read_capability() {
        let bridge = make_bridge(
            caps(&[]), // no capabilities
            vec!["tasks"],
        );

        let resp = bridge
            .handle_request(HostRequest::StoreRead {
                collection: "tasks".into(),
                id: "some-id".into(),
            })
            .await;

        assert!(!resp.success);
        assert!(resp.error.unwrap().contains("capability not granted: StorageRead"));
    }

    #[tokio::test]
    async fn store_write_without_storage_write_capability() {
        let bridge = make_bridge(
            caps(&[Capability::StorageRead]), // read only, no write
            vec!["tasks"],
        );

        let resp = bridge
            .handle_request(HostRequest::StoreWrite {
                collection: "tasks".into(),
                data: json!({"title": "Test"}),
            })
            .await;

        assert!(!resp.success);
        assert!(resp.error.unwrap().contains("capability not granted: StorageWrite"));
    }

    #[tokio::test]
    async fn config_get_without_config_read_capability() {
        let bridge = make_bridge(caps(&[]), vec![]);

        let resp = bridge
            .handle_request(HostRequest::ConfigGet {
                key: "some_key".into(),
            })
            .await;

        assert!(!resp.success);
        assert!(resp.error.unwrap().contains("capability not granted: ConfigRead"));
    }

    #[tokio::test]
    async fn event_subscribe_without_events_subscribe_capability() {
        let bridge = make_bridge(caps(&[]), vec![]);

        let resp = bridge
            .handle_request(HostRequest::EventSubscribe {
                event_type: "task.created".into(),
            })
            .await;

        assert!(!resp.success);
        assert!(resp
            .error
            .unwrap()
            .contains("capability not granted: EventsSubscribe"));
    }

    #[tokio::test]
    async fn event_emit_without_events_emit_capability() {
        let bridge = make_bridge(caps(&[]), vec![]);

        let resp = bridge
            .handle_request(HostRequest::EventEmit {
                event_type: "task.created".into(),
                payload: json!({}),
            })
            .await;

        assert!(!resp.success);
        assert!(resp
            .error
            .unwrap()
            .contains("capability not granted: EventsEmit"));
    }

    #[tokio::test]
    async fn log_without_logging_capability() {
        let bridge = make_bridge(caps(&[]), vec![]);

        let resp = bridge
            .handle_request(HostRequest::LogInfo {
                message: "test".into(),
            })
            .await;

        assert!(!resp.success);
        assert!(resp.error.unwrap().contains("capability not granted: Logging"));
    }

    #[tokio::test]
    async fn http_request_without_http_outbound_capability() {
        let bridge = make_bridge(caps(&[]), vec![]);

        let resp = bridge
            .handle_request(HostRequest::HttpRequest {
                url: "https://example.com/api".into(),
                method: "GET".into(),
                headers: None,
                body: None,
            })
            .await;

        assert!(!resp.success);
        assert!(resp
            .error
            .unwrap()
            .contains("capability not granted: HttpOutbound"));
    }

    // ── Test: Capabilities work when granted ──

    #[tokio::test]
    async fn config_get_succeeds_with_capability() {
        let bridge = make_bridge(caps(&[Capability::ConfigRead]), vec![]);

        let resp = bridge
            .handle_request(HostRequest::ConfigGet {
                key: "theme".into(),
            })
            .await;

        assert!(resp.success);
    }

    #[tokio::test]
    async fn event_subscribe_succeeds_with_capability() {
        let bridge = make_bridge(caps(&[Capability::EventsSubscribe]), vec![]);

        let resp = bridge
            .handle_request(HostRequest::EventSubscribe {
                event_type: "task.created".into(),
            })
            .await;

        assert!(resp.success);
    }

    #[tokio::test]
    async fn event_emit_succeeds_with_capability() {
        let bridge = make_bridge(caps(&[Capability::EventsEmit]), vec![]);

        let resp = bridge
            .handle_request(HostRequest::EventEmit {
                event_type: "task.created".into(),
                payload: json!({"id": "123"}),
            })
            .await;

        assert!(resp.success);
    }

    #[tokio::test]
    async fn log_succeeds_with_capability() {
        let bridge = make_bridge(caps(&[Capability::Logging]), vec![]);

        for req in [
            HostRequest::LogInfo {
                message: "info msg".into(),
            },
            HostRequest::LogWarn {
                message: "warn msg".into(),
            },
            HostRequest::LogError {
                message: "error msg".into(),
            },
        ] {
            let resp = bridge.handle_request(req).await;
            assert!(resp.success);
        }
    }

    // ── Test: HTTP domain scoping ──

    #[tokio::test]
    async fn http_request_rejected_for_undeclared_domain() {
        let bridge = make_bridge(caps(&[Capability::HttpOutbound]), vec![]);
        // bridge is configured with allowed_http_domains: ["example.com"]

        let resp = bridge
            .handle_request(HostRequest::HttpRequest {
                url: "https://evil.com/steal-data".into(),
                method: "GET".into(),
                headers: None,
                body: None,
            })
            .await;

        assert!(!resp.success);
        assert!(resp.error.unwrap().contains("not allowed"));
    }

    #[tokio::test]
    async fn http_request_rejects_invalid_url() {
        let bridge = make_bridge(caps(&[Capability::HttpOutbound]), vec![]);

        let resp = bridge
            .handle_request(HostRequest::HttpRequest {
                url: "not a valid url".into(),
                method: "GET".into(),
                headers: None,
                body: None,
            })
            .await;

        assert!(!resp.success);
        assert!(resp.error.unwrap().contains("invalid URL"));
    }

    // ── Test: Rate limiting ──

    #[tokio::test]
    async fn rate_limit_enforced() {
        let storage = Arc::new(MockStorage::new());
        let bus = Arc::new(MessageBus::new());
        let config = WasmPluginConfig {
            plugin_id: "com.test.rate-limited".into(),
            display_name: "Rate Limited Plugin".into(),
            version: "1.0.0".into(),
            capabilities: caps(&[Capability::ConfigRead]),
            declared_collections: vec![],
            memory_limit_bytes: 64 * 1024 * 1024,
            execution_timeout: Duration::from_secs(30),
            rate_limit_per_second: 5, // very low limit for testing
            allowed_http_domains: vec![],
        };
        let bridge = WasmHostBridge::new(config, storage, bus);

        // First 5 calls should succeed
        for _ in 0..5 {
            let resp = bridge
                .handle_request(HostRequest::ConfigGet {
                    key: "test".into(),
                })
                .await;
            assert!(resp.success, "expected success but got: {:?}", resp.error);
        }

        // 6th call should be rate limited
        let resp = bridge
            .handle_request(HostRequest::ConfigGet {
                key: "test".into(),
            })
            .await;
        assert!(!resp.success);
        assert!(resp.error.unwrap().contains("rate limit exceeded"));
    }

    #[test]
    fn rate_limit_state_resets_after_window() {
        let mut state = RateLimitState::new(2);
        assert!(state.check_and_increment().is_ok());
        assert!(state.check_and_increment().is_ok());
        assert!(state.check_and_increment().is_err());

        // Simulate window reset by manipulating window_start
        state.window_start = std::time::Instant::now() - Duration::from_secs(2);
        assert!(state.check_and_increment().is_ok());
    }

    // ── Test: Store operations with proper capabilities ──

    #[tokio::test]
    async fn store_write_and_query() {
        let bridge = make_bridge(
            caps(&[Capability::StorageRead, Capability::StorageWrite]),
            vec!["items"],
        );

        // Write two records
        for i in 0..2 {
            let resp = bridge
                .handle_request(HostRequest::StoreWrite {
                    collection: "items".into(),
                    data: json!({"name": format!("item-{}", i)}),
                })
                .await;
            assert!(resp.success);
        }

        // Query all
        let resp = bridge
            .handle_request(HostRequest::StoreQuery {
                collection: "items".into(),
                filters: json!({}),
                limit: Some(10),
                offset: None,
            })
            .await;
        assert!(resp.success);
        let result: QueryResult = serde_json::from_value(resp.data.unwrap()).unwrap();
        assert_eq!(result.total, 2);
    }

    #[tokio::test]
    async fn store_write_and_delete() {
        let bridge = make_bridge(
            caps(&[Capability::StorageRead, Capability::StorageWrite]),
            vec!["items"],
        );

        // Write a record
        let resp = bridge
            .handle_request(HostRequest::StoreWrite {
                collection: "items".into(),
                data: json!({"name": "to-delete"}),
            })
            .await;
        assert!(resp.success);
        let record: Record = serde_json::from_value(resp.data.unwrap()).unwrap();

        // Delete it
        let resp = bridge
            .handle_request(HostRequest::StoreDelete {
                collection: "items".into(),
                id: record.id.clone(),
            })
            .await;
        assert!(resp.success);
        assert_eq!(resp.data.unwrap()["deleted"], true);

        // Verify it's gone
        let resp = bridge
            .handle_request(HostRequest::StoreRead {
                collection: "items".into(),
                id: record.id,
            })
            .await;
        assert!(resp.success);
        assert!(resp.data.unwrap().is_null());
    }

    // ── Test: WasmRuntime registration ──

    #[test]
    fn runtime_register_plugin() {
        let storage: Arc<dyn StorageAdapter> = Arc::new(MockStorage::new());
        let bus = Arc::new(MessageBus::new());
        let mut runtime = WasmRuntime::new(storage, bus);

        let config = WasmPluginConfig {
            plugin_id: "com.test.wasm".into(),
            ..Default::default()
        };

        assert!(runtime.register_plugin(config).is_ok());
        assert_eq!(runtime.plugin_count(), 1);
        assert!(runtime.get_bridge("com.test.wasm").is_some());
    }

    #[test]
    fn runtime_duplicate_registration_fails() {
        let storage: Arc<dyn StorageAdapter> = Arc::new(MockStorage::new());
        let bus = Arc::new(MessageBus::new());
        let mut runtime = WasmRuntime::new(storage, bus);

        let config1 = WasmPluginConfig {
            plugin_id: "com.test.dupe".into(),
            ..Default::default()
        };
        let config2 = WasmPluginConfig {
            plugin_id: "com.test.dupe".into(),
            ..Default::default()
        };

        assert!(runtime.register_plugin(config1).is_ok());
        assert!(runtime.register_plugin(config2).is_err());
    }

    #[test]
    fn runtime_unregister_plugin() {
        let storage: Arc<dyn StorageAdapter> = Arc::new(MockStorage::new());
        let bus = Arc::new(MessageBus::new());
        let mut runtime = WasmRuntime::new(storage, bus);

        let config = WasmPluginConfig {
            plugin_id: "com.test.remove".into(),
            ..Default::default()
        };

        runtime.register_plugin(config).unwrap();
        assert_eq!(runtime.plugin_count(), 1);

        assert!(runtime.unregister_plugin("com.test.remove"));
        assert_eq!(runtime.plugin_count(), 0);
        assert!(runtime.get_bridge("com.test.remove").is_none());
    }

    // ── Test: WasmPluginConfig defaults ──

    #[test]
    fn config_defaults() {
        let config = WasmPluginConfig::default();
        assert_eq!(config.memory_limit_bytes, 64 * 1024 * 1024);
        assert_eq!(config.execution_timeout, Duration::from_secs(30));
        assert_eq!(config.rate_limit_per_second, 1000);
        assert!(config.capabilities.is_empty());
        assert!(config.declared_collections.is_empty());
    }

    // ── Test: HostResponse construction ──

    #[test]
    fn host_response_ok() {
        let resp = HostResponse::ok(json!({"value": 42}));
        assert!(resp.success);
        assert!(resp.data.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn host_response_err() {
        let resp = HostResponse::err("something went wrong");
        assert!(!resp.success);
        assert!(resp.data.is_none());
        assert_eq!(resp.error.unwrap(), "something went wrong");
    }

    // ── Test: Multiple collections isolation ──

    #[tokio::test]
    async fn plugin_can_access_multiple_declared_collections() {
        let bridge = make_bridge(
            caps(&[Capability::StorageRead, Capability::StorageWrite]),
            vec!["tasks", "notes"],
        );

        // Write to "tasks"
        let resp = bridge
            .handle_request(HostRequest::StoreWrite {
                collection: "tasks".into(),
                data: json!({"title": "Task 1"}),
            })
            .await;
        assert!(resp.success);

        // Write to "notes"
        let resp = bridge
            .handle_request(HostRequest::StoreWrite {
                collection: "notes".into(),
                data: json!({"content": "Note 1"}),
            })
            .await;
        assert!(resp.success);

        // But not to "contacts"
        let resp = bridge
            .handle_request(HostRequest::StoreWrite {
                collection: "contacts".into(),
                data: json!({"name": "Bad"}),
            })
            .await;
        assert!(!resp.success);
    }

    // ── Test: Store delete requires StorageWrite capability ──

    #[tokio::test]
    async fn store_delete_without_write_capability_fails() {
        let bridge = make_bridge(
            caps(&[Capability::StorageRead]), // read only
            vec!["tasks"],
        );

        let resp = bridge
            .handle_request(HostRequest::StoreDelete {
                collection: "tasks".into(),
                id: "some-id".into(),
            })
            .await;

        assert!(!resp.success);
        assert!(resp.error.unwrap().contains("capability not granted: StorageWrite"));
    }
}
