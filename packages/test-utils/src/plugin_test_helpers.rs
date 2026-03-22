//! Generic test helpers for Life Engine Core plugins.
//!
//! Provides reusable async test functions that exercise common plugin
//! lifecycle and event handling patterns, eliminating boilerplate across
//! connector plugin test suites.

use life_engine_plugin_sdk::prelude::*;

/// Exercise the full plugin lifecycle: `on_load` then `on_unload`.
///
/// Creates a `PluginContext` scoped to the plugin's ID, calls `on_load`,
/// and then `on_unload`, asserting both succeed.
///
/// # Panics
///
/// Panics if either `on_load` or `on_unload` returns an error.
pub async fn test_plugin_lifecycle<P: CorePlugin>(plugin: &mut P) {
    let ctx = PluginContext::new(plugin.id());
    plugin
        .on_load(&ctx)
        .await
        .expect("on_load should succeed");
    plugin
        .on_unload()
        .await
        .expect("on_unload should succeed");
}

/// Assert that a plugin's `handle_event` returns `Ok(())` for a test event.
///
/// Sends a synthetic `CoreEvent` with type `"test.event"` and an empty
/// payload, asserting the plugin handles it without error.
///
/// # Panics
///
/// Panics if `handle_event` returns an error.
pub async fn test_handle_event_ok<P: CorePlugin>(plugin: &P) {
    let event = create_test_core_event();
    let result = plugin.handle_event(&event).await;
    assert!(
        result.is_ok(),
        "handle_event should return Ok, got: {:?}",
        result.err()
    );
}

/// Create a synthetic `CoreEvent` for testing.
///
/// Returns a `CoreEvent` with:
///
/// - `event_type`: `"test.event"`
/// - `payload`: empty JSON object
/// - `source_plugin`: `"com.test.other"`
/// - `timestamp`: current UTC time
pub fn create_test_core_event() -> CoreEvent {
    CoreEvent {
        event_type: "test.event".to_string(),
        payload: serde_json::json!({}),
        source_plugin: "com.test.other".to_string(),
        timestamp: chrono::Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct DummyPlugin {
        loaded: bool,
    }

    impl DummyPlugin {
        fn new() -> Self {
            Self { loaded: false }
        }
    }

    #[async_trait]
    impl CorePlugin for DummyPlugin {
        fn id(&self) -> &str {
            "com.test.dummy"
        }
        fn display_name(&self) -> &str {
            "Dummy Plugin"
        }
        fn version(&self) -> &str {
            "0.0.1"
        }
        fn capabilities(&self) -> Vec<Capability> {
            vec![Capability::Logging]
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
    async fn test_plugin_lifecycle_helper() {
        let mut plugin = DummyPlugin::new();
        assert!(!plugin.loaded);
        test_plugin_lifecycle(&mut plugin).await;
        // After lifecycle, plugin should be unloaded
        assert!(!plugin.loaded);
    }

    #[tokio::test]
    async fn test_handle_event_ok_helper() {
        let plugin = DummyPlugin::new();
        test_handle_event_ok(&plugin).await;
    }

    #[test]
    fn create_test_core_event_has_correct_fields() {
        let event = create_test_core_event();
        assert_eq!(event.event_type, "test.event");
        assert_eq!(event.source_plugin, "com.test.other");
        assert_eq!(event.payload, serde_json::json!({}));
    }
}
