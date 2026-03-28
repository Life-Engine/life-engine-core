//! Storage router that dispatches operations to the correct adapter.
//!
//! `StorageRouter` holds references to a `DocumentStorageAdapter` and a
//! `BlobStorageAdapter`, routing document operations to the document adapter
//! and blob operations to the blob adapter. It enforces per-operation-class
//! timeouts, emits structured tracing metrics for every operation, and
//! aggregates health from both adapters.

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;
use tracing::instrument;

use crate::blob::{BlobInput, BlobKey, BlobMeta, BlobStorageAdapter};
use crate::storage::{
    AdapterCapabilities, ChangeEvent, CollectionDescriptor, DocumentList, DocumentStorageAdapter,
    FilterNode, HealthReport, HealthStatus, QueryDescriptor, StorageError,
};

/// Timeout configuration for storage operations, in milliseconds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutConfig {
    /// Timeout for document read operations (get, query, count).
    pub document_read_ms: u64,
    /// Timeout for document write operations (create, update, delete, batch, migrate).
    pub document_write_ms: u64,
    /// Timeout for blob read operations (retrieve, exists, list, metadata).
    pub blob_read_ms: u64,
    /// Timeout for blob write operations (store, copy, delete).
    pub blob_write_ms: u64,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            document_read_ms: 5000,
            document_write_ms: 10000,
            blob_read_ms: 10000,
            blob_write_ms: 30000,
        }
    }
}

/// Routes storage operations to the appropriate adapter with timeout enforcement
/// and health aggregation.
pub struct StorageRouter {
    doc_adapter: Arc<dyn DocumentStorageAdapter>,
    blob_adapter: Arc<dyn BlobStorageAdapter>,
    timeouts: TimeoutConfig,
}

/// TOML-deserializable storage configuration used by the startup sequence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Name of the document adapter to use (e.g. "sqlite").
    pub document_adapter: String,
    /// Name of the blob adapter to use (e.g. "filesystem").
    pub blob_adapter: String,
    /// Timeout configuration.
    #[serde(default)]
    pub timeouts: TimeoutConfig,
    /// Required document adapter capabilities.
    #[serde(default)]
    pub required_capabilities: RequiredCapabilities,
}

/// Capabilities that must be present on the document adapter.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RequiredCapabilities {
    #[serde(default)]
    pub transactions: bool,
    #[serde(default)]
    pub full_text_search: bool,
    #[serde(default)]
    pub watch: bool,
    #[serde(default)]
    pub batch_operations: bool,
    #[serde(default)]
    pub encryption: bool,
}

/// Errors that can occur during storage router startup.
#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error("unknown document adapter: {0}")]
    UnknownDocumentAdapter(String),
    #[error("unknown blob adapter: {0}")]
    UnknownBlobAdapter(String),
    #[error("capability mismatch: {0}")]
    CapabilityMismatch(String),
    #[error("health check failed: {0}")]
    HealthCheckFailed(String),
    #[error("config error: {0}")]
    ConfigError(String),
}

/// A factory function that produces a document adapter from a name.
pub type DocAdapterFactory =
    Box<dyn Fn(&str) -> Option<Arc<dyn DocumentStorageAdapter>> + Send + Sync>;

/// A factory function that produces a blob adapter from a name.
pub type BlobAdapterFactory =
    Box<dyn Fn(&str) -> Option<Arc<dyn BlobStorageAdapter>> + Send + Sync>;

impl StorageRouter {
    /// Create a new `StorageRouter` with the given adapters and timeout config.
    pub fn new(
        doc_adapter: Arc<dyn DocumentStorageAdapter>,
        blob_adapter: Arc<dyn BlobStorageAdapter>,
        timeouts: TimeoutConfig,
    ) -> Self {
        Self {
            doc_adapter,
            blob_adapter,
            timeouts,
        }
    }

