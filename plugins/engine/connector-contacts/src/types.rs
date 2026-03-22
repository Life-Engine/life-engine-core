use serde::{Deserialize, Serialize};

/// Sync state tracking for incremental contacts sync.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContactsSyncState {
    /// CardDAV sync-token from the last sync.
    pub carddav_sync_token: Option<String>,
    /// Google People API sync token from the last sync.
    pub google_sync_token: Option<String>,
}
