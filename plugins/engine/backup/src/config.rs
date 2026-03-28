use serde::Deserialize;

/// Configuration for the backup plugin.
///
/// Note: Encryption is always applied — it cannot be disabled.
/// The passphrase and Argon2 params are in the runtime `BackupConfig`.
#[derive(Debug, Clone, Deserialize)]
pub struct BackupPluginConfig {
    /// Storage backend: "local", "s3", or "webdav".
    pub backend: String,
    /// Cron schedule expression for automated backups.
    pub schedule: Option<String>,
    /// Number of days to retain backups.
    pub retention_days: u32,
}

impl Default for BackupPluginConfig {
    fn default() -> Self {
        Self {
            backend: "local".to_string(),
            schedule: None,
            retention_days: 30,
        }
    }
}
