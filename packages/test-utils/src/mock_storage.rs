//! In-memory mock implementation of `DocumentStorageAdapter` for testing.

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use life_engine_traits::storage::{
    AdapterCapabilities, ChangeEvent, ChangeType, CollectionDescriptor, DocumentList,
    DocumentStorageAdapter, FilterNode, FilterOperator, HealthCheck, HealthReport, HealthStatus,
    Pagination, QueryDescriptor, SortDirection, StorageError,
};

/// In-memory document storage for testing.
///
/// Uses `HashMap<String, HashMap<String, Value>>` mapping
/// `collection → id → document`. All operations are guarded by a
/// `tokio::sync::RwLock` so the mock is `Send + Sync`.
pub struct MockDocumentStorageAdapter {
    /// collection name → (document id → document value)
    data: RwLock<HashMap<String, HashMap<String, Value>>>,
    /// Broadcast sender for change events per collection.
    watchers: RwLock<HashMap<String, Vec<mpsc::Sender<ChangeEvent>>>>,
}

impl MockDocumentStorageAdapter {
    /// Create a new empty mock store.
    pub fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
            watchers: RwLock::new(HashMap::new()),
        }
    }

    /// Emit a change event to all watchers of the given collection.
    async fn emit(&self, collection: &str, document_id: &str, change_type: ChangeType) {
        let event = ChangeEvent {
            change_type,
            collection: collection.to_string(),
            document_id: document_id.to_string(),
            timestamp: Utc::now(),
        };
        let mut watchers = self.watchers.write().await;
        if let Some(senders) = watchers.get_mut(collection) {
            senders.retain(|tx| tx.try_send(event.clone()).is_ok());
        }
    }
}

