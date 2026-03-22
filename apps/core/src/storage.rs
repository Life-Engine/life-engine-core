//! Storage adapter trait for pluggable persistence backends.
//!
//! Defines the async trait that all storage implementations must satisfy.
//! The SQLite/SQLCipher implementation is provided in WP 1.2.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A stored record in the document model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Record {
    /// Unique record identifier.
    pub id: String,
    /// The plugin that owns this record.
    pub plugin_id: String,
    /// The collection this record belongs to (e.g. "tasks", "contacts").
    pub collection: String,
    /// The record payload as a JSON value.
    pub data: Value,
    /// Optimistic concurrency version number.
    pub version: i64,
    /// The user who owns this record (None for legacy/shared data).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// The household this record belongs to (None for single-user mode).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub household_id: Option<String>,
    /// When the record was created.
    pub created_at: DateTime<Utc>,
    /// When the record was last updated.
    pub updated_at: DateTime<Utc>,
}

/// Filters for querying records.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryFilters {
    /// Equality filters: field must equal value exactly.
    #[serde(default)]
    pub equality: Vec<FieldFilter>,

    /// Comparison filters: $gte, $lte.
    #[serde(default)]
    pub comparison: Vec<ComparisonFilter>,

    /// Text search: $contains.
    #[serde(default)]
    pub text_search: Vec<TextFilter>,

    /// Logical AND group: all inner filters must match.
    #[serde(default)]
    pub and: Vec<QueryFilters>,

    /// Logical OR group: at least one inner filter must match.
    #[serde(default)]
    pub or: Vec<QueryFilters>,
}

/// A simple field = value equality filter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldFilter {
    /// The JSON field path to filter on.
    pub field: String,
    /// The value to match.
    pub value: Value,
}

/// A comparison filter ($gte, $lte).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonFilter {
    /// The JSON field path.
    pub field: String,
    /// The comparison operator.
    pub operator: ComparisonOp,
    /// The value to compare against.
    pub value: Value,
}

/// Comparison operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComparisonOp {
    /// Greater than or equal.
    Gte,
    /// Less than or equal.
    Lte,
    /// Greater than.
    Gt,
    /// Less than.
    Lt,
}

/// A text search filter ($contains).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextFilter {
    /// The JSON field path to search.
    pub field: String,
    /// The substring to search for.
    pub contains: String,
}

/// Sort options for query results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortOptions {
    /// The field to sort by.
    pub sort_by: String,
    /// The sort direction.
    pub sort_dir: SortDirection,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortDirection {
    /// Ascending order.
    Asc,
    /// Descending order.
    Desc,
}

/// Pagination options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    /// Maximum number of records to return (default 50, max 1000).
    pub limit: u32,
    /// Number of records to skip.
    pub offset: u32,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            limit: 50,
            offset: 0,
        }
    }
}

impl Pagination {
    /// Clamp the limit to the maximum allowed value.
    pub fn clamped(self) -> Self {
        Self {
            limit: self.limit.min(1000),
            offset: self.offset,
        }
    }
}

/// The result of a paginated query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    /// The matching records.
    pub records: Vec<Record>,
    /// Total number of matching records (before pagination).
    pub total: u64,
    /// The limit that was applied.
    pub limit: u32,
    /// The offset that was applied.
    pub offset: u32,
}

/// Async trait for pluggable storage backends.
///
/// All methods take `plugin_id` and `collection` to scope operations
/// to a specific plugin's data partition.
#[async_trait]
pub trait StorageAdapter: Send + Sync {
    /// Retrieve a single record by its ID.
    async fn get(
        &self,
        plugin_id: &str,
        collection: &str,
        id: &str,
    ) -> anyhow::Result<Option<Record>>;

    /// Create a new record. Returns the created record with server-assigned
    /// fields (id, version, timestamps).
    async fn create(
        &self,
        plugin_id: &str,
        collection: &str,
        data: Value,
    ) -> anyhow::Result<Record>;

    /// Update an existing record by ID. Uses optimistic concurrency:
    /// the update succeeds only if the provided `version` matches.
    async fn update(
        &self,
        plugin_id: &str,
        collection: &str,
        id: &str,
        data: Value,
        version: i64,
    ) -> anyhow::Result<Record>;

