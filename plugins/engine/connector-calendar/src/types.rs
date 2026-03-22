use serde::{Deserialize, Serialize};

/// Sync state tracking for incremental calendar sync.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CalendarSyncState {
    /// CalDAV sync-token from the last sync.
    pub caldav_sync_token: Option<String>,
    /// Google Calendar sync token from the last sync.
    pub google_sync_token: Option<String>,
}
