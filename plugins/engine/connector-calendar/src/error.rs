use life_engine_plugin_sdk::prelude::*;
use thiserror::Error;

/// Errors specific to the calendar connector plugin.
#[derive(Debug, Error)]
pub enum CalendarConnectorError {
    #[error("CalDAV not configured")]
    CalDavNotConfigured,
    #[error("Google Calendar not configured")]
    GoogleNotConfigured,
    #[error("sync failed: {0}")]
    SyncFailed(String),
    #[error("OAuth2 flow failed: {0}")]
    OAuthFailed(String),
    #[error("unknown action: {0}")]
    UnknownAction(String),
}

impl EngineError for CalendarConnectorError {
    fn code(&self) -> &str {
        match self {
            Self::CalDavNotConfigured => "CALENDAR_001",
            Self::GoogleNotConfigured => "CALENDAR_002",
            Self::SyncFailed(_) => "CALENDAR_003",
            Self::OAuthFailed(_) => "CALENDAR_004",
            Self::UnknownAction(_) => "CALENDAR_005",
        }
    }

    fn severity(&self) -> Severity {
        match self {
            Self::UnknownAction(_) => Severity::Fatal,
            Self::CalDavNotConfigured | Self::GoogleNotConfigured => Severity::Fatal,
            Self::SyncFailed(_) => Severity::Retryable,
            Self::OAuthFailed(_) => Severity::Fatal,
        }
    }

    fn source_module(&self) -> &str {
        "connector-calendar"
    }
}
