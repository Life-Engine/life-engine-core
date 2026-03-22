//! Shared sync primitives used by both federation (hub-to-hub) and
//! Core-to-App sync paths.
//!
//! Extracts the common change-record types, cursor tracking, and
//! last-write-wins conflict resolution so they are DRY across sync
//! implementations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Change record types ─────────────────────────────────────────────

/// A single change record describing a mutation to a stored record.
///
/// Used by both federation pull responses and Core-to-App sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeRecord {
    /// Record ID.
    pub id: String,
    /// Collection name.
    pub collection: String,
    /// The operation that produced this change.
    pub operation: ChangeOperation,
    /// Record data (None for deletes).
    pub data: Option<serde_json::Value>,
    /// Record version (optimistic concurrency).
    pub version: i64,
    /// When the change occurred.
    pub timestamp: DateTime<Utc>,
}

/// Types of change operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeOperation {
    Create,
    Update,
    Delete,
}

/// A batch of changes pulled from a remote source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullResponse {
    /// The changes since the given cursor.
    pub changes: Vec<ChangeRecord>,
    /// Cursor for the next pull (opaque string, typically a timestamp).
    pub cursor: String,
}

// ── Cursor tracking ─────────────────────────────────────────────────

/// Tracks the last-sync cursor per source per collection.
///
/// Used by federation to track peer cursors and available for any
/// sync implementation that needs cursor-based change tracking.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncCursors {
    /// Map of source_id -> collection -> cursor.
    pub cursors: HashMap<String, HashMap<String, String>>,
}

impl SyncCursors {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the cursor for a specific source and collection.
    pub fn get(&self, source_id: &str, collection: &str) -> Option<&str> {
        self.cursors
            .get(source_id)
            .and_then(|cols| cols.get(collection))
            .map(|s| s.as_str())
    }

    /// Set the cursor for a specific source and collection.
    pub fn set(&mut self, source_id: &str, collection: &str, cursor: String) {
        self.cursors
            .entry(source_id.to_string())
            .or_default()
            .insert(collection.to_string(), cursor);
    }
}

// ── Last-write-wins change application ──────────────────────────────

