//! Core types used by the Plugin SDK.
//!
//! Defines the capability model, route registration types, event types,
//! and the plugin context that Core provides to loaded plugins.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;

// Capability is now defined in life-engine-traits and re-exported here
// to provide a single source of truth across the SDK and runtime.
pub use life_engine_traits::Capability;

/// HTTP methods for plugin route registration and outbound requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    /// HTTP GET
    Get,
    /// HTTP POST
    Post,
    /// HTTP PUT
    Put,
    /// HTTP DELETE
    Delete,
    /// HTTP PATCH
    Patch,
    /// HTTP HEAD
    Head,
    /// HTTP OPTIONS
    Options,
    /// WebDAV PROPFIND (RFC 4918) — used by CalDAV/CardDAV for collection discovery.
    Propfind,
    /// WebDAV/CalDAV/CardDAV REPORT (RFC 3253) — used for calendar-query,
    /// calendar-multiget, addressbook-query, and addressbook-multiget.
    Report,
    /// CalDAV MKCALENDAR (RFC 4791) — creates a calendar collection.
    Mkcalendar,
    /// WebDAV MKCOL (RFC 4918) — creates a collection (e.g. addressbook).
    Mkcol,
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Patch => "PATCH",
            HttpMethod::Head => "HEAD",
            HttpMethod::Options => "OPTIONS",
            HttpMethod::Propfind => "PROPFIND",
            HttpMethod::Report => "REPORT",
            HttpMethod::Mkcalendar => "MKCALENDAR",
            HttpMethod::Mkcol => "MKCOL",
        };
        f.write_str(s)
    }
}

/// A route that a plugin exposes to Core.
///
/// Core mounts all plugin routes under `/api/plugins/{plugin-id}/`.
/// For example, a plugin with ID `com.life-engine.todos` that registers
/// a route with path `/items` is reachable at
/// `/api/plugins/com.life-engine.todos/items`.
#[derive(Debug, Clone)]
pub struct PluginRoute {
    /// The HTTP method this route responds to.
    pub method: HttpMethod,
    /// The path relative to the plugin's namespace.
    pub path: String,
}

/// An event dispatched through the Core event bus.
///
/// Plugins that declare `EventsSubscribe` receive events via
/// `CorePlugin::handle_event`. Plugins that declare `EventsEmit`
/// can publish events through the `PluginContext`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreEvent {
    /// The type identifier for this event (e.g. `task.created`).
    pub event_type: String,
    /// The event payload as a JSON value.
    pub payload: serde_json::Value,
    /// The ID of the plugin that emitted this event.
    pub source_plugin: String,
    /// When the event was created.
    pub timestamp: DateTime<Utc>,
}

/// A private collection schema declared by a plugin.
///
/// Plugins use this to declare custom data collections with their own
/// JSON Schema. Core registers these under the `{plugin_id}/{collection_name}`
/// namespace to avoid collisions with Core CDM schemas and other plugins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionSchema {
    /// The collection name (e.g. `"recipes"`).
    pub name: String,
    /// The JSON Schema that governs documents in this collection.
    pub schema: serde_json::Value,
}

/// Trait for scoped credential access from within a plugin.
///
/// Implementations scope all operations to the owning plugin's ID,
/// so plugins cannot access other plugins' credentials.
#[async_trait::async_trait]
pub trait CredentialAccess: Send + Sync {
    /// Retrieve a credential value by service name.
    async fn get_credential(&self, service_name: &str) -> anyhow::Result<Option<String>>;
    /// Store a credential value by service name.
    async fn store_credential(&self, service_name: &str, value: &str) -> anyhow::Result<()>;
    /// Delete a credential by service name.
    async fn delete_credential(&self, service_name: &str) -> anyhow::Result<bool>;
    /// List all credential keys for this plugin.
    async fn list_credential_keys(&self) -> anyhow::Result<Vec<String>>;
}

