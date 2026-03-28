<!--
domain: storage-router
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Technical Design — Storage Router

## Purpose

This document defines the internal architecture, data structures, and conventions for the Storage Router. The router is the single dispatch layer between `StorageContext` (the plugin-facing query builder) and the underlying document and blob adapters. It owns configuration parsing, adapter lifecycle, timeout enforcement, metrics emission, and health aggregation.

## Crate Location

The router lives in `packages/storage-router/src/`. It depends on:

- `packages/traits` — for `DocumentStorageAdapter`, `BlobStorageAdapter`, and `HealthReport`
- `packages/types` — for `StorageError`, `PipelineMessage`, and storage operation types
- `toml` — for configuration parsing
- `tokio` — for async timeout wrapping
- `tracing` — for structured logging

## Configuration Model

The router parses `storage.toml` into the following Rust structs:

```rust
#[derive(Debug, Deserialize)]
pub struct StorageConfig {
    pub document: AdapterConfig,
    pub blob: AdapterConfig,
    pub timeouts: TimeoutConfig,
}

#[derive(Debug, Deserialize)]
pub struct AdapterConfig {
    pub adapter: String,
    #[serde(flatten)]
    pub settings: toml::Value,
    #[serde(default)]
    pub require: RequireConfig,
}

#[derive(Debug, Default, Deserialize)]
pub struct RequireConfig {
    #[serde(default)]
    pub capabilities: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct TimeoutConfig {
    pub document_read_ms: u64,
    pub document_write_ms: u64,
    pub blob_read_ms: u64,
    pub blob_write_ms: u64,
}
```

The `AdapterConfig.settings` field uses `serde(flatten)` to capture all adapter-specific keys (e.g., `path`, `encryption_key_file`) as an untyped `toml::Value`. This value is passed through to the adapter's `init` method, which deserialises its own fields.

## Adapter Registry

The registry is a static map populated at compile time. In v1, it contains two entries.

```rust
pub struct AdapterRegistry {
    document_adapters: HashMap<String, Box<dyn DocumentStorageAdapter>>,
    blob_adapters: HashMap<String, Box<dyn BlobStorageAdapter>>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            document_adapters: HashMap::new(),
            blob_adapters: HashMap::new(),
        };
        registry.document_adapters.insert(
            "sqlite".to_string(),
            Box::new(SqliteDocumentAdapter::new()),
        );
        registry.blob_adapters.insert(
            "filesystem".to_string(),
            Box::new(FilesystemBlobAdapter::new()),
        );
        registry
    }

    pub fn take_document_adapter(
        &mut self,
        name: &str,
    ) -> Result<Box<dyn DocumentStorageAdapter>, StorageError> {
        self.document_adapters
            .remove(name)
            .ok_or_else(|| StorageError::UnknownAdapter {
                name: name.to_string(),
                available: self.document_adapters.keys().cloned().collect(),
            })
    }

    pub fn take_blob_adapter(
        &mut self,
        name: &str,
    ) -> Result<Box<dyn BlobStorageAdapter>, StorageError> {
        self.blob_adapters
            .remove(name)
            .ok_or_else(|| StorageError::UnknownAdapter {
                name: name.to_string(),
                available: self.blob_adapters.keys().cloned().collect(),
            })
    }
}
```

Adapters are taken (moved) from the registry during startup. The registry is consumed; no adapter is shared or cloned.

## Storage Router Struct

```rust
pub struct StorageRouter {
    document_adapter: Box<dyn DocumentStorageAdapter>,
    blob_adapter: Box<dyn BlobStorageAdapter>,
    document_adapter_name: String,
    blob_adapter_name: String,
    timeouts: TimeoutConfig,
}
```

The router holds owned adapter instances plus the adapter names (for metrics logging) and the parsed timeout configuration.

## Startup Sequence

The `StorageRouter::start` function implements the full startup sequence:

```rust
impl StorageRouter {
    pub async fn start(
        config_path: &Path,
        mut registry: AdapterRegistry,
    ) -> Result<Self, StorageError> {
        // 1. Parse storage.toml
        let config_str = std::fs::read_to_string(config_path)
            .map_err(|e| StorageError::ConfigMissing {
                path: config_path.to_path_buf(),
                source: e,
            })?;
        let config: StorageConfig = toml::from_str(&config_str)
            .map_err(|e| StorageError::ConfigParse { source: e })?;

        // 2. Look up adapters in registry
        let mut doc_adapter = registry
            .take_document_adapter(&config.document.adapter)?;
        let mut blob_adapter = registry
            .take_blob_adapter(&config.blob.adapter)?;

        // 3. Initialise adapters
        doc_adapter.init(&config.document.settings).await?;
        blob_adapter.init(&config.blob.settings).await?;

        // 4. Validate capabilities
        let doc_caps = doc_adapter.capabilities();
        for required in &config.document.require.capabilities {
            if !doc_caps.contains(required) {
                return Err(StorageError::MissingCapability {
                    adapter: config.document.adapter.clone(),
                    capability: required.clone(),
                });
            }
        }
        let blob_caps = blob_adapter.capabilities();
        for required in &config.blob.require.capabilities {
            if !blob_caps.contains(required) {
                return Err(StorageError::MissingCapability {
                    adapter: config.blob.adapter.clone(),
                    capability: required.clone(),
                });
            }
        }

        // 5. Run migrations on document adapter
        doc_adapter.migrate().await?;

        // 6. Health check
        let doc_health = doc_adapter.health().await;
        let blob_health = blob_adapter.health().await;
        if doc_health.status == HealthStatus::Unhealthy {
            return Err(StorageError::AdapterUnhealthy {
                adapter: config.document.adapter.clone(),
                reason: doc_health.message,
            });
        }
        if blob_health.status == HealthStatus::Unhealthy {
            return Err(StorageError::AdapterUnhealthy {
                adapter: config.blob.adapter.clone(),
                reason: blob_health.message,
            });
        }

        tracing::info!(
            document_adapter = %config.document.adapter,
            blob_adapter = %config.blob.adapter,
            "Storage router started successfully"
        );

        Ok(Self {
            document_adapter: doc_adapter,
            blob_adapter: blob_adapter,
            document_adapter_name: config.document.adapter,
            blob_adapter_name: config.blob.adapter,
            timeouts: config.timeouts,
        })
    }
}
```

## Timeout Wrapping

Every adapter call is wrapped with `tokio::time::timeout`. The timeout duration is selected based on the operation class.

```rust
impl StorageRouter {
    async fn with_timeout<F, T>(
        &self,
        timeout_ms: u64,
        operation: &str,
        target: &str,
        adapter_name: &str,
        fut: F,
    ) -> Result<T, StorageError>
    where
        F: Future<Output = Result<T, StorageError>>,
    {
        let start = std::time::Instant::now();
        let duration = std::time::Duration::from_millis(timeout_ms);

        match tokio::time::timeout(duration, fut).await {
            Ok(result) => {
                let elapsed = start.elapsed().as_millis() as u64;
                let status = match &result {
                    Ok(_) => "ok".to_string(),
                    Err(e) => e.variant_name().to_string(),
                };
                tracing::info!(
                    operation = operation,
                    target = target,
                    duration_ms = elapsed,
                    status = %status,
                    adapter = adapter_name,
                    "storage operation complete"
                );
                result
            }
            Err(_) => {
                tracing::warn!(
                    operation = operation,
                    target = target,
                    duration_ms = timeout_ms,
                    status = "Timeout",
                    adapter = adapter_name,
                    "storage operation timed out"
                );
                Err(StorageError::Timeout {
                    operation: operation.to_string(),
                    timeout_ms,
                })
            }
        }
    }
}
```