impl Default for MockDocumentStorageAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract the `"id"` field from a document `Value`, returning `None` if
/// the document is not an object or has no string `"id"` field.
fn extract_id(doc: &Value) -> Option<String> {
    doc.as_object()
        .and_then(|o| o.get("id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Evaluate a `FilterNode` tree against a single document.
fn matches_filter(doc: &Value, filter: &FilterNode) -> bool {
    match filter {
        FilterNode::And(children) => children.iter().all(|c| matches_filter(doc, c)),
        FilterNode::Or(children) => children.iter().any(|c| matches_filter(doc, c)),
        FilterNode::Not(child) => !matches_filter(doc, child),
        FilterNode::Comparison { field, operator: op, value } => {
            let field_val = resolve_field(doc, field);
            match op {
                FilterOperator::Eq => field_val.as_ref() == Some(value),
                FilterOperator::Ne => field_val.as_ref() != Some(value),
                FilterOperator::Gt => cmp_json(field_val.as_ref(), value) == Some(std::cmp::Ordering::Greater),
                FilterOperator::Gte => matches!(cmp_json(field_val.as_ref(), value), Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)),
                FilterOperator::Lt => cmp_json(field_val.as_ref(), value) == Some(std::cmp::Ordering::Less),
                FilterOperator::Lte => matches!(cmp_json(field_val.as_ref(), value), Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)),
                FilterOperator::In => {
                    if let Some(arr) = value.as_array() {
                        field_val.as_ref().is_some_and(|fv| arr.contains(fv))
                    } else {
                        false
                    }
                }
                FilterOperator::NotIn => {
                    if let Some(arr) = value.as_array() {
                        field_val.as_ref().is_some_and(|fv| !arr.contains(fv))
                    } else {
                        true
                    }
                }
                FilterOperator::Contains => {
                    match &field_val {
                        Some(Value::String(s)) => {
                            value.as_str().is_some_and(|needle| s.contains(needle))
                        }
                        Some(Value::Array(arr)) => arr.contains(value),
                        _ => false,
                    }
                }
                FilterOperator::StartsWith => {
                    match &field_val {
                        Some(Value::String(s)) => {
                            value.as_str().is_some_and(|prefix| s.starts_with(prefix))
                        }
                        _ => false,
                    }
                }
                FilterOperator::Exists => field_val.is_some(),
            }
        }
    }
}

/// Resolve a dot-separated field path against a JSON value.
fn resolve_field(doc: &Value, path: &str) -> Option<Value> {
    let mut current = doc.clone();
    for segment in path.split('.') {
        current = current.as_object()?.get(segment)?.clone();
    }
    Some(current)
}

/// Compare two JSON values, supporting numbers and strings.
fn cmp_json(a: Option<&Value>, b: &Value) -> Option<std::cmp::Ordering> {
    let a = a?;
    match (a, b) {
        (Value::Number(an), Value::Number(bn)) => {
            let af = an.as_f64()?;
            let bf = bn.as_f64()?;
            af.partial_cmp(&bf)
        }
        (Value::String(a_s), Value::String(b_s)) => Some(a_s.cmp(b_s)),
        _ => None,
    }
}

#[async_trait]
impl DocumentStorageAdapter for MockDocumentStorageAdapter {
    async fn get(&self, collection: &str, id: &str) -> Result<Value, StorageError> {
        let data = self.data.read().await;
        data.get(collection)
            .and_then(|coll| coll.get(id))
            .cloned()
            .ok_or_else(|| StorageError::NotFound {
                collection: collection.to_string(),
                id: id.to_string(),
            })
    }

    async fn create(&self, collection: &str, document: Value) -> Result<Value, StorageError> {
        let mut data = self.data.write().await;
        let coll = data.entry(collection.to_string()).or_default();

        let mut doc = document;

        // Ensure the document has an id field.
        let id = if let Some(existing_id) = extract_id(&doc) {
            if coll.contains_key(&existing_id) {
                return Err(StorageError::AlreadyExists {
                    collection: collection.to_string(),
                    id: existing_id,
                });
            }
            existing_id
        } else {
            let generated = Uuid::new_v4().to_string();
            if let Some(obj) = doc.as_object_mut() {
                obj.insert("id".to_string(), Value::String(generated.clone()));
            }
            generated
        };

        coll.insert(id.clone(), doc.clone());
        drop(data);
        self.emit(collection, &id, ChangeType::Created).await;
        Ok(doc)
    }

    async fn update(
        &self,
        collection: &str,
        id: &str,
        document: Value,
    ) -> Result<Value, StorageError> {
        let mut data = self.data.write().await;
        let coll = data.get_mut(collection).ok_or_else(|| StorageError::NotFound {
            collection: collection.to_string(),
            id: id.to_string(),
        })?;

        if !coll.contains_key(id) {
            return Err(StorageError::NotFound {
                collection: collection.to_string(),
                id: id.to_string(),
            });
        }

        let mut doc = document;
        if let Some(obj) = doc.as_object_mut() {
            obj.insert("id".to_string(), Value::String(id.to_string()));
        }

        coll.insert(id.to_string(), doc.clone());
        drop(data);
        self.emit(collection, id, ChangeType::Updated).await;
        Ok(doc)
    }

    async fn partial_update(
        &self,
        collection: &str,
        id: &str,
        fields: Value,
    ) -> Result<Value, StorageError> {
        let mut data = self.data.write().await;
        let coll = data.get_mut(collection).ok_or_else(|| StorageError::NotFound {
            collection: collection.to_string(),
            id: id.to_string(),
        })?;

        let existing = coll.get_mut(id).ok_or_else(|| StorageError::NotFound {
            collection: collection.to_string(),
            id: id.to_string(),
        })?;

        // Merge fields into the existing document.
        if let (Some(existing_obj), Some(patch_obj)) =
            (existing.as_object_mut(), fields.as_object())
        {
            for (k, v) in patch_obj {
                existing_obj.insert(k.clone(), v.clone());
            }
        }

        let result = existing.clone();
        drop(data);
        self.emit(collection, id, ChangeType::Updated).await;
        Ok(result)
    }

    async fn delete(&self, collection: &str, id: &str) -> Result<(), StorageError> {
        let mut data = self.data.write().await;
        let coll = data.get_mut(collection).ok_or_else(|| StorageError::NotFound {
            collection: collection.to_string(),
            id: id.to_string(),
        })?;

        if coll.remove(id).is_none() {
            return Err(StorageError::NotFound {
                collection: collection.to_string(),
                id: id.to_string(),
            });
        }

        drop(data);
        self.emit(collection, id, ChangeType::Deleted).await;
        Ok(())
    }

    async fn query(&self, descriptor: QueryDescriptor) -> Result<DocumentList, StorageError> {
        let data = self.data.read().await;
        let coll = data.get(&descriptor.collection);

        let mut docs: Vec<Value> = match coll {
            Some(c) => c.values().cloned().collect(),
            None => vec![],
        };

        // Apply filters.
        if let Some(ref filter) = descriptor.filter {
            docs.retain(|doc| matches_filter(doc, filter));
        }

        let total_count = docs.len() as u64;

        // Apply sorting.
        if !descriptor.sort.is_empty() {
            docs.sort_by(|a, b| {
                for sf in &descriptor.sort {
                    let av = resolve_field(a, &sf.field);
                    let bv = resolve_field(b, &sf.field);
                    let ord = cmp_json(av.as_ref(), bv.as_ref().unwrap_or(&Value::Null));
                    if let Some(ord) = ord {
                        let ord = match sf.direction {
                            SortDirection::Asc => ord,
                            SortDirection::Desc => ord.reverse(),
                        };
                        if ord != std::cmp::Ordering::Equal {
                            return ord;
                        }
                    }
                }
                std::cmp::Ordering::Equal
            });
        }

        // Apply pagination (cursor-based).
        let Pagination { limit, cursor } = &descriptor.pagination;
        if let Some(cursor_val) = cursor {
            // Simple cursor: skip documents until we find the cursor id.
            if let Some(pos) = docs.iter().position(|d| {
                d.get("id")
                    .or_else(|| d.get("_id_"))
                    .and_then(|v| v.as_str())
                    .map_or(false, |id| id == cursor_val.as_str())
            }) {
                docs = docs[(pos + 1)..].to_vec();
            }
        }
        docs.truncate(*limit as usize);

        // Apply field projection.
        if let Some(ref fields) = descriptor.fields {
            docs = docs
                .into_iter()
                .map(|doc| {
                    let mut projected = serde_json::Map::new();
                    if let Some(obj) = doc.as_object() {
                        for field in fields {
                            if let Some(val) = obj.get(field.as_str()) {
                                projected.insert(field.clone(), val.clone());
                            }
                        }
                    }
                    Value::Object(projected)
                })
                .collect();
        }

        Ok(DocumentList {
            documents: docs,
            total_count,
            next_cursor: None,
        })
    }

    async fn count(
        &self,
        collection: &str,
        filters: Option<FilterNode>,
    ) -> Result<u64, StorageError> {
        let data = self.data.read().await;
        let coll = match data.get(collection) {
            Some(c) => c,
            None => return Ok(0),
        };

        match filters {
            Some(filter) => {
                let count = coll.values().filter(|doc| matches_filter(doc, &filter)).count();
                Ok(count as u64)
            }
            None => Ok(coll.len() as u64),
        }
    }

    async fn batch_create(
        &self,
        collection: &str,
        documents: Vec<Value>,
    ) -> Result<Vec<Value>, StorageError> {
        let mut data = self.data.write().await;
        let coll = data.entry(collection.to_string()).or_default();

        // Pre-check: ensure no conflicts exist before inserting anything (atomic).
        let mut prepared: Vec<(String, Value)> = Vec::with_capacity(documents.len());
        for doc in documents {
            let mut doc = doc;
            let id = if let Some(existing_id) = extract_id(&doc) {
                if coll.contains_key(&existing_id) {
                    return Err(StorageError::AlreadyExists {
                        collection: collection.to_string(),
                        id: existing_id,
                    });
                }
                existing_id
            } else {
                let generated = Uuid::new_v4().to_string();
                if let Some(obj) = doc.as_object_mut() {
                    obj.insert("id".to_string(), Value::String(generated.clone()));
                }
                generated
            };
            prepared.push((id, doc));
        }

        // Also check for duplicate IDs within the batch itself.
        let mut seen = std::collections::HashSet::new();
        for (id, _) in &prepared {
            if !seen.insert(id.clone()) {
                return Err(StorageError::AlreadyExists {
                    collection: collection.to_string(),
                    id: id.clone(),
                });
            }
        }

        let mut results = Vec::with_capacity(prepared.len());
        for (id, doc) in prepared {
            coll.insert(id, doc.clone());
            results.push(doc);
        }

        Ok(results)
    }

    async fn batch_update(
        &self,
        collection: &str,
        updates: Vec<(String, Value)>,
    ) -> Result<Vec<Value>, StorageError> {
        let mut data = self.data.write().await;
        let coll = data.get_mut(collection).ok_or_else(|| StorageError::NotFound {
            collection: collection.to_string(),
            id: String::new(),
        })?;

        // Pre-check: all IDs must exist.
        for (id, _) in &updates {
            if !coll.contains_key(id) {
                return Err(StorageError::NotFound {
                    collection: collection.to_string(),
                    id: id.clone(),
                });
            }
        }

        let mut results = Vec::with_capacity(updates.len());
        for (id, mut doc) in updates {
            if let Some(obj) = doc.as_object_mut() {
                obj.insert("id".to_string(), Value::String(id.clone()));
            }
            coll.insert(id, doc.clone());
            results.push(doc);
        }

        Ok(results)
    }

    async fn batch_delete(
        &self,
        collection: &str,
        ids: Vec<String>,
    ) -> Result<(), StorageError> {
        let mut data = self.data.write().await;
        let coll = data.get_mut(collection).ok_or_else(|| StorageError::NotFound {
            collection: collection.to_string(),
            id: String::new(),
        })?;

        // Pre-check: all IDs must exist.
        for id in &ids {
            if !coll.contains_key(id) {
                return Err(StorageError::NotFound {
                    collection: collection.to_string(),
                    id: id.clone(),
                });
            }
        }

        for id in &ids {
            coll.remove(id);
        }

        Ok(())
    }

    async fn watch(
        &self,
        collection: &str,
    ) -> Result<mpsc::Receiver<ChangeEvent>, StorageError> {
        let (tx, rx) = mpsc::channel(64);
        let mut watchers = self.watchers.write().await;
        watchers
            .entry(collection.to_string())
            .or_default()
            .push(tx);
        Ok(rx)
    }

    async fn migrate(&self, _descriptor: CollectionDescriptor) -> Result<(), StorageError> {
        // No-op for in-memory mock — nothing to migrate.
        Ok(())
    }

    async fn health(&self) -> Result<HealthReport, StorageError> {
        Ok(HealthReport {
            status: HealthStatus::Healthy,
            message: Some("mock adapter".to_string()),
            checks: vec![HealthCheck {
                name: "memory".to_string(),
                status: HealthStatus::Healthy,
                message: None,
            }],
        })
    }

    fn capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities {
            indexing: true,
            transactions: true,
            full_text_search: true,
            watch: true,
            batch_operations: true,
            encryption: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use life_engine_traits::storage::SortField;

    #[tokio::test]
    async fn mock_document_crud() {
        let store = MockDocumentStorageAdapter::new();

        // Create
        let doc = serde_json::json!({"id": "1", "name": "Alice", "age": 30});
        let created = store.create("users", doc.clone()).await.unwrap();
        assert_eq!(created["name"], "Alice");

        // Get
        let fetched = store.get("users", "1").await.unwrap();
        assert_eq!(fetched["name"], "Alice");
        assert_eq!(fetched["age"], 30);

        // Update
        let updated_doc = serde_json::json!({"name": "Alice Updated", "age": 31});
        let updated = store.update("users", "1", updated_doc).await.unwrap();
        assert_eq!(updated["name"], "Alice Updated");
        assert_eq!(updated["id"], "1");

        // Verify update persisted
        let fetched = store.get("users", "1").await.unwrap();
        assert_eq!(fetched["name"], "Alice Updated");

        // Delete
        store.delete("users", "1").await.unwrap();
        let result = store.get("users", "1").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), StorageError::NotFound { .. }));
    }

    #[tokio::test]
    async fn mock_document_partial_update() {
        let store = MockDocumentStorageAdapter::new();
        let doc = serde_json::json!({"id": "1", "name": "Alice", "age": 30, "city": "Sydney"});
        store.create("users", doc).await.unwrap();

        // Partial update: change age only
        let patch = serde_json::json!({"age": 31});
        let updated = store.partial_update("users", "1", patch).await.unwrap();
        assert_eq!(updated["age"], 31);
        assert_eq!(updated["name"], "Alice");
        assert_eq!(updated["city"], "Sydney");
    }

    #[tokio::test]
    async fn mock_document_create_conflict() {
        let store = MockDocumentStorageAdapter::new();
        let doc = serde_json::json!({"id": "1", "name": "Alice"});
        store.create("users", doc.clone()).await.unwrap();

        let result = store.create("users", doc).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StorageError::AlreadyExists { .. }
        ));
    }

    #[tokio::test]
    async fn mock_document_create_generates_id() {
        let store = MockDocumentStorageAdapter::new();
        let doc = serde_json::json!({"name": "Bob"});
        let created = store.create("users", doc).await.unwrap();
        assert!(created.get("id").is_some());
        let id = created["id"].as_str().unwrap();
        assert!(!id.is_empty());

        // Verify can fetch by generated id
        let fetched = store.get("users", id).await.unwrap();
        assert_eq!(fetched["name"], "Bob");
    }

    #[tokio::test]
    async fn mock_document_not_found() {
        let store = MockDocumentStorageAdapter::new();
        let result = store.get("users", "nonexistent").await;
        assert!(matches!(result.unwrap_err(), StorageError::NotFound { .. }));

        let result = store.update("users", "nonexistent", serde_json::json!({})).await;
        assert!(matches!(result.unwrap_err(), StorageError::NotFound { .. }));

        let result = store.delete("users", "nonexistent").await;
        assert!(matches!(result.unwrap_err(), StorageError::NotFound { .. }));
    }

    #[tokio::test]
    async fn mock_document_query_with_filter() {
        let store = MockDocumentStorageAdapter::new();

        store
            .create("users", serde_json::json!({"id": "1", "name": "Alice", "age": 30}))
            .await
            .unwrap();
        store
            .create("users", serde_json::json!({"id": "2", "name": "Bob", "age": 25}))
            .await
            .unwrap();
        store
            .create("users", serde_json::json!({"id": "3", "name": "Charlie", "age": 35}))
            .await
            .unwrap();

        // Filter: age >= 30
        let result = store
            .query(QueryDescriptor {
                collection: "users".to_string(),
                filter: Some(FilterNode::Comparison {
                    field: "age".to_string(),
                    operator: FilterOperator::Gte,
                    value: serde_json::json!(30),
                }),
                sort: vec![],
                pagination: Pagination::default(),
                fields: None,
                text_search: None,
            })
            .await
            .unwrap();

        assert_eq!(result.total_count, 2);
        assert_eq!(result.documents.len(), 2);

        let names: Vec<&str> = result
            .documents
            .iter()
            .filter_map(|d| d["name"].as_str())
            .collect();
        assert!(names.contains(&"Alice"));
        assert!(names.contains(&"Charlie"));
    }

    #[tokio::test]
    async fn mock_document_query_with_sort_and_pagination() {
        let store = MockDocumentStorageAdapter::new();

        for i in 1..=5 {
            store
                .create(
                    "items",
                    serde_json::json!({"id": i.to_string(), "name": format!("item-{i}"), "order": i}),
                )
                .await
                .unwrap();
        }

        let result = store
            .query(QueryDescriptor {
                collection: "items".to_string(),
                filter: None,
                sort: vec![SortField {
                    field: "order".to_string(),
                    direction: SortDirection::Desc,
                }],
                pagination: Pagination {
                    limit: 3,
                    cursor: None,
                },
                fields: None,
                text_search: None,
            })
            .await
            .unwrap();

        assert_eq!(result.total_count, 5);
        assert_eq!(result.documents.len(), 3);
        // Sorted desc: [5, 4, 3, 2, 1], limit 3 → [5, 4, 3]
        assert_eq!(result.documents[0]["order"], 5);
        assert_eq!(result.documents[1]["order"], 4);
        assert_eq!(result.documents[2]["order"], 3);
    }

    #[tokio::test]
    async fn mock_document_count() {
        let store = MockDocumentStorageAdapter::new();

        store.create("c", serde_json::json!({"id": "1", "x": 1})).await.unwrap();
        store.create("c", serde_json::json!({"id": "2", "x": 2})).await.unwrap();
        store.create("c", serde_json::json!({"id": "3", "x": 3})).await.unwrap();

        assert_eq!(store.count("c", None).await.unwrap(), 3);

        let filtered = store
            .count(
                "c",
                Some(FilterNode::Comparison {
                    field: "x".to_string(),
                    operator: FilterOperator::Gt,
                    value: serde_json::json!(1),
                }),
            )
            .await
            .unwrap();
        assert_eq!(filtered, 2);
    }

    #[tokio::test]
    async fn mock_document_batch_create_and_delete() {
        let store = MockDocumentStorageAdapter::new();

        let docs = vec![
            serde_json::json!({"id": "a", "val": 1}),
            serde_json::json!({"id": "b", "val": 2}),
            serde_json::json!({"id": "c", "val": 3}),
        ];

        let created = store.batch_create("items", docs).await.unwrap();
        assert_eq!(created.len(), 3);

        // Verify all exist
        assert!(store.get("items", "a").await.is_ok());
        assert!(store.get("items", "b").await.is_ok());
        assert!(store.get("items", "c").await.is_ok());

        // Batch delete
        store
            .batch_delete("items", vec!["a".to_string(), "b".to_string(), "c".to_string()])
            .await
            .unwrap();

        // Verify all gone
        assert!(store.get("items", "a").await.is_err());
        assert!(store.get("items", "b").await.is_err());
        assert!(store.get("items", "c").await.is_err());
    }

    #[tokio::test]
    async fn mock_document_batch_update() {
        let store = MockDocumentStorageAdapter::new();

        store.create("c", serde_json::json!({"id": "1", "v": "old1"})).await.unwrap();
        store.create("c", serde_json::json!({"id": "2", "v": "old2"})).await.unwrap();

        let updates = vec![
            ("1".to_string(), serde_json::json!({"v": "new1"})),
            ("2".to_string(), serde_json::json!({"v": "new2"})),
        ];
        let results = store.batch_update("c", updates).await.unwrap();
        assert_eq!(results.len(), 2);

        assert_eq!(store.get("c", "1").await.unwrap()["v"], "new1");
        assert_eq!(store.get("c", "2").await.unwrap()["v"], "new2");
    }

    #[tokio::test]
    async fn mock_document_batch_create_atomic_on_conflict() {
        let store = MockDocumentStorageAdapter::new();
        store.create("c", serde_json::json!({"id": "existing"})).await.unwrap();

        // Batch where the second doc conflicts — nothing should be inserted.
        let result = store
            .batch_create(
                "c",
                vec![
                    serde_json::json!({"id": "new1"}),
                    serde_json::json!({"id": "existing"}),
                ],
            )
            .await;

        assert!(result.is_err());
        // The first doc should not have been inserted (atomic failure).
        assert!(store.get("c", "new1").await.is_err());
    }

    #[tokio::test]
    async fn mock_document_health() {
        let store = MockDocumentStorageAdapter::new();
        let report = store.health().await.unwrap();
        assert_eq!(report.status, HealthStatus::Healthy);
        assert!(!report.checks.is_empty());
    }

    #[tokio::test]
    async fn mock_document_capabilities() {
        let store = MockDocumentStorageAdapter::new();
        let caps = store.capabilities();
        assert!(caps.indexing);
        assert!(caps.transactions);
        assert!(caps.full_text_search);
        assert!(caps.watch);
        assert!(caps.batch_operations);
        assert!(caps.encryption);
    }

    #[tokio::test]
    async fn mock_document_migrate_is_noop() {
        let store = MockDocumentStorageAdapter::new();
        let result = store
            .migrate(CollectionDescriptor {
                name: "test".to_string(),
                plugin_id: "test-plugin".to_string(),
                fields: vec![],
                indexes: vec![],
            })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn mock_document_watch_emits_events() {
        let store = MockDocumentStorageAdapter::new();
        let mut rx = store.watch("users").await.unwrap();

        store
            .create("users", serde_json::json!({"id": "1", "name": "Alice"}))
            .await
            .unwrap();

        let event = rx.try_recv().unwrap();
        assert_eq!(event.change_type, ChangeType::Created);
        assert_eq!(event.collection, "users");
        assert_eq!(event.document_id, "1");
    }
}
