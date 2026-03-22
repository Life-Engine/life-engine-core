use serde::Deserialize;

/// Configuration for the email connector plugin.
#[derive(Debug, Clone, Deserialize)]
pub struct EmailConnectorConfig {
    /// IMAP server hostname.
    pub imap_host: Option<String>,
    /// IMAP server port (default: 993).
    pub imap_port: u16,
    /// SMTP server hostname.
    pub smtp_host: Option<String>,
    /// SMTP server port (default: 587).
    pub smtp_port: u16,
    /// Interval between sync operations in seconds.
    pub sync_interval_secs: u64,
}

impl Default for EmailConnectorConfig {
    fn default() -> Self {
        Self {
            imap_host: None,
            imap_port: 993,
            smtp_host: None,
            smtp_port: 587,
            sync_interval_secs: 300,
        }
    }
}
