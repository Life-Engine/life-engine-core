//! Health check endpoint returning system status, uptime, and plugin info.

use crate::config::{CoreConfig, LogReloadHandle};
use crate::conflict::ConflictStore;
use crate::credential_store::SqliteCredentialStore;
use crate::federation::FederationStore;
use crate::household::HouseholdStore;
use crate::identity::IdentityStore;
use crate::message_bus::MessageBus;
use crate::plugin_loader::{PluginLoader, PluginStatus};
use crate::rate_limit::GeneralRateLimiter;
use crate::schema_registry::ValidatedStorage;
use crate::search::SearchEngine;
use crate::sqlite_storage::SqliteStorage;
use axum::extract::State;
use axum::Json;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, RwLock};

/// Shared application state accessible to route handlers.
#[derive(Clone)]
pub struct AppState {
    /// When the server started.
    pub start_time: Instant,
    /// The plugin loader.
    pub plugin_loader: Arc<Mutex<PluginLoader>>,
    /// The storage backend (optional until fully wired).
    pub storage: Option<Arc<SqliteStorage>>,
    /// The message bus for event broadcasting.
    pub message_bus: Arc<MessageBus>,
    /// The conflict store for sync conflict resolution.
    pub conflict_store: Option<Arc<ConflictStore>>,
    /// The validated storage layer with schema validation and quarantine.
    pub validated_storage: Option<Arc<ValidatedStorage>>,
    /// The full-text search engine.
    pub search_engine: Option<Arc<SearchEngine>>,
    /// The encrypted credential store.
    pub credential_store: Option<Arc<SqliteCredentialStore>>,
    /// The household store for multi-user support.
    pub household_store: Option<Arc<HouseholdStore>>,
    /// The federation store for hub-to-hub sync.
    pub federation_store: Option<Arc<FederationStore>>,
    /// The identity credential store.
    pub identity_store: Option<Arc<IdentityStore>>,
    /// The live configuration (readable and writable at runtime).
    pub config: Arc<RwLock<CoreConfig>>,
    /// Path to the config YAML file for persisting changes.
    pub config_path: Option<PathBuf>,
    /// Handle for hot-reloading the tracing EnvFilter (log level).
    pub log_reload_handle: Option<LogReloadHandle>,
    /// Shared rate limiter for runtime reconfiguration.
    pub rate_limiter: Option<GeneralRateLimiter>,
}

/// Health response body.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// Overall system status.
    pub status: String,
    /// Application version.
    pub version: String,
    /// Seconds since the server started.
    pub uptime_seconds: u64,
    /// Number of currently loaded plugins.
    pub loaded_plugins: usize,
    /// Information about each plugin.
    pub plugins: Vec<PluginHealthInfo>,
}

/// Plugin information in the health response.
#[derive(Debug, Serialize)]
pub struct PluginHealthInfo {
    /// Plugin ID.
    pub id: String,
    /// Plugin version.
    pub version: String,
    /// Plugin status.
    pub status: String,
}

/// GET /api/system/health
pub async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    let uptime = state.start_time.elapsed().as_secs();
    let loader = state.plugin_loader.lock().await;

    let plugins: Vec<PluginHealthInfo> = loader
        .plugin_info()
        .into_iter()
        .map(|info| PluginHealthInfo {
            id: info.id,
            version: info.version,
            status: match info.status {
                PluginStatus::Registered => "registered".into(),
                PluginStatus::Loaded => "loaded".into(),
                PluginStatus::Failed(ref msg) => format!("failed: {msg}"),
                PluginStatus::Unloaded => "unloaded".into(),
            },
        })
        .collect();

    let loaded_count = loader.loaded_count();

    Json(HealthResponse {
        status: "ok".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        uptime_seconds: uptime,
        loaded_plugins: loaded_count,
        plugins,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::default_app_state;

    #[tokio::test]
    async fn health_returns_ok_status() {
        let state = default_app_state();
        let Json(response) = health_check(State(state)).await;
        assert_eq!(response.status, "ok");
        assert_eq!(response.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(response.loaded_plugins, 0);
        assert!(response.plugins.is_empty());
    }

    #[tokio::test]
    async fn health_uptime_increases() {
        let mut state = default_app_state();
        state.start_time = Instant::now() - std::time::Duration::from_secs(10);
        let Json(response) = health_check(State(state)).await;
        assert!(response.uptime_seconds >= 10);
    }

    #[tokio::test]
    async fn health_reflects_loaded_plugins() {
        use async_trait::async_trait;
        use life_engine_plugin_sdk::types::{Capability, CoreEvent, PluginContext, PluginRoute};
        use life_engine_plugin_sdk::{CorePlugin, Result};

        struct TestPlugin;

        #[async_trait]
        impl CorePlugin for TestPlugin {
            fn id(&self) -> &str {
                "com.test.health"
            }
            fn display_name(&self) -> &str {
                "Health Test"
            }
            fn version(&self) -> &str {
                "2.0.0"
            }
            fn capabilities(&self) -> Vec<Capability> {
                vec![]
            }
            async fn on_load(&mut self, _ctx: &PluginContext) -> Result<()> {
                Ok(())
            }
            async fn on_unload(&mut self) -> Result<()> {
                Ok(())
            }
            fn routes(&self) -> Vec<PluginRoute> {
                vec![]
            }
            async fn handle_event(&self, _event: &CoreEvent) -> Result<()> {
                Ok(())
            }
        }

        let mut loader = PluginLoader::new();
        loader.register(Box::new(TestPlugin)).unwrap();
        loader.load_all().await;

        let mut state = default_app_state();
        state.plugin_loader = Arc::new(Mutex::new(loader));

        let Json(response) = health_check(State(state)).await;
        assert_eq!(response.loaded_plugins, 1);
        assert_eq!(response.plugins.len(), 1);
        assert_eq!(response.plugins[0].id, "com.test.health");
        assert_eq!(response.plugins[0].version, "2.0.0");
        assert_eq!(response.plugins[0].status, "loaded");
    }
}