    /// Query records with filters, sorting, and pagination.
    async fn query(
        &self,
        plugin_id: &str,
        collection: &str,
        filters: QueryFilters,
        sort: Option<SortOptions>,
        pagination: Pagination,
    ) -> anyhow::Result<QueryResult>;

    /// Delete a record by ID. Returns true if the record was deleted.
    async fn delete(
        &self,
        plugin_id: &str,
        collection: &str,
        id: &str,
    ) -> anyhow::Result<bool>;

    /// List all records in a collection with pagination.
    async fn list(
        &self,
        plugin_id: &str,
        collection: &str,
        sort: Option<SortOptions>,
        pagination: Pagination,
    ) -> anyhow::Result<QueryResult>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// In-memory mock implementation for testing the trait contract.
    struct MockStorage {
        records: Mutex<HashMap<String, Record>>,
    }

    impl MockStorage {
        fn new() -> Self {
            Self {
                records: Mutex::new(HashMap::new()),
            }
        }

        fn make_key(plugin_id: &str, collection: &str, id: &str) -> String {
            format!("{plugin_id}:{collection}:{id}")
        }
    }

    #[async_trait]
    impl StorageAdapter for MockStorage {
        async fn get(
            &self,
            plugin_id: &str,
            collection: &str,
            id: &str,
        ) -> anyhow::Result<Option<Record>> {
            let key = Self::make_key(plugin_id, collection, id);
            let records = self.records.lock().unwrap();
            Ok(records.get(&key).cloned())
        }

        async fn create(
            &self,
            plugin_id: &str,
            collection: &str,
            data: Value,
        ) -> anyhow::Result<Record> {
            let id = uuid::Uuid::new_v4().to_string();
            let now = Utc::now();
            let record = Record {
                id: id.clone(),
                plugin_id: plugin_id.into(),
                collection: collection.into(),
                data,
                version: 1,
                user_id: None,
                household_id: None,
                created_at: now,
                updated_at: now,
            };
            let key = Self::make_key(plugin_id, collection, &id);
            self.records.lock().unwrap().insert(key, record.clone());
            Ok(record)
        }

        async fn update(
            &self,
            plugin_id: &str,
            collection: &str,
            id: &str,
            data: Value,
            version: i64,
        ) -> anyhow::Result<Record> {
            let key = Self::make_key(plugin_id, collection, id);
            let mut records = self.records.lock().unwrap();
            let record = records
                .get(&key)
                .ok_or_else(|| anyhow::anyhow!("not found"))?;
            if record.version != version {
                return Err(anyhow::anyhow!("version mismatch"));
            }
            let updated = Record {
                data,
                version: version + 1,
                updated_at: Utc::now(),
                ..record.clone()
            };
            records.insert(key, updated.clone());
            Ok(updated)
        }

        async fn query(
            &self,
            plugin_id: &str,
            collection: &str,
            _filters: QueryFilters,
            _sort: Option<SortOptions>,
            pagination: Pagination,
        ) -> anyhow::Result<QueryResult> {
            let records = self.records.lock().unwrap();
            let matching: Vec<Record> = records
                .values()
                .filter(|r| r.plugin_id == plugin_id && r.collection == collection)
                .cloned()
                .collect();
            let total = matching.len() as u64;
            let paged = matching
                .into_iter()
                .skip(pagination.offset as usize)
                .take(pagination.limit as usize)
                .collect();
            Ok(QueryResult {
                records: paged,
                total,
                limit: pagination.limit,
                offset: pagination.offset,
            })
        }

        async fn delete(
            &self,
            plugin_id: &str,
            collection: &str,
            id: &str,
        ) -> anyhow::Result<bool> {
            let key = Self::make_key(plugin_id, collection, id);
            Ok(self.records.lock().unwrap().remove(&key).is_some())
        }

        async fn list(
            &self,
            plugin_id: &str,
            collection: &str,
            _sort: Option<SortOptions>,
            pagination: Pagination,
        ) -> anyhow::Result<QueryResult> {
            self.query(
                plugin_id,
                collection,
                QueryFilters::default(),
                None,
                pagination,
            )
            .await
        }
    }

