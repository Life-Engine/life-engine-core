use life_engine_plugin_sdk::prelude::*;
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
    #[error("unknown action: {0}")]
    UnknownAction(String),
    #[error("execution failed: {0}")]
    ExecutionFailed(String),
}

impl EngineError for EmailConnectorError {
    fn code(&self) -> &str {
        match self {
            Self::ImapNotConfigured => "EMAIL_001",
            Self::SmtpNotConfigured => "EMAIL_002",
            Self::SyncFailed(_) => "EMAIL_003",
            Self::SendFailed(_) => "EMAIL_004",
            Self::UnknownAction(_) => "EMAIL_005",
            Self::ExecutionFailed(_) => "EMAIL_006",
        }
    }

    fn severity(&self) -> Severity {
        match self {
            Self::UnknownAction(_) => Severity::Fatal,
            Self::ImapNotConfigured | Self::SmtpNotConfigured => Severity::Fatal,
            Self::SyncFailed(_) | Self::SendFailed(_) => Severity::Retryable,
            Self::ExecutionFailed(_) => Severity::Fatal,
        }
    }

    fn source_module(&self) -> &str {
        "connector-email"
    }
}
