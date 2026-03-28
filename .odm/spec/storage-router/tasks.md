<!--
domain: storage-router
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Implementation Plan — Storage Router

## Task Overview

This plan implements the Storage Router across `packages/storage-router/`. Work begins with configuration parsing and error types, then builds the adapter registry, followed by the router itself with timeout wrapping, metrics emission, and health aggregation. Each task targets a narrow slice of functionality that can be tested in isolation.

**Progress:** 0 / 12 tasks complete

## Steering Document Compliance

- Configuration validation at startup follows Parse, Don't Validate — downstream code trusts the parsed config
- Capability checking before accepting traffic follows Defence in Depth
- `AdapterRegistry` follows Open/Closed Principle — new adapters register without modifying the router
- Single TOML file with clear structure follows The Pit of Success
- Adapters receive only their own config section, following Principle of Least Privilege

## Atomic Task Requirements

- **File Scope:** 1-3 related files maximum
- **Time Boxing:** 15-30 minutes per task
- **Single Purpose:** one testable outcome per task
- **Specific Files:** exact file paths specified
- **Agent-Friendly:** clear input/output, minimal context switching

---

## 1.1 — Error Types and Configuration Structs

> spec: ./brief.md

- [ ] Define router-specific StorageError variants
  <!-- file: packages/storage-router/src/error.rs -->
  <!-- purpose: Define ConfigMissing, ConfigParse, UnknownAdapter, MissingCapability, AdapterUnhealthy, and Timeout error variants -->
  <!-- requirements: 1.2, 1.3, 1.7, 2.3, 3.4, 4.3, 6.5 -->

- [ ] Define StorageConfig, AdapterConfig, RequireConfig, and TimeoutConfig structs
  <!-- file: packages/storage-router/src/config.rs -->
  <!-- purpose: Define serde-deserializable structs for storage.toml with AdapterConfig using serde(flatten) for adapter-specific settings -->
  <!-- requirements: 1.1, 1.4, 1.5, 1.6, 1.7 -->

- [ ] Add configuration parsing function with validation
  <!-- file: packages/storage-router/src/config.rs -->
  <!-- purpose: Implement parse_config(path) that reads storage.toml, deserialises to StorageConfig, and returns descriptive errors for missing file or invalid syntax -->
  <!-- requirements: 1.1, 1.2, 1.3, 1.7 -->

## 1.2 — Adapter Registry

> spec: ./brief.md

- [ ] Define AdapterRegistry struct with built-in adapter registration
  <!-- file: packages/storage-router/src/registry.rs -->
  <!-- purpose: Implement AdapterRegistry with HashMap-based document and blob adapter storage, pre-populated with sqlite and filesystem adapters -->
  <!-- requirements: 2.1, 2.2 -->

- [ ] Implement take_document_adapter and take_blob_adapter methods
  <!-- file: packages/storage-router/src/registry.rs -->
  <!-- purpose: Implement methods that move adapters out of the registry by name, returning UnknownAdapter error with available names on miss -->
  <!-- requirements: 2.1, 2.3 -->

## 1.3 — Router Core and Startup Sequence

> spec: ./brief.md

- [ ] Define StorageRouter struct and start method (parse, lookup, init)
  <!-- file: packages/storage-router/src/router.rs -->
  <!-- purpose: Implement StorageRouter::start covering steps 1-3 of startup: parse config, look up adapters in registry, initialise adapters with config -->
  <!-- requirements: 4.1, 3.1, 3.2 -->

- [ ] Add capability validation and health check to start method
  <!-- file: packages/storage-router/src/router.rs -->
  <!-- purpose: Extend StorageRouter::start with steps 4-7: validate capabilities, run migrations, run health checks, refuse to start on failure -->
  <!-- requirements: 3.3, 3.4, 3.5, 4.1, 4.2, 4.3, 4.4 -->

## 2.1 — Timeout Wrapping and Metrics

> spec: ./brief.md

- [ ] Implement with_timeout helper with structured logging
  <!-- file: packages/storage-router/src/router.rs -->
  <!-- purpose: Implement generic with_timeout method that wraps a future with tokio::time::timeout and emits tracing structured log with operation, target, duration_ms, status, adapter -->
  <!-- requirements: 6.5, 6.6, 7.1, 7.2, 7.3 -->

- [ ] Implement document routing methods with timeout wrapping
  <!-- file: packages/storage-router/src/router.rs -->
  <!-- purpose: Add document_get, document_list, document_count, document_create, document_update, document_partial_update, document_delete, document_migrate methods that delegate to document adapter through with_timeout -->
  <!-- requirements: 5.1, 5.3, 6.1, 6.2 -->

- [ ] Implement blob routing methods with timeout wrapping
  <!-- file: packages/storage-router/src/router.rs -->
  <!-- purpose: Add blob_store, blob_retrieve, blob_exists, blob_list, blob_metadata, blob_copy, blob_delete methods that delegate to blob adapter through with_timeout -->
  <!-- requirements: 5.2, 5.3, 6.3, 6.4 -->

## 2.2 — Health Aggregation and Module Exports

> spec: ./brief.md

- [ ] Implement RouterHealthReport and health aggregation
  <!-- file: packages/storage-router/src/health.rs -->
  <!-- purpose: Define RouterHealthReport struct and implement health() method with worst-case aggregation across document and blob adapters -->
  <!-- requirements: 8.1, 8.2, 8.3, 8.4 -->

- [ ] Create lib.rs with public re-exports
  <!-- file: packages/storage-router/src/lib.rs -->
  <!-- purpose: Re-export StorageRouter, StorageConfig, AdapterRegistry, RouterHealthReport, and error types as the crate's public API -->
  <!-- requirements: 4.2 -->
