use life_engine_plugin_sdk::prelude::*;
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
    #[error("unknown action: {0}")]
    UnknownAction(String),
}

impl EngineError for SearchIndexerError {
    fn code(&self) -> &str {
        match self {
            Self::NotInitialized => "SEARCH_001",
            Self::IndexingFailed(_) => "SEARCH_002",
            Self::QueryFailed(_) => "SEARCH_003",
            Self::UnknownAction(_) => "SEARCH_004",
        }
    }

    fn severity(&self) -> Severity {
        match self {
            Self::UnknownAction(_) => Severity::Fatal,
            Self::NotInitialized => Severity::Fatal,
            Self::IndexingFailed(_) => Severity::Retryable,
            Self::QueryFailed(_) => Severity::Retryable,
        }
    }

    fn source_module(&self) -> &str {
        "search-indexer"
    }
}
