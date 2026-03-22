use thiserror::Error;

/// Errors specific to the search indexer plugin.
#[derive(Debug, Error)]
pub enum SearchIndexerError {
    #[error("index not initialized")]
    NotInitialized,
    #[error("indexing failed: {0}")]
    IndexingFailed(String),
    #[error("search query failed: {0}")]
    QueryFailed(String),
}
