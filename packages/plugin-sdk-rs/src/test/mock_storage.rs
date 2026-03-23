//! Mock storage context for plugin testing.
//!
//! Provides an in-memory implementation of the StorageContext API so
//! plugin authors can test their plugins without a real database.

use life_engine_types::{
    FilterOp, PipelineMessage, QueryFilter, SortDirection, SortField,
};
use std::collections::HashMap;
use uuid::Uuid;

/// In-memory mock storage that mirrors the [`StorageContext`](crate::StorageContext) API.
///
/// Data is stored in a `HashMap<String, Vec<PipelineMessage>>` keyed by collection name.
/// Supports the same fluent query, insert, update, and delete operations as the real
/// `StorageContext`, allowing plugin authors to unit-test without a running database.
///
/// # Example
///
/// ```rust,ignore
/// use life_engine_plugin_sdk::test::{MockStorageContext, MockMessageBuilder};
///
/// let mut store = MockStorageContext::new("my-plugin");
/// let msg = MockMessageBuilder::note("Test Note", "Body text").build();
/// store.insert("notes", msg);
/// store.assert_inserted("notes", 1);
/// ```
pub struct MockStorageContext {
    plugin_id: String,
    data: HashMap<String, Vec<PipelineMessage>>,
}

impl MockStorageContext {
    /// Create a new empty mock storage context for the given plugin.
    pub fn new(plugin_id: impl Into<String>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            data: HashMap::new(),
        }
    }

    /// Start building a read query against the given collection.
    pub fn query(&self, collection: &str) -> MockQueryBuilder<'_> {
        MockQueryBuilder {
            ctx: self,
            collection: collection.to_string(),
            filters: Vec::new(),
            sort: Vec::new(),
            limit: None,
            offset: None,
        }
    }

    /// Insert a new record into a collection.
    pub fn insert(&mut self, collection: &str, message: PipelineMessage) {
        self.data
            .entry(collection.to_string())
            .or_default()
            .push(message);
    }

    /// Update a record by ID in a collection.
    ///
    /// Replaces the first record whose `metadata.correlation_id` matches `id`.
    /// Returns `true` if a record was found and updated.
    pub fn update(&mut self, collection: &str, id: Uuid, message: PipelineMessage) -> bool {
        if let Some(records) = self.data.get_mut(collection) {
            if let Some(pos) = records
                .iter()
                .position(|r| r.metadata.correlation_id == id)
            {
                records[pos] = message;
                return true;
            }
        }
        false
    }

    /// Delete a record by ID from a collection.
    ///
    /// Removes the first record whose `metadata.correlation_id` matches `id`.
    /// Returns `true` if a record was found and removed.
    pub fn delete(&mut self, collection: &str, id: Uuid) -> bool {
        if let Some(records) = self.data.get_mut(collection) {
            if let Some(pos) = records
                .iter()
                .position(|r| r.metadata.correlation_id == id)
            {
                records.remove(pos);
                return true;
            }
        }
        false
    }

    /// Assert that exactly `count` records have been inserted into the collection.
    ///
    /// # Panics
    ///
    /// Panics if the actual count does not match.
    pub fn assert_inserted(&self, collection: &str, count: usize) {
        let actual = self.data.get(collection).map_or(0, |v| v.len());
        assert_eq!(
            actual, count,
            "expected {count} records in '{collection}', found {actual}"
        );
    }

    /// Assert that a record with the given correlation ID exists in the collection.
    ///
    /// # Panics
    ///
    /// Panics if no matching record is found.
    pub fn assert_contains(&self, collection: &str, id: Uuid) {
        let found = self
            .data
            .get(collection)
            .map_or(false, |v| v.iter().any(|r| r.metadata.correlation_id == id));
        assert!(
            found,
            "expected collection '{collection}' to contain record with id {id}"
        );
    }

    /// Return a snapshot of all stored data for debugging.
    pub fn dump(&self) -> &HashMap<String, Vec<PipelineMessage>> {
        &self.data
    }

    /// Return the plugin ID this mock was created with.
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    /// Execute an internal query against the in-memory data.
    fn execute_query(&self, query: &MockQueryBuilder<'_>) -> Vec<PipelineMessage> {
        let records = match self.data.get(&query.collection) {
            Some(r) => r.clone(),
            None => return vec![],
        };

        // Apply filters
        let mut results: Vec<PipelineMessage> = records
            .into_iter()
            .filter(|record| {
                query.filters.iter().all(|f| match_filter(record, f))
            })
            .collect();

        // Apply sorting
        for sort_field in query.sort.iter().rev() {
            results.sort_by(|a, b| {
                let va = extract_json_field(a, &sort_field.field);
                let vb = extract_json_field(b, &sort_field.field);
                let cmp = compare_json_values(&va, &vb);
                match sort_field.direction {
                    SortDirection::Asc => cmp,
                    SortDirection::Desc => cmp.reverse(),
                }
            });
        }

        // Apply offset
        let offset = query.offset.unwrap_or(0) as usize;
        if offset >= results.len() {
            return vec![];
        }
        let results = results.into_iter().skip(offset).collect::<Vec<_>>();

        // Apply limit
        let limit = query.limit.unwrap_or(1000).min(1000) as usize;
        results.into_iter().take(limit).collect()
    }
}

