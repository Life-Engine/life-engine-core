use std::path::PathBuf;

use serde::Deserialize;

/// Configuration for the search indexer plugin.
#[derive(Debug, Clone, Deserialize)]
pub struct SearchIndexerConfig {
    /// Path to the Tantivy index directory.
    pub index_path: PathBuf,
    /// Maximum number of documents to buffer before committing.
    pub commit_threshold: usize,
}

impl Default for SearchIndexerConfig {
    fn default() -> Self {
        Self {
            index_path: PathBuf::from("data/search-index"),
            commit_threshold: 1000,
        }
    }
}