/// Context provided to a `CorePlugin` during `on_load`.
///
/// `PluginContext` is the context for native, in-process plugins that
/// implement [`CorePlugin`](crate::traits::CorePlugin). It provides
/// scoped access to Core services (credentials, config) based on the
/// plugin's declared capabilities.
///
/// For the WASM plugin model, see [`ActionContext`](crate::context::ActionContext),
/// which provides equivalent service access through host function bindings
/// across the WASM boundary.
#[derive(Clone)]
pub struct PluginContext {
    /// The unique identifier of the plugin this context belongs to.
    plugin_id: String,
    /// Optional scoped credential access for the plugin.
    credentials: Option<Arc<dyn CredentialAccess>>,
    /// Optional plugin configuration loaded from the engine config.
    config: Option<serde_json::Value>,
}

impl fmt::Debug for PluginContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PluginContext")
            .field("plugin_id", &self.plugin_id)
            .field("credentials", &self.credentials.is_some())
            .finish()
    }
}

impl PluginContext {
    /// Creates a new `PluginContext` for the given plugin without credential access.
    pub fn new(plugin_id: impl Into<String>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            credentials: None,
            config: None,
        }
    }

    /// Creates a new `PluginContext` with scoped credential access.
    pub fn with_credentials(
        plugin_id: impl Into<String>,
        credentials: Arc<dyn CredentialAccess>,
    ) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            credentials: Some(credentials),
            config: None,
        }
    }

    /// Creates a new `PluginContext` with configuration and optional credentials.
    pub fn with_config(
        plugin_id: impl Into<String>,
        config: serde_json::Value,
        credentials: Option<Arc<dyn CredentialAccess>>,
    ) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            credentials,
            config: Some(config),
        }
    }

    /// Returns the plugin ID this context is scoped to.
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    /// Returns the plugin configuration, if any was provided.
    pub fn config(&self) -> Option<serde_json::Value> {
        self.config.clone()
    }

    /// Returns whether credential access is available.
    pub fn has_credentials(&self) -> bool {
        self.credentials.is_some()
    }

    /// Retrieve a credential value by service name.
    ///
    /// Returns an error if credential access is not configured.
    pub async fn get_credential(&self, service_name: &str) -> anyhow::Result<Option<String>> {
        match &self.credentials {
            Some(creds) => creds.get_credential(service_name).await,
            None => Err(anyhow::anyhow!("credential access not available")),
        }
    }

    /// Store a credential value by service name.
    ///
    /// Returns an error if credential access is not configured.
    pub async fn store_credential(&self, service_name: &str, value: &str) -> anyhow::Result<()> {
        match &self.credentials {
            Some(creds) => creds.store_credential(service_name, value).await,
            None => Err(anyhow::anyhow!("credential access not available")),
        }
    }

    /// Delete a credential by service name.
    ///
    /// Returns an error if credential access is not configured.
    pub async fn delete_credential(&self, service_name: &str) -> anyhow::Result<bool> {
        match &self.credentials {
            Some(creds) => creds.delete_credential(service_name).await,
            None => Err(anyhow::anyhow!("credential access not available")),
        }
    }

    /// List all credential keys for this plugin.
    ///
    /// Returns an error if credential access is not configured.
    pub async fn list_credential_keys(&self) -> anyhow::Result<Vec<String>> {
        match &self.credentials {
            Some(creds) => creds.list_credential_keys().await,
            None => Err(anyhow::anyhow!("credential access not available")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_serialization_roundtrip() {
        let cap = Capability::StorageRead;
        let json = serde_json::to_string(&cap).expect("serialize");
        let deserialized: Capability = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(cap, deserialized);
    }

    #[test]
    fn all_capabilities_are_distinct() {
        use std::collections::HashSet;
        let caps = vec![
            Capability::StorageRead,
            Capability::StorageWrite,
            Capability::StorageDelete,
            Capability::StorageBlobRead,
            Capability::StorageBlobWrite,
            Capability::StorageBlobDelete,
            Capability::HttpOutbound,
            Capability::CredentialsRead,
            Capability::CredentialsWrite,
            Capability::EventsSubscribe,
            Capability::EventsEmit,
            Capability::ConfigRead,
            Capability::Logging,
        ];
        let set: HashSet<_> = caps.iter().collect();
        assert_eq!(set.len(), 13);
    }

    /// Verify that every SDK-declared capability round-trips through Display/FromStr,
    /// proving the SDK re-export is the same type as the runtime traits::Capability.
    #[test]
    fn sdk_capabilities_roundtrip_through_runtime_strings() {
        let all_caps = [
            Capability::StorageRead,
            Capability::StorageWrite,
            Capability::StorageDelete,
            Capability::StorageBlobRead,
            Capability::StorageBlobWrite,
            Capability::StorageBlobDelete,
            Capability::HttpOutbound,
            Capability::EventsEmit,
            Capability::EventsSubscribe,
            Capability::ConfigRead,
            Capability::CredentialsRead,
            Capability::CredentialsWrite,
            Capability::Logging,
        ];

        for cap in &all_caps {
            let display = cap.to_string();
            let parsed: Capability = display.parse().unwrap_or_else(|_| {
                panic!("SDK capability {cap:?} display string '{display}' failed to parse back via FromStr")
            });
            assert_eq!(*cap, parsed, "round-trip mismatch for {cap:?}");
        }
    }

    #[test]
    fn http_method_serialization_roundtrip() {
        let method = HttpMethod::Post;
        let json = serde_json::to_string(&method).expect("serialize");
        let deserialized: HttpMethod = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(method, deserialized);
    }

    #[test]
    fn core_event_serialization_roundtrip() {
        let event = CoreEvent {
            event_type: "task.created".to_string(),
            payload: serde_json::json!({"id": "123"}),
            source_plugin: "com.life-engine.todos".to_string(),
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&event).expect("serialize");
        let deserialized: CoreEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(event.event_type, deserialized.event_type);
        assert_eq!(event.source_plugin, deserialized.source_plugin);
    }

    #[test]
    fn plugin_context_creation() {
        let ctx = PluginContext::new("com.life-engine.test");
        assert_eq!(ctx.plugin_id(), "com.life-engine.test");
        assert!(!ctx.has_credentials());
    }

    #[test]
    fn plugin_context_with_credentials() {
        use std::sync::Arc;

        struct MockCredentials;

        #[async_trait::async_trait]
        impl CredentialAccess for MockCredentials {
            async fn get_credential(
                &self,
                _service_name: &str,
            ) -> anyhow::Result<Option<String>> {
                Ok(Some("test-value".into()))
            }
            async fn store_credential(
                &self,
                _service_name: &str,
                _value: &str,
            ) -> anyhow::Result<()> {
                Ok(())
            }
            async fn delete_credential(&self, _service_name: &str) -> anyhow::Result<bool> {
                Ok(true)
            }
            async fn list_credential_keys(&self) -> anyhow::Result<Vec<String>> {
                Ok(vec!["key1".into()])
            }
        }

        let ctx = PluginContext::with_credentials(
            "com.life-engine.test",
            Arc::new(MockCredentials),
        );
        assert_eq!(ctx.plugin_id(), "com.life-engine.test");
        assert!(ctx.has_credentials());
    }

    #[tokio::test]
    async fn plugin_context_no_credentials_returns_error() {
        let ctx = PluginContext::new("com.life-engine.test");
        let result = ctx.get_credential("some_key").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("credential access not available"));
    }

    #[test]
    fn plugin_context_debug_output() {
        let ctx = PluginContext::new("com.life-engine.test");
        let debug = format!("{ctx:?}");
        assert!(debug.contains("com.life-engine.test"));
        assert!(debug.contains("false")); // credentials: false
    }

    #[test]
    fn plugin_route_construction() {
        let route = PluginRoute {
            method: HttpMethod::Get,
            path: "/items".to_string(),
        };
        assert_eq!(route.method, HttpMethod::Get);
        assert_eq!(route.path, "/items");
    }

    #[test]
    fn collection_schema_construction() {
        let schema = CollectionSchema {
            name: "recipes".to_string(),
            schema: serde_json::json!({
                "type": "object",
                "required": ["id", "title"],
                "properties": {
                    "id": { "type": "string" },
                    "title": { "type": "string" }
                }
            }),
        };
        assert_eq!(schema.name, "recipes");
        assert!(schema.schema.is_object());
    }

    #[test]
    fn collection_schema_serialization_roundtrip() {
        let schema = CollectionSchema {
            name: "recipes".to_string(),
            schema: serde_json::json!({
                "type": "object",
                "required": ["id"],
                "properties": { "id": { "type": "string" } }
            }),
        };
        let json = serde_json::to_string(&schema).expect("serialize");
        let deserialized: CollectionSchema = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.name, "recipes");
    }
}
