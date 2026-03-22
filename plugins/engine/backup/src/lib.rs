//! Encrypted Remote Backup plugin for Life Engine Core.
//!
//! Provides full and incremental backup of all collections to local,
//! S3-compatible, or WebDAV storage targets. Backups are encrypted
//! with the master passphrase (same Argon2id key derivation as
//! SQLCipher) and support configurable scheduling and retention.

pub mod backend;
pub mod crypto;
pub mod engine;
pub mod retention;
pub mod schedule;
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