    /// Perform the full startup sequence:
    /// 1. Parse `StorageConfig`
    /// 2. Resolve adapter names to concrete implementations via the provided factories
    /// 3. Validate required capabilities against adapter capabilities
    /// 4. Run health checks on both adapters
    /// 5. Return a ready-to-use `StorageRouter`
    pub async fn from_config(
        config: StorageConfig,
        resolve_doc: &DocAdapterFactory,
        resolve_blob: &BlobAdapterFactory,
    ) -> Result<Self, StartupError> {
        let doc_adapter = resolve_doc(&config.document_adapter).ok_or_else(|| {
            StartupError::UnknownDocumentAdapter(config.document_adapter.clone())
        })?;

        let blob_adapter = resolve_blob(&config.blob_adapter).ok_or_else(|| {
            StartupError::UnknownBlobAdapter(config.blob_adapter.clone())
        })?;

        check_capabilities(&doc_adapter.capabilities(), &config.required_capabilities)?;

        let router = Self::new(doc_adapter, blob_adapter, config.timeouts);

        let health = router
            .health()
            .await
            .map_err(|e| StartupError::HealthCheckFailed(e.to_string()))?;

        if health.status == HealthStatus::Unhealthy {
            return Err(StartupError::HealthCheckFailed(format!(
                "storage unhealthy: {}",
                health.message.unwrap_or_default()
            )));
        }

        tracing::info!(
            doc_adapter = config.document_adapter,
            blob_adapter = config.blob_adapter,
            health = ?health.status,
            "storage router started"
        );

        Ok(router)
    }

    /// Apply a timeout to an async operation, returning `StorageError::Timeout`
    /// if the deadline is exceeded.
    async fn with_timeout<T>(
        &self,
        duration: Duration,
        op_name: &str,
        fut: impl std::future::Future<Output = Result<T, StorageError>>,
    ) -> Result<T, StorageError> {
        match tokio::time::timeout(duration, fut).await {
            Ok(result) => result,
            Err(_) => Err(StorageError::Timeout {
                message: format!(
                    "{} timed out after {}ms",
                    op_name,
                    duration.as_millis()
                ),
            }),
        }
    }

    fn doc_read_timeout(&self) -> Duration {
        Duration::from_millis(self.timeouts.document_read_ms)
    }

    fn doc_write_timeout(&self) -> Duration {
        Duration::from_millis(self.timeouts.document_write_ms)
    }

    fn blob_read_timeout(&self) -> Duration {
        Duration::from_millis(self.timeouts.blob_read_ms)
    }

    fn blob_write_timeout(&self) -> Duration {
        Duration::from_millis(self.timeouts.blob_write_ms)
    }

    // -- Document read operations ---------------------------------------------

    /// Retrieve a single document by collection and ID.
    #[instrument(skip(self), fields(op = "doc_get"))]
    pub async fn doc_get(&self, collection: &str, id: &str) -> Result<Value, StorageError> {
        let timeout = self.doc_read_timeout();
        self.with_timeout(timeout, "doc_get", self.doc_adapter.get(collection, id))
            .await
    }

    /// Execute a query and return matching documents.
    #[instrument(skip(self, descriptor), fields(op = "doc_query"))]
    pub async fn doc_query(&self, descriptor: QueryDescriptor) -> Result<DocumentList, StorageError> {
        let timeout = self.doc_read_timeout();
        self.with_timeout(timeout, "doc_query", self.doc_adapter.query(descriptor))
            .await
    }

    /// Count documents matching optional filters.
    #[instrument(skip(self, filters), fields(op = "doc_count"))]
    pub async fn doc_count(
        &self,
        collection: &str,
        filters: Option<FilterNode>,
    ) -> Result<u64, StorageError> {
        let timeout = self.doc_read_timeout();
        self.with_timeout(timeout, "doc_count", self.doc_adapter.count(collection, filters))
            .await
    }

    // -- Document write operations --------------------------------------------

    /// Insert a new document into the collection.
    #[instrument(skip(self, document), fields(op = "doc_create"))]
    pub async fn doc_create(
        &self,
        collection: &str,
        document: Value,
    ) -> Result<Value, StorageError> {
        let timeout = self.doc_write_timeout();
        self.with_timeout(timeout, "doc_create", self.doc_adapter.create(collection, document))
            .await
    }

    /// Replace an existing document (full update).
    #[instrument(skip(self, document), fields(op = "doc_update"))]
    pub async fn doc_update(
        &self,
        collection: &str,
        id: &str,
        document: Value,
    ) -> Result<Value, StorageError> {
        let timeout = self.doc_write_timeout();
        self.with_timeout(
            timeout,
            "doc_update",
            self.doc_adapter.update(collection, id, document),
        )
        .await
    }

