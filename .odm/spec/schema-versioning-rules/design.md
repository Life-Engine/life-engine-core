<!--
domain: schema-versioning-rules
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Schema Versioning Rules — Design

## Purpose

This document describes the technical design for enforcing schema versioning rules across Life Engine. It covers the compatibility classification algorithm, CI enforcement tooling, migration transform API, deprecation annotation conventions, and JSON Schema file layout. All CDM schema changes and plugin private collection schema changes must conform to these designs.

## Version Coupling

CDM schema versions are not independent artefacts. The version is the SDK semver version, sourced from two locations:

- **Rust SDK** — `packages/plugin-sdk-rs/Cargo.toml` `version` field
- **TypeScript SDK** — `packages/plugin-sdk-js/package.json` `version` field

These two versions must always be in sync. There is no separate schema version number. Plugin manifests reference the SDK version via `minShellVersion` using standard semver range syntax.

Version bump rules:

- **Patch** (`1.3.0` to `1.3.1`) — Bug fixes only. Zero schema changes permitted.
- **Minor** (`1.3.0` to `1.4.0`) — Non-breaking schema additions only (optional fields, new enum values, new collections, relaxed constraints).
- **Major** (`1.x` to `2.0.0`) — Breaking schema changes. Requires migration transforms and triggers the 12-month maintenance lifecycle.

## Compatibility Classification Algorithm

The `schema-compat` CI tool performs a structural diff between the proposed schema and the schema on `main`. Each detected change is classified using the following decision tree.

### Non-breaking changes

These changes are safe for minor releases:

- **New optional field** — The field has `"required": false` or is absent from the `required` array. Existing plugins ignore unknown fields.
- **New enum value** — A value is appended to an existing `enum` array. SDK documentation must note that consumers must handle unknown values with a fallback arm.
- **New collection** — A new `.schema.json` file is added under `docs/schemas/vN/`. Plugins that do not declare the collection are unaffected.
- **Relaxed constraint** — A numeric constraint is widened (`minLength` decreased, `maxLength` increased, `minimum` decreased, `maximum` increased) or a `pattern`/`format` is removed.

### Breaking changes

These changes require a major version bump:

- **Field removed** — A key present in the base schema is absent in the proposed schema.
- **Field renamed** — Detected as a simultaneous removal and addition where the type and position match. Treated as breaking regardless.
- **Type changed** — The `type` value for an existing field differs between base and proposed.
- **Required field added** — A new field appears in the `required` array.
- **Enum value removed** — A value present in the base `enum` array is absent in the proposed array.
- **Constraint tightened** — A numeric constraint is narrowed or a `pattern`/`format` is added to a field that previously had none.
- **Default value changed** — Treated as breaking unless explicitly overridden with a justification comment in the PR.

### Classification pseudocode

```rust
enum ChangeKind {
    NonBreaking,
    Breaking,
}

fn classify_change(base: &Schema, proposed: &Schema) -> Vec<(String, ChangeKind)> {
    let mut changes = Vec::new();

    // Detect removed fields
    for field in base.fields() {
        if !proposed.has_field(field.name()) {
            changes.push((field.name().into(), ChangeKind::Breaking));
        }
    }

    // Detect added fields
    for field in proposed.fields() {
        if !base.has_field(field.name()) {
            if proposed.is_required(field.name()) {
                changes.push((field.name().into(), ChangeKind::Breaking));
            } else {
                changes.push((field.name().into(), ChangeKind::NonBreaking));
            }
        }
    }

    // Detect type changes
    for field in base.fields() {
        if let Some(proposed_field) = proposed.get_field(field.name()) {
            if field.type_id() != proposed_field.type_id() {
                changes.push((field.name().into(), ChangeKind::Breaking));
            }
        }
    }

    // Detect enum value changes
    for field in base.enum_fields() {
        if let Some(proposed_field) = proposed.get_field(field.name()) {
            let removed: Vec<_> = field.enum_values()
                .filter(|v| !proposed_field.has_enum_value(v))
                .collect();
            for v in removed {
                changes.push((format!("{}::{}", field.name(), v), ChangeKind::Breaking));
            }
        }
    }

    // Detect constraint changes
    for field in base.fields() {
        if let Some(proposed_field) = proposed.get_field(field.name()) {
            if is_constraint_tightened(field, proposed_field) {
                changes.push((field.name().into(), ChangeKind::Breaking));
            }
            if is_constraint_relaxed(field, proposed_field) {
                changes.push((field.name().into(), ChangeKind::NonBreaking));
            }
        }
    }

    changes
}
```

## CI Enforcement

The schema compatibility check runs as a dedicated step in the `schema-compat` CI job. It triggers on any PR that modifies files under `docs/schemas/` or `packages/types/`.

### Check flow

1. Checkout the `main` branch schema files as the baseline.
2. Checkout the PR branch schema files as the proposed version.
3. Run `classify_change()` on each modified schema file.
4. If any change is classified as `Breaking`:
   - Compare the SDK major version on the PR branch against `main`.
   - If the major version has not been incremented, fail the check with a message listing every breaking change detected.
   - If the major version has been incremented, verify that migration entries exist in the manifest for every affected collection. Pass if present; fail if missing.
5. If all changes are `NonBreaking`, pass unconditionally.

### Bypass prevention

- The `schema-compat` job is configured as a required status check on the `main` branch. It cannot be skipped with `[skip ci]`.
- Any PR that modifies the `schema-compat` job definition must receive explicit approval from a maintainer.

## Migration Transform API

Migration transforms are defined in the plugin manifest under the `migrations` key. The format is the same for CDM migrations (shipped by SDK maintainers) and plugin private collection migrations (shipped by plugin authors).

