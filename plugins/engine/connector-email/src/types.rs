use serde::{Deserialize, Serialize};

/// Sync state tracking for incremental IMAP sync.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncState {
    /// UIDVALIDITY value from the last sync.
    pub uid_validity: Option<u32>,
    /// Highest UID seen in the last sync.
    pub last_uid: Option<u32>,
}