Operation class mapping:

- **Document reads** (`get`, `list`, `count`) use `timeouts.document_read_ms`
- **Document writes** (`create`, `update`, `partial_update`, `delete`, batch variants, `migrate`) use `timeouts.document_write_ms`
- **Blob reads** (`retrieve`, `exists`, `list`, `metadata`) use `timeouts.blob_read_ms`
- **Blob writes** (`store`, `copy`, `delete`) use `timeouts.blob_write_ms`

## Routing Dispatch

Each public method on `StorageRouter` delegates to the correct adapter through `with_timeout`. Example for a document `get`:

```rust
impl StorageRouter {
    pub async fn document_get(
        &self,
        collection: &str,
        id: &str,
    ) -> Result<PipelineMessage, StorageError> {
        self.with_timeout(
            self.timeouts.document_read_ms,
            "get",
            collection,
            &self.document_adapter_name,
            self.document_adapter.get(collection, id),
        )
        .await
    }

    pub async fn blob_store(
        &self,
        key: &str,
        data: &[u8],
        metadata: &BlobMetadata,
    ) -> Result<(), StorageError> {
        self.with_timeout(
            self.timeouts.blob_write_ms,
            "store",
            key,
            &self.blob_adapter_name,
            self.blob_adapter.store(key, data, metadata),
        )
        .await
    }
}
```

All other operations follow the same pattern: select timeout class, call `with_timeout`, pass through parameters unchanged.

## Health Aggregation

```rust
impl StorageRouter {
    pub async fn health(&self) -> RouterHealthReport {
        let doc_health = self.document_adapter.health().await;
        let blob_health = self.blob_adapter.health().await;

        let aggregate_status = match (&doc_health.status, &blob_health.status) {
            (HealthStatus::Healthy, HealthStatus::Healthy) => HealthStatus::Healthy,
            (HealthStatus::Unhealthy, _) | (_, HealthStatus::Unhealthy) => {
                HealthStatus::Unhealthy
            }
            _ => HealthStatus::Degraded,
        };

        RouterHealthReport {
            status: aggregate_status,
            document: doc_health,
            blob: blob_health,
        }
    }
}

pub struct RouterHealthReport {
    pub status: HealthStatus,
    pub document: HealthReport,
    pub blob: HealthReport,
}
```

## Error Types

The router adds these variants to `StorageError`:

- **`ConfigMissing`** — `storage.toml` not found at expected path
- **`ConfigParse`** — TOML syntax error with line/column detail
- **`UnknownAdapter`** — adapter name not in registry, includes available names
- **`MissingCapability`** — adapter does not report a required capability
- **`AdapterUnhealthy`** — adapter reported `Unhealthy` during startup health check
- **`Timeout`** — adapter call exceeded configured timeout, includes operation name and timeout value

## File Layout

All router code lives under `packages/storage-router/src/`:

- **`lib.rs`** — Re-exports `StorageRouter`, `StorageConfig`, `AdapterRegistry`, `RouterHealthReport`
- **`config.rs`** — `StorageConfig`, `AdapterConfig`, `RequireConfig`, `TimeoutConfig` structs and parsing logic
- **`registry.rs`** — `AdapterRegistry` struct and built-in adapter registration
- **`router.rs`** — `StorageRouter` struct, startup sequence, routing dispatch, timeout wrapping, metrics emission
- **`health.rs`** — `RouterHealthReport` struct and aggregation logic
- **`error.rs`** — Router-specific `StorageError` variants

## Conventions

- All public types are re-exported from `lib.rs`
- All adapter calls go through `with_timeout` — no direct adapter calls from outside the router
- Structured logging uses `tracing::info!` for successful operations and `tracing::warn!` for timeouts and errors
- Configuration errors cause the engine to refuse to start with a descriptive log message; they never panic
- The router is `Send + Sync` and is shared across async tasks via `Arc<StorageRouter>`