### Manifest schema

```json
{
  "version": "2.0.0",
  "migrations": [
    {
      "from": "1.x",
      "collection": "tasks",
      "transform": "./migrations/v2-tasks.js"
    },
    {
      "from": "1.x",
      "collection": "events",
      "transform": "./migrations/v2-events.js"
    }
  ]
}
```

Each migration entry specifies:

- **from** — A semver range identifying the source version. Core matches records whose last-written version falls within this range.
- **collection** — The collection name this migration applies to.
- **transform** — Path to the transform script relative to the package root.

### Transform function contract

Every transform script must export a default function with the following contract:

- **Input** — A single record object (the stored JSON document).
- **Output** — The transformed record object.
- **No exceptions** — If a record cannot be migrated, the function returns the record unchanged and logs a warning via the provided logger.
- **Idempotent** — Applying the transform twice produces the same result as applying it once. This is mandatory because a partial migration interrupted by restart will re-process some records.

Example transform adding a required `category` field with a fallback:

```javascript
export default function migrate(record) {
  if (record.category !== undefined) {
    return record; // already migrated — idempotency guard
  }
  return { ...record, category: "uncategorised" };
}
```

Example transform renaming a field:

```javascript
export default function migrate(record) {
  if (record.deadline !== undefined) {
    return record; // already migrated
  }
  const { due_date, ...rest } = record;
  return { ...rest, deadline: due_date };
}
```

### Migration execution

Core runs migrations on startup after detecting an SDK or plugin version change:

1. For each collection with a pending migration, iterate over all records in `plugin_data` where `plugin_id` and `collection` match.
2. For each record, invoke the transform function.
3. If the returned record differs from the input, write the updated record back to storage with an incremented `version`.
4. Log results to the `audit_log`: plugin ID, collection name, source version range, records processed, records modified, and any records that failed.
5. Migration logs follow the standard 90-day audit retention policy.

## Major Version Lifecycle

When a breaking change requires a new major SDK version, the lifecycle proceeds through five phases:

- **Release** — The new major version (e.g. `2.0.0`) ships. Core begins accepting records in both the old and new format. New schema files are published at versioned URIs.
- **Maintenance window opens** — The previous major version receives security and critical bug fixes only for 12 months. No features or schema additions are backported.
- **Migration runs** — On first startup after the upgrade, Core runs migration transforms. Migration is idempotent so restarts are safe.
- **Deprecation period** — Plugin authors have 12 months to update. Deprecation notices appear in CHANGELOG and SDK annotations.
- **End of life** — After 12 months, the previous major is dropped. Core stops accepting old-format records.

The 12-month window is non-negotiable. It provides a stable timeline for third-party plugin authors.

## Deprecation Annotation Conventions

Deprecation must appear in three places before a field or enum value can be removed.

### CHANGELOG

A `## Deprecated` section entry in the release notes for the minor version that introduces the deprecation:

```markdown
## Deprecated

- `tasks.priority` — Use `tasks.urgency` instead. Will be removed in v3.0.0.
```

### Rust SDK

The `#[deprecated]` attribute on the struct field or enum variant:

```rust
pub struct Task {
    pub title: String,
    pub status: TaskStatus,
    #[deprecated(since = "2.5.0", note = "Use `urgency` instead. Will be removed in v3.0.0.")]
    pub priority: Option<u8>,
    pub urgency: Option<u8>,
}
```

The `since` value is the first minor release in which the deprecation was announced.

### TypeScript SDK

The `@deprecated` JSDoc annotation on the interface property:

```typescript
interface Task {
  title: string;
  status: TaskStatus;
  /** @deprecated Use `urgency` instead. Will be removed in v3.0.0. */
  priority?: number;
  urgency?: number;
}
```

### Removal precondition

A field may not be removed in a major release unless it was deprecated in at least one minor release of the preceding major cycle. The CI check verifies this by checking the git history for the deprecation annotation.

## JSON Schema File Convention

### URI scheme

CDM JSON Schema files are published at versioned URIs. The `$id` field in each schema includes the major SDK version:

```
https://life-engine.org/schemas/v1/tasks.schema.json
https://life-engine.org/schemas/v1/events.schema.json
https://life-engine.org/schemas/v2/tasks.schema.json
```

### Monorepo layout

Schema files are stored under `docs/schemas/` with version-prefixed directories:

- `docs/schemas/v1/tasks.schema.json`
- `docs/schemas/v1/events.schema.json`
- `docs/schemas/v2/tasks.schema.json`

### Stable symlink

An unprefixed path (`docs/schemas/tasks.schema.json`) points to the current stable major version via symlink. Tooling that does not need version pinning uses the unprefixed path. Tooling requiring reproducibility uses the versioned path.

### Meta-schema

All CDM schema files use `https://json-schema.org/draft/2020-12/schema` as the `$schema` declaration, consistent with the plugin manifest schema.

### Lifecycle

Old version URIs remain accessible for the full 12-month maintenance window. They are removed only when the previous major version reaches end of life.

## Plugin Private Collection Versioning

Plugin private collections follow the same compatibility classification and migration rules as CDM collections, with the following scoping:

- The plugin's own manifest `version` field governs its schema versioning, not the SDK version.
- Breaking private collection changes require a plugin major version bump.
- Migration transforms are shipped in the plugin's manifest, not the SDK.
- Core runs the plugin's transforms on startup after detecting a plugin version change.
- Patch releases are schema-frozen for private collections, just as for CDM collections.

Because private collections are namespaced to the plugin (e.g. `com.example.pomodoro/pomodoro_sessions`), breaking schema changes in one plugin never affect another plugin's data.