/// Fluent query builder for the mock storage.
pub struct MockQueryBuilder<'a> {
    ctx: &'a MockStorageContext,
    collection: String,
    filters: Vec<QueryFilter>,
    sort: Vec<SortField>,
    limit: Option<u32>,
    offset: Option<u32>,
}

impl<'a> MockQueryBuilder<'a> {
    /// Filter where `field` equals `value`.
    pub fn where_eq(mut self, field: &str, value: impl Into<serde_json::Value>) -> Self {
        self.filters.push(QueryFilter {
            field: field.to_string(),
            operator: FilterOp::Eq,
            value: value.into(),
        });
        self
    }

    /// Filter where `field` is greater than or equal to `value`.
    pub fn where_gte(mut self, field: &str, value: impl Into<serde_json::Value>) -> Self {
        self.filters.push(QueryFilter {
            field: field.to_string(),
            operator: FilterOp::Gte,
            value: value.into(),
        });
        self
    }

    /// Filter where `field` is less than or equal to `value`.
    pub fn where_lte(mut self, field: &str, value: impl Into<serde_json::Value>) -> Self {
        self.filters.push(QueryFilter {
            field: field.to_string(),
            operator: FilterOp::Lte,
            value: value.into(),
        });
        self
    }

    /// Filter where `field` contains `value`.
    pub fn where_contains(mut self, field: &str, value: impl Into<serde_json::Value>) -> Self {
        self.filters.push(QueryFilter {
            field: field.to_string(),
            operator: FilterOp::Contains,
            value: value.into(),
        });
        self
    }

    /// Sort results by `field` in ascending order.
    pub fn order_by(mut self, field: &str) -> Self {
        self.sort.push(SortField {
            field: field.to_string(),
            direction: SortDirection::Asc,
        });
        self
    }

    /// Sort results by `field` in descending order.
    pub fn order_by_desc(mut self, field: &str) -> Self {
        self.sort.push(SortField {
            field: field.to_string(),
            direction: SortDirection::Desc,
        });
        self
    }

    /// Limit the number of results (capped at 1000).
    pub fn limit(mut self, n: u32) -> Self {
        self.limit = Some(n.min(1000));
        self
    }

    /// Skip the first `n` results for pagination.
    pub fn offset(mut self, n: u32) -> Self {
        self.offset = Some(n);
        self
    }

    /// Execute the query and return matching records.
    pub fn execute(self) -> Vec<PipelineMessage> {
        self.ctx.execute_query(&self)
    }
}

/// Check if a record matches a single filter by serializing the payload
/// to JSON and extracting the field value.
fn match_filter(record: &PipelineMessage, filter: &QueryFilter) -> bool {
    let field_value = extract_json_field(record, &filter.field);

    match filter.operator {
        FilterOp::Eq => field_value == filter.value,
        FilterOp::NotEq => field_value != filter.value,
        FilterOp::Gte => compare_json_values(&field_value, &filter.value) != std::cmp::Ordering::Less,
        FilterOp::Lte => compare_json_values(&field_value, &filter.value) != std::cmp::Ordering::Greater,
        FilterOp::Contains => {
            if let (serde_json::Value::String(haystack), serde_json::Value::String(needle)) =
                (&field_value, &filter.value)
            {
                haystack.contains(needle.as_str())
            } else {
                false
            }
        }
    }
}

/// Extract a JSON field value from a PipelineMessage by serializing the payload.
fn extract_json_field(record: &PipelineMessage, field: &str) -> serde_json::Value {
    let payload_json = match serde_json::to_value(&record.payload) {
        Ok(v) => v,
        Err(_) => return serde_json::Value::Null,
    };

    // Navigate into CDM data: the payload is { "type": "Cdm", "data": { "collection": "...", "value": { ... } } }
    let data = payload_json
        .get("data")
        .and_then(|d| d.get("value"))
        .unwrap_or(&payload_json);

    // Support dot-notation for nested fields
    let mut current = data;
    for part in field.split('.') {
        match current.get(part) {
            Some(v) => current = v,
            None => return serde_json::Value::Null,
        }
    }
    current.clone()
}

