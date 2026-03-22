# ADR-014: Extensions namespacing convention

## Status
Accepted

## Context

Every canonical data model (tasks, events, contacts, emails, notes, files) includes an `extensions` field intended for plugin-specific data. The field is described only as "Namespaced object for plugin-specific fields" with no formal specification of the key format, structure rules, validation, or conflict semantics.

Without a convention, plugin authors have no guidance on how to use extensions correctly. Two independent plugins could write to the same key, silently overwriting each other's data. Core has no rule for whether to replace or merge extension data during writes. The lack of a namespace scheme makes it impossible for Core to enforce access boundaries, violating the Principle of Least Privilege.

The plugin manifest schema already uses a reverse-domain `id` format (e.g. `com.life-engine.todos`). This existing identifier is a natural namespace key.

## Decision

Extension keys MUST be the plugin's reverse-domain `id` from its `plugin.json` manifest. Each plugin's namespace value MUST be a JSON object (not a primitive or array). The resulting structure is:

```json
{
  "extensions": {
    "com.life-engine.todos": { "priority_color": "#ff0000" },
    "org.example.time-tracker": { "tracked_seconds": 3600 }
  }
}
```

The following rules apply:

- Plugins MUST only read and write their own namespace. Core enforces this at the API boundary by stripping writes to namespaces that do not match the authenticated plugin's `id`.
- Core MUST preserve unknown namespaces during writes. When a plugin updates a record, Core merges the plugin's namespace into the existing extensions object rather than replacing it. This prevents one plugin from deleting another plugin's data.
- The prefix `org.life-engine.*` is reserved for first-party extensions.
- Extension data is NOT validated against any schema by Core. The contents of each namespace are opaque to Core and meaningful only to the owning plugin.
- JSON schemas enforce the key format using `patternProperties` with the same reverse-domain regex used in the plugin manifest `id` field, and reject non-matching keys via `additionalProperties: false`.

## Consequences

Positive consequences:

- Key collisions between plugins are impossible when the convention is followed, since each plugin has a globally unique reverse-domain identifier.
- Core can enforce access boundaries at the API layer by comparing the authenticated plugin's `id` against the extension namespace being written.
- The merge-on-write semantics prevent data loss when multiple plugins extend the same record.
- The convention reuses the existing plugin `id` format, adding no new concepts for plugin authors to learn.
- JSON schema validation catches malformed extension keys at development time rather than in production.

Negative consequences:

- Extension keys are verbose (e.g. `com.life-engine.todos` rather than `todos`). This is an intentional trade-off: verbosity prevents collisions.
- Core must implement merge logic for the extensions field rather than simple replacement. This adds complexity to the storage write path.
- Plugins cannot read other plugins' extension data without explicit API support. This is a feature (Principle of Least Privilege) but may frustrate plugin authors who want to integrate with other plugins' data.

## Alternatives Considered

**Short string keys** (e.g. `todos`, `time-tracker`) were rejected because they provide no collision resistance. Two independent plugin authors could easily choose the same short name.

**UUID keys** were rejected because they are not human-readable. Debugging extension data in the database or API responses would require a lookup table to determine which plugin owns each namespace.

**No convention (freeform keys)** was rejected because it provides no safety guarantees and makes it impossible for Core to enforce access boundaries.

**Prefix-based convention** (e.g. `plugin:todos:priority_color` as a flat key) was rejected because nested objects under a single namespace key are more ergonomic to work with in JavaScript and Rust than flat prefixed keys. Nested objects also enable JSON schema validation of the key format without complex regex matching on every key in the extensions object.
