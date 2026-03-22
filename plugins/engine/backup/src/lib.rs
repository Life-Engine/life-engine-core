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
        assert_eq!(plugin.id(), "com.life-engine.backup");
        assert_eq!(plugin.display_name(), "Encrypted Remote Backup");
        assert_eq!(plugin.version(), "0.1.0");
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
        let ctx = PluginContext::new(plugin.id());
        plugin.on_load(&ctx).await.expect("on_load should succeed");
        plugin
            .on_unload()
            .await
            .expect("on_unload should succeed");
    }

    #[test]
    fn default_impl() {
        let plugin = BackupPlugin::default();
        assert_eq!(plugin.id(), "com.life-engine.backup");
        assert!(plugin.config.is_none());
    }
}
