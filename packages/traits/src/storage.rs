//! Storage adapter traits and supporting types.
//!
//! Defines the `DocumentStorageAdapter` trait, `StorageError`, query types,
//! health types, and capability descriptors. These contracts are the backbone
//! of the storage layer — every concrete adapter (SQLite, filesystem, etc.)
//! implements these traits.
//!
//! The legacy `StorageBackend` trait is preserved for backwards compatibility
//! with existing code that has not yet migrated.

use std::fmt;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use tokio::sync::mpsc;

use life_engine_types::{PipelineMessage, StorageMutation, StorageQuery};

use crate::EngineError;

// ── Legacy trait (pre-migration) ────────────────────────────────────

/// Legacy storage backend trait.
///
/// This trait predates the new adapter-based architecture. It will be
/// removed once all storage consumers migrate to `DocumentStorageAdapter`.
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Execute a read query and return matching records.
    async fn execute(
        &self,
        query: StorageQuery,
    ) -> Result<Vec<PipelineMessage>, Box<dyn EngineError>>;

    /// Execute a write mutation (insert, update, or delete).
    async fn mutate(&self, op: StorageMutation) -> Result<(), Box<dyn EngineError>>;

    /// Initialize the storage backend from configuration.
    async fn init(config: toml::Value, key: [u8; 32]) -> Result<Self, Box<dyn EngineError>>
    where
        Self: Sized;
}

// ── Error types ─────────────────────────────────────────────────────

/// Comprehensive storage error type used by both document and blob adapters.
#[derive(Debug)]
pub enum StorageError {
    /// The requested document or blob was not found.
    NotFound {
        collection: String,
        id: String,
    },
    /// A document with the same id already exists.
    AlreadyExists {
        collection: String,
        id: String,
    },
    /// A document or blob failed validation.
    ValidationFailed {
        message: String,
        field: Option<String>,
    },
    /// A plugin lacks the required capability for this operation.
    PermissionDenied {
        message: String,
    },
    /// A schema or version conflict was detected.
    SchemaConflict {
        collection: String,
        message: String,
    },
    /// The operation timed out.
    Timeout {
        message: String,
    },
    /// The storage backend connection failed.
    ConnectionFailed {
        message: String,
    },
    /// The requested operation is not supported by this adapter.
    UnsupportedOperation {
        operation: String,
    },
    /// An internal error occurred.
    Internal {
        message: String,
    },
    /// An invalid blob key was provided.
    InvalidKey {
        message: String,
    },
    /// A transaction failed.
    TransactionFailed {
        reason: String,
    },
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::NotFound { collection, id } => {
                write!(f, "not found: {collection}/{id}")
            }
            StorageError::AlreadyExists { collection, id } => {
                write!(f, "already exists: {collection}/{id}")
            }
            StorageError::ValidationFailed { message, field } => {
                if let Some(fld) = field {
                    write!(f, "validation failed on '{fld}': {message}")
                } else {
                    write!(f, "validation failed: {message}")
                }
            }
            StorageError::PermissionDenied { message } => {
                write!(f, "permission denied: {message}")
            }
            StorageError::SchemaConflict {
                collection,
                message,
            } => {
                write!(f, "schema conflict on '{collection}': {message}")
            }
            StorageError::Timeout { message } => {
                write!(f, "timeout: {message}")
            }
            StorageError::ConnectionFailed { message } => {
                write!(f, "connection failed: {message}")
            }
            StorageError::UnsupportedOperation { operation } => {
                write!(f, "unsupported operation: {operation}")
            }
            StorageError::Internal { message } => {
                write!(f, "internal error: {message}")
            }
            StorageError::InvalidKey { message } => {
                write!(f, "invalid key: {message}")
            }
            StorageError::TransactionFailed { reason } => {
                write!(f, "transaction failed: {reason}")
            }
        }
    }
}

impl std::error::Error for StorageError {}

impl StorageError {
    /// Returns `true` if the error is transient and the operation may succeed on retry.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            StorageError::Timeout { .. }
                | StorageError::ConnectionFailed { .. }
                | StorageError::TransactionFailed { .. }
        )
    }
}

// ── Query types ─────────────────────────────────────────────────────

/// Describes a query against a document collection.
#[derive(Debug, Clone, Default)]
pub struct QueryDescriptor {
    /// The collection to query.
    pub collection: String,
    /// Optional filter tree.
    pub filter: Option<FilterNode>,
    /// Sort ordering.
    pub sort: Vec<SortField>,
    /// Pagination control.
    pub pagination: Pagination,
    /// Optional field projection (return only these fields).
    pub fields: Option<Vec<String>>,
    /// Optional full-text search query.
    pub text_search: Option<String>,
}

