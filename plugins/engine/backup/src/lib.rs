//! Encrypted Remote Backup plugin for Life Engine Core.
//!
//! Provides full and incremental backup of all collections to local,
//! S3-compatible, or WebDAV storage targets. Backups are encrypted
//! with the master passphrase (same Argon2id key derivation as
//! SQLCipher) and support configurable scheduling and retention.

pub mod backend;
pub mod config;
pub mod crypto;
pub mod engine;
pub mod error;
pub mod retention;
pub mod schedule;
pub mod steps;
pub mod transform;
pub mod types;

use anyhow::Result;
use async_trait::async_trait;
use life_engine_plugin_sdk::prelude::*;

/// The Encrypted Remote Backup plugin.
pub struct BackupPlugin {
    /// Plugin configuration, set during on_load.
    #[allow(dead_code)]
    config: Option<types::BackupConfig>,
}

impl BackupPlugin {
    pub fn new() -> Self {
        Self { config: None }
    }
}

impl Default for BackupPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for BackupPlugin {
    fn id(&self) -> &str {
        "com.life-engine.backup"
    }

    fn display_name(&self) -> &str {
        "Encrypted Remote Backup"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn actions(&self) -> Vec<Action> {
        vec![
            Action::new("backup", "Run a full backup"),
            Action::new("backup_incremental", "Run an incremental backup"),
            Action::new("restore", "Restore from a backup"),
            Action::new("restore_partial", "Restore specific collections from a backup"),
            Action::new("list_backups", "List available backups"),
            Action::new("status", "Get backup status and schedule"),
        ]
    }

    fn execute(
        &self,
        action: &str,
        input: PipelineMessage,
    ) -> std::result::Result<PipelineMessage, Box<dyn EngineError>> {
        match action {
            "backup" | "backup_incremental" | "restore" | "restore_partial"
            | "list_backups" | "status" => Ok(input),
            other => Err(Box::new(
                crate::error::BackupError::UnknownAction(other.to_string()),
            )),
        }
    }
}

life_engine_plugin_sdk::register_plugin!(BackupPlugin);

#[async_trait]
impl CorePlugin for BackupPlugin {
    fn id(&self) -> &str {
        "com.life-engine.backup"
    }

    fn display_name(&self) -> &str {
        "Encrypted Remote Backup"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![
            Capability::StorageRead,
            Capability::StorageWrite,
            Capability::ConfigRead,
            Capability::Logging,
        ]
    }

    async fn on_load(&mut self, _ctx: &PluginContext) -> Result<()> {
        tracing::info!("Backup plugin loaded");
        Ok(())
    }

    async fn on_unload(&mut self) -> Result<()> {
        tracing::info!("Backup plugin unloaded");
        Ok(())
    }

    fn routes(&self) -> Vec<PluginRoute> {
        vec![
            PluginRoute {
                method: HttpMethod::Post,
                path: "/backup".to_string(),
            },
            PluginRoute {
                method: HttpMethod::Post,
                path: "/backup/incremental".to_string(),
            },
            PluginRoute {
                method: HttpMethod::Post,
                path: "/restore".to_string(),
            },
            PluginRoute {
                method: HttpMethod::Post,
                path: "/restore/partial".to_string(),
            },
            PluginRoute {
                method: HttpMethod::Get,
                path: "/backups".to_string(),
            },
            PluginRoute {
                method: HttpMethod::Get,
                path: "/status".to_string(),
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
        let plugin = BackupPlugin::new();
        assert_eq!(CorePlugin::id(&plugin), "com.life-engine.backup");
        assert_eq!(CorePlugin::display_name(&plugin), "Encrypted Remote Backup");
        assert_eq!(CorePlugin::version(&plugin), "0.1.0");
    }

    #[test]
    fn plugin_capabilities() {
        let plugin = BackupPlugin::new();
        let caps = plugin.capabilities();
        assert_eq!(caps.len(), 4);
        assert!(caps.contains(&Capability::StorageRead));
        assert!(caps.contains(&Capability::StorageWrite));
        assert!(caps.contains(&Capability::ConfigRead));
        assert!(caps.contains(&Capability::Logging));
    }

    #[test]
    fn plugin_routes_registered() {
        let plugin = BackupPlugin::new();
        let routes = plugin.routes();
        assert_eq!(routes.len(), 6);

        let paths: Vec<&str> = routes.iter().map(|r| r.path.as_str()).collect();
        assert!(paths.contains(&"/backup"));
        assert!(paths.contains(&"/backup/incremental"));
        assert!(paths.contains(&"/restore"));
        assert!(paths.contains(&"/restore/partial"));
        assert!(paths.contains(&"/backups"));
        assert!(paths.contains(&"/status"));
    }

    #[test]
    fn plugin_routes_methods() {
        let plugin = BackupPlugin::new();
        let routes = plugin.routes();

        let post_routes: Vec<&str> = routes
            .iter()
            .filter(|r| r.method == HttpMethod::Post)
            .map(|r| r.path.as_str())
            .collect();
        assert_eq!(post_routes.len(), 4);

        let get_routes: Vec<&str> = routes
            .iter()
            .filter(|r| r.method == HttpMethod::Get)
            .map(|r| r.path.as_str())
            .collect();
        assert_eq!(get_routes.len(), 2);
    }

    #[tokio::test]
    async fn plugin_lifecycle() {
        let mut plugin = BackupPlugin::new();
        let ctx = PluginContext::new(CorePlugin::id(&plugin));
        plugin.on_load(&ctx).await.expect("on_load should succeed");
        plugin
            .on_unload()
            .await
            .expect("on_unload should succeed");
    }

    #[test]
    fn default_impl() {
        let plugin = BackupPlugin::default();
        assert_eq!(CorePlugin::id(&plugin), "com.life-engine.backup");
        assert!(plugin.config.is_none());
    }

    // --- WASM Plugin trait tests ---

    #[test]
    fn wasm_plugin_id_matches_core() {
        let plugin = BackupPlugin::new();
        assert_eq!(Plugin::id(&plugin), CorePlugin::id(&plugin));
    }

    #[test]
    fn wasm_plugin_actions() {
        let plugin = BackupPlugin::new();
        let actions = Plugin::actions(&plugin);
        let names: Vec<&str> = actions.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, vec!["backup", "backup_incremental", "restore", "restore_partial", "list_backups", "status"]);
    }

    #[test]
    fn wasm_plugin_execute_known_action() {
        let plugin = BackupPlugin::new();
        let msg = PipelineMessage {
            metadata: MessageMetadata {
                correlation_id: uuid::Uuid::new_v4(),
                source: "test".into(),
                timestamp: chrono::Utc::now(),
                auth_context: None,
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
        let result = Plugin::execute(&plugin, "backup", msg);
        assert!(result.is_ok());
    }

    #[test]
    fn wasm_plugin_execute_unknown_action() {
        let plugin = BackupPlugin::new();
        let msg = PipelineMessage {
            metadata: MessageMetadata {
                correlation_id: uuid::Uuid::new_v4(),
                source: "test".into(),
                timestamp: chrono::Utc::now(),
                auth_context: None,
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
        assert_eq!(err.code(), "BACKUP_005");
    }
}
