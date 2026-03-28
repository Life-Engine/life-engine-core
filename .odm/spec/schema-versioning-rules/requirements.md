<!--
domain: schema-versioning-rules
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Requirements Document — Schema Versioning Rules

## Introduction

These requirements govern all schema changes to CDM collections and plugin private collections in Life Engine. CDM schema versions are coupled to SDK semver — there is no separate schema version number. The crate version of `packages/plugin-sdk-rs` and the package version of `packages/plugin-sdk-js` are the authoritative schema version.

The rules protect three audiences: SDK maintainers who need clear break/non-break criteria, plugin authors who need upgrade safety guarantees, and Core contributors who need to know when and how to write migrations. All CDM schema changes must conform to these rules before merging.

## Alignment with Product Vision

- **Parse, Don't Validate** — Schema validation at write time ensures only valid data enters storage; versioning rules guarantee the schema definition itself remains stable within a major version
- **Open/Closed Principle** — The additive-only rule means CDM schemas are open for extension (new optional fields, new collections) but closed for modification within a major version
- **Principle of Least Surprise** — Plugin authors can upgrade minor SDK versions without fear of breakage; deprecation notices give advance warning of future removals
- **Defence in Depth** — CI enforcement catches compatibility violations before merge; the 12-month maintenance window provides a safety net for the ecosystem

## Requirements

### Requirement 1 — SDK-Coupled Version Numbering

**User Story:** As an SDK maintainer, I want CDM schema versions coupled to SDK semver so that there is a single version number to reason about and no version mapping confusion.

#### Acceptance Criteria

- 1.1. WHEN a CDM schema change is proposed THEN the system SHALL use the SDK crate/package version as the schema version, with no separate schema version number.
- 1.2. WHEN a non-breaking schema change ships THEN the SDK SHALL release it in a minor version bump (e.g. `1.3.0` to `1.4.0`).
- 1.3. WHEN a breaking schema change ships THEN the SDK SHALL release it in a major version bump (e.g. `1.x` to `2.0.0`).
- 1.4. WHEN a patch release ships (e.g. `1.3.0` to `1.3.1`) THEN the release SHALL NOT contain any schema changes — patch releases are schema-frozen.
- 1.5. WHEN a plugin declares `minShellVersion` in its manifest THEN standard semver tooling SHALL handle compatibility checks using the SDK version.

### Requirement 2 — Non-Breaking Change Classification

**User Story:** As an SDK maintainer, I want a definitive list of non-breaking changes so that I can confidently ship schema additions in minor releases.

#### Acceptance Criteria

- 2.1. WHEN an optional field is added to a CDM schema THEN the change SHALL be classified as non-breaking and MAY ship in a minor release.
- 2.2. WHEN a new enum value is added THEN the change SHALL be classified as non-breaking, provided SDK documentation notes that enum fields may gain new values in minor releases and consumers must use a default/fallback arm.
- 2.3. WHEN a new CDM collection is added THEN the change SHALL be classified as non-breaking because plugins that do not declare the collection are unaffected.
- 2.4. WHEN an existing constraint is relaxed (e.g. reducing `minLength`, removing a `pattern`) THEN the change SHALL be classified as non-breaking because previously valid data remains valid.

### Requirement 3 — Breaking Change Classification

**User Story:** As an SDK maintainer, I want a definitive list of breaking changes so that I never accidentally ship a compatibility-breaking change in a minor release.

#### Acceptance Criteria

- 3.1. WHEN a field is removed from a CDM schema THEN the change SHALL be classified as breaking.
- 3.2. WHEN a field is renamed THEN the change SHALL be classified as breaking (functionally identical to removing the old field and adding a new one).
- 3.3. WHEN a field's type is changed THEN the change SHALL be classified as breaking.
- 3.4. WHEN a required field is added THEN the change SHALL be classified as breaking because existing records lack the field and old SDK writes produce invalid data.
- 3.5. WHEN an enum value is removed THEN the change SHALL be classified as breaking because plugins writing that value will fail validation.
- 3.6. WHEN field semantics change without a structural change THEN the change SHALL be classified as breaking.
- 3.7. WHEN an existing constraint is tightened (e.g. adding a `pattern`, reducing `maxLength`, adding a `minimum`) THEN the change SHALL be classified as breaking.

### Requirement 4 — Edge Case Classification

**User Story:** As an SDK maintainer, I want guidance on ambiguous schema changes so that I default to the safe classification.

#### Acceptance Criteria

- 4.1. WHEN a default value is changed THEN the change SHALL be treated as breaking unless no consumer could plausibly depend on the old default.
- 4.2. WHEN enum values are reordered THEN the change SHALL be treated as non-breaking only after auditing SDK code to confirm no integer-index dependency exists.
- 4.3. WHEN format validation is added to an existing string field THEN the change SHALL be treated as breaking because existing data may fail the new validation.
- 4.4. WHEN the classification is ambiguous THEN the change SHALL default to breaking — a major version bump is lower cost than silent data corruption.

### Requirement 5 — Additive-Only Rule

**User Story:** As a plugin author, I want a guarantee that CDM schemas only grow within a major version so that upgrading to any minor release is safe.

#### Acceptance Criteria

- 5.1. WHEN a CDM schema change is proposed within a major version THEN the change SHALL be limited to: adding optional fields, adding enum values, adding new collections, or relaxing constraints.
- 5.2. WHEN any other CDM schema modification is proposed within a major version THEN the system SHALL reject the change and require a major version bump.
- 5.3. WHEN the additive-only rule is enforced THEN the system SHALL run the check in CI as described in Requirement 6.

### Requirement 6 — CI Enforcement

**User Story:** As a Core contributor, I want CI to automatically block incompatible schema changes so that breaking changes cannot merge without a major version bump.

