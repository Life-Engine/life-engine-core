use serde::Deserialize;

/// Configuration for the backup plugin.
#[derive(Debug, Clone, Deserialize)]
pub struct BackupPluginConfig {
    /// Storage backend: "local", "s3", or "webdav".
    pub backend: String,
    /// Cron schedule expression for automated backups.
    pub schedule: Option<String>,
    /// Number of days to retain backups.
    pub retention_days: u32,
    /// Whether encryption is enabled.
    pub encryption_enabled: bool,
}

impl Default for BackupPluginConfig {
    fn default() -> Self {
        Self {
            backend: "local".to_string(),
            schedule: None,
            retention_days: 30,
            encryption_enabled: true,
        }
    }
}
