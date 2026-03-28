//! StorageContext fluent query builder for plugin authors.
//!
//! Provides an ergonomic API for reading and writing collections
//! without importing database crates directly.

use std::collections::HashSet;

use life_engine_traits::{Capability, CapabilityViolation, EngineError, StorageBackend};
use life_engine_types::{
    FilterOp, PipelineMessage, QueryFilter, SortDirection, SortField, StorageMutation, StorageQuery,
};
use uuid::Uuid;

/// Context for storage operations scoped to a specific plugin.
///
/// `StorageContext` holds a reference to a [`StorageBackend`], the
/// calling plugin's ID, and its approved capabilities. All operations
/// are capability-checked before execution.
///
/// # Example
///
/// ```rust,ignore
/// let results = ctx.query("contacts")
///     .where_eq("source", "google")
///     .order_by("updated_at")
///     .limit(50)
///     .execute()
///     .await?;
/// ```
pub struct StorageContext<S: StorageBackend> {
    backend: S,
    plugin_id: String,
    capabilities: HashSet<Capability>,
}

impl<S: StorageBackend> StorageContext<S> {
    /// Create a new `StorageContext` for the given backend and plugin.
    pub fn new(
        backend: S,
        plugin_id: impl Into<String>,
        capabilities: HashSet<Capability>,
    ) -> Self {
        Self {
            backend,
            plugin_id: plugin_id.into(),
            capabilities,
        }
    }

    /// Check that the plugin has the required capability, returning a
    /// `CapabilityViolation` error if not.
    fn require(&self, cap: Capability, context: &str) -> Result<(), Box<dyn EngineError>> {
        if self.capabilities.contains(&cap) {
            Ok(())
        } else {
            Err(Box::new(CapabilityViolation {
                capability: cap,
                plugin_id: self.plugin_id.clone(),
                context: context.to_string(),
                at_load_time: false,
            }))
        }
    }

    /// Start building a read query against the given collection.
    ///
    /// The `storage:read` capability is checked when the query is executed.
    pub fn query(&self, collection: &str) -> QueryBuilder<'_, S> {
        QueryBuilder {
            ctx: self,
            collection: collection.to_string(),
            filters: Vec::new(),
            sort: Vec::new(),
            limit: None,
            offset: None,
        }
    }

    /// Insert a new record into a collection.
    ///
    /// Requires the `storage:write` capability.
    pub async fn insert(
        &self,
        collection: &str,
        message: PipelineMessage,
    ) -> Result<(), Box<dyn EngineError>> {
        self.require(Capability::StorageWrite, "insert into storage")?;
        self.backend
            .mutate(StorageMutation::Insert {
                plugin_id: self.plugin_id.clone(),
                collection: collection.to_string(),
                data: message,
            })
            .await
    }

    /// Update an existing record with optimistic concurrency control.
    ///
    /// Requires the `storage:write` capability.
    pub async fn update(
        &self,
        collection: &str,
        id: Uuid,
        message: PipelineMessage,
        expected_version: u64,
    ) -> Result<(), Box<dyn EngineError>> {
        self.require(Capability::StorageWrite, "update storage record")?;
        self.backend
            .mutate(StorageMutation::Update {
                plugin_id: self.plugin_id.clone(),
                collection: collection.to_string(),
                id,
                data: message,
                expected_version,
            })
            .await
    }

    /// Delete a record from a collection.
    ///
    /// Requires the `storage:write` capability.
    pub async fn delete(
        &self,
        collection: &str,
        id: Uuid,
    ) -> Result<(), Box<dyn EngineError>> {
        self.require(Capability::StorageWrite, "delete storage record")?;
        self.backend
            .mutate(StorageMutation::Delete {
                plugin_id: self.plugin_id.clone(),
                collection: collection.to_string(),
                id,
            })
            .await
    }
}

/// Fluent query builder for constructing storage read operations.
///
/// Created by [`StorageContext::query`]. Chain filter, sort, and
/// pagination methods, then call [`execute`](QueryBuilder::execute)
/// to run the query.
pub struct QueryBuilder<'a, S: StorageBackend> {
    ctx: &'a StorageContext<S>,
    collection: String,
    filters: Vec<QueryFilter>,
    sort: Vec<SortField>,
    limit: Option<u32>,
    offset: Option<u32>,
}

