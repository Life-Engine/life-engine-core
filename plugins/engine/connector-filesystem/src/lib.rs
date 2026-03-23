//! Local filesystem and S3-compatible cloud storage connector plugin
//! for Life Engine Core.
//!
//! This connector plugin provides two storage backends:
//!
//! - `local` — Scans, indexes, and watches local directories for files.
//!   Supports glob-based include/exclude patterns, SHA-256 checksums,
//!   and incremental change detection.
//! - `s3` — Connects to S3-compatible cloud storage (AWS S3, MinIO).
//!   The actual AWS SDK integration is behind the `integration` feature.
//! - `normalizer` — Converts filesystem metadata to CDM `FileMetadata`.
//!
//! # Architecture
//!
//! - `local` — Local filesystem scanning with configurable watch paths
//! - `s3` — S3/MinIO client with sync state tracking
//! - `normalizer` — File metadata to CDM conversion with MIME detection

pub mod config;
pub mod error;
pub mod local;
pub mod normalizer;
pub mod s3;
pub mod steps;
pub mod transform;
pub mod types;

use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use life_engine_plugin_sdk::prelude::*;
use life_engine_plugin_sdk::types::Capability;

use crate::local::{LocalFsConfig, LocalFsConnector};

/// The filesystem connector plugin.
///
/// Manages local filesystem scanning and optional S3-compatible cloud
/// storage. Watches configured directories for changes and normalizes
/// file metadata into the Life Engine CDM.
pub struct FilesystemConnectorPlugin {
    /// The local filesystem connector, initialised after configuration.
    local: Option<LocalFsConnector>,
    /// Interval between automatic scan operations.
    scan_interval: Duration,
    /// Timestamp of the last successful scan.
    last_scan: Option<DateTime<Utc>>,
}

impl FilesystemConnectorPlugin {
    /// Create a new filesystem connector plugin with default settings.
    pub fn new() -> Self {
        Self {
            local: None,
            scan_interval: Duration::from_secs(300), // 5 minutes
            last_scan: None,
        }
    }

    /// Create a new filesystem connector with a custom scan interval.
    pub fn with_scan_interval(scan_interval: Duration) -> Self {
        Self {
            scan_interval,
            ..Self::new()
        }
    }

    /// Configure the local filesystem connector.
    pub fn configure_local(&mut self, config: LocalFsConfig) {
        self.local = Some(LocalFsConnector::new(config));
    }

    /// Returns whether the local connector is configured.
    pub fn has_local(&self) -> bool {
        self.local.is_some()
    }

    /// Returns the configured scan interval.
    pub fn scan_interval(&self) -> Duration {
        self.scan_interval
    }

    /// Returns the timestamp of the last successful scan.
    pub fn last_scan(&self) -> Option<DateTime<Utc>> {
        self.last_scan
    }

    /// Returns a reference to the local connector, if configured.
    pub fn local_connector(&self) -> Option<&LocalFsConnector> {
        self.local.as_ref()
    }

    /// Returns a mutable reference to the local connector, if configured.
    pub fn local_connector_mut(&mut self) -> Option<&mut LocalFsConnector> {
        self.local.as_mut()
    }
}

impl Default for FilesystemConnectorPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for FilesystemConnectorPlugin {
    fn id(&self) -> &str {
        "com.life-engine.connector-filesystem"
    }

    fn display_name(&self) -> &str {
        "Filesystem Connector"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn actions(&self) -> Vec<Action> {
        vec![
            Action::new("scan", "Trigger a filesystem scan"),
            Action::new("status", "Get the current scan status"),
            Action::new("changes", "List detected file changes since last scan"),
        ]
    }

    fn execute(
        &self,
        action: &str,
        input: PipelineMessage,
    ) -> std::result::Result<PipelineMessage, Box<dyn EngineError>> {
        match action {
            "scan" | "status" | "changes" => Ok(input),
            other => Err(Box::new(
                crate::error::FilesystemConnectorError::UnknownAction(other.to_string()),
            )),
        }
    }
}

life_engine_plugin_sdk::register_plugin!(FilesystemConnectorPlugin);

#[async_trait]
impl CorePlugin for FilesystemConnectorPlugin {
    fn id(&self) -> &str {
        "com.life-engine.connector-filesystem"
    }

    fn display_name(&self) -> &str {
        "Filesystem Connector"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![
            Capability::StorageRead,
            Capability::StorageWrite,
        ]
    }

    async fn on_load(&mut self, ctx: &PluginContext) -> Result<()> {
        tracing::info!(
            plugin_id = ctx.plugin_id(),
            "filesystem connector plugin loaded"
        );
        Ok(())
    }

    async fn on_unload(&mut self) -> Result<()> {
        self.local = None;
        self.last_scan = None;
        tracing::info!("filesystem connector plugin unloaded");
        Ok(())
    }

