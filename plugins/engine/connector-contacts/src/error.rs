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
}
