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
}