    /// Merge fields into an existing document (partial update).
    #[instrument(skip(self, fields), fields(op = "doc_partial_update"))]
    pub async fn doc_partial_update(
        &self,
        collection: &str,
        id: &str,
        fields: Value,
    ) -> Result<Value, StorageError> {
        let timeout = self.doc_write_timeout();
        self.with_timeout(
            timeout,
            "doc_partial_update",
            self.doc_adapter.partial_update(collection, id, fields),
        )
        .await
    }

    /// Delete a document by collection and ID.
    #[instrument(skip(self), fields(op = "doc_delete"))]
    pub async fn doc_delete(&self, collection: &str, id: &str) -> Result<(), StorageError> {
        let timeout = self.doc_write_timeout();
        self.with_timeout(timeout, "doc_delete", self.doc_adapter.delete(collection, id))
            .await
    }

    /// Atomically insert multiple documents.
    #[instrument(skip(self, documents), fields(op = "doc_batch_create", count = documents.len()))]
    pub async fn doc_batch_create(
        &self,
        collection: &str,
        documents: Vec<Value>,
    ) -> Result<Vec<Value>, StorageError> {
        let timeout = self.doc_write_timeout();
        self.with_timeout(
            timeout,
            "doc_batch_create",
            self.doc_adapter.batch_create(collection, documents),
        )
        .await
    }

    /// Atomically replace multiple documents.
    #[instrument(skip(self, updates), fields(op = "doc_batch_update", count = updates.len()))]
    pub async fn doc_batch_update(
        &self,
        collection: &str,
        updates: Vec<(String, Value)>,
    ) -> Result<Vec<Value>, StorageError> {
        let timeout = self.doc_write_timeout();
        self.with_timeout(
            timeout,
            "doc_batch_update",
            self.doc_adapter.batch_update(collection, updates),
        )
        .await
    }

    /// Atomically delete multiple documents by ID.
    #[instrument(skip(self, ids), fields(op = "doc_batch_delete", count = ids.len()))]
    pub async fn doc_batch_delete(
        &self,
        collection: &str,
        ids: Vec<String>,
    ) -> Result<(), StorageError> {
        let timeout = self.doc_write_timeout();
        self.with_timeout(
            timeout,
            "doc_batch_delete",
            self.doc_adapter.batch_delete(collection, ids),
        )
        .await
    }

    /// Subscribe to changes on a collection.
    #[instrument(skip(self), fields(op = "doc_watch"))]
    pub async fn doc_watch(
        &self,
        collection: &str,
    ) -> Result<mpsc::Receiver<ChangeEvent>, StorageError> {
        let timeout = self.doc_write_timeout();
        self.with_timeout(timeout, "doc_watch", self.doc_adapter.watch(collection))
            .await
    }

    /// Create or update a collection's schema.
    #[instrument(skip(self, descriptor), fields(op = "doc_migrate"))]
    pub async fn doc_migrate(
        &self,
        descriptor: CollectionDescriptor,
    ) -> Result<(), StorageError> {
        let timeout = self.doc_write_timeout();
        self.with_timeout(timeout, "doc_migrate", self.doc_adapter.migrate(descriptor))
            .await
    }

    // -- Blob read operations -------------------------------------------------

    /// Retrieve a blob's data and metadata by key.
    #[instrument(skip(self), fields(op = "blob_retrieve"))]
    pub async fn blob_retrieve(
        &self,
        key: BlobKey,
    ) -> Result<(Vec<u8>, BlobMeta), StorageError> {
        let timeout = self.blob_read_timeout();
        self.with_timeout(timeout, "blob_retrieve", self.blob_adapter.retrieve(key))
            .await
    }

    /// Check whether a blob exists at the given key.
    #[instrument(skip(self), fields(op = "blob_exists"))]
    pub async fn blob_exists(&self, key: BlobKey) -> Result<bool, StorageError> {
        let timeout = self.blob_read_timeout();
        self.with_timeout(timeout, "blob_exists", self.blob_adapter.exists(key))
            .await
    }

    /// List blobs whose keys start with the given prefix.
    #[instrument(skip(self), fields(op = "blob_list"))]
    pub async fn blob_list(&self, prefix: &str) -> Result<Vec<BlobMeta>, StorageError> {
        let timeout = self.blob_read_timeout();
        self.with_timeout(timeout, "blob_list", self.blob_adapter.list(prefix))
            .await
    }