    fn routes(&self) -> Vec<PluginRoute> {
        vec![
            PluginRoute {
                method: HttpMethod::Post,
                path: "/scan".into(),
            },
            PluginRoute {
                method: HttpMethod::Get,
                path: "/status".into(),
            },
            PluginRoute {
                method: HttpMethod::Get,
                path: "/changes".into(),
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
    use std::path::PathBuf;

    #[test]
    fn plugin_id_is_correct() {
        let plugin = FilesystemConnectorPlugin::new();
        assert_eq!(CorePlugin::id(&plugin), "com.life-engine.connector-filesystem");
    }

    #[test]
    fn plugin_display_name() {
        let plugin = FilesystemConnectorPlugin::new();
        assert_eq!(CorePlugin::display_name(&plugin), "Filesystem Connector");
    }

    #[test]
    fn plugin_version() {
        let plugin = FilesystemConnectorPlugin::new();
        assert_eq!(CorePlugin::version(&plugin), "0.1.0");
    }

    #[test]
    fn plugin_capabilities() {
        use life_engine_test_utils::assert_plugin_capabilities;
        let plugin = FilesystemConnectorPlugin::new();
        assert_plugin_capabilities!(plugin, [
            Capability::StorageRead,
            Capability::StorageWrite,
        ]);
    }

    #[test]
    fn plugin_routes() {
        use life_engine_test_utils::assert_plugin_routes;
        let plugin = FilesystemConnectorPlugin::new();
        assert_plugin_routes!(plugin, ["/scan", "/status", "/changes"]);
    }

    #[tokio::test]
    async fn plugin_lifecycle() {
        let mut plugin = FilesystemConnectorPlugin::new();
        assert!(!plugin.has_local());

        let ctx = PluginContext::new(CorePlugin::id(&plugin));
        plugin.on_load(&ctx).await.expect("on_load should succeed");

        plugin.configure_local(LocalFsConfig {
            watch_paths: vec![PathBuf::from("/tmp/test")],
            include_patterns: vec![],
            exclude_patterns: vec![],
            compute_checksums: true,
        });
        assert!(plugin.has_local());

        plugin.on_unload().await.expect("on_unload should succeed");
        assert!(!plugin.has_local());
        assert!(plugin.last_scan().is_none());
    }

    #[tokio::test]
    async fn handle_event_returns_ok() {
        let plugin = FilesystemConnectorPlugin::new();
        life_engine_test_utils::plugin_test_helpers::test_handle_event_ok(&plugin).await;
    }

    #[test]
    fn default_scan_interval() {
        let plugin = FilesystemConnectorPlugin::new();
        assert_eq!(plugin.scan_interval(), Duration::from_secs(300));
    }

    #[test]
    fn custom_scan_interval() {
        let plugin = FilesystemConnectorPlugin::with_scan_interval(Duration::from_secs(60));
        assert_eq!(plugin.scan_interval(), Duration::from_secs(60));
    }

    #[test]
    fn default_impl() {
        let plugin = FilesystemConnectorPlugin::default();
        assert_eq!(CorePlugin::id(&plugin), "com.life-engine.connector-filesystem");
    }

    #[test]
    fn no_last_scan_initially() {
        let plugin = FilesystemConnectorPlugin::new();
        assert!(plugin.last_scan().is_none());
    }

    #[test]
    fn local_connector_accessor() {
        let mut plugin = FilesystemConnectorPlugin::new();
        assert!(plugin.local_connector().is_none());

        plugin.configure_local(LocalFsConfig {
            watch_paths: vec![PathBuf::from("/tmp")],
            ..Default::default()
        });
        assert!(plugin.local_connector().is_some());
    }

    // --- WASM Plugin trait tests ---

    #[test]
    fn wasm_plugin_id_matches_core() {
        let plugin = FilesystemConnectorPlugin::new();
        assert_eq!(Plugin::id(&plugin), CorePlugin::id(&plugin));
    }

    #[test]
    fn wasm_plugin_actions() {
        let plugin = FilesystemConnectorPlugin::new();
        let actions = Plugin::actions(&plugin);
        let names: Vec<&str> = actions.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, vec!["scan", "status", "changes"]);
    }

    #[test]
    fn wasm_plugin_execute_known_action() {
        use chrono::Utc;
        use uuid::Uuid;

        let plugin = FilesystemConnectorPlugin::new();
        let msg = PipelineMessage {
            metadata: MessageMetadata {
                correlation_id: Uuid::new_v4(),
                source: "test".into(),
                timestamp: Utc::now(),
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
        let result = Plugin::execute(&plugin, "scan", msg);
        assert!(result.is_ok());
    }

    #[test]
    fn wasm_plugin_execute_unknown_action() {
        use chrono::Utc;
        use uuid::Uuid;

        let plugin = FilesystemConnectorPlugin::new();
        let msg = PipelineMessage {
            metadata: MessageMetadata {
                correlation_id: Uuid::new_v4(),
                source: "test".into(),
                timestamp: Utc::now(),
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
        assert_eq!(err.code(), "FILESYSTEM_004");
    }
}