impl<'a, S: StorageBackend> QueryBuilder<'a, S> {
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
    ///
    /// Requires the `storage:read` capability.
    pub async fn execute(self) -> Result<Vec<PipelineMessage>, Box<dyn EngineError>> {
        self.ctx
            .require(Capability::StorageRead, "query storage")?;
        let query = StorageQuery {
            collection: self.collection,
            plugin_id: self.ctx.plugin_id.clone(),
            filters: self.filters,
            sort: self.sort,
            limit: self.limit,
            offset: self.offset,
        };
        self.ctx.backend.execute(query).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use chrono::Utc;
    use life_engine_types::{
        CdmType, MessageMetadata, Note, NoteFormat, TypedPayload,
    };
    use std::sync::{Arc, Mutex};

    fn all_caps() -> HashSet<Capability> {
        HashSet::from([Capability::StorageRead, Capability::StorageWrite])
    }

    fn read_only() -> HashSet<Capability> {
        HashSet::from([Capability::StorageRead])
    }

    fn write_only() -> HashSet<Capability> {
        HashSet::from([Capability::StorageWrite])
    }

    fn no_caps() -> HashSet<Capability> {
        HashSet::new()
    }

    fn test_message() -> PipelineMessage {
        PipelineMessage {
            metadata: MessageMetadata {
                correlation_id: Uuid::new_v4(),
                source: "test".to_string(),
                timestamp: Utc::now(),
                auth_context: None,
                warnings: vec![],
            },
            payload: TypedPayload::Cdm(Box::new(CdmType::Note(Note {
                id: Uuid::new_v4(),
                source: "test".to_string(),
                source_id: "test-1".to_string(),
                title: "Test note".to_string(),
                body: "Body".to_string(),
                format: Some(NoteFormat::Plain),
                pinned: Some(false),
                tags: vec![],
                extensions: Default::default(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }))),
        }
    }

    /// Simple mock storage backend for testing StorageContext.
    struct MockBackend {
        queries: Arc<Mutex<Vec<StorageQuery>>>,
        mutations: Arc<Mutex<Vec<StorageMutation>>>,
    }

    impl MockBackend {
        fn new() -> Self {
            Self {
                queries: Arc::new(Mutex::new(Vec::new())),
                mutations: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    #[async_trait]
    impl StorageBackend for MockBackend {
        async fn execute(
            &self,
            query: StorageQuery,
        ) -> Result<Vec<PipelineMessage>, Box<dyn EngineError>> {
            self.queries.lock().unwrap().push(query);
            Ok(vec![])
        }

        async fn mutate(&self, op: StorageMutation) -> Result<(), Box<dyn EngineError>> {
            self.mutations.lock().unwrap().push(op);
            Ok(())
        }

        async fn init(_config: toml::Value, _key: [u8; 32]) -> Result<Self, Box<dyn EngineError>>
        where
            Self: Sized,
        {
            Ok(Self::new())
        }
    }

    #[tokio::test]
    async fn query_builder_constructs_correct_query() {
        let backend = MockBackend::new();
        let queries = backend.queries.clone();
        let ctx = StorageContext::new(backend, "test-plugin", all_caps());

        let _results = ctx
            .query("contacts")
            .where_eq("source", "google")
            .where_gte("updated_at", "2026-01-01")
            .order_by("updated_at")
            .limit(50)
            .offset(10)
            .execute()
            .await
            .unwrap();

        let captured = queries.lock().unwrap();
        assert_eq!(captured.len(), 1);
        let q = &captured[0];
        assert_eq!(q.collection, "contacts");
        assert_eq!(q.plugin_id, "test-plugin");
        assert_eq!(q.filters.len(), 2);
        assert_eq!(q.filters[0].field, "source");
        assert_eq!(q.filters[0].operator, FilterOp::Eq);
        assert_eq!(q.filters[1].field, "updated_at");
        assert_eq!(q.filters[1].operator, FilterOp::Gte);
        assert_eq!(q.sort.len(), 1);
        assert_eq!(q.sort[0].field, "updated_at");
        assert_eq!(q.sort[0].direction, SortDirection::Asc);
        assert_eq!(q.limit, Some(50));
        assert_eq!(q.offset, Some(10));
    }

    #[tokio::test]
    async fn limit_capped_at_1000() {
        let backend = MockBackend::new();
        let queries = backend.queries.clone();
        let ctx = StorageContext::new(backend, "test-plugin", all_caps());

        ctx.query("events").limit(5000).execute().await.unwrap();

        let captured = queries.lock().unwrap();
        assert_eq!(captured[0].limit, Some(1000));
    }

    #[tokio::test]
    async fn insert_creates_correct_mutation() {
        let backend = MockBackend::new();
        let mutations = backend.mutations.clone();
        let ctx = StorageContext::new(backend, "test-plugin", all_caps());

        let msg = test_message();
        ctx.insert("events", msg).await.unwrap();

        let captured = mutations.lock().unwrap();
        assert_eq!(captured.len(), 1);
        match &captured[0] {
            StorageMutation::Insert {
                plugin_id,
                collection,
                ..
            } => {
                assert_eq!(plugin_id, "test-plugin");
                assert_eq!(collection, "events");
            }
            _ => panic!("Expected Insert mutation"),
        }
    }

    #[tokio::test]
    async fn update_creates_correct_mutation() {
        let backend = MockBackend::new();
        let mutations = backend.mutations.clone();
        let ctx = StorageContext::new(backend, "test-plugin", all_caps());

        let id = Uuid::new_v4();
        let msg = test_message();
        ctx.update("events", id, msg, 3).await.unwrap();

        let captured = mutations.lock().unwrap();
        match &captured[0] {
            StorageMutation::Update {
                plugin_id,
                collection,
                id: record_id,
                expected_version,
                ..
            } => {
                assert_eq!(plugin_id, "test-plugin");
                assert_eq!(collection, "events");
                assert_eq!(*record_id, id);
                assert_eq!(*expected_version, 3);
            }
            _ => panic!("Expected Update mutation"),
        }
    }

    #[tokio::test]
    async fn delete_creates_correct_mutation() {
        let backend = MockBackend::new();
        let mutations = backend.mutations.clone();
        let ctx = StorageContext::new(backend, "test-plugin", all_caps());

        let id = Uuid::new_v4();
        ctx.delete("events", id).await.unwrap();

        let captured = mutations.lock().unwrap();
        match &captured[0] {
            StorageMutation::Delete {
                plugin_id,
                collection,
                id: record_id,
            } => {
                assert_eq!(plugin_id, "test-plugin");
                assert_eq!(collection, "events");
                assert_eq!(*record_id, id);
            }
            _ => panic!("Expected Delete mutation"),
        }
    }

    #[tokio::test]
    async fn query_with_contains_and_desc_sort() {
        let backend = MockBackend::new();
        let queries = backend.queries.clone();
        let ctx = StorageContext::new(backend, "test-plugin", all_caps());

        ctx.query("notes")
            .where_contains("title", "meeting")
            .where_lte("created_at", "2026-12-31")
            .order_by_desc("created_at")
            .limit(25)
            .execute()
            .await
            .unwrap();

        let captured = queries.lock().unwrap();
        let q = &captured[0];
        assert_eq!(q.filters.len(), 2);
        assert_eq!(q.filters[0].operator, FilterOp::Contains);
        assert_eq!(q.filters[1].operator, FilterOp::Lte);
        assert_eq!(q.sort[0].direction, SortDirection::Desc);
        assert_eq!(q.limit, Some(25));
    }

    // --- Capability check tests ---

    #[tokio::test]
    async fn read_with_storage_read_succeeds() {
        let backend = MockBackend::new();
        let ctx = StorageContext::new(backend, "test-plugin", read_only());

        let result = ctx.query("events").execute().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn read_without_storage_read_returns_cap_002() {
        let backend = MockBackend::new();
        let ctx = StorageContext::new(backend, "test-plugin", no_caps());

        let result = ctx.query("events").execute().await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), "CAP_002");
    }

    #[tokio::test]
    async fn write_with_storage_write_succeeds() {
        let backend = MockBackend::new();
        let ctx = StorageContext::new(backend, "test-plugin", write_only());

        let msg = test_message();
        let result = ctx.insert("events", msg).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn write_without_storage_write_returns_cap_002() {
        let backend = MockBackend::new();
        let ctx = StorageContext::new(backend, "test-plugin", read_only());

        let msg = test_message();
        let result = ctx.insert("events", msg).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), "CAP_002");
    }

    #[tokio::test]
    async fn update_without_storage_write_returns_cap_002() {
        let backend = MockBackend::new();
        let ctx = StorageContext::new(backend, "test-plugin", read_only());

        let msg = test_message();
        let result = ctx.update("events", Uuid::new_v4(), msg, 1).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), "CAP_002");
    }

    #[tokio::test]
    async fn delete_without_storage_write_returns_cap_002() {
        let backend = MockBackend::new();
        let ctx = StorageContext::new(backend, "test-plugin", read_only());

        let result = ctx.delete("events", Uuid::new_v4()).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), "CAP_002");
    }
}
