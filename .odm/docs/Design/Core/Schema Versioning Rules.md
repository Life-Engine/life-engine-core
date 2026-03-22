---
title: "Schema Versioning Rules"
tags: [schema, versioning, data, design]
created: 2026-03-21
---

# Schema Versioning Rules

## Purpose

These rules exist to protect the interoperability contract between Core, the two SDKs, and every plugin in the ecosystem. Because canonical collection schemas are the shared data language of Life Engine — defined once in `packages/types/` and consumed by every plugin that reads or writes standard data — a schema change that breaks existing plugins would corrupt data or crash plugin code for users who have not yet updated.

The rules serve three audiences:

- **SDK maintainers** — who need clear criteria for whether a proposed change requires a major version bump
- **Plugin authors** — who need confidence that a minor SDK update will not break their code
- **Core contributors** — who need to understand when migration transforms are required and how to write them

All canonical schema changes must conform to these rules before merging. Plugin private collection changes follow a subset of these rules as described in [[#Plugin Private Collection Versioning]].

Related specifications: [[Canonical Data Models]] and [[Data Layer]].

## Version Numbering

Canonical schema versions are not independent artefacts — they are coupled to the SDK semver version. The version lives in the crate version of `packages/plugin-sdk-rs` and the package version of `packages/plugin-sdk-js`. There is no separate schema version number.

This means:

- A non-breaking schema addition ships in a minor SDK release (e.g. `1.3.0` → `1.4.0`)
- A breaking schema change ships in a major SDK release (e.g. `1.x` → `2.0.0`)
- Patch releases (`1.3.0` → `1.3.1`) must not change schemas at all — patch releases are for bug fixes only

Plugin authors declare `minShellVersion` in their manifest using standard semver. Existing semver tooling handles compatibility checks automatically because the version is the SDK version.

## Compatibility Classification

### Non-breaking changes

A change is non-breaking when a plugin compiled against the old schema continues to work correctly after the platform updates to the new schema. The following changes are non-breaking and may ship in a minor SDK release:

- **Adding an optional field** — Existing plugins ignore fields they do not know about. For example, adding `reminder_minutes` (optional integer) to the `tasks` schema does not affect plugins that only read `title` and `status`.
- **Adding a new enum value** — Safe only if consumers handle unknown values gracefully (e.g. treating an unknown `status` value as `pending`). The SDK documentation must note that enum fields may gain new values in minor releases and consumers must use a default/fallback arm.
- **Adding a new canonical collection** — Plugins that do not declare the new collection in their manifest are unaffected.
- **Relaxing an existing constraint** — Changing a `minLength: 3` to `minLength: 1`, or removing a `pattern` restriction, is non-breaking because previously valid data remains valid.

### Breaking changes

A change is breaking when a plugin that compiled against the old schema may produce incorrect output, fail to deserialise data, or violate invariants it previously relied on. The following changes are breaking and must not ship without a major SDK version bump:

- **Removing a field** — Any plugin reading that field will receive `None`/`undefined` where it expected a value. Example: removing `priority` from `tasks` would break any plugin that uses priority-based sorting.
- **Renaming a field** — Functionally identical to removing the old field and adding a new one. Example: renaming `due_date` to `deadline` is breaking even if the semantics are identical.
- **Changing a field's type** — Example: changing `size` on `files` from `integer` to `string` breaks Rust code that deserialises it as `u64`.
- **Adding a required field** — Existing records in the database do not have the new field, so any plugin that writes records using the old SDK will produce invalid data. Example: adding a required `category` field to `tasks` would make every existing task record fail validation.
- **Removing an enum value** — Any plugin that writes the removed value will produce data that fails Core's validation. Example: removing `cancelled` from the task status enum would cause all cancelled tasks to fail round-trip.
- **Changing semantics without changing structure** — Example: redefining `source` from "the plugin that created the record" to "the external system that owns the record" is a breaking change even though the field type remains `string`. Behaviour changes are breaking.
- **Tightening an existing constraint** — Adding a `pattern` to a field that previously had none, reducing `maxLength`, or adding a `minimum` to an integer field. Existing data may no longer pass validation.

### Edge cases

Some changes are neither clearly breaking nor clearly non-breaking. These must be evaluated case by case:

- **Changing a default value** — If a plugin relies on the default being applied at the storage layer and reads the field expecting the old default, changing the default is a breaking change in practice. Treat as breaking unless no consumer could plausibly depend on the old default.
- **Reordering enum values** — In JSON Schema, enum order has no semantic meaning. However, if any SDK code assigns integer indices to enum variants (e.g. for compact storage or sorting), reordering is breaking. Audit SDK code before treating as non-breaking.
- **Adding format validation to an existing field** — Example: adding `"format": "date-time"` to a field that previously accepted any string. Existing records that contain informal date strings (e.g. `"tomorrow"`) will now fail validation. Treat as breaking.

When in doubt, classify as breaking. A major version bump is a low cost compared to silent data corruption.

## Additive-Only Rule

Within a major SDK version, canonical schemas must only grow — never shrink or change existing definitions. This is the additive-only rule:

- Fields may be added (if optional)
- Enum values may be added (with the caveat above)
- New collections may be added
- Constraints may be relaxed

No other changes to canonical schemas are permitted within a major version. This rule is enforced in CI as described in [[#CI Enforcement]].

The additive-only rule gives plugin authors a firm guarantee: upgrading to any `1.x` release will not break their plugin. They may adopt new fields at their own pace.

## CI Enforcement

Every pull request that modifies a file under `docs/schemas/` or `packages/types/` must pass a schema compatibility check before merging. The check compares the proposed schema against the schema on `main`:

- It identifies all changes and classifies each as breaking or non-breaking using the rules in [[#Compatibility Classification]]
- If any breaking change is detected and the SDK major version has not been incremented, the check fails and the PR is blocked
- If the SDK major version has been incremented, the check confirms the migration format is present (see [[#Migration Format]]) and passes
- Non-breaking changes with no major version bump always pass

The check runs as a dedicated step in the `schema-compat` CI job. It must not be bypassed with `[skip ci]` or equivalent. Any PR that disables or removes the schema compatibility check must be rejected at review.

## Major Version Lifecycle

When a breaking change requires a new major SDK version, the following lifecycle applies:

- **Release** — The new major version (e.g. `2.0.0`) ships with the breaking changes. The new schema files are published at versioned URIs (see [[#JSON Schema File Convention]]). Core begins accepting records in both the old and new format during the overlap period.
- **Maintenance window opens** — The previous major version enters a 12-month maintenance window. During this window, the previous major receives security fixes and critical bug fixes only. No new features or schema additions are backported.
- **Migration runs** — When Core starts up after an SDK upgrade, it runs migration transforms (see [[#Migration Format]]) over all records in the relevant collections. Migration is idempotent, so a restart after a partial migration is safe.
- **Deprecation period** — Plugin authors have 12 months to update their plugins. Deprecation notices are published in the CHANGELOG and propagated through SDK annotations (see [[#Deprecation Notices]]).
- **End of life** — After 12 months, support for the previous major version is dropped. Core stops accepting records in the old format. Any plugin that has not been updated to the new major will fail to write data.

This 12-month window is non-negotiable. The window provides a stable migration timeline for third-party plugin authors who cannot follow release schedules as closely as the core team.

## Migration Format

Migration transforms are defined in the plugin manifest under the `migrations` key. Each migration entry specifies the source version range and the path to a transform script relative to the plugin package root.

```json
{
  "version": "2.0.0",
  "migrations": [
    {
      "from": "1.x",
      "transform": "./migrations/v2.js"
    }
  ]
}
```

The transform script must export a default function that:

- Receives a single record object (the stored JSON document) as its only argument
- Returns the transformed record object
- Must not throw — if a record cannot be migrated, it must return the record unchanged and log a warning
- Must be idempotent — applying the transform twice must produce the same result as applying it once

Example transform for the hypothetical addition of a required `category` field with a default fallback:

```js
export default function migrate(record) {
  if (record.category !== undefined) {
    return record; // already migrated
  }
  return { ...record, category: "uncategorised" };
}
```

Core logs every migration run with the following information: plugin ID, collection name, source version range, record count processed, record count modified, and any records that could not be migrated. Migration logs are written to the audit log and are retained with the same 90-day policy as other audit entries.

For canonical collections, migration transforms are part of the SDK release, not the plugin. SDK maintainers ship the transform alongside the new major version. For plugin private collections, the plugin author ships the transform in their own manifest.

## Plugin Private Collection Versioning

Plugins use standard semver in their manifest `version` field. The manifest version is the version of the plugin itself, and the plugin author is solely responsible for their own schema evolution.

The following rules apply to plugin private collections:

- The plugin must follow the same compatibility classification rules as canonical collections when incrementing its own version. A breaking change to a private collection schema requires a plugin major version bump.
- When a plugin releases a new major version, it must include migration entries in its manifest for each private collection that changed in a breaking way.
- Core will run the plugin's migration transforms on startup after a plugin update, using the same idempotent transform API described in [[#Migration Format]].
- Core validates all records against the schema declared in the plugin manifest. A plugin that ships a tightened schema without a migration will cause previously valid records to fail validation on the next write.
- Plugins must not change their private collection schema in a patch release. Patch releases must be schema-frozen.

Because private collections are namespaced to the plugin (e.g. `com.example.pomodoro/pomodoro_sessions`), a breaking schema change in one plugin can never affect another plugin's data. The isolation is structural.

## Deprecation Notices

When a field or enum value is scheduled for removal in the next major version, it must be deprecated before removal. Deprecation must appear in all three places:

- **CHANGELOG** — A `## Deprecated` section entry describing what is deprecated, why, and which major version will remove it. The entry must name a target removal version (e.g. "will be removed in v3.0.0").
- **Rust SDK** — The deprecated struct field or enum variant must carry a `#[deprecated(since = "1.5.0", note = "Use `new_field` instead. Will be removed in v2.0.0.")]` attribute. The `since` value must be the first minor release in which the deprecation was announced.
- **TypeScript SDK** — The deprecated interface property or type must carry a `@deprecated Use `newField` instead. Will be removed in v2.0.0.` JSDoc annotation.

Deprecation annotations allow plugin authors to receive compiler warnings in their own build pipelines, which is the primary channel through which ecosystem-wide deprecations are communicated. CHANGELOG entries are secondary.

A field may not be removed in a major release unless it was deprecated in at least one minor release of the preceding major cycle.

## JSON Schema File Convention

JSON Schema files for canonical collections are published at versioned URIs. The `$id` field in each schema file must include the major SDK version:

```
https://life-engine.org/schemas/v1/tasks.schema.json
https://life-engine.org/schemas/v1/events.schema.json
```

When a new major version ships, new schema files are published at the new version URI:

```
https://life-engine.org/schemas/v2/tasks.schema.json
```

The old URIs remain accessible for the full 12-month maintenance window and are removed only when the previous major version reaches end of life.

In the monorepo, the files are stored under `docs/schemas/` with version-prefixed directories:

- `docs/schemas/v1/tasks.schema.json`
- `docs/schemas/v1/events.schema.json`
- and so on for each canonical collection

A symlink or redirect at `docs/schemas/tasks.schema.json` (no version prefix) points to the current stable major. Tooling that does not need to pin a version may use the unprefixed path. Tooling that must pin a version for reproducibility must use the versioned path.

The `$schema` meta-schema declaration in all canonical schema files must use `https://json-schema.org/draft/2020-12/schema` for consistency with the plugin manifest schema.
