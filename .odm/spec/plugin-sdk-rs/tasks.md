<!--
domain: sdk
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Plugin SDK RS — Tasks

## 1.1 — CorePlugin Trait Definition
> spec: ./brief.md
> depends: none

- Define the `CorePlugin` async trait in `crates/plugin-sdk/src/trait.rs`
- Include all 8 methods: `id`, `display_name`, `version`, `capabilities`, `on_load`, `on_unload`, `routes`, `handle_event`
- Add `#[async_trait]` derive and `Send + Sync` bounds
- Export the trait from `crates/plugin-sdk/src/lib.rs`

**Files:** `crates/plugin-sdk/src/trait.rs`, `crates/plugin-sdk/src/lib.rs`
**Est:** 20 min

## 1.2 — PluginContext Struct
> spec: ./brief.md
> depends: 1.1

- Define `PluginContext` in `crates/plugin-sdk/src/context.rs`
- Include fields for scoped storage handle, config reader, event bus handle, and logger
- Implement accessor methods that enforce capability scoping
- Add documentation comments describing each field's purpose

**Files:** `crates/plugin-sdk/src/context.rs`, `crates/plugin-sdk/src/lib.rs`
**Est:** 25 min

## 1.3 — Capability Enum
> spec: ./brief.md
> depends: none

- Define the `Capability` enum in `crates/plugin-sdk/src/capability.rs`
- Include all 9 variants: `StorageRead`, `StorageWrite`, `HttpOutbound`, `CredentialsRead`, `CredentialsWrite`, `EventsSubscribe`, `EventsEmit`, `ConfigRead`, `Logging`
- Implement `Display` for colon-delimited serialisation (e.g. `storage:read`)
- Implement `FromStr` for deserialisation from manifest strings
- Add serde `Serialize`/`Deserialize` derives

**Files:** `crates/plugin-sdk/src/capability.rs`, `crates/plugin-sdk/src/lib.rs`
**Est:** 20 min

## 1.4 — Route Types and Macro
> spec: ./brief.md
> depends: 1.1

- Define `PluginRoute` struct in `crates/plugin-sdk/src/route.rs` with method, path, and handler fields
- Define `CoreEvent` struct for the event bus
- Implement a `plugin_routes!` convenience macro that reduces route registration boilerplate
- Export all types from `crates/plugin-sdk/src/lib.rs`

**Files:** `crates/plugin-sdk/src/route.rs`, `crates/plugin-sdk/src/lib.rs`
**Est:** 25 min

## 1.5 — Canonical Rust Structs
> spec: ./brief.md
> depends: none

- Define Rust structs for all 7 canonical types in `crates/plugin-sdk/src/types/`
- One file per type: `event.rs`, `task.rs`, `contact.rs`, `note.rs`, `email.rs`, `file.rs`, `credential.rs`
- Add `serde::Serialize` and `serde::Deserialize` derives to each
- Include an `extensions: HashMap<String, serde_json::Value>` field on each struct for plugin-specific data
- Add a `types/mod.rs` that re-exports all types

**Files:** `crates/plugin-sdk/src/types/mod.rs`, `crates/plugin-sdk/src/types/event.rs`, `crates/plugin-sdk/src/types/task.rs`
**Est:** 30 min

## 1.6 — Builder Pattern
> spec: ./brief.md
> depends: 1.1, 1.3, 1.4

- Implement `PluginBuilder` in `crates/plugin-sdk/src/builder.rs`
- Accept plugin ID in `new()`, chain `display_name()`, `version()`, `capability()`, `route()`, `on_load()`, `on_unload()`
- Validate required fields in `build()` and return `Result<impl CorePlugin, BuildError>`
- Define `BuildError` enum with descriptive variants for each missing field

**Files:** `crates/plugin-sdk/src/builder.rs`, `crates/plugin-sdk/src/lib.rs`
**Est:** 25 min

## 1.7 — WASM Build Configuration
> spec: ./brief.md
> depends: 1.1, 1.2, 1.3

- Add `wasm32-wasi` target configuration to `crates/plugin-sdk/Cargo.toml`
- Create a `.cargo/config.toml` with default WASM build settings
- Add CI build step that compiles the SDK to `wasm32-wasi` and verifies the output
- Document the build command in the crate-level doc comment

**Files:** `crates/plugin-sdk/Cargo.toml`, `crates/plugin-sdk/.cargo/config.toml`
**Est:** 20 min
