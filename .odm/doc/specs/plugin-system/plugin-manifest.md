---
title: Plugin Manifest Specification
type: reference
created: 2026-03-28
status: active
tags:
  - spec
  - plugin
  - manifest
---

# Plugin Manifest Specification

Every plugin must ship a `manifest.toml` file at the root of its plugin directory. The manifest declares the plugin's identity, actions, capabilities, collection access, events, configuration schema, and route bindings. Core validates the manifest at load time and rejects plugins with invalid or incomplete manifests.

## Plugin Identity

The `[plugin]` section is required and identifies the plugin:

```toml
[plugin]
id = "connector-email"
name = "Email Connector"
version = "1.0.0"
description = "IMAP/SMTP email sync"
author = "Life Engine"
license = "MIT"
```

Required fields:

- **id** — Unique kebab-case identifier. Must be globally unique across all loaded plugins.
- **name** — Human-readable display name.
- **version** — Semantic version string (major.minor.patch).

Optional fields:

- **description** — Short summary of what the plugin does.
- **author** — Plugin author or organisation.
- **license** — SPDX license identifier.

## Actions

Each `[actions.<name>]` section declares a named entry point that workflows can invoke as a step. See [[plugin-actions]] for the runtime contract.

```toml
[actions.fetch]
description = "Fetch new emails from IMAP"
timeout_ms = 30000

[actions.send]
description = "Send email via SMTP"
timeout_ms = 10000
```

- **description** — Required. Human-readable summary of what the action does.
- **timeout_ms** — Optional. Maximum execution time in milliseconds. Enforced by the Extism host-level timeout. If omitted, Core applies a default timeout.

A plugin must declare at least one action (excluding lifecycle hooks).

## Capabilities

The `[capabilities]` section declares what [[host-functions|host functions]] the plugin needs. All capabilities are deny-by-default.

```toml
[capabilities]
storage_doc = ["read", "write"]
storage_blob = ["read", "write"]
http_outbound = true
events_emit = true
events_subscribe = true
config_read = true
```

- **storage_doc** — List of permitted operations: `"read"`, `"write"`, `"delete"`.
- **storage_blob** — List of permitted operations: `"read"`, `"write"`, `"delete"`.
- **http_outbound** — Boolean. Enables outbound HTTP requests.
- **events_emit** — Boolean. Enables emitting events to the [[event-bus]].
- **events_subscribe** — Boolean. Enables event-triggered workflows.
- **config_read** — Boolean. Enables reading the plugin's runtime config.

Omitted capabilities default to denied.

## Collections

Each `[collections.<name>]` section declares a document collection the plugin reads from or writes to. See [[storage-context]] for the underlying storage model.

```toml
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
```

- **schema** — Required. Either a `cdm:<name>` reference to a [[cdm-specification|CDM]] schema or a relative path to a local JSON Schema file.
- **access** — Required. One of `"read"`, `"write"`, or `"read-write"`.
- **extensions** — Optional. List of extension field paths this plugin adds to a shared CDM collection. Must use the `ext.<plugin-id>.<field>` naming convention.
- **extension_schema** — Optional. Relative path to a JSON Schema validating the extension fields.
- **extension_indexes** — Optional. Extension fields to index.
- **indexes** — Optional. Fields to index for plugin-owned collections.
- **strict** — Optional. When `true`, all documents must validate against the schema on write. Defaults to `false`.

## Events

The `[events]` section declares which events the plugin emits and subscribes to:

```toml
[events.emit]
events = ["connector-email.fetch.completed", "connector-email.fetch.failed", "connector-email.send.completed"]

[events.subscribe]
events = ["system.startup"]
```

- **emit.events** — List of event names this plugin may emit. Emitting an undeclared event is rejected at runtime. Event names must follow the dot-separated convention: `<plugin-id>.<action>.<outcome>`.
- **subscribe.events** — List of event names this plugin responds to. Used by the [[trigger-system]] to wire event-triggered workflows.

## Configuration

```toml
[config]
schema = "schemas/config.json"
```

- **schema** — Required if the plugin accepts runtime configuration. Relative path to a JSON Schema file. Core validates the plugin's config section against this schema at load time. The plugin accesses its config via the `config_read` [[host-functions|host function]].

## Trust Model

- **First-party plugins** — All declared capabilities are auto-granted at load time.
- **Third-party plugins** — Each declared capability requires explicit approval in Core's configuration file. If any required capability is not approved, the plugin fails to load.
- Core validates the manifest against these trust rules during the Load phase of the [[plugin-system#Lifecycle|plugin lifecycle]].

## Validation Rules

Core validates every manifest at load time. A manifest must satisfy all of the following:

- All required fields in `[plugin]` are present and non-empty.
- At least one action is declared.
- Each action references a valid WASM export (verified after module instantiation).
- Each collection's `schema` path resolves to an existing file or a recognised `cdm:` prefix.
- Each `extension_schema` path resolves to a valid JSON Schema file.
- All event names in `[events.emit]` and `[events.subscribe]` follow the dot-separated naming convention.
- The `[config].schema` path, if present, resolves to a valid JSON Schema file.
- No unknown top-level sections are present.

Validation failure at any point aborts plugin loading and Core logs the specific error.
