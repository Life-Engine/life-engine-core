//! The `CorePlugin` trait that all Life Engine Core plugins must implement.
//!
//! This is the single entry point that Core uses to manage the plugin
//! lifecycle, discover routes, and dispatch events.

use anyhow::Result;
use async_trait::async_trait;

use crate::types::{Capability, CollectionSchema, CoreEvent, PluginContext, PluginRoute};

/// The trait that every Core plugin must implement.
///
/// Core uses this trait to manage the plugin lifecycle:
///
/// 1. Load the plugin and call `on_load` with a scoped `PluginContext`
/// 2. Discover routes via `routes()` and mount them under the plugin namespace
/// 3. Dispatch events via `handle_event` for subscribed event types
/// 4. Call `on_unload` when the plugin is being removed
///
/// # Example
///
/// ```rust,ignore
/// use life_engine_plugin_sdk::prelude::*;
///
/// struct MyPlugin;
///
/// #[async_trait]
/// impl CorePlugin for MyPlugin {
///     fn id(&self) -> &str { "com.example.my-plugin" }
///     fn display_name(&self) -> &str { "My Plugin" }
///     fn version(&self) -> &str { "0.1.0" }
///     fn capabilities(&self) -> Vec<Capability> { vec![Capability::StorageRead] }
///     async fn on_load(&mut self, _ctx: &PluginContext) -> Result<()> { Ok(()) }
///     async fn on_unload(&mut self) -> Result<()> { Ok(()) }
///     fn routes(&self) -> Vec<PluginRoute> { vec![] }
///     async fn handle_event(&self, _event: &CoreEvent) -> Result<()> { Ok(()) }
/// }
/// ```
#[async_trait]
pub trait CorePlugin: Send + Sync {
    /// Returns the unique plugin identifier in reverse-domain format.
    ///
    /// Example: `com.life-engine.google-calendar`
    fn id(&self) -> &str;

    /// Returns a human-readable name for UI and logging.
    fn display_name(&self) -> &str;

    /// Returns the plugin version string (semver).
    fn version(&self) -> &str;

    /// Declares the scoped capabilities this plugin requires.
    ///
    /// Core grants only the requested capabilities at load time.
    fn capabilities(&self) -> Vec<Capability>;

    /// Called when Core loads the plugin.
    ///
    /// Receives a `PluginContext` for accessing storage, config, events,
    /// and logging. Use this for initialisation logic.
    async fn on_load(&mut self, ctx: &PluginContext) -> Result<()>;

    /// Called when Core unloads the plugin.
    ///
    /// Use this for cleanup: close connections, flush buffers, etc.
    async fn on_unload(&mut self) -> Result<()>;

    /// Returns the HTTP routes this plugin exposes.
    ///
    /// Core mounts them under `/api/plugins/{plugin-id}/`.
    fn routes(&self) -> Vec<PluginRoute>;

    /// Called when a subscribed event is emitted on the event bus.
    async fn handle_event(&self, event: &CoreEvent) -> Result<()>;

    /// Returns the private collection schemas declared by this plugin.
    ///
    /// Core registers each returned schema under the namespaced key
    /// `{plugin_id}/{collection_name}` in the schema registry. The
    /// default implementation returns an empty vec (no private collections).
    fn collections(&self) -> Vec<CollectionSchema> {
        vec![]
    }

    /// Handle an HTTP request to a plugin-registered route.
    ///
    /// Core invokes this method when a request matches one of the routes
    /// returned by `routes()`. The default implementation returns an error
    /// for plugins that only register routes for discovery purposes.
    ///
    /// # Arguments
    ///
    /// - `method` — The HTTP method of the incoming request.
    /// - `path` — The route path relative to the plugin's namespace.
    /// - `body` — The request body as a JSON value.
    ///
    /// # Returns
    ///
    /// A JSON value representing the response body.
    async fn handle_route(
        &self,
        _method: &crate::types::HttpMethod,
        _path: &str,
        _body: serde_json::Value,
    ) -> Result<serde_json::Value> {
        Err(anyhow::anyhow!("route handling not implemented"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestPlugin {
        loaded: bool,
    }

    impl TestPlugin {
        fn new() -> Self {
            Self { loaded: false }
        }
    }

    #[async_trait]
    impl CorePlugin for TestPlugin {
        fn id(&self) -> &str {
            "com.test.plugin"
        }
        fn display_name(&self) -> &str {
            "Test Plugin"
        }
        fn version(&self) -> &str {
            "0.1.0"
        }
        fn capabilities(&self) -> Vec<Capability> {
            vec![Capability::StorageRead, Capability::Logging]
        }
        async fn on_load(&mut self, _ctx: &PluginContext) -> Result<()> {
            self.loaded = true;
            Ok(())
        }
        async fn on_unload(&mut self) -> Result<()> {
            self.loaded = false;
            Ok(())
        }
        fn routes(&self) -> Vec<PluginRoute> {
            vec![]
        }
        async fn handle_event(&self, _event: &CoreEvent) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn plugin_lifecycle() {
        let mut plugin = TestPlugin::new();
        assert!(!plugin.loaded);

        let ctx = PluginContext::new(plugin.id());
        plugin.on_load(&ctx).await.expect("on_load should succeed");
        assert!(plugin.loaded);

        plugin
            .on_unload()
            .await
            .expect("on_unload should succeed");
        assert!(!plugin.loaded);
    }

    #[test]
    fn plugin_metadata() {
        let plugin = TestPlugin::new();
        assert_eq!(plugin.id(), "com.test.plugin");
        assert_eq!(plugin.display_name(), "Test Plugin");
        assert_eq!(plugin.version(), "0.1.0");
        assert_eq!(plugin.capabilities().len(), 2);
    }

    #[test]
    fn collections_default_returns_empty_vec() {
        let plugin = TestPlugin::new();
        assert!(plugin.collections().is_empty());
    }

    #[tokio::test]
    async fn handle_event_default() {
        let plugin = TestPlugin::new();
        let event = CoreEvent {
            event_type: "test.event".to_string(),
            payload: serde_json::json!({}),
            source_plugin: "com.test.other".to_string(),
            timestamp: chrono::Utc::now(),
        };
        let result = plugin.handle_event(&event).await;
        assert!(result.is_ok());
    }
}
