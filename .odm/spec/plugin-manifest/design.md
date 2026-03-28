<!--
domain: plugin-manifest
status: draft
tier: 1
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Plugin Manifest Design

## Overview

The plugin manifest is a TOML file (`manifest.toml`) at the root of each plugin directory. Core parses and validates the manifest during the Load phase of the plugin lifecycle. The manifest drives capability enforcement, collection provisioning, event wiring, and trust model decisions.

## Manifest Structure

The manifest contains the following top-level sections. No other top-level sections are permitted.

- `[plugin]` — Plugin identity (required)
- `[actions.<name>]` — Action declarations (at least one required)
- `[capabilities]` — Host function access requests (optional, deny-by-default)
- `[collections.<name>]` — Collection declarations (optional)
- `[events]` — Event emit and subscribe declarations (optional)
- `[config]` — Configuration schema reference (optional)

## Data Structures

### PluginManifest

The top-level deserialized manifest struct:

```rust
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    pub plugin: PluginIdentity,
    #[serde(default)]
    pub actions: HashMap<String, ActionDecl>,
    #[serde(default)]
    pub capabilities: CapabilityDecl,
    #[serde(default)]
    pub collections: HashMap<String, CollectionDecl>,
    #[serde(default)]
    pub events: EventDecl,
    #[serde(default)]
    pub config: Option<ConfigDecl>,
}
```

The `#[serde(deny_unknown_fields)]` attribute ensures unknown top-level sections cause a parse error (Requirement 8.1).

### PluginIdentity

```rust
#[derive(Debug, Deserialize)]
pub struct PluginIdentity {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
}
```

Validation rules applied after deserialization:

- `id` must match the regex `^[a-z][a-z0-9]*(-[a-z0-9]+)*$` (kebab-case)
- `version` must match semver `^[0-9]+\.[0-9]+\.[0-9]+$`
- `id`, `name`, `version` must be non-empty

### ActionDecl

```rust
#[derive(Debug, Deserialize)]
pub struct ActionDecl {
    pub description: String,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}
```

The action name is the key in the `[actions.<name>]` TOML section. After WASM module instantiation, Core verifies each action name corresponds to an exported function.

### CapabilityDecl

```rust
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct CapabilityDecl {
    pub storage_doc: Vec<StorageOp>,
    pub storage_blob: Vec<StorageOp>,
    pub http_outbound: bool,
    pub events_emit: bool,
    pub events_subscribe: bool,
    pub config_read: bool,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StorageOp {
    Read,
    Write,
    Delete,
}
```

Omitted fields default to empty/false (deny-by-default). At runtime, Core checks the plugin's `CapabilityDecl` before dispatching any host function call.

### CollectionDecl

```rust
#[derive(Debug, Deserialize)]
pub struct CollectionDecl {
    pub schema: String,
    pub access: CollectionAccess,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub extension_schema: Option<String>,
    #[serde(default)]
    pub extension_indexes: Vec<String>,
    #[serde(default)]
    pub indexes: Vec<String>,
    #[serde(default)]
    pub strict: bool,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum CollectionAccess {
    Read,
    Write,
    ReadWrite,
}
```

Schema resolution:

- If `schema` starts with `cdm:`, Core resolves it to the matching SDK-shipped schema file (e.g., `cdm:emails` resolves to `schemas/cdm/emails.schema.json`)
- Otherwise, Core resolves `schema` as a path relative to the plugin directory

Extension field naming:

- Each entry in `extensions` must match `ext.<plugin-id>.<field>` where `<plugin-id>` matches the declaring plugin's `id`
- Core rejects manifests where extension fields reference a different plugin's namespace

### EventDecl

```rust
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct EventDecl {
    pub emit: Option<EventList>,
    pub subscribe: Option<EventList>,
}

#[derive(Debug, Deserialize)]
pub struct EventList {
    pub events: Vec<String>,
}
```

Event name validation:

- Each event name must contain at least two dots (e.g., `connector-email.fetch.completed`)
- Emit event names must start with the declaring plugin's `id` prefix