/// Compare two JSON values for ordering.
fn compare_json_values(a: &serde_json::Value, b: &serde_json::Value) -> std::cmp::Ordering {
    use serde_json::Value;
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            let fa = a.as_f64().unwrap_or(0.0);
            let fb = b.as_f64().unwrap_or(0.0);
            fa.partial_cmp(&fb).unwrap_or(std::cmp::Ordering::Equal)
        }
        (Value::String(a), Value::String(b)) => a.cmp(b),
        (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
        (Value::Null, Value::Null) => std::cmp::Ordering::Equal,
        (Value::Null, _) => std::cmp::Ordering::Less,
        (_, Value::Null) => std::cmp::Ordering::Greater,
        _ => {
            let sa = a.to_string();
            let sb = b.to_string();
            sa.cmp(&sb)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use life_engine_types::{
        CdmType, MessageMetadata, Note, NoteFormat, TypedPayload,
    };

    fn make_note(title: &str, source: &str) -> PipelineMessage {
        PipelineMessage {
            metadata: MessageMetadata {
                correlation_id: Uuid::new_v4(),
                source: "test".to_string(),
                timestamp: Utc::now(),
                auth_context: None,
            },
            payload: TypedPayload::Cdm(Box::new(CdmType::Note(Note {
                id: Uuid::new_v4(),
                source: source.to_string(),
                source_id: "test-1".to_string(),
                title: title.to_string(),
                body: "Body".to_string(),
                format: Some(NoteFormat::Plain),
                pinned: Some(false),
                tags: vec![],
                extensions: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }))),
        }
    }

    #[test]
    fn insert_and_query() {
        let mut store = MockStorageContext::new("test-plugin");
        store.insert("notes", make_note("Hello", "google"));
        store.insert("notes", make_note("World", "google"));

        store.assert_inserted("notes", 2);

        let results = store.query("notes").execute();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn query_with_eq_filter() {
        let mut store = MockStorageContext::new("test-plugin");
        store.insert("notes", make_note("Meeting Notes", "google"));
        store.insert("notes", make_note("Shopping List", "apple"));

        let results = store
            .query("notes")
            .where_eq("source", "google")
            .execute();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn query_with_contains_filter() {
        let mut store = MockStorageContext::new("test-plugin");
        store.insert("notes", make_note("Team Meeting Notes", "test"));
        store.insert("notes", make_note("Shopping List", "test"));
        store.insert("notes", make_note("Meeting Agenda", "test"));

        let results = store
            .query("notes")
            .where_contains("title", "Meeting")
            .execute();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn query_with_limit_and_offset() {
        let mut store = MockStorageContext::new("test-plugin");
        for i in 0..10 {
            store.insert("notes", make_note(&format!("Note {i}"), "test"));
        }

        let results = store.query("notes").limit(3).offset(2).execute();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn limit_capped_at_1000() {
        let mut store = MockStorageContext::new("test-plugin");
        store.insert("notes", make_note("Test", "test"));

        let results = store.query("notes").limit(5000).execute();
        // Only 1 record exists, so we just check it doesn't panic
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn delete_by_id() {
        let mut store = MockStorageContext::new("test-plugin");
        let msg = make_note("To Delete", "test");
        let id = msg.metadata.correlation_id;
        store.insert("notes", msg);
        store.assert_inserted("notes", 1);

        assert!(store.delete("notes", id));
        store.assert_inserted("notes", 0);
    }

    #[test]
    fn update_by_id() {
        let mut store = MockStorageContext::new("test-plugin");
        let msg = make_note("Original", "test");
        let id = msg.metadata.correlation_id;
        store.insert("notes", msg);

        let updated = make_note("Updated", "test");
        assert!(store.update("notes", id, updated));

        let results = store.query("notes").execute();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn assert_contains_passes() {
        let mut store = MockStorageContext::new("test-plugin");
        let msg = make_note("Test", "test");
        let id = msg.metadata.correlation_id;
        store.insert("notes", msg);
        store.assert_contains("notes", id);
    }

    #[test]
    #[should_panic(expected = "expected collection")]
    fn assert_contains_fails() {
        let store = MockStorageContext::new("test-plugin");
        store.assert_contains("notes", Uuid::new_v4());
    }

    #[test]
    fn empty_collection_query() {
        let store = MockStorageContext::new("test-plugin");
        let results = store.query("nonexistent").execute();
        assert!(results.is_empty());
    }

    #[test]
    fn query_with_order_by() {
        let mut store = MockStorageContext::new("test-plugin");
        store.insert("notes", make_note("Banana", "test"));
        store.insert("notes", make_note("Apple", "test"));
        store.insert("notes", make_note("Cherry", "test"));

        let results = store.query("notes").order_by("title").execute();
        assert_eq!(results.len(), 3);
        // Extract titles to verify ordering
        let titles: Vec<String> = results
            .iter()
            .filter_map(|r| {
                serde_json::to_value(&r.payload)
                    .ok()
                    .and_then(|v| v.get("data")?.get("value")?.get("title")?.as_str().map(String::from))
            })
            .collect();
        assert_eq!(titles, vec!["Apple", "Banana", "Cherry"]);
    }
}
