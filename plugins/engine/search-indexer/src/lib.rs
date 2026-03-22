//! Full-text search indexing plugin for Life Engine Core.
//!
//! Indexes all collections into a Tantivy full-text search index,
//! enabling fast search across emails, contacts, calendar events,
//! files, tasks, and notes.
//!
//! # Architecture
//!
//! - `config` — Plugin configuration (index path, tokenizer settings)
//! - `error` — Module-specific error types
//! - `steps` — Pipeline step handlers for indexing operations
//! - `transform` — PipelineMessage input/output mapping
//! - `types` — Module-internal types

pub mod config;
pub mod error;
pub mod steps;
pub mod transform;
pub mod types;

use anyhow::Result;
use async_trait::async_trait;
use life_engine_plugin_sdk::prelude::*;
use life_engine_plugin_sdk::types::Capability;

/// The search indexer plugin.
///
/// Maintains a Tantivy full-text search index and indexes records
/// from all collections as they are created, updated, or deleted.
pub struct SearchIndexerPlugin {
    /// Plugin configuration, set during on_load.
    #[allow(dead_code)]
    config: Option<config::SearchIndexerConfig>,
}

impl SearchIndexerPlugin {
    pub fn new() -> Self {
        Self { config: None }
    }
}

impl Default for SearchIndexerPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CorePlugin for SearchIndexerPlugin {
    fn id(&self) -> &str {
        "com.life-engine.search-indexer"
    }

    fn display_name(&self) -> &str {
        "Search Indexer"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![
            Capability::StorageRead,
            Capability::EventsSubscribe,
            Capability::Logging,
        ]
    }

    async fn on_load(&mut self, _ctx: &PluginContext) -> Result<()> {
        tracing::info!("search indexer plugin loaded");
        Ok(())
    }

    async fn on_unload(&mut self) -> Result<()> {
        tracing::info!("search indexer plugin unloaded");
        Ok(())
    }

    fn routes(&self) -> Vec<PluginRoute> {
        vec![
            PluginRoute {
                method: HttpMethod::Get,
                path: "/search".into(),
            },
            PluginRoute {
                method: HttpMethod::Post,
                path: "/reindex".into(),
            },
            PluginRoute {
                method: HttpMethod::Get,
                path: "/status".into(),
            },
        ]
    }

    async fn handle_event(&self, _event: &CoreEvent) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_metadata() {
        let plugin = SearchIndexerPlugin::new();
        assert_eq!(plugin.id(), "com.life-engine.search-indexer");
        assert_eq!(plugin.display_name(), "Search Indexer");
        assert_eq!(plugin.version(), "0.1.0");
    }

    #[test]
    fn plugin_capabilities() {
        let plugin = SearchIndexerPlugin::new();
        let caps = plugin.capabilities();
        assert!(caps.contains(&Capability::StorageRead));
        assert!(caps.contains(&Capability::EventsSubscribe));
        assert!(caps.contains(&Capability::Logging));
    }

    #[test]
    fn plugin_routes_registered() {
        let plugin = SearchIndexerPlugin::new();
        let routes = plugin.routes();
        assert_eq!(routes.len(), 3);
        let paths: Vec<&str> = routes.iter().map(|r| r.path.as_str()).collect();
        assert!(paths.contains(&"/search"));
        assert!(paths.contains(&"/reindex"));
        assert!(paths.contains(&"/status"));
    }

    #[tokio::test]
    async fn plugin_lifecycle() {
        let mut plugin = SearchIndexerPlugin::new();
        let ctx = PluginContext::new(plugin.id());
        plugin.on_load(&ctx).await.expect("on_load should succeed");
        plugin.on_unload().await.expect("on_unload should succeed");
    }

    #[test]
    fn default_impl() {
        let plugin = SearchIndexerPlugin::default();
        assert_eq!(plugin.id(), "com.life-engine.search-indexer");
    }
}
