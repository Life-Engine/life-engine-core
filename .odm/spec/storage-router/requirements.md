<!--
domain: storage-router
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Requirements Document — Storage Router

## Introduction

The Storage Router is the single entry point for all persistent storage operations in Life Engine. It reads `storage.toml` to determine which adapter handles document operations and which handles blob operations, enforces per-operation-class timeouts, validates adapter capabilities at startup, emits structured metrics, and aggregates health from both adapters.

In v1, routing is static: one document adapter and one blob adapter, both compiled into the binary. There is no per-collection routing. The router is consumed by `StorageContext` and is the only component that holds live adapter references.

## Alignment with Product Vision

- **Defence in Depth** — Capability validation at startup ensures the encryption adapter is active before the engine accepts traffic
- **Parse, Don't Validate** — `storage.toml` is fully validated at startup; downstream code trusts the parsed configuration
- **Open/Closed Principle** — The `AdapterRegistry` allows future adapters without modifying the router
- **The Pit of Success** — A single TOML file with clear defaults makes correct configuration easy
- **Principle of Least Privilege** — Adapters only receive their own configuration section; the router does not expose cross-adapter internals

## Requirements

### Requirement 1 — Configuration Parsing

**User Story:** As an operator, I want storage configured via a single TOML file so that I can set adapter choices and timeouts in one place without modifying code.

#### Acceptance Criteria

- 1.1. WHEN the engine starts THEN the router SHALL read and parse `storage.toml` from the engine root directory.
- 1.2. WHEN `storage.toml` is missing THEN the engine SHALL refuse to start and log an error identifying the missing file.
- 1.3. WHEN `storage.toml` contains invalid TOML syntax THEN the engine SHALL refuse to start and log a parse error with line and column information.
- 1.4. WHEN `storage.toml` is valid THEN the router SHALL extract the `[document]` section with `adapter` (required), and adapter-specific fields passed through to the adapter's `init` method.
- 1.5. WHEN `storage.toml` is valid THEN the router SHALL extract the `[blob]` section with `adapter` (required), and adapter-specific fields passed through to the adapter's `init` method.
- 1.6. WHEN `storage.toml` is valid THEN the router SHALL extract the `[timeouts]` section with `document_read_ms`, `document_write_ms`, `blob_read_ms`, and `blob_write_ms` values.
- 1.7. WHEN a required field (`adapter`) is missing from either section THEN the engine SHALL refuse to start and log which section and field is missing.

### Requirement 2 — Adapter Registry and Lookup

**User Story:** As a Core developer, I want a static registry of compiled-in adapters so that the router can look up adapters by name at startup.

#### Acceptance Criteria

- 2.1. WHEN the system initialises THEN the `AdapterRegistry` SHALL contain a `HashMap<String, Box<dyn DocumentStorageAdapter>>` for document adapters and a `HashMap<String, Box<dyn BlobStorageAdapter>>` for blob adapters.
- 2.2. WHEN the registry is populated THEN it SHALL contain the built-in `"sqlite"` document adapter and the built-in `"filesystem"` blob adapter.
- 2.3. WHEN `storage.toml` references an adapter name not present in the registry THEN the engine SHALL refuse to start and log the unknown adapter name and the available adapter names.

### Requirement 3 — Adapter Initialisation and Capability Validation

**User Story:** As an operator, I want the engine to verify adapter capabilities at startup so that misconfiguration (e.g., missing encryption) is caught before any data is served.

#### Acceptance Criteria

- 3.1. WHEN the router starts THEN it SHALL call `init` on the document adapter with the `[document]` configuration section.
- 3.2. WHEN the router starts THEN it SHALL call `init` on the blob adapter with the `[blob]` configuration section.
- 3.3. WHEN `require.capabilities` is configured for a section THEN the router SHALL query the adapter's reported capabilities and compare them to the required list.
- 3.4. WHEN an adapter does not report a required capability THEN the engine SHALL refuse to start and log which capability is missing and which adapter failed the check.
- 3.5. WHEN both adapters report all required capabilities THEN the router SHALL proceed to the migration and health check phases.