#### Acceptance Criteria

- 6.1. WHEN a PR modifies any file under `docs/schemas/` or `packages/types/` THEN the `schema-compat` CI job SHALL run a compatibility check comparing the proposed schema against the schema on `main`.
- 6.2. WHEN the check detects a breaking change and the SDK major version has not been incremented THEN the check SHALL fail and the PR SHALL be blocked.
- 6.3. WHEN the check detects a breaking change and the SDK major version has been incremented THEN the check SHALL verify that migration transforms are present and pass if they are.
- 6.4. WHEN the check detects only non-breaking changes with no major version bump THEN the check SHALL pass.
- 6.5. WHEN a PR uses `[skip ci]` or equivalent to bypass the schema compatibility check THEN the check SHALL still run — schema compatibility checks must not be bypassable.
- 6.6. WHEN a PR attempts to disable or remove the schema compatibility check THEN reviewers SHALL reject the PR.

### Requirement 7 — Major Version Lifecycle

**User Story:** As a plugin author, I want a predictable timeline for major version transitions so that I have adequate time to update my plugin.

#### Acceptance Criteria

- 7.1. WHEN a new major SDK version ships THEN Core SHALL accept records in both the old and new format during the overlap period.
- 7.2. WHEN a new major SDK version ships THEN the previous major version SHALL enter a 12-month maintenance window receiving only security fixes and critical bug fixes.
- 7.3. WHEN the 12-month maintenance window expires THEN Core SHALL stop accepting records in the old format and the previous major version SHALL reach end of life.
- 7.4. WHEN the 12-month window is active THEN no new features or schema additions SHALL be backported to the previous major.

### Requirement 8 — Migration Format

**User Story:** As a Core contributor, I want a standardised migration transform format so that I can write and test data migrations consistently.

#### Acceptance Criteria

- 8.1. WHEN a breaking CDM schema change ships in a major release THEN the SDK release SHALL include migration transform entries under the `migrations` key in the manifest.
- 8.2. WHEN a migration entry is defined THEN it SHALL specify the source version range (`from`) and the path to a transform script (`transform`) relative to the package root.
- 8.3. WHEN a transform function executes THEN it SHALL receive a single record object, return the transformed record, never throw, and be idempotent.
- 8.4. WHEN a record cannot be migrated THEN the transform SHALL return the record unchanged and log a warning.
- 8.5. WHEN Core starts after an SDK upgrade THEN it SHALL run migration transforms over all records in relevant collections.
- 8.6. WHEN Core runs migrations THEN it SHALL log: plugin ID, collection name, source version range, record count processed, record count modified, and any records that could not be migrated, to the audit log with the standard 90-day retention.
- 8.7. WHEN a migration is interrupted and Core restarts THEN re-running the migration SHALL produce the same result as running it once (idempotency guarantee).

### Requirement 9 — Plugin Private Collection Versioning

**User Story:** As a plugin author, I want clear rules for versioning my private collection schemas so that my users' data is protected across plugin updates.

#### Acceptance Criteria

- 9.1. WHEN a plugin makes a breaking change to a private collection schema THEN the plugin SHALL increment its manifest major version.
- 9.2. WHEN a plugin releases a new major version with private collection changes THEN the plugin manifest SHALL include migration entries for each affected private collection.
- 9.3. WHEN Core starts after a plugin update THEN it SHALL run the plugin's migration transforms using the same idempotent transform API as CDM migrations.
- 9.4. WHEN Core validates records in a private collection THEN it SHALL validate against the schema declared in the plugin's manifest.
- 9.5. WHEN a plugin ships a tightened schema without a migration THEN previously valid records SHALL fail validation on the next write, so plugins must include migrations.
- 9.6. WHEN a plugin ships a patch release THEN the release SHALL NOT contain any private collection schema changes — patch releases are schema-frozen.

### Requirement 10 — Deprecation Notices

**User Story:** As a plugin author, I want deprecation warnings in my build output so that I know which fields or values will be removed in the next major version.

#### Acceptance Criteria

- 10.1. WHEN a field or enum value is scheduled for removal THEN a `## Deprecated` entry SHALL be added to the CHANGELOG naming what is deprecated, why, and the target removal version.
- 10.2. WHEN a Rust SDK struct field or enum variant is deprecated THEN it SHALL carry a `#[deprecated(since = "X.Y.0", note = "...")]` attribute where `since` is the first minor release announcing the deprecation.
- 10.3. WHEN a TypeScript SDK interface property or type is deprecated THEN it SHALL carry a `@deprecated` JSDoc annotation with a migration note and target removal version.
- 10.4. WHEN a major release removes a field THEN that field SHALL have been deprecated in at least one minor release of the preceding major cycle.

### Requirement 11 — JSON Schema File Convention

**User Story:** As a Core contributor, I want a consistent file layout and URI scheme for published JSON Schemas so that tooling can resolve schemas reliably.

#### Acceptance Criteria

- 11.1. WHEN a CDM JSON Schema file is published THEN its `$id` field SHALL include the major SDK version (e.g. `https://life-engine.org/schemas/v1/tasks.schema.json`).
- 11.2. WHEN a new major version ships THEN new schema files SHALL be published at the new version URI and old URIs SHALL remain accessible for the full 12-month maintenance window.
- 11.3. WHEN schema files are stored in the monorepo THEN they SHALL live under `docs/schemas/` with version-prefixed directories (e.g. `docs/schemas/v1/tasks.schema.json`).
- 11.4. WHEN an unprefixed path exists (e.g. `docs/schemas/tasks.schema.json`) THEN it SHALL resolve to the current stable major version via symlink or redirect.
- 11.5. WHEN a CDM schema file declares `$schema` THEN it SHALL use `https://json-schema.org/draft/2020-12/schema`.