/// A tree of filter conditions that can be composed with boolean logic.
#[derive(Debug, Clone)]
pub enum FilterNode {
    /// Match when all children match.
    And(Vec<FilterNode>),
    /// Match when any child matches.
    Or(Vec<FilterNode>),
    /// Negate the child.
    Not(Box<FilterNode>),
    /// A leaf comparison.
    Comparison {
        field: String,
        operator: FilterOperator,
        value: Value,
    },
}

/// Comparison operators for filter conditions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterOperator {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
    In,
    NotIn,
    Contains,
    StartsWith,
    Exists,
}

/// A single sort directive.
#[derive(Debug, Clone)]
pub struct SortField {
    pub field: String,
    pub direction: SortDirection,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

/// Pagination control for list/query operations.
#[derive(Debug, Clone)]
pub struct Pagination {
    /// Maximum number of results to return.
    pub limit: u64,
    /// Cursor-based pagination token (opaque to callers).
    pub cursor: Option<String>,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            limit: 100,
            cursor: None,
        }
    }
}

// ── Result types ────────────────────────────────────────────────────

/// The return type for document list/query operations.
#[derive(Debug, Clone)]
pub struct DocumentList {
    /// The matched documents.
    pub documents: Vec<Value>,
    /// Total count of matching documents (may differ from `documents.len()` due to pagination).
    pub total_count: u64,
    /// Cursor for the next page, if more results exist.
    pub next_cursor: Option<String>,
}

// ── Change event types ──────────────────────────────────────────────

/// An event emitted when a document changes in a watched collection.
#[derive(Debug, Clone)]
pub struct ChangeEvent {
    /// The collection that changed.
    pub collection: String,
    /// The id of the changed document.
    pub document_id: String,
    /// What kind of change occurred.
    pub change_type: ChangeType,
    /// When the change occurred.
    pub timestamp: DateTime<Utc>,
}

/// The type of change observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    Created,
    Updated,
    Deleted,
}

// ── Migration types ─────────────────────────────────────────────────

/// Describes a collection to be created or migrated by an adapter.
#[derive(Debug, Clone)]
pub struct CollectionDescriptor {
    /// Collection name.
    pub name: String,
    /// ID of the plugin that owns this collection.
    pub plugin_id: String,
    /// Field definitions for the collection.
    pub fields: Vec<FieldDescriptor>,
    /// Index hints (advisory).
    pub indexes: Vec<String>,
}

/// Describes a single field in a collection.
#[derive(Debug, Clone)]
pub struct FieldDescriptor {
    pub name: String,
    pub field_type: FieldType,
    pub required: bool,
}

/// Data types for collection fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldType {
    String,
    Integer,
    Float,
    Boolean,
    DateTime,
    Uuid,
    Json,
    Array,
    Object,
}

// ── Health types ────────────────────────────────────────────────────

/// Health status reported by a storage adapter.
#[derive(Debug, Clone)]
pub struct HealthReport {
    /// Overall health status.
    pub status: HealthStatus,
    /// Optional human-readable summary.
    pub message: Option<String>,
    /// Individual health checks.
    pub checks: Vec<HealthCheck>,
}

/// Health status levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// A single health check result.
#[derive(Debug, Clone)]
pub struct HealthCheck {
    pub name: String,
    pub status: HealthStatus,
    pub message: Option<String>,
}

// ── Capability types ────────────────────────────────────────────────

/// Capabilities reported by a document storage adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterCapabilities {
    /// Whether the adapter supports index creation.
    pub indexing: bool,
    /// Whether the adapter supports ACID transactions.
    pub transactions: bool,
    /// Whether the adapter supports full-text search.
    pub full_text_search: bool,
    /// Whether the adapter supports change watching.
    pub watch: bool,
    /// Whether the adapter supports batch operations.
    pub batch_operations: bool,
    /// Whether the adapter supports at-rest encryption.
    pub encryption: bool,
}

// ── Document Storage Adapter trait ──────────────────────────────────

/// Async trait for document storage adapter implementations.
///
/// All adapters must be `Send + Sync` so they can be shared across async tasks.
#[async_trait]
pub trait DocumentStorageAdapter: Send + Sync {
    /// Retrieve a single document by id.
    async fn get(&self, collection: &str, id: &str) -> Result<Value, StorageError>;

    /// Create a new document, returning the stored document (with system fields set).
    async fn create(&self, collection: &str, document: Value) -> Result<Value, StorageError>;

    /// Replace a document by id, returning the updated document.
    async fn update(
        &self,
        collection: &str,
        id: &str,
        document: Value,
    ) -> Result<Value, StorageError>;

    /// Apply a partial update (JSON merge patch) to a document.
    async fn partial_update(
        &self,
        collection: &str,
        id: &str,
        patch: Value,
    ) -> Result<Value, StorageError>;

