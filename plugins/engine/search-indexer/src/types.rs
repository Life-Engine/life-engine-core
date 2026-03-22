use serde::{Deserialize, Serialize};

/// A search result returned by the indexer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// The ID of the matched record.
    pub record_id: String,
    /// The collection the record belongs to.
    pub collection: String,
    /// Relevance score.
    pub score: f32,
}