/// Apply a single change record to local storage using last-write-wins
/// conflict resolution.
///
/// - **Create**: insert the record if data is present.
/// - **Update**: if the remote version is higher than local, overwrite.
///   If local is newer, skip (last-write-wins). If no local record
///   exists, create it.
/// - **Delete**: remove the record.
pub async fn apply_change(
    storage: &dyn crate::storage::StorageAdapter,
    namespace: &str,
    change: &ChangeRecord,
) -> anyhow::Result<()> {
    match change.operation {
        ChangeOperation::Create => {
            if let Some(ref data) = change.data {
                storage
                    .create_with_id(namespace, &change.collection, &change.id, data.clone())
                    .await?;
            }
        }
        ChangeOperation::Update => {
            if let Some(ref data) = change.data {
                match storage.get(namespace, &change.collection, &change.id).await? {
                    Some(existing) => {
                        if change.version > existing.version {
                            storage
                                .update(
                                    namespace,
                                    &change.collection,
                                    &change.id,
                                    data.clone(),
                                    existing.version,
                                )
                                .await?;
                        }
                        // else: local version is newer, skip (last-write-wins).
                    }
                    None => {
                        // Record doesn't exist locally, create it preserving
                        // the federated ID.
                        storage
                            .create_with_id(namespace, &change.collection, &change.id, data.clone())
                            .await?;
                    }
                }
            }
        }
        ChangeOperation::Delete => {
            // Check version before deleting to enforce last-write-wins (LWW).
            if let Some(existing) = storage.get(namespace, &change.collection, &change.id).await? {
                if change.version <= existing.version {
                    // Local version is newer or equal; skip the stale delete.
                    return Ok(());
                }
            }
            storage
                .delete(namespace, &change.collection, &change.id)
                .await?;
        }
    }

    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{
        Pagination, QueryFilters, QueryResult, Record, SortOptions, StorageAdapter,
    };
    use async_trait::async_trait;
    use serde_json::json;
    use std::sync::Mutex;

    /// In-memory storage for sync primitive tests.
    struct TestStorage {
        records: Mutex<HashMap<String, Record>>,
    }

    impl TestStorage {
        fn new() -> Self {
            Self {
                records: Mutex::new(HashMap::new()),
            }
        }

        fn make_key(plugin_id: &str, collection: &str, id: &str) -> String {
            format!("{plugin_id}:{collection}:{id}")
        }

        fn records_in_collection(&self, plugin_id: &str, collection: &str) -> Vec<Record> {
            let records = self.records.lock().unwrap();
            records
                .values()
                .filter(|r| r.plugin_id == plugin_id && r.collection == collection)
                .cloned()
                .collect()
        }
    }

    #[async_trait]
    impl StorageAdapter for TestStorage {
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
            data: serde_json::Value,
        ) -> anyhow::Result<Record> {
            let id = uuid::Uuid::new_v4().to_string();
            self.create_with_id(plugin_id, collection, &id, data).await
        }

        async fn create_with_id(
            &self,
            plugin_id: &str,
            collection: &str,
            id: &str,
            data: serde_json::Value,
        ) -> anyhow::Result<Record> {
            let now = Utc::now();
            let record = Record {
                id: id.to_string(),
                plugin_id: plugin_id.into(),
                collection: collection.into(),
                data,
                version: 1,
                user_id: None,
                household_id: None,
                created_at: now,
                updated_at: now,
            };
            let key = Self::make_key(plugin_id, collection, id);
            self.records.lock().unwrap().insert(key, record.clone());
            Ok(record)
        }

        async fn update(
            &self,
            plugin_id: &str,
            collection: &str,
            id: &str,
            data: serde_json::Value,
            version: i64,
        ) -> Result<Record, crate::storage::StorageError> {
            use crate::storage::StorageError;
            let key = Self::make_key(plugin_id, collection, id);
            let mut records = self.records.lock().unwrap();
            let record = records
                .get(&key)
                .ok_or(StorageError::NotFound)?;
            if record.version != version {
                return Err(StorageError::VersionMismatch);
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

    // ── apply_change tests ──────────────────────────────────────────

    #[tokio::test]
    async fn apply_create_inserts_record() {
        let storage = TestStorage::new();
        let change = ChangeRecord {
            id: "rec-1".into(),
            collection: "events".into(),
            operation: ChangeOperation::Create,
            data: Some(json!({"title": "Birthday"})),
            version: 1,
            timestamp: Utc::now(),
        };

        apply_change(&storage, "sync:peer-a", &change).await.unwrap();

        let records = storage.records_in_collection("sync:peer-a", "events");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].data, json!({"title": "Birthday"}));
        // Federated ID must be preserved (F-062).
        assert_eq!(records[0].id, "rec-1");
    }

    #[tokio::test]
    async fn apply_update_overwrites_lower_version() {
        let storage = TestStorage::new();
        let created = storage
            .create("sync:peer-a", "events", json!({"title": "Old"}))
            .await
            .unwrap();

        let change = ChangeRecord {
            id: created.id.clone(),
            collection: "events".into(),
            operation: ChangeOperation::Update,
            data: Some(json!({"title": "New"})),
            version: 2,
            timestamp: Utc::now(),
        };

        apply_change(&storage, "sync:peer-a", &change).await.unwrap();

        let record = storage
            .get("sync:peer-a", "events", &created.id)
            .await
            .unwrap()
            .expect("should exist");
        assert_eq!(record.data, json!({"title": "New"}));
    }

    #[tokio::test]
    async fn apply_delete_removes_record() {
        let storage = TestStorage::new();
        let created = storage
            .create("sync:peer-a", "events", json!({"title": "Delete me"}))
            .await
            .unwrap();

        let change = ChangeRecord {
            id: created.id.clone(),
            collection: "events".into(),
            operation: ChangeOperation::Delete,
            data: None,
            version: 2,
            timestamp: Utc::now(),
        };

        apply_change(&storage, "sync:peer-a", &change).await.unwrap();

        let record = storage
            .get("sync:peer-a", "events", &created.id)
            .await
            .unwrap();
        assert!(record.is_none());
    }

    // ── SyncCursors tests ───────────────────────────────────────────

    #[test]
    fn cursors_get_set() {
        let mut cursors = SyncCursors::new();
        assert!(cursors.get("src-1", "events").is_none());

        cursors.set("src-1", "events", "2026-03-22T00:00:00Z".into());
        assert_eq!(
            cursors.get("src-1", "events"),
            Some("2026-03-22T00:00:00Z")
        );
    }

    #[test]
    fn cursors_per_collection() {
        let mut cursors = SyncCursors::new();
        cursors.set("src-1", "events", "cursor-a".into());
        cursors.set("src-1", "contacts", "cursor-b".into());

        assert_eq!(cursors.get("src-1", "events"), Some("cursor-a"));
        assert_eq!(cursors.get("src-1", "contacts"), Some("cursor-b"));
    }

    // ── Serialization tests ─────────────────────────────────────────

    #[test]
    fn change_operation_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&ChangeOperation::Create).unwrap(),
            "\"create\""
        );
        assert_eq!(
            serde_json::to_string(&ChangeOperation::Update).unwrap(),
            "\"update\""
        );
        assert_eq!(
            serde_json::to_string(&ChangeOperation::Delete).unwrap(),
            "\"delete\""
        );
    }
}