    /// Retrieve metadata for a blob without downloading the data.
    #[instrument(skip(self), fields(op = "blob_metadata"))]
    pub async fn blob_metadata(&self, key: BlobKey) -> Result<BlobMeta, StorageError> {
        let timeout = self.blob_read_timeout();
        self.with_timeout(timeout, "blob_metadata", self.blob_adapter.metadata(key))
            .await
    }

    // -- Blob write operations ------------------------------------------------

    /// Store a blob at the given key, returning its metadata.
    #[instrument(skip(self, input), fields(op = "blob_store"))]
    pub async fn blob_store(
        &self,
        key: BlobKey,
        input: BlobInput,
    ) -> Result<BlobMeta, StorageError> {
        let timeout = self.blob_write_timeout();
        self.with_timeout(timeout, "blob_store", self.blob_adapter.store(key, input))
            .await
    }

    /// Copy a blob from source to destination.
    #[instrument(skip(self), fields(op = "blob_copy"))]
    pub async fn blob_copy(
        &self,
        source: BlobKey,
        dest: BlobKey,
    ) -> Result<BlobMeta, StorageError> {
        let timeout = self.blob_write_timeout();
        self.with_timeout(timeout, "blob_copy", self.blob_adapter.copy(source, dest))
            .await
    }

    /// Delete a blob by key.
    #[instrument(skip(self), fields(op = "blob_delete"))]
    pub async fn blob_delete(&self, key: BlobKey) -> Result<(), StorageError> {
        let timeout = self.blob_write_timeout();
        self.with_timeout(timeout, "blob_delete", self.blob_adapter.delete(key))
            .await
    }

    // -- Health ---------------------------------------------------------------

    /// Aggregate health from both adapters. Worst status wins.
    #[instrument(skip(self), fields(op = "health"))]
    pub async fn health(&self) -> Result<HealthReport, StorageError> {
        let (doc_health, blob_health) = tokio::join!(
            self.doc_adapter.health(),
            self.blob_adapter.health()
        );

        let doc_report = doc_health?;
        let blob_report = blob_health?;

        let overall = worst_status(doc_report.status, blob_report.status);

        let mut checks = Vec::new();
        checks.extend(doc_report.checks.iter().cloned());
        checks.extend(blob_report.checks.iter().cloned());

        Ok(HealthReport {
            status: overall,
            message: Some(format!(
                "document: {:?}, blob: {:?}",
                doc_report.status, blob_report.status
            )),
            checks,
        })
    }

    /// Return a reference to the document adapter.
    pub fn doc_adapter(&self) -> &dyn DocumentStorageAdapter {
        self.doc_adapter.as_ref()
    }

    /// Return a reference to the blob adapter.
    pub fn blob_adapter(&self) -> &dyn BlobStorageAdapter {
        self.blob_adapter.as_ref()
    }
}

/// Check that the adapter provides all required capabilities.
fn check_capabilities(
    actual: &AdapterCapabilities,
    required: &RequiredCapabilities,
) -> Result<(), StartupError> {
    let mut missing = Vec::new();
    if required.transactions && !actual.transactions {
        missing.push("transactions");
    }
    if required.full_text_search && !actual.full_text_search {
        missing.push("full_text_search");
    }
    if required.watch && !actual.watch {
        missing.push("watch");
    }
    if required.batch_operations && !actual.batch_operations {
        missing.push("batch_operations");
    }
    if required.encryption && !actual.encryption {
        missing.push("encryption");
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(StartupError::CapabilityMismatch(format!(
            "adapter missing required capabilities: {}",
            missing.join(", ")
        )))
    }
}

