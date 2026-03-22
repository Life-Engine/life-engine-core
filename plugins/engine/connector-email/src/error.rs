use thiserror::Error;

/// Errors specific to the email connector plugin.
#[derive(Debug, Error)]
pub enum EmailConnectorError {
    #[error("IMAP not configured")]
    ImapNotConfigured,
    #[error("SMTP not configured")]
    SmtpNotConfigured,
    #[error("sync failed: {0}")]
    SyncFailed(String),
    #[error("send failed: {0}")]
    SendFailed(String),
}
