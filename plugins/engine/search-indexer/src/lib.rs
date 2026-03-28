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

impl Plugin for SearchIndexerPlugin {
    fn id(&self) -> &str {
        "com.life-engine.search-indexer"
    }

    fn display_name(&self) -> &str {
        "Search Indexer"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn actions(&self) -> Vec<Action> {
        vec![
            Action::new("search", "Search across all indexed collections"),
            Action::new("reindex", "Trigger a full reindex of all collections"),
            Action::new("status", "Get indexer status and statistics"),
        ]
    }

    fn execute(
        &self,
        action: &str,
        input: PipelineMessage,
    ) -> std::result::Result<PipelineMessage, Box<dyn EngineError>> {
        match action {
            "search" | "reindex" | "status" => Ok(input),
            other => Err(Box::new(
                crate::error::SearchIndexerError::UnknownAction(other.to_string()),
            )),
        }
    }
}

life_engine_plugin_sdk::register_plugin!(SearchIndexerPlugin);

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
        assert_eq!(CorePlugin::id(&plugin), "com.life-engine.search-indexer");
        assert_eq!(CorePlugin::display_name(&plugin), "Search Indexer");
        assert_eq!(CorePlugin::version(&plugin), "0.1.0");
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
        let ctx = PluginContext::new(CorePlugin::id(&plugin));
        plugin.on_load(&ctx).await.expect("on_load should succeed");
        plugin.on_unload().await.expect("on_unload should succeed");
    }

    #[test]
    fn default_impl() {
        let plugin = SearchIndexerPlugin::default();
        assert_eq!(CorePlugin::id(&plugin), "com.life-engine.search-indexer");
    }

    // --- WASM Plugin trait tests ---

    #[test]
    fn wasm_plugin_id_matches_core() {
        let plugin = SearchIndexerPlugin::new();
        assert_eq!(Plugin::id(&plugin), CorePlugin::id(&plugin));
    }

    #[test]
    fn wasm_plugin_actions() {
        let plugin = SearchIndexerPlugin::new();
        let actions = Plugin::actions(&plugin);
        let names: Vec<&str> = actions.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, vec!["search", "reindex", "status"]);
    }

    #[test]
    fn wasm_plugin_execute_known_action() {
        let plugin = SearchIndexerPlugin::new();
        let msg = PipelineMessage {
            metadata: MessageMetadata {
                correlation_id: uuid::Uuid::new_v4(),
                source: "test".into(),
                timestamp: chrono::Utc::now(),
                auth_context: None,
                warnings: vec![],
            },
            payload: TypedPayload::Cdm(Box::new(CdmType::Task(life_engine_plugin_sdk::Task {
                    id: uuid::Uuid::new_v4(),
                    title: "test".into(),
                    description: None,
                    status: life_engine_plugin_sdk::TaskStatus::Pending,
                    priority: life_engine_plugin_sdk::TaskPriority::Medium,
                    due_date: None,
                    completed_at: None,
                    tags: vec![],
                    assignee: None,
                    parent_id: None,
                    source: "test".into(),
                    source_id: "t-1".into(),
                    extensions: None,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                }))),
        };
        let result = Plugin::execute(&plugin, "search", msg);
        assert!(result.is_ok());
    }

    #[test]
    fn wasm_plugin_execute_unknown_action() {
        let plugin = SearchIndexerPlugin::new();
        let msg = PipelineMessage {
            metadata: MessageMetadata {
                correlation_id: uuid::Uuid::new_v4(),
                source: "test".into(),
                timestamp: chrono::Utc::now(),
                auth_context: None,
                warnings: vec![],
            },
            payload: TypedPayload::Cdm(Box::new(CdmType::Task(life_engine_plugin_sdk::Task {
                    id: uuid::Uuid::new_v4(),
                    title: "test".into(),
                    description: None,
                    status: life_engine_plugin_sdk::TaskStatus::Pending,
                    priority: life_engine_plugin_sdk::TaskPriority::Medium,
                    due_date: None,
                    completed_at: None,
                    tags: vec![],
                    assignee: None,
                    parent_id: None,
                    source: "test".into(),
                    source_id: "t-1".into(),
                    extensions: None,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                }))),
        };
        let result = Plugin::execute(&plugin, "nonexistent", msg);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), "SEARCH_004");
    }
}
