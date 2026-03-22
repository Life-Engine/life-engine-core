use serde::Deserialize;

/// Configuration for the contacts connector plugin.
#[derive(Debug, Clone, Deserialize)]
pub struct ContactsConnectorConfig {
    /// CardDAV server URL.
    pub carddav_server_url: Option<String>,
    /// CardDAV username.
    pub carddav_username: Option<String>,
    /// CardDAV addressbook path.
    pub carddav_addressbook_path: Option<String>,
    /// Google OAuth2 client ID.
    pub google_client_id: Option<String>,
    /// Interval between sync operations in seconds.
    pub sync_interval_secs: u64,
}

impl Default for ContactsConnectorConfig {
    fn default() -> Self {
        Self {
            carddav_server_url: None,
            carddav_username: None,
            carddav_addressbook_path: None,
            google_client_id: None,
            sync_interval_secs: 300,
        }
    }
}
