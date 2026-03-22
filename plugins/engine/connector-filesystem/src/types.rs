use serde::{Deserialize, Serialize};

/// A detected file change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    /// Path to the changed file.
    pub path: String,
    /// Type of change.
    pub change_type: FileChangeType,
}

/// The type of file change detected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileChangeType {
    Created,
    Modified,
    Deleted,
}
