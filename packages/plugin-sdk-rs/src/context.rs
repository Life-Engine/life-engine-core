//! Typed plugin context with client accessors for WASM plugin actions.
//!
//! `ActionContext` is the context object passed to `#[plugin_action]` functions.
//! It provides typed access to storage, events, config, and HTTP via client
//! traits that the host implements through host function bindings.

use async_trait::async_trait;
use serde_json::Value;
use std::fmt;
use std::sync::Arc;

use crate::error::PluginError;
use crate::types::HttpMethod;

/// Typed storage operations scoped to the plugin.
///
/// Provides document-level read and write access. The host enforces
/// capability checks — calling these without the required capability
/// returns `Err(PluginError::CapabilityDenied)`.
#[async_trait]
pub trait StorageClient: Send + Sync {
    /// Read a document by ID from the given collection.
    async fn doc_read(&self, collection: &str, id: &str) -> Result<Option<Value>, PluginError>;

    /// Write (upsert) a document into the given collection.
    async fn doc_write(&self, collection: &str, id: &str, data: Value) -> Result<(), PluginError>;

    /// Delete a document by ID from the given collection.
    async fn doc_delete(&self, collection: &str, id: &str) -> Result<bool, PluginError>;

    /// Query documents in a collection with a filter expression.
    async fn doc_query(&self, collection: &str, filter: Value) -> Result<Vec<Value>, PluginError>;
}

/// Event emission for plugins.
///
/// Allows plugins to emit events onto the Core event bus. Requires
/// the `EventsEmit` capability.
#[async_trait]
pub trait EventClient: Send + Sync {
    /// Emit an event with the given type and payload.
    async fn emit(&self, event_type: &str, payload: Value) -> Result<(), PluginError>;
}

/// Configuration access for plugins.
///
/// Provides read-only access to the plugin's configuration values.
/// Requires the `ConfigRead` capability.
#[async_trait]
pub trait ConfigClient: Send + Sync {
    /// Read a configuration value by key.
    async fn read(&self, key: &str) -> Result<Option<Value>, PluginError>;
}

/// Outbound HTTP client for plugins.
///
/// Allows plugins to make HTTP requests to external APIs. Requires
/// the `HttpOutbound` capability.
#[async_trait]
pub trait HttpClient: Send + Sync {
    /// Make an HTTP request and return the response body.
    async fn request(
        &self,
        method: HttpMethod,
        url: &str,
        headers: Option<Value>,
        body: Option<String>,
    ) -> Result<HttpResponse, PluginError>;
}

/// Response from an outbound HTTP request.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// HTTP status code.
    pub status: u16,
    /// Response headers as key-value pairs.
    pub headers: Value,
    /// Response body as a string.
    pub body: String,
}

/// Context passed to plugin action functions.
///
/// Provides typed access to host services through client traits:
///
/// - `storage` — Document read/write operations
/// - `events` — Event emission
/// - `config` — Configuration access
/// - `http` — Outbound HTTP requests
///
/// Each client enforces the plugin's declared capabilities at the host
/// level. Attempting to use a service without the required capability
/// returns `PluginError::CapabilityDenied`.
pub struct ActionContext {
    plugin_id: String,
    /// Typed storage client for document operations.
    pub storage: Arc<dyn StorageClient>,
    /// Event client for emitting events to the Core bus.
    pub events: Arc<dyn EventClient>,
    /// Configuration client for reading plugin config.
    pub config: Arc<dyn ConfigClient>,
    /// HTTP client for outbound requests.
    pub http: Arc<dyn HttpClient>,
}

impl ActionContext {
    /// Create a new `ActionContext` with all client implementations.
    pub fn new(
        plugin_id: impl Into<String>,
        storage: Arc<dyn StorageClient>,
        events: Arc<dyn EventClient>,
        config: Arc<dyn ConfigClient>,
        http: Arc<dyn HttpClient>,
    ) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            storage,
            events,
            config,
            http,
        }
    }

    /// Returns the plugin ID this context is scoped to.
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }
}