    /// Delete a document by id.
    async fn delete(&self, collection: &str, id: &str) -> Result<(), StorageError>;

    /// Query documents using a descriptor, returning a paginated list.
    async fn query(&self, descriptor: QueryDescriptor) -> Result<DocumentList, StorageError>;

    /// Count documents matching an optional filter.
    async fn count(
        &self,
        collection: &str,
        filter: Option<FilterNode>,
    ) -> Result<u64, StorageError>;

    /// Create multiple documents in a single operation.
    async fn batch_create(
        &self,
        collection: &str,
        documents: Vec<Value>,
    ) -> Result<Vec<Value>, StorageError>;

    /// Update multiple documents in a single operation.
    async fn batch_update(
        &self,
        collection: &str,
        updates: Vec<(String, Value)>,
    ) -> Result<Vec<Value>, StorageError>;

    /// Delete multiple documents by id.
    async fn batch_delete(
        &self,
        collection: &str,
        ids: Vec<String>,
    ) -> Result<(), StorageError>;

    /// Subscribe to change events on a collection.
    async fn watch(
        &self,
        collection: &str,
    ) -> Result<mpsc::Receiver<ChangeEvent>, StorageError>;

    /// Create or update a collection's schema/structure.
    async fn migrate(&self, descriptor: CollectionDescriptor) -> Result<(), StorageError>;

    /// Report the adapter's current health status.
    async fn health(&self) -> Result<HealthReport, StorageError>;

    /// Report the adapter's capabilities (sync, no I/O).
    fn capabilities(&self) -> AdapterCapabilities;
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -- StorageError tests --

    #[test]
    fn not_found_display() {
        let err = StorageError::NotFound {
            collection: "events".into(),
            id: "123".into(),
        };
        assert_eq!(err.to_string(), "not found: events/123");
        assert!(!err.is_retryable());
    }

    #[test]
    fn timeout_is_retryable() {
        let err = StorageError::Timeout {
            message: "get timed out after 5000ms".into(),
        };
        assert!(err.is_retryable());
    }

    #[test]
    fn connection_failed_is_retryable() {
        let err = StorageError::ConnectionFailed {
            message: "refused".into(),
        };
        assert!(err.is_retryable());
    }

    #[test]
    fn validation_failed_not_retryable() {
        let err = StorageError::ValidationFailed {
            message: "bad field".into(),
            field: Some("name".into()),
        };
        assert!(!err.is_retryable());
    }

    #[test]
    fn permission_denied_display() {
        let err = StorageError::PermissionDenied {
            message: "plugin 'com.example.plugin' lacks 'DocWrite'".into(),
        };
        assert!(err.to_string().contains("com.example.plugin"));
        assert!(err.to_string().contains("DocWrite"));
    }

    // -- Query type tests --

    #[test]
    fn pagination_defaults() {
        let p = Pagination::default();
        assert_eq!(p.limit, 100);
        assert!(p.cursor.is_none());
    }

    #[test]
    fn query_descriptor_defaults() {
        let q = QueryDescriptor::default();
        assert!(q.filter.is_none());
        assert!(q.sort.is_empty());
        assert!(q.fields.is_none());
        assert!(q.text_search.is_none());
    }

    #[test]
    fn filter_node_composition() {
        let filter = FilterNode::And(vec![
            FilterNode::Comparison {
                field: "status".into(),
                operator: FilterOperator::Eq,
                value: json!("active"),
            },
            FilterNode::Not(Box::new(FilterNode::Comparison {
                field: "archived".into(),
                operator: FilterOperator::Eq,
                value: json!(true),
            })),
        ]);
        // Verify it compiles and debug-prints.
        let _ = format!("{filter:?}");
    }

    // -- Health type tests --

    #[test]
    fn health_status_ordering() {
        assert!(HealthStatus::Healthy < HealthStatus::Degraded);
        assert!(HealthStatus::Degraded < HealthStatus::Unhealthy);
    }

    // -- DocumentList tests --

    #[test]
    fn document_list_construction() {
        let list = DocumentList {
            documents: vec![json!({"id": "1"}), json!({"id": "2"})],
            total_count: 42,
            next_cursor: Some("abc".into()),
        };
        assert_eq!(list.documents.len(), 2);
        assert_eq!(list.total_count, 42);
        assert_eq!(list.next_cursor.as_deref(), Some("abc"));
    }

    // -- Trait object safety --

    #[test]
    fn document_storage_adapter_is_object_safe() {
        fn _assert_object_safe(_: &dyn DocumentStorageAdapter) {}
        fn _assert_boxed(_: Box<dyn DocumentStorageAdapter>) {}
    }
}
