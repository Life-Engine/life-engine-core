use serde::Deserialize;

/// Configuration for the calendar connector plugin.
#[derive(Debug, Clone, Deserialize)]
pub struct CalendarConnectorConfig {
    /// CalDAV server URL.
    pub caldav_server_url: Option<String>,
    /// CalDAV username.
    pub caldav_username: Option<String>,
    /// CalDAV calendar path.
    pub caldav_calendar_path: Option<String>,
    /// Google OAuth2 client ID.
    pub google_client_id: Option<String>,
    /// Interval between sync operations in seconds.
    pub sync_interval_secs: u64,
}

impl Default for CalendarConnectorConfig {
    fn default() -> Self {
        Self {
            caldav_server_url: None,
            caldav_username: None,
            caldav_calendar_path: None,
            google_client_id: None,
            sync_interval_secs: 300,
        }
    }
}
