use life_engine_plugin_sdk::prelude::*;
use thiserror::Error;

/// Errors specific to the contacts connector plugin.
#[derive(Debug, Error)]
pub enum ContactsConnectorError {
    #[error("CardDAV not configured")]
    CardDavNotConfigured,
    #[error("Google Contacts not configured")]
    GoogleNotConfigured,
    #[error("sync failed: {0}")]
    SyncFailed(String),
    #[error("unknown action: {0}")]
    UnknownAction(String),
}

impl EngineError for ContactsConnectorError {
    fn code(&self) -> &str {
        match self {
            Self::CardDavNotConfigured => "CONTACTS_001",
            Self::GoogleNotConfigured => "CONTACTS_002",
            Self::SyncFailed(_) => "CONTACTS_003",
            Self::UnknownAction(_) => "CONTACTS_004",
        }
    }

    fn severity(&self) -> Severity {
        match self {
            Self::UnknownAction(_) => Severity::Fatal,
            Self::CardDavNotConfigured | Self::GoogleNotConfigured => Severity::Fatal,
            Self::SyncFailed(_) => Severity::Retryable,
        }
    }

    fn source_module(&self) -> &str {
        "connector-contacts"
    }
}