### Requirement 4 — Startup Sequence

**User Story:** As a Core developer, I want a well-defined startup sequence so that adapter readiness is verified before the engine accepts traffic.

#### Acceptance Criteria

- 4.1. WHEN the engine starts THEN the router SHALL execute these steps in order: parse config, look up adapters in registry, initialise adapters, validate capabilities, run document adapter migrations, run health checks on both adapters.
- 4.2. WHEN document adapter migrations complete successfully and both adapters report healthy THEN the router SHALL register itself as available for `StorageContext`.
- 4.3. WHEN either adapter reports `Unhealthy` during the startup health check THEN the engine SHALL refuse to start and log which adapter is unhealthy and the reason.
- 4.4. WHEN the startup sequence completes successfully THEN the router SHALL log a summary including adapter names, capability validation results, and health status.

### Requirement 5 — Operation Routing

**User Story:** As a Core developer, I want the router to dispatch document operations to the document adapter and blob operations to the blob adapter so that plugins do not need to know which adapter is active.

#### Acceptance Criteria

- 5.1. WHEN a document operation (`get`, `list`, `count`, `create`, `update`, `partial_update`, `delete`, batch variants, `migrate`) is received THEN the router SHALL delegate it to the configured document adapter.
- 5.2. WHEN a blob operation (`store`, `retrieve`, `exists`, `list`, `metadata`, `copy`, `delete`) is received THEN the router SHALL delegate it to the configured blob adapter.
- 5.3. WHEN the router delegates an operation THEN it SHALL pass through all operation parameters without modification.

### Requirement 6 — Timeout Enforcement

**User Story:** As a Core developer, I want adapter calls wrapped with timeouts so that a hung adapter does not block the engine indefinitely.

#### Acceptance Criteria

- 6.1. WHEN a document read operation (`get`, `list`, `count`) is executed THEN the router SHALL enforce the `document_read_ms` timeout.
- 6.2. WHEN a document write operation (`create`, `update`, `partial_update`, `delete`, batch variants, `migrate`) is executed THEN the router SHALL enforce the `document_write_ms` timeout.
- 6.3. WHEN a blob read operation (`retrieve`, `exists`, `list`, `metadata`) is executed THEN the router SHALL enforce the `blob_read_ms` timeout.
- 6.4. WHEN a blob write operation (`store`, `copy`, `delete`) is executed THEN the router SHALL enforce the `blob_write_ms` timeout.
- 6.5. WHEN an adapter call exceeds its configured timeout THEN the router SHALL return `StorageError::Timeout` to the caller immediately.
- 6.6. WHEN a timeout occurs THEN the router SHALL NOT cancel the in-flight adapter operation (the adapter may still complete independently).

### Requirement 7 — Structured Metrics Logging

**User Story:** As a Core developer, I want every storage operation logged with timing data so that I can diagnose performance issues and audit storage access.

#### Acceptance Criteria

- 7.1. WHEN any adapter operation completes (success or failure) THEN the router SHALL emit a structured log entry.
- 7.2. WHEN the log entry is emitted THEN it SHALL contain the fields: `operation` (method name), `collection` or `key` (target), `duration_ms` (wall-clock time), `status` (`ok` or error variant name), and `adapter` (adapter name).
- 7.3. WHEN an operation times out THEN the log entry SHALL record `status` as `Timeout` and `duration_ms` as the configured timeout value.

### Requirement 8 — Health Aggregation

**User Story:** As an operator, I want a single health status that reflects the worst-case across all adapters so that monitoring is straightforward.

#### Acceptance Criteria

- 8.1. WHEN both adapters report `Healthy` THEN the router SHALL report `Healthy`.
- 8.2. WHEN either adapter reports `Degraded` and neither reports `Unhealthy` THEN the router SHALL report `Degraded`.
- 8.3. WHEN either adapter reports `Unhealthy` THEN the router SHALL report `Unhealthy`.
- 8.4. WHEN the router's health is queried THEN it SHALL include the individual health reports from both adapters in its response.