    #[tokio::test]
    async fn create_and_get() {
        let storage = MockStorage::new();
        let data = serde_json::json!({"title": "Test"});
        let created = storage.create("plug1", "tasks", data.clone()).await.unwrap();
        assert_eq!(created.version, 1);
        assert_eq!(created.plugin_id, "plug1");
        assert_eq!(created.collection, "tasks");

        let fetched = storage
            .get("plug1", "tasks", &created.id)
            .await
            .unwrap()
            .expect("should find record");
        assert_eq!(fetched.data, data);
    }

    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let storage = MockStorage::new();
        let result = storage.get("plug1", "tasks", "nope").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn update_increments_version() {
        let storage = MockStorage::new();
        let created = storage
            .create("plug1", "tasks", serde_json::json!({"v": 1}))
            .await
            .unwrap();
        let updated = storage
            .update(
                "plug1",
                "tasks",
                &created.id,
                serde_json::json!({"v": 2}),
                1,
            )
            .await
            .unwrap();
        assert_eq!(updated.version, 2);
        assert_eq!(updated.data, serde_json::json!({"v": 2}));
    }

    #[tokio::test]
    async fn update_with_wrong_version_fails() {
        let storage = MockStorage::new();
        let created = storage
            .create("plug1", "tasks", serde_json::json!({"v": 1}))
            .await
            .unwrap();
        let result = storage
            .update(
                "plug1",
                "tasks",
                &created.id,
                serde_json::json!({"v": 2}),
                999,
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn delete_existing_record() {
        let storage = MockStorage::new();
        let created = storage
            .create("plug1", "tasks", serde_json::json!({}))
            .await
            .unwrap();
        let deleted = storage
            .delete("plug1", "tasks", &created.id)
            .await
            .unwrap();
        assert!(deleted);

        let after = storage.get("plug1", "tasks", &created.id).await.unwrap();
        assert!(after.is_none());
    }

    #[tokio::test]
    async fn delete_nonexistent_returns_false() {
        let storage = MockStorage::new();
        let deleted = storage.delete("plug1", "tasks", "nope").await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn list_returns_paginated() {
        let storage = MockStorage::new();
        for i in 0..5 {
            storage
                .create("plug1", "tasks", serde_json::json!({"i": i}))
                .await
                .unwrap();
        }
        let result = storage
            .list("plug1", "tasks", None, Pagination { limit: 2, offset: 0 })
            .await
            .unwrap();
        assert_eq!(result.total, 5);
        assert_eq!(result.records.len(), 2);
        assert_eq!(result.limit, 2);
    }

    #[tokio::test]
    async fn query_scoped_to_plugin_and_collection() {
        let storage = MockStorage::new();
        storage
            .create("plug1", "tasks", serde_json::json!({}))
            .await
            .unwrap();
        storage
            .create("plug2", "tasks", serde_json::json!({}))
            .await
            .unwrap();

        let result = storage
            .query(
                "plug1",
                "tasks",
                QueryFilters::default(),
                None,
                Pagination::default(),
            )
            .await
            .unwrap();
        assert_eq!(result.total, 1);
    }

    #[test]
    fn pagination_clamp() {
        let p = Pagination {
            limit: 5000,
            offset: 0,
        };
        let clamped = p.clamped();
        assert_eq!(clamped.limit, 1000);
    }

    #[test]
    fn pagination_default() {
        let p = Pagination::default();
        assert_eq!(p.limit, 50);
        assert_eq!(p.offset, 0);
    }

    #[test]
    fn record_serialization_roundtrip() {
        let now = Utc::now();
        let record = Record {
            id: "r1".into(),
            plugin_id: "plug1".into(),
            collection: "tasks".into(),
            data: serde_json::json!({"title": "Hello"}),
            version: 3,
            user_id: None,
            household_id: None,
            created_at: now,
            updated_at: now,
        };
        let json = serde_json::to_string(&record).unwrap();
        let restored: Record = serde_json::from_str(&json).unwrap();
        assert_eq!(record, restored);
    }

    #[test]
    fn sort_direction_values() {
        assert_ne!(SortDirection::Asc, SortDirection::Desc);
    }

    #[test]
    fn comparison_op_values() {
        assert_ne!(ComparisonOp::Gte, ComparisonOp::Lte);
        assert_ne!(ComparisonOp::Gt, ComparisonOp::Lt);
    }
}
