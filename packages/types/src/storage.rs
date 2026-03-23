//! Storage query and mutation types for the data layer.
//!
//! These types define the interface between the storage backend trait
//! and its implementations. `StorageQuery` describes read operations
//! and `StorageMutation` describes write operations.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::PipelineMessage;

/// Describes a read operation against a storage backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageQuery {
    /// The collection to query (e.g., "events", "tasks", "contacts").
    pub collection: String,
    /// The plugin that owns this data.
    pub plugin_id: String,
    /// Filters to apply (combined with AND logic).
    pub filters: Vec<QueryFilter>,
    /// Sort ordering.
    pub sort: Vec<SortField>,
    /// Maximum number of records to return (capped at 1000).
    pub limit: Option<u32>,
    /// Number of records to skip for pagination.
    pub offset: Option<u32>,
}

/// A single filter condition for a storage query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryFilter {
    /// The field path to filter on.
    pub field: String,
    /// The comparison operator.
    pub operator: FilterOp,
    /// The value to compare against.
    pub value: serde_json::Value,
}

/// Comparison operators for query filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterOp {
    /// Exact equality.
    Eq,
    /// Greater than or equal to.
    Gte,
    /// Less than or equal to.
    Lte,
    /// Substring or element containment.
    Contains,
    /// Not equal.
    NotEq,
}

/// A sort field and direction for query results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortField {
    /// The field path to sort by.
    pub field: String,
    /// The sort direction.
    pub direction: SortDirection,
}

/// Sort direction for query results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortDirection {
    /// Ascending order.
    Asc,
    /// Descending order.
    Desc,
}

/// Describes a write operation against a storage backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageMutation {
    /// Insert a new record.
    Insert {
        /// The plugin that owns this data.
        plugin_id: String,
        /// The collection to insert into.
        collection: String,
        /// The data to insert.
        data: PipelineMessage,
    },
    /// Update an existing record with optimistic concurrency control.
    Update {
        /// The plugin that owns this data.
        plugin_id: String,
        /// The collection containing the record.
        collection: String,
        /// The record identifier.
        id: Uuid,
        /// The updated data.
        data: PipelineMessage,
        /// Expected version for optimistic concurrency control.
        expected_version: u64,
    },
    /// Delete a record.
    Delete {
        /// The plugin that owns this data.
        plugin_id: String,
        /// The collection containing the record.
        collection: String,
        /// The record identifier.
        id: Uuid,
    },
}
