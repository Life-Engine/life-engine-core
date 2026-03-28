//! Lifecycle hook traits for plugin init and shutdown.
//!
//! Plugins may optionally implement `init` and `shutdown` hooks. If declared
//! in the plugin manifest, Core calls `init` once after WASM instantiation
//! and `shutdown` once before unloading the module. Both have no-op defaults
//! so plugins that don't need lifecycle management can omit them.

use async_trait::async_trait;

use crate::context::ActionContext;
use crate::error::PluginError;

/// Lifecycle hooks for plugin initialisation and teardown.
///
/// Both methods have default no-op implementations, so plugins only
/// override the hooks they need.
///
/// - `init` — Called once after WASM module instantiation. Use it to
///   validate config, warm caches, or establish connections. Returning
///   `Err` fails the plugin load.
///
/// - `shutdown` — Called once before the module is unloaded. Use it to
///   flush buffers or close connections.
#[async_trait]
pub trait LifecycleHooks: Send + Sync {
    /// Called once immediately after WASM module instantiation.
    ///
    /// Receives the `ActionContext` for accessing host services.
    /// Return `Err(PluginError)` to abort the plugin load.
    async fn init(&self, _ctx: &ActionContext) -> Result<(), PluginError> {
        Ok(())
    }

    /// Called once when the plugin is being unloaded.
    ///
    /// Use this for cleanup: flush state, close connections, etc.
    async fn shutdown(&self, _ctx: &ActionContext) -> Result<(), PluginError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{
        ActionContext, ConfigClient, EventClient, HttpClient, HttpResponse, StorageClient,
    };
    use crate::error::PluginError;
    use crate::types::HttpMethod;
    use serde_json::Value;
    use std::sync::Arc;

    // Minimal mock clients for building an ActionContext in tests.
    struct NoopStorage;
    struct NoopEvents;
    struct NoopConfig;
    struct NoopHttp;

    #[async_trait]
    impl StorageClient for NoopStorage {
        async fn doc_read(&self, _: &str, _: &str) -> Result<Option<Value>, PluginError> {
            Ok(None)
        }
        async fn doc_write(&self, _: &str, _: &str, _: Value) -> Result<(), PluginError> {
            Ok(())
        }
        async fn doc_delete(&self, _: &str, _: &str) -> Result<bool, PluginError> {
            Ok(false)
        }
        async fn doc_query(&self, _: &str, _: Value) -> Result<Vec<Value>, PluginError> {
            Ok(vec![])
        }
    }

    #[async_trait]
    impl EventClient for NoopEvents {
        async fn emit(&self, _: &str, _: Value) -> Result<(), PluginError> {
            Ok(())
        }
    }

    #[async_trait]
    impl ConfigClient for NoopConfig {
        async fn read(&self, _: &str) -> Result<Option<Value>, PluginError> {
            Ok(None)
        }
    }

    #[async_trait]
    impl HttpClient for NoopHttp {
        async fn request(
            &self,
            _: HttpMethod,
            _: &str,
            _: Option<Value>,
            _: Option<String>,
        ) -> Result<HttpResponse, PluginError> {
            Ok(HttpResponse {
                status: 200,
                headers: serde_json::json!({}),
                body: String::new(),
            })
        }
    }

    fn test_context() -> ActionContext {
        ActionContext::new(
            "com.test.lifecycle",
            Arc::new(NoopStorage),
            Arc::new(NoopEvents),
            Arc::new(NoopConfig),
            Arc::new(NoopHttp),
        )
    }

    // A plugin that uses the default (no-op) lifecycle hooks.
    struct DefaultHooksPlugin;
    impl LifecycleHooks for DefaultHooksPlugin {}

    #[tokio::test]
    async fn default_init_is_noop() {
        let plugin = DefaultHooksPlugin;
        let ctx = test_context();
        let result = plugin.init(&ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn default_shutdown_is_noop() {
        let plugin = DefaultHooksPlugin;
        let ctx = test_context();
        let result = plugin.shutdown(&ctx).await;
        assert!(result.is_ok());
    }

    // A plugin that overrides init to validate config.
    struct ValidatingPlugin {
        require_key: String,
    }

    #[async_trait]
    impl LifecycleHooks for ValidatingPlugin {
        async fn init(&self, ctx: &ActionContext) -> Result<(), PluginError> {
            let val = ctx.config.read(&self.require_key).await?;
            if val.is_none() {
                return Err(PluginError::ValidationError {
                    message: format!("required config key '{}' not found", self.require_key),
                    detail: None,
                });
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn init_can_return_error() {
        let plugin = ValidatingPlugin {
            require_key: "api_key".into(),
        };
        let ctx = test_context(); // NoopConfig returns None for all keys
        let result = plugin.init(&ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), "VALIDATION_ERROR");
        assert!(err.message().contains("api_key"));
    }
}
