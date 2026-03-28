---
title: Plugin Manifest
type: adr
created: 2026-03-28
status: draft
tags:
  - architecture
  - plugin-sdk
  - manifest
  - core
---

# Plugin Manifest

## Overview

Every plugin ships a `manifest.toml` alongside its WASM binary. The manifest declares the plugin's identity, actions, capabilities, collection access, events, configuration schema, and route bindings. Core reads manifests at startup and uses them to grant capabilities, register actions, and configure storage.

The manifest is the single source of truth for what a plugin can do. Anything not declared in the manifest is denied at runtime.

## Full Manifest Example

```toml
[plugin]
id = "connector-email"
name = "Email Connector"
version = "1.0.0"
sdk_version = "0.1.0"
description = "Syncs email via IMAP/SMTP and normalises to CDM schemas"
author = "Life Engine"
license = "MIT"
trust = "first-party"

[actions.fetch]
description = "Fetch new emails from configured IMAP server"
timeout_ms = 30000

[actions.send]
description = "Send an email via SMTP"
timeout_ms = 10000

[capabilities]
storage_doc = ["read", "write"]
storage_blob = ["read", "write"]
http_outbound = true
events_emit = true
events_subscribe = false
config_read = true

[collections.emails]
schema = "cdm:emails"
access = "read-write"
extensions = ["ext.connector-email.imap_uid", "ext.connector-email.folder"]
extension_schema = "schemas/email-extensions.json"
extension_indexes = ["ext.connector-email.imap_uid"]

[collections.contacts]
schema = "cdm:contacts"
access = "read"

[collections.sync_state]
schema = "schemas/sync-state.json"
indexes = ["last_sync"]
strict = true

[events.emit]
names = ["connector-email.fetch.completed", "connector-email.fetch.failed"]

[config]
schema = "schemas/config.json"
```

## Sections

### `[plugin]` — Identity

Required fields:

- **id** — Unique identifier. Kebab-case. Used for collection namespacing, blob key prefixing, extension field namespacing, event prefixing, and capability scoping. Must be globally unique across all loaded plugins.
- **name** — Human-readable display name.
- **version** — Semver string. Used for schema evolution and upgrade paths.
- **sdk_version** — The SDK version this plugin was built against. Core validates compatibility at load time.

Optional fields:

- **description** — Short description of the plugin's purpose.
- **author** — Plugin author name or organisation.
- **license** — SPDX license identifier.
- **trust** — One of `first-party` or `third-party`. First-party plugins have capabilities auto-granted. Third-party plugins require explicit approval in Core config. Default is `third-party`.

### `[actions.<name>]` — Plugin Actions

Each action is a named entry point that the workflow engine can invoke. Actions map to exported WASM functions.

- **description** — Human-readable description of what the action does. Optional but recommended.
- **timeout_ms** — Maximum execution time for a single invocation. If the action exceeds this, the Extism host cancels it and returns an error to the workflow engine. Default is `5000` (5 seconds).

Action names are kebab-case and must be unique within the plugin. A plugin can declare any number of actions. The WASM binary must export a function matching each declared action name.

### `[capabilities]` — Capability Declarations

Declare what the plugin needs access to. All capabilities are deny-by-default.

- **storage_doc** — List of document storage operations: `read`, `write`, `delete`. Maps to `storage:doc:read`, `storage:doc:write`, `storage:doc:delete`.
- **storage_blob** — List of blob storage operations: `read`, `write`, `delete`. Maps to `storage:blob:read`, `storage:blob:write`, `storage:blob:delete`.
- **http_outbound** — Boolean. Grants the `http:outbound` capability for making external HTTP requests.
- **events_emit** — Boolean. Grants `events:emit` for publishing events to the event bus.
- **events_subscribe** — Boolean. Reserved for future use. Plugins subscribe to events via workflow triggers, not directly.
- **config_read** — Boolean. Grants `config:read` for reading the plugin's own configuration section.

Capabilities that are not declared default to denied. A plugin that only reads documents needs only `storage_doc = ["read"]`.

### `[collections.<name>]` — Collection Declarations

Declare which collections the plugin accesses and how. Every collection a plugin touches must be declared here.

Two types of collections:

- **Shared collections** — Use a CDM or custom schema. Multiple plugins can declare access to the same shared collection. The collection name is used as-is.
- **Plugin-scoped collections** — Private to the plugin. Stored as `{plugin_id}.{collection_name}` to prevent collisions. A collection is plugin-scoped if no other plugin declares the same name.

Collection fields:

- **schema** — Path to a JSON Schema file (relative to plugin directory), or `cdm:<name>` to adopt a CDM recommended schema. Optional. Omit for schemaless collections.
- **access** — One of `read`, `write`, or `read-write`. Determines which storage capabilities are scoped to this collection.
- **indexes** — List of field paths the adapter should index. Optional.
- **strict** — Boolean. If `true`, reject documents with fields not in the schema. Default `false`.
- **extensions** — List of extension field paths this plugin adds to shared collections. Uses the `ext.{plugin_id}.{field_name}` format.
- **extension_schema** — Path to a JSON Schema file for the extension fields. Optional.
- **extension_indexes** — Extension fields the adapter should index. Optional.

### `[events.emit]` — Event Declarations

Declare which events the plugin can emit. Emitting an undeclared event is rejected at runtime.

- **names** — List of event names this plugin can emit. Must follow dot-separated naming and be prefixed with the plugin ID: `{plugin_id}.{event_name}`.

### `[config]` — Configuration Schema

Declare the plugin's runtime configuration shape.

- **schema** — Path to a JSON Schema file describing the expected configuration. Core validates the user-provided config against this schema at startup.

Plugin configuration is provided by the user in Core's main config file under a `[plugins.{plugin_id}]` section. The plugin reads its config at runtime via the `config_read` host function.

## Manifest Validation

Core validates the manifest at plugin load time:

1. Required fields present (`id`, `name`, `version`, `sdk_version`)
2. SDK version is compatible with the running Core version
3. Every declared action has a corresponding WASM export
4. Capability declarations are well-formed
5. Schema file paths exist and are valid JSON Schema
6. Event names follow the `{plugin_id}.*` prefix convention
7. Extension field paths follow the `ext.{plugin_id}.*` prefix convention
8. No collection name collisions with other loaded plugins (for plugin-scoped collections)

Validation failures produce clear error messages and prevent the plugin from loading. Core continues loading other plugins.

## Trust Model

- **First-party** (`trust = "first-party"`) — Capabilities are auto-granted. The plugin is assumed to be part of the Life Engine distribution.
- **Third-party** (`trust = "third-party"`, the default) — Capabilities must be explicitly approved in Core's config file under `[plugins.{plugin_id}.approved_capabilities]`. If a third-party plugin declares a capability that is not approved, Core logs a warning and loads the plugin with reduced capabilities.

This distinction exists to protect users who install community plugins. The capabilities system ensures that a plugin cannot silently escalate its access.
