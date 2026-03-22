<!--
domain: sdk
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Plugin SDK RS — Requirements

## 1 — Plugin Trait

- **1.1** — WHEN a plugin author implements `Plugin` on a struct, THEN the compiler SHALL enforce that all five methods (`id`, `display_name`, `version`, `actions`, `execute`) are implemented.
- **1.2** — WHEN `id()` is called, THEN it SHALL return a non-empty string in reverse-domain format (e.g. `com.life-engine.connector-email`).
- **1.3** — WHEN `display_name()` is called, THEN it SHALL return a human-readable name for UI and logging.
- **1.4** — WHEN `version()` is called, THEN it SHALL return a valid semver string.
- **1.5** — WHEN `actions()` is called, THEN it SHALL return a `Vec<Action>` declaring all pipeline actions the plugin provides.
- **1.6** — WHEN `execute(action, input)` is called with an action name and a `PipelineMessage`, THEN it SHALL return a `Result<PipelineMessage>` containing the output or an error implementing `EngineError`.

## 2 — Re-exports from packages/types

- **2.1** — WHEN the SDK is compiled, THEN it SHALL re-export all 7 CDM type structs: Event, Task, Contact, Email, Note, File, Credential.
- **2.2** — WHEN the SDK is compiled, THEN it SHALL re-export `PipelineMessage`, `MessageMetadata`, and `TypedPayload`.
- **2.3** — WHEN a CDM struct is serialised with serde, THEN it SHALL produce valid JSON matching the platform schema.
- **2.4** — WHEN a CDM struct is deserialised from JSON, THEN unknown fields SHALL be preserved in an `extensions` map.
- **2.5** — WHEN a CDM struct includes an `id` field, THEN it SHALL be typed as `String` and required.
- **2.6** — WHEN a CDM struct includes timestamp fields (`_created`, `_updated`), THEN they SHALL be ISO 8601 / RFC 3339 strings.

## 3 — Re-exports from packages/traits

- **3.1** — WHEN the SDK is compiled, THEN it SHALL re-export the `Plugin` trait from `packages/traits`.
- **3.2** — WHEN the SDK is compiled, THEN it SHALL re-export the `EngineError` trait from `packages/traits`.
- **3.3** — WHEN a plugin author imports from the SDK, THEN they SHALL NOT need to add `packages/types` or `packages/traits` as direct dependencies.

## 4 — PipelineMessage Envelope

- **4.1** — WHEN a `PipelineMessage` is constructed, THEN it SHALL contain `metadata` (`MessageMetadata`) and `payload` (`TypedPayload`).
- **4.2** — WHEN `MessageMetadata` is constructed, THEN it SHALL include a correlation ID, source, timestamp, and auth context.
- **4.3** — WHEN `TypedPayload` is constructed, THEN it SHALL be either `Cdm(CdmType)` for one of the 7 canonical types or `Custom(SchemaValidated<Value>)` for plugin-defined types.
- **4.4** — WHEN a `PipelineMessage` is passed between pipeline steps, THEN metadata SHALL be preserved and payload SHALL be the output of the previous step.

## 5 — EngineError Trait

- **5.1** — WHEN a plugin defines an error type, THEN it SHALL implement the `EngineError` trait.
- **5.2** — WHEN `code()` is called on an error, THEN it SHALL return a module-scoped error code string (e.g. `EMAIL_001`).
- **5.3** — WHEN `severity()` is called on an error, THEN it SHALL return one of `Fatal`, `Retryable`, or `Warning`.
- **5.4** — WHEN `source_module()` is called on an error, THEN it SHALL return the plugin or module identifier.
- **5.5** — WHEN the workflow engine receives a `Fatal` error, THEN it SHALL abort the pipeline. WHEN it receives `Retryable`, THEN it SHALL retry up to the configured limit. WHEN it receives `Warning`, THEN it SHALL log and continue.

## 6 — StorageContext

- **6.1** — WHEN a plugin uses `StorageContext`, THEN it SHALL provide a fluent query builder API for reading collections.
- **6.2** — WHEN `storage.query("contacts")` is called, THEN it SHALL begin a query builder chain scoped to the named collection.
- **6.3** — WHEN `.where_eq(field, value)` is called on a query builder, THEN it SHALL add an equality filter.
- **6.4** — WHEN `.order_by(field)` is called on a query builder, THEN it SHALL set the sort order.
- **6.5** — WHEN `.limit(n)` is called on a query builder, THEN it SHALL cap the result count.
- **6.6** — WHEN `.execute()` is called on a query builder, THEN it SHALL return a `Result<Vec<PipelineMessage>>`.
- **6.7** — WHEN a plugin performs a write via `StorageContext`, THEN it SHALL produce a `StorageMutation` value that the active `StorageBackend` translates to a native operation.
- **6.8** — WHEN a plugin uses `StorageContext`, THEN access SHALL be scoped to the plugin's declared `storage:read` or `storage:write` capabilities.

## 7 — Helper Macros

- **7.1** — WHEN a plugin author uses the registration macro, THEN it SHALL generate the WASM entry-point boilerplate needed by Extism.
- **7.2** — WHEN the registration macro is applied to a struct implementing `Plugin`, THEN the generated code SHALL expose the struct's `execute` method as the WASM callable entry point.
- **7.3** — WHEN the registration macro is used, THEN it SHALL NOT require the plugin author to write any unsafe code.

## 8 — Test Utilities

- **8.1** — WHEN a plugin author uses the mock `StorageContext`, THEN it SHALL behave identically to the real `StorageContext` API but store data in memory.
- **8.2** — WHEN a plugin author uses the mock `PipelineMessage` builder, THEN it SHALL produce valid `PipelineMessage` instances with sensible defaults for metadata.
- **8.3** — WHEN a plugin author runs tests with mock utilities, THEN no running Core instance or database SHALL be required.

## 9 — WASM Target

- **9.1** — WHEN the SDK crate is compiled with `--target wasm32-wasi`, THEN it SHALL produce a valid WASM module with no host-specific dependencies.
- **9.2** — WHEN the resulting `.wasm` binary is loaded by Extism, THEN it SHALL expose the expected entry points for action dispatch.
- **9.3** — WHEN the WASM module executes, THEN it SHALL have no direct filesystem, network, or OS access outside of host-provided functions.

## 10 — Versioning

- **10.1** — WHEN the SDK ships a minor release, THEN it SHALL contain only additive changes; no removals or breaking signature changes.
- **10.2** — WHEN a new major release ships, THEN the previous major version SHALL continue to receive security fixes for 12 months.
- **10.3** — WHEN Core loads a plugin, THEN it SHALL check the SDK version declared in the WASM manifest and reject plugins built against unsupported major versions.
- **10.4** — WHEN the SDK version changes, THEN it SHALL NOT require a corresponding Core version change. The SDK is versioned independently.