### ConfigDecl

```rust
#[derive(Debug, Deserialize)]
pub struct ConfigDecl {
    pub schema: String,
}
```

The `schema` path is resolved relative to the plugin directory and must point to a valid JSON Schema file.

## Validation Pipeline

Manifest validation runs in a defined order during the Load phase:

1. **TOML parse** — Deserialize the file. Reject on syntax errors or unknown top-level fields.
2. **Identity validation** — Check required fields, kebab-case `id`, semver `version`.
3. **Duplicate check** — Verify `id` is not already registered in the plugin registry.
4. **Action validation** — Ensure at least one action exists and each has a `description`.
5. **Capability-section cross-check** — If `[events.emit]` is present, `events_emit` must be `true`. If `[events.subscribe]` is present, `events_subscribe` must be `true`. If `[config]` is present, `config_read` must be `true`.
6. **Collection validation** — Verify each collection has `schema` and `access`. Resolve schema paths. Validate extension field namespacing.
7. **Event name validation** — Check all event names follow the dot-separated convention.
8. **Config schema resolution** — Resolve and validate the config schema path.
9. **Trust model enforcement** — Apply first-party/third-party rules to grant or deny capabilities.
10. **WASM export check** — After module instantiation, verify each declared action maps to an exported function.

If any step fails, Core aborts loading for that plugin, logs the error with the plugin id and field path, and continues loading other plugins.

## Trust Model

Plugin trust classification:

- **First-party** — Plugins shipped with Core or placed in the first-party plugin directory. All declared capabilities are auto-granted.
- **Third-party** — Plugins installed from external sources. Each declared capability must appear in an approval list in Core's configuration file.

Core's configuration file contains an approval section:

```toml
[plugin_approvals."my-third-party-plugin"]
storage_doc = ["read"]
http_outbound = true
```

If the plugin's manifest declares capabilities beyond what the approval section grants, the plugin fails to load with a clear error listing the unapproved capabilities.

## Example Manifest

A complete example manifest for an email connector plugin:

```toml
[plugin]
id = "connector-email"
name = "Email Connector"
version = "1.0.0"
description = "IMAP/SMTP email sync"
author = "Life Engine"
license = "MIT"

[actions.fetch]
description = "Fetch new emails from IMAP"
timeout_ms = 30000

[actions.send]
description = "Send email via SMTP"
timeout_ms = 10000

[capabilities]
storage_doc = ["read", "write"]
storage_blob = ["read", "write"]
http_outbound = true
events_emit = true
events_subscribe = true
config_read = true

[collections.emails]
schema = "cdm:emails"
access = "read-write"
extensions = ["ext.connector-email.thread_id", "ext.connector-email.imap_uid"]
extension_schema = "schemas/email-extensions.json"
extension_indexes = ["ext.connector-email.thread_id"]

[collections.email_sync_state]
schema = "schemas/sync-state.json"
indexes = ["last_sync"]
strict = true

[events.emit]
events = [
    "connector-email.fetch.completed",
    "connector-email.fetch.failed",
    "connector-email.send.completed",
]

[events.subscribe]
events = ["system.startup"]

[config]
schema = "schemas/config.json"
```

## File Conventions

- The manifest file must be named `manifest.toml` and placed at the root of the plugin directory
- Schema files referenced by relative paths live within the plugin directory (commonly in a `schemas/` subdirectory)
- Plugin directories are located in a configured plugin root directory scanned by Core at startup

## Error Reporting

Validation errors use a structured format including:

- **plugin_id** — The plugin's declared `id`, or `"<unknown>"` if the identity section failed to parse
- **field_path** — Dot-separated path to the offending field (e.g., `actions.fetch.description`)
- **error_kind** — One of: `MissingField`, `InvalidFormat`, `SchemaNotFound`, `DuplicateId`, `UnknownSection`, `CapabilityMismatch`, `ExportMismatch`, `UnapprovedCapability`, `ParseError`
- **message** — Human-readable description of the error