/// Return the worst of two health statuses.
/// Unhealthy > Degraded > Healthy.
fn worst_status(a: HealthStatus, b: HealthStatus) -> HealthStatus {
    match (a, b) {
        (HealthStatus::Unhealthy, _) | (_, HealthStatus::Unhealthy) => HealthStatus::Unhealthy,
        (HealthStatus::Degraded, _) | (_, HealthStatus::Degraded) => HealthStatus::Degraded,
        _ => HealthStatus::Healthy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use async_trait::async_trait;
    use serde_json::json;

    use crate::blob::BlobAdapterCapabilities;
    use crate::storage::{
        AdapterCapabilities, HealthCheck,
    };

    // -- Slow mock adapter for timeout testing --------------------------------

    struct SlowDocAdapter {
        delay: Duration,
    }

    #[async_trait]
    impl DocumentStorageAdapter for SlowDocAdapter {
        async fn get(&self, collection: &str, id: &str) -> Result<Value, StorageError> {
            tokio::time::sleep(self.delay).await;
            Ok(json!({"id": id, "collection": collection}))
        }
        async fn create(&self, _collection: &str, doc: Value) -> Result<Value, StorageError> {
            tokio::time::sleep(self.delay).await;
            Ok(doc)
        }
        async fn update(&self, _c: &str, _id: &str, doc: Value) -> Result<Value, StorageError> {
            tokio::time::sleep(self.delay).await;
            Ok(doc)
        }
        async fn partial_update(&self, _c: &str, _id: &str, f: Value) -> Result<Value, StorageError> {
            tokio::time::sleep(self.delay).await;
            Ok(f)
        }
        async fn delete(&self, _c: &str, _id: &str) -> Result<(), StorageError> {
            tokio::time::sleep(self.delay).await;
            Ok(())
        }
        async fn query(&self, _d: QueryDescriptor) -> Result<DocumentList, StorageError> {
            tokio::time::sleep(self.delay).await;
            Ok(DocumentList { documents: vec![], total_count: 0, next_cursor: None })
        }
        async fn count(&self, _c: &str, _f: Option<FilterNode>) -> Result<u64, StorageError> {
            tokio::time::sleep(self.delay).await;
            Ok(0)
        }
        async fn batch_create(&self, _c: &str, docs: Vec<Value>) -> Result<Vec<Value>, StorageError> {
            tokio::time::sleep(self.delay).await;
            Ok(docs)
        }
        async fn batch_update(&self, _c: &str, _u: Vec<(String, Value)>) -> Result<Vec<Value>, StorageError> {
            tokio::time::sleep(self.delay).await;
            Ok(vec![])
        }
        async fn batch_delete(&self, _c: &str, _ids: Vec<String>) -> Result<(), StorageError> {
            tokio::time::sleep(self.delay).await;
            Ok(())
        }
        async fn watch(&self, _c: &str) -> Result<mpsc::Receiver<ChangeEvent>, StorageError> {
            let (_tx, rx) = mpsc::channel(1);
            Ok(rx)
        }
        async fn migrate(&self, _d: CollectionDescriptor) -> Result<(), StorageError> {
            tokio::time::sleep(self.delay).await;
            Ok(())
        }
        async fn health(&self) -> Result<HealthReport, StorageError> {
            Ok(HealthReport {
                status: HealthStatus::Healthy,
                message: Some("slow doc".into()),
                checks: vec![HealthCheck { name: "slow".into(), status: HealthStatus::Healthy, message: None }],
            })
        }
        fn capabilities(&self) -> AdapterCapabilities {
            AdapterCapabilities {
                indexing: true, transactions: false, full_text_search: false,
                watch: false, batch_operations: true, encryption: false,
            }
        }
    }

    // -- Configurable-health mock for aggregation tests -----------------------

    struct HealthDocAdapter(HealthStatus);

    #[async_trait]
    impl DocumentStorageAdapter for HealthDocAdapter {
        async fn get(&self, _c: &str, _id: &str) -> Result<Value, StorageError> { Ok(json!({})) }
        async fn create(&self, _c: &str, d: Value) -> Result<Value, StorageError> { Ok(d) }
        async fn update(&self, _c: &str, _id: &str, d: Value) -> Result<Value, StorageError> { Ok(d) }
        async fn partial_update(&self, _c: &str, _id: &str, f: Value) -> Result<Value, StorageError> { Ok(f) }
        async fn delete(&self, _c: &str, _id: &str) -> Result<(), StorageError> { Ok(()) }
        async fn query(&self, _d: QueryDescriptor) -> Result<DocumentList, StorageError> {
            Ok(DocumentList { documents: vec![], total_count: 0, next_cursor: None })
        }
        async fn count(&self, _c: &str, _f: Option<FilterNode>) -> Result<u64, StorageError> { Ok(0) }
        async fn batch_create(&self, _c: &str, d: Vec<Value>) -> Result<Vec<Value>, StorageError> { Ok(d) }
        async fn batch_update(&self, _c: &str, _u: Vec<(String, Value)>) -> Result<Vec<Value>, StorageError> { Ok(vec![]) }
        async fn batch_delete(&self, _c: &str, _ids: Vec<String>) -> Result<(), StorageError> { Ok(()) }
        async fn watch(&self, _c: &str) -> Result<mpsc::Receiver<ChangeEvent>, StorageError> {
            let (_tx, rx) = mpsc::channel(1);
            Ok(rx)
        }
        async fn migrate(&self, _d: CollectionDescriptor) -> Result<(), StorageError> { Ok(()) }
        async fn health(&self) -> Result<HealthReport, StorageError> {
            Ok(HealthReport {
                status: self.0,
                message: None,
                checks: vec![HealthCheck { name: "doc".into(), status: self.0, message: None }],
            })
        }
        fn capabilities(&self) -> AdapterCapabilities {
            AdapterCapabilities {
                indexing: false, transactions: false, full_text_search: false,
                watch: false, batch_operations: false, encryption: false,
            }
        }
    }

    struct HealthBlobAdapter(HealthStatus);

    #[async_trait]
    impl BlobStorageAdapter for HealthBlobAdapter {
        async fn store(&self, _k: BlobKey, _i: BlobInput) -> Result<BlobMeta, StorageError> { unreachable!() }
        async fn retrieve(&self, _k: BlobKey) -> Result<(Vec<u8>, BlobMeta), StorageError> { unreachable!() }
        async fn delete(&self, _k: BlobKey) -> Result<(), StorageError> { Ok(()) }
        async fn exists(&self, _k: BlobKey) -> Result<bool, StorageError> { Ok(false) }
        async fn copy(&self, _s: BlobKey, _d: BlobKey) -> Result<BlobMeta, StorageError> { unreachable!() }
        async fn list(&self, _p: &str) -> Result<Vec<BlobMeta>, StorageError> { Ok(vec![]) }
        async fn metadata(&self, _k: BlobKey) -> Result<BlobMeta, StorageError> { unreachable!() }
        async fn health(&self) -> Result<HealthReport, StorageError> {
            Ok(HealthReport {
                status: self.0,
                message: None,
                checks: vec![HealthCheck { name: "blob".into(), status: self.0, message: None }],
            })
        }
        fn capabilities(&self) -> BlobAdapterCapabilities {
            BlobAdapterCapabilities::default()
        }
    }

    // -- Helper to build a router from mock adapters --------------------------

    fn make_router(
        doc: impl DocumentStorageAdapter + 'static,
        blob: impl BlobStorageAdapter + 'static,
        timeouts: TimeoutConfig,
    ) -> StorageRouter {
        StorageRouter::new(Arc::new(doc), Arc::new(blob), timeouts)
    }

    fn fast_router() -> StorageRouter {
        make_router(
            HealthDocAdapter(HealthStatus::Healthy),
            HealthBlobAdapter(HealthStatus::Healthy),
            TimeoutConfig::default(),
        )
    }

    // =========================================================================
    // Test 1: Routes document ops to document adapter
    // =========================================================================

    #[tokio::test]
    async fn routes_doc_get_to_document_adapter() {
        let router = fast_router();
        let result = router.doc_get("users", "1").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn routes_blob_list_to_blob_adapter() {
        let router = fast_router();
        let result = router.blob_list("plugin-a/").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // =========================================================================
    // Test 2: Timeout enforcement
    // =========================================================================

    #[tokio::test]
    async fn timeout_on_slow_doc_read() {
        let router = make_router(
            SlowDocAdapter { delay: Duration::from_millis(200) },
            HealthBlobAdapter(HealthStatus::Healthy),
            TimeoutConfig {
                document_read_ms: 50,
                document_write_ms: 5000,
                blob_read_ms: 5000,
                blob_write_ms: 5000,
            },
        );

        let result = router.doc_get("users", "1").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), StorageError::Timeout { .. }));
    }

    #[tokio::test]
    async fn timeout_on_slow_doc_write() {
        let router = make_router(
            SlowDocAdapter { delay: Duration::from_millis(200) },
            HealthBlobAdapter(HealthStatus::Healthy),
            TimeoutConfig {
                document_read_ms: 5000,
                document_write_ms: 50,
                blob_read_ms: 5000,
                blob_write_ms: 5000,
            },
        );

        let result = router.doc_create("users", json!({"name": "Alice"})).await;
        assert!(matches!(result.unwrap_err(), StorageError::Timeout { .. }));
    }

    #[tokio::test]
    async fn no_timeout_when_fast_enough() {
        let router = make_router(
            SlowDocAdapter { delay: Duration::from_millis(5) },
            HealthBlobAdapter(HealthStatus::Healthy),
            TimeoutConfig {
                document_read_ms: 500,
                document_write_ms: 500,
                blob_read_ms: 500,
                blob_write_ms: 500,
            },
        );

        let result = router.doc_get("users", "1").await;
        assert!(result.is_ok());
    }

    // =========================================================================
    // Test 3: Health aggregation (worst status wins)
    // =========================================================================

    #[tokio::test]
    async fn health_both_healthy() {
        let router = make_router(
            HealthDocAdapter(HealthStatus::Healthy),
            HealthBlobAdapter(HealthStatus::Healthy),
            TimeoutConfig::default(),
        );
        let report = router.health().await.unwrap();
        assert_eq!(report.status, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn health_degraded_wins_over_healthy() {
        let router = make_router(
            HealthDocAdapter(HealthStatus::Healthy),
            HealthBlobAdapter(HealthStatus::Degraded),
            TimeoutConfig::default(),
        );
        let report = router.health().await.unwrap();
        assert_eq!(report.status, HealthStatus::Degraded);
    }

    #[tokio::test]
    async fn health_unhealthy_wins_over_degraded() {
        let router = make_router(
            HealthDocAdapter(HealthStatus::Unhealthy),
            HealthBlobAdapter(HealthStatus::Degraded),
            TimeoutConfig::default(),
        );
        let report = router.health().await.unwrap();
        assert_eq!(report.status, HealthStatus::Unhealthy);
    }

    #[tokio::test]
    async fn health_includes_checks_from_both_adapters() {
        let router = make_router(
            HealthDocAdapter(HealthStatus::Healthy),
            HealthBlobAdapter(HealthStatus::Healthy),
            TimeoutConfig::default(),
        );
        let report = router.health().await.unwrap();
        assert_eq!(report.checks.len(), 2);
        let names: Vec<&str> = report.checks.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"doc"));
        assert!(names.contains(&"blob"));
    }

    // =========================================================================
    // Test: worst_status unit
    // =========================================================================

    #[test]
    fn worst_status_function() {
        assert_eq!(worst_status(HealthStatus::Healthy, HealthStatus::Healthy), HealthStatus::Healthy);
        assert_eq!(worst_status(HealthStatus::Healthy, HealthStatus::Degraded), HealthStatus::Degraded);
        assert_eq!(worst_status(HealthStatus::Degraded, HealthStatus::Healthy), HealthStatus::Degraded);
        assert_eq!(worst_status(HealthStatus::Unhealthy, HealthStatus::Healthy), HealthStatus::Unhealthy);
        assert_eq!(worst_status(HealthStatus::Healthy, HealthStatus::Unhealthy), HealthStatus::Unhealthy);
        assert_eq!(worst_status(HealthStatus::Unhealthy, HealthStatus::Degraded), HealthStatus::Unhealthy);
        assert_eq!(worst_status(HealthStatus::Degraded, HealthStatus::Unhealthy), HealthStatus::Unhealthy);
    }

    // =========================================================================
    // Test: Capability checking
    // =========================================================================

    #[test]
    fn capability_check_passes_when_all_met() {
        let caps = AdapterCapabilities {
            indexing: true,
            transactions: true,
            full_text_search: false,
            watch: false,
            batch_operations: true,
            encryption: false,
        };
        let required = RequiredCapabilities {
            transactions: true,
            batch_operations: true,
            ..Default::default()
        };
        assert!(check_capabilities(&caps, &required).is_ok());
    }

    #[test]
    fn capability_check_fails_on_mismatch() {
        let caps = AdapterCapabilities {
            indexing: true,
            transactions: false,
            full_text_search: false,
            watch: false,
            batch_operations: true,
            encryption: false,
        };
        let required = RequiredCapabilities {
            transactions: true,
            ..Default::default()
        };
        let err = check_capabilities(&caps, &required).unwrap_err();
        assert!(err.to_string().contains("transactions"));
    }

    // =========================================================================
    // Test: from_config startup sequence
    // =========================================================================

    #[tokio::test]
    async fn from_config_resolves_adapters_and_checks_health() {
        let config = StorageConfig {
            document_adapter: "sqlite".into(),
            blob_adapter: "filesystem".into(),
            timeouts: TimeoutConfig::default(),
            required_capabilities: RequiredCapabilities::default(),
        };

        let doc_factory: DocAdapterFactory = Box::new(|name| {
            if name == "sqlite" {
                Some(Arc::new(HealthDocAdapter(HealthStatus::Healthy)) as Arc<dyn DocumentStorageAdapter>)
            } else {
                None
            }
        });

        let blob_factory: BlobAdapterFactory = Box::new(|name| {
            if name == "filesystem" {
                Some(Arc::new(HealthBlobAdapter(HealthStatus::Healthy)) as Arc<dyn BlobStorageAdapter>)
            } else {
                None
            }
        });

        let router = StorageRouter::from_config(config, &doc_factory, &blob_factory).await;
        assert!(router.is_ok());
    }

    #[tokio::test]
    async fn from_config_rejects_unknown_adapter() {
        let config = StorageConfig {
            document_adapter: "postgres".into(),
            blob_adapter: "filesystem".into(),
            timeouts: TimeoutConfig::default(),
            required_capabilities: RequiredCapabilities::default(),
        };

        let doc_factory: DocAdapterFactory = Box::new(|_| None);
        let blob_factory: BlobAdapterFactory = Box::new(|name| {
            if name == "filesystem" {
                Some(Arc::new(HealthBlobAdapter(HealthStatus::Healthy)) as Arc<dyn BlobStorageAdapter>)
            } else {
                None
            }
        });

        let result = StorageRouter::from_config(config, &doc_factory, &blob_factory).await;
        assert!(matches!(result, Err(StartupError::UnknownDocumentAdapter(_))));
    }

    #[tokio::test]
    async fn from_config_rejects_capability_mismatch() {
        let config = StorageConfig {
            document_adapter: "sqlite".into(),
            blob_adapter: "filesystem".into(),
            timeouts: TimeoutConfig::default(),
            required_capabilities: RequiredCapabilities {
                encryption: true,
                ..Default::default()
            },
        };

        let doc_factory: DocAdapterFactory = Box::new(|name| {
            if name == "sqlite" {
                Some(Arc::new(HealthDocAdapter(HealthStatus::Healthy)) as Arc<dyn DocumentStorageAdapter>)
            } else {
                None
            }
        });
        let blob_factory: BlobAdapterFactory = Box::new(|name| {
            if name == "filesystem" {
                Some(Arc::new(HealthBlobAdapter(HealthStatus::Healthy)) as Arc<dyn BlobStorageAdapter>)
            } else {
                None
            }
        });

        let result = StorageRouter::from_config(config, &doc_factory, &blob_factory).await;
        assert!(matches!(result, Err(StartupError::CapabilityMismatch(_))));
    }

    #[tokio::test]
    async fn from_config_rejects_unhealthy_adapters() {
        let config = StorageConfig {
            document_adapter: "sqlite".into(),
            blob_adapter: "filesystem".into(),
            timeouts: TimeoutConfig::default(),
            required_capabilities: RequiredCapabilities::default(),
        };

        let doc_factory: DocAdapterFactory = Box::new(|name| {
            if name == "sqlite" {
                Some(Arc::new(HealthDocAdapter(HealthStatus::Unhealthy)) as Arc<dyn DocumentStorageAdapter>)
            } else {
                None
            }
        });
        let blob_factory: BlobAdapterFactory = Box::new(|name| {
            if name == "filesystem" {
                Some(Arc::new(HealthBlobAdapter(HealthStatus::Healthy)) as Arc<dyn BlobStorageAdapter>)
            } else {
                None
            }
        });

        let result = StorageRouter::from_config(config, &doc_factory, &blob_factory).await;
        assert!(matches!(result, Err(StartupError::HealthCheckFailed(_))));
    }
}
