use std::path::PathBuf;

use serde::Deserialize;

/// Configuration for the filesystem connector plugin.
#[derive(Debug, Clone, Deserialize)]
pub struct FilesystemConnectorConfig {
    /// Directories to watch for changes.
    pub watch_paths: Vec<PathBuf>,
    /// Glob patterns to include.
    pub include_patterns: Vec<String>,
    /// Glob patterns to exclude.
    pub exclude_patterns: Vec<String>,
    /// Whether to compute SHA-256 checksums for files.
    pub compute_checksums: bool,
    /// Interval between scan operations in seconds.
    pub scan_interval_secs: u64,
}

impl Default for FilesystemConnectorConfig {
    fn default() -> Self {
        Self {
            watch_paths: Vec::new(),
            include_patterns: Vec::new(),
            exclude_patterns: Vec::new(),
            compute_checksums: true,
            scan_interval_secs: 300,
        }
    }
}
