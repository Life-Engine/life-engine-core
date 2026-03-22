<!--
domain: sdk
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Spec — Plugin SDK RS

## Contents

- [[#Purpose]]
- [[#Package]]
- [[#Plugin Trait]]
- [[#PipelineMessage Envelope]]
- [[#EngineError Trait]]
- [[#StorageContext]]
- [[#Canonical Collection Types]]
- [[#Helper Macros]]
- [[#Test Utilities]]
- [[#WASM Target]]
- [[#Versioning]]
- [[#Acceptance Criteria]]

## Purpose

This spec defines the Rust SDK for Life Engine plugin authors. The SDK is the single crate a plugin author depends on. It re-exports everything from `packages/types` and `packages/traits`, and provides additional DX features: a `StorageContext` query builder, helper macros for plugin registration, and test utilities.

Internal module developers (building storage backends, transports, or the workflow engine) depend on `packages/types` + `packages/traits` directly. They do not use the SDK.

Reference: [[03 - Projects/Life Engine/Design/Core/Plugins]]

## Package

The SDK is published as the `life-engine-plugin-sdk` crate. Plugin authors add it as their sole Life Engine dependency:

```toml
[dependencies]
life-engine-plugin-sdk = "1"
```

The crate re-exports all public types, traits, and macros needed to author a plugin. It has no dependency on Core internals.

## Plugin Trait

Every plugin implements the `Plugin` trait (defined in `packages/traits`, re-exported via the SDK). This is the single contract between plugins and Core.

```rust
#[async_trait]
pub trait Plugin: Send + Sync {
    fn id(&self) -> &str;
    fn display_name(&self) -> &str;
    fn version(&self) -> &str;
    fn actions(&self) -> Vec<Action>;
    async fn execute(&self, action: &str, input: PipelineMessage) -> Result<PipelineMessage>;
}
```

Method responsibilities:

- **id** — Returns the unique plugin identifier in reverse-domain format (e.g. `com.life-engine.connector-email`).
- **display_name** — Returns a human-readable name for UI and logging.
- **version** — Returns the plugin version string (semver).
- **actions** — Declares the pipeline actions this plugin provides. Each action is a step that can be referenced in a workflow definition.
- **execute** — Called by the workflow engine when a pipeline step invokes one of this plugin's actions. Receives the action name and a `PipelineMessage` as input, returns a `PipelineMessage` as output.

Key differences from the previous architecture:

- No `CorePlugin` trait — it is now simply `Plugin`.
- No `PluginContext` — replaced by `StorageContext` (for storage) and config passed at init.
- No `routes()` method — plugins declare actions, transports handle routing.
- No `capabilities()` method — capabilities are declared in `manifest.toml`, not in Rust code.
- No `on_load()` / `on_unload()` lifecycle methods — the plugin lifecycle is managed by the WASM runtime.
- No `handle_event()` method — event handling is done through actions invoked by workflows.

## PipelineMessage Envelope

The standard envelope for all data flowing through workflows. Defined in `packages/types`, re-exported via the SDK.

```rust
pub struct PipelineMessage {
    pub metadata: MessageMetadata,
    pub payload: TypedPayload,
}

pub struct MessageMetadata {
    pub correlation_id: String,
    pub source: String,
    pub timestamp: String,
    pub auth_context: AuthContext,
}

pub enum TypedPayload {
    Cdm(CdmType),
    Custom(SchemaValidated<Value>),
}
```

- **metadata** — Correlation ID for tracing, source identifier, timestamp, and auth context from the originating request.
- **payload** — Either a CDM type (one of the 7 canonical collection types) or a custom type validated against a JSON Schema declared in the plugin manifest.

Every plugin action receives a `PipelineMessage` and returns a `PipelineMessage`. This standard contract is what makes plugins composable — any plugin's output can be another plugin's input, as long as the schemas are compatible.

## EngineError Trait

The error contract for all module boundaries. Defined in `packages/traits`, re-exported via the SDK.

```rust
pub trait EngineError: std::error::Error {
    fn code(&self) -> &str;
    fn severity(&self) -> Severity;
    fn source_module(&self) -> &str;
}

pub enum Severity {
    Fatal,
    Retryable,
    Warning,
}
```

Plugin authors define their own error types and implement `EngineError`. The workflow engine uses severity to decide behavior:

- **Fatal** — Abort the pipeline, run error handler if configured.
- **Retryable** — Retry the step up to the configured limit, then fail.
- **Warning** — Log and continue.

## StorageContext

Plugins interact with storage through a query builder abstraction provided by the SDK. The `StorageContext` produces `StorageQuery` and `StorageMutation` values. The active `StorageBackend` (injected by the host) translates these to native database queries. Plugins never import database crates directly.

Fluent query API:

```rust
let results = storage
    .query("contacts")
    .where_eq("email", "alice@example.com")
    .order_by("last_name")
    .limit(10)
    .execute()
    .await?;
```

Write API:

```rust
storage
    .insert("contacts", contact_message)
    .await?;

storage
    .update("contacts", id, updated_message)
    .await?;

storage
    .delete("contacts", id)
    .await?;
```

All storage operations are scoped by the plugin's declared capabilities (`storage:read`, `storage:write`). A plugin that calls a storage method it was not granted receives an error.

## Canonical Collection Types

The SDK re-exports the 7 CDM type structs from `packages/types`. These structs have `serde::Serialize` and `serde::Deserialize` derives for JSON serialisation.

The canonical types are:

- **Event** — Calendar events with recurrence support
- **Task** — Actionable items with status and priority
- **Contact** — People with structured name, email, phone, and address fields
- **Note** — Text content with optional tags
- **Email** — Email messages with threading and attachments
- **File** — File metadata with checksum
- **Credential** — Secure credential storage with typed claims

Each struct includes an `extensions: HashMap<String, serde_json::Value>` field for plugin-specific data. Unknown fields encountered during deserialisation are preserved in this map.

## Helper Macros

The SDK provides macros to reduce plugin registration boilerplate:

```rust
use life_engine_plugin_sdk::register_plugin;

register_plugin!(MyPlugin);
```

The `register_plugin!` macro generates the WASM entry-point code required by Extism, wiring the struct's `Plugin` trait implementation to the callable WASM exports. Plugin authors do not write any unsafe code.

## Test Utilities

The SDK ships test utilities so plugin authors can unit-test without a running Core instance:

- **Mock StorageContext** — In-memory implementation of the `StorageContext` API. Supports the same fluent query builder and write methods. Stores data in a `HashMap` for assertion.
- **Mock PipelineMessage builder** — Produces valid `PipelineMessage` instances with sensible defaults for metadata (auto-generated correlation ID, current timestamp, test auth context). Allows overriding any field.

Example usage:

```rust
use life_engine_plugin_sdk::test::{mock_storage, mock_message};

#[tokio::test]
async fn test_fetch_action() {
    let storage = mock_storage();
    let input = mock_message()
        .with_payload(TypedPayload::Cdm(CdmType::Emails(vec![])))
        .build();

    let plugin = MyPlugin::new(storage);
    let output = plugin.execute("fetch", input).await.unwrap();

    assert!(matches!(output.payload, TypedPayload::Cdm(CdmType::Emails(_))));
}
```

## WASM Target

Plugins compile to the `wasm32-wasi` target. The Core runtime uses Extism to load and execute plugin WASM modules.

To build a plugin:

```bash
cargo build --target wasm32-wasi --release
```

The resulting `.wasm` file is the distributable plugin artifact. Core loads it at runtime without requiring the plugin source code or a Rust toolchain on the host machine.

WASI provides the plugin with a sandboxed environment. File system access, network access, and OS access are all mediated by the runtime and constrained by the plugin's declared capabilities in `manifest.toml`.

## Versioning

The SDK is versioned independently from Core.

- **Minor releases** — Additive changes only. New optional trait methods, new CDM fields, new helper functions. No removals or breaking signature changes.
- **Major releases** — Breaking changes permitted. When a new major version ships, the previous major version continues to receive security fixes for 12 months. Core maintains compatibility with both versions during the overlap window.

Plugin authors pin to a major version in their `Cargo.toml` and receive non-breaking updates automatically.

## Acceptance Criteria

A community plugin author can:

1. Add `life-engine-plugin-sdk` as their sole Life Engine dependency
2. Implement the `Plugin` trait on their own struct
3. Declare actions that receive and return `PipelineMessage`
4. Use `StorageContext` to read and write collections via the fluent query builder
5. Use helper macros for registration boilerplate
6. Unit-test with mock `StorageContext` and mock `PipelineMessage` builders
7. Compile the plugin to `wasm32-wasi` target
8. Drop the resulting `.wasm` and `manifest.toml` into Core's plugins directory and have it work