impl fmt::Debug for ActionContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActionContext")
            .field("plugin_id", &self.plugin_id)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockStorage;
    struct MockEvents;
    struct MockConfig;
    struct MockHttp;

    #[async_trait]
    impl StorageClient for MockStorage {
        async fn doc_read(&self, _collection: &str, _id: &str) -> Result<Option<Value>, PluginError> {
            Ok(Some(serde_json::json!({"id": "1", "name": "test"})))
        }
        async fn doc_write(&self, _collection: &str, _id: &str, _data: Value) -> Result<(), PluginError> {
            Ok(())
        }
        async fn doc_delete(&self, _collection: &str, _id: &str) -> Result<bool, PluginError> {
            Ok(true)
        }
        async fn doc_query(&self, _collection: &str, _filter: Value) -> Result<Vec<Value>, PluginError> {
            Ok(vec![])
        }
    }

    #[async_trait]
    impl EventClient for MockEvents {
        async fn emit(&self, _event_type: &str, _payload: Value) -> Result<(), PluginError> {
            Ok(())
        }
    }

    #[async_trait]
    impl ConfigClient for MockConfig {
        async fn read(&self, _key: &str) -> Result<Option<Value>, PluginError> {
            Ok(Some(serde_json::json!("config-value")))
        }
    }

    #[async_trait]
    impl HttpClient for MockHttp {
        async fn request(
            &self,
            _method: HttpMethod,
            _url: &str,
            _headers: Option<Value>,
            _body: Option<String>,
        ) -> Result<HttpResponse, PluginError> {
            Ok(HttpResponse {
                status: 200,
                headers: serde_json::json!({}),
                body: "ok".to_string(),
            })
        }
    }

    fn mock_context() -> ActionContext {
        ActionContext::new(
            "com.test.plugin",
            Arc::new(MockStorage),
            Arc::new(MockEvents),
            Arc::new(MockConfig),
            Arc::new(MockHttp),
        )
    }

    #[test]
    fn action_context_plugin_id() {
        let ctx = mock_context();
        assert_eq!(ctx.plugin_id(), "com.test.plugin");
    }

    #[test]
    fn action_context_debug() {
        let ctx = mock_context();
        let debug = format!("{ctx:?}");
        assert!(debug.contains("com.test.plugin"));
    }

    #[tokio::test]
    async fn storage_client_provides_typed_access() {
        let ctx = mock_context();
        let result = ctx.storage.doc_read("contacts", "1").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[tokio::test]
    async fn event_client_emit() {
        let ctx = mock_context();
        let result = ctx.events.emit("test.event", serde_json::json!({})).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn config_client_read() {
        let ctx = mock_context();
        let result = ctx.config.read("api_key").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(serde_json::json!("config-value")));
    }

    #[tokio::test]
    async fn http_client_request() {
        let ctx = mock_context();
        let result = ctx
            .http
            .request(HttpMethod::Get, "https://example.com", None, None)
            .await;
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert_eq!(resp.status, 200);
    }

    #[tokio::test]
    async fn storage_write_and_delete() {
        let ctx = mock_context();
        let write_result = ctx
            .storage
            .doc_write("contacts", "1", serde_json::json!({"name": "test"}))
            .await;
        assert!(write_result.is_ok());

        let delete_result = ctx.storage.doc_delete("contacts", "1").await;
        assert!(delete_result.is_ok());
        assert!(delete_result.unwrap());
    }

    #[tokio::test]
    async fn capability_denied_from_storage() {
        struct DeniedStorage;
        #[async_trait]
        impl StorageClient for DeniedStorage {
            async fn doc_read(&self, _: &str, _: &str) -> Result<Option<Value>, PluginError> {
                Err(PluginError::CapabilityDenied {
                    message: "storage:read not granted".into(),
                    detail: None,
                })
            }
            async fn doc_write(&self, _: &str, _: &str, _: Value) -> Result<(), PluginError> {
                Err(PluginError::CapabilityDenied {
                    message: "storage:write not granted".into(),
                    detail: None,
                })
            }
            async fn doc_delete(&self, _: &str, _: &str) -> Result<bool, PluginError> {
                Err(PluginError::CapabilityDenied {
                    message: "storage:write not granted".into(),
                    detail: None,
                })
            }
            async fn doc_query(&self, _: &str, _: Value) -> Result<Vec<Value>, PluginError> {
                Err(PluginError::CapabilityDenied {
                    message: "storage:read not granted".into(),
                    detail: None,
                })
            }
        }

        let ctx = ActionContext::new(
            "com.test.denied",
            Arc::new(DeniedStorage),
            Arc::new(MockEvents),
            Arc::new(MockConfig),
            Arc::new(MockHttp),
        );

        let result = ctx.storage.doc_read("contacts", "1").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), "CAPABILITY_DENIED");
    }
}
