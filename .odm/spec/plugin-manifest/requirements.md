<!--
domain: plugin-manifest
status: draft
tier: 1
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Plugin Manifest Requirements

## Requirement 1 — Plugin Identity

**User Story:** As a plugin author, I want to declare my plugin's identity in `manifest.toml` so that Core can uniquely identify and display it.

#### Acceptance Criteria

- 1.1. WHEN a manifest contains `[plugin]` with `id`, `name`, and `version` fields THEN Core SHALL accept the identity section as valid.
- 1.2. WHEN `id` is missing or empty THEN Core SHALL reject the manifest with a validation error naming the missing field.
- 1.3. WHEN `name` is missing or empty THEN Core SHALL reject the manifest with a validation error naming the missing field.
- 1.4. WHEN `version` is missing or empty THEN Core SHALL reject the manifest with a validation error naming the missing field.
- 1.5. WHEN `id` is not in kebab-case format THEN Core SHALL reject the manifest with a format error.
- 1.6. WHEN `version` does not follow semantic versioning (major.minor.patch) THEN Core SHALL reject the manifest with a format error.
- 1.7. WHEN two loaded plugins share the same `id` THEN Core SHALL reject the second plugin and log a duplicate-id error.
- 1.8. WHEN optional fields (`description`, `author`, `license`) are omitted THEN Core SHALL accept the manifest without error.

## Requirement 2 — Action Declaration

**User Story:** As a plugin author, I want to declare actions with optional timeouts so that workflows can invoke my plugin's entry points with bounded execution time.

#### Acceptance Criteria

- 2.1. WHEN a manifest declares at least one `[actions.<name>]` section with a `description` field THEN Core SHALL register the action as a valid workflow step.
- 2.2. WHEN no actions are declared THEN Core SHALL reject the manifest with a "no actions declared" error.
- 2.3. WHEN an action omits `description` THEN Core SHALL reject the manifest with a validation error for the action.
- 2.4. WHEN an action declares `timeout_ms` THEN Core SHALL enforce that timeout via the Extism host-level timeout.
- 2.5. WHEN an action omits `timeout_ms` THEN Core SHALL apply the system default timeout.
- 2.6. WHEN an action name does not correspond to a valid WASM export THEN Core SHALL reject the plugin after module instantiation with an export-mismatch error.

## Requirement 3 — Capability Declaration

**User Story:** As a plugin author, I want to declare capabilities so that Core grants only the host functions my plugin needs.

#### Acceptance Criteria

- 3.1. WHEN `[capabilities]` declares `storage_doc` with a list of operations THEN Core SHALL grant only those storage document operations to the plugin.
- 3.2. WHEN `[capabilities]` declares `storage_blob` with a list of operations THEN Core SHALL grant only those storage blob operations to the plugin.
- 3.3. WHEN `[capabilities]` declares `http_outbound = true` THEN Core SHALL enable outbound HTTP for the plugin.
- 3.4. WHEN `[capabilities]` declares `events_emit = true` THEN Core SHALL enable event emission for the plugin.
- 3.5. WHEN `[capabilities]` declares `events_subscribe = true` THEN Core SHALL enable event subscription for the plugin.
- 3.6. WHEN `[capabilities]` declares `config_read = true` THEN Core SHALL enable config reading for the plugin.
- 3.7. WHEN a capability is omitted from `[capabilities]` THEN Core SHALL deny that capability (deny-by-default).
- 3.8. WHEN a plugin invokes a host function for a denied capability THEN Core SHALL return a capability-denied error.

## Requirement 4 — Collection Declaration

**User Story:** As a plugin author, I want to declare collections with schema references and access levels so that Core provisions storage and enforces access control.

#### Acceptance Criteria

- 4.1. WHEN a collection declares `schema` with a `cdm:<name>` prefix THEN Core SHALL resolve it to the corresponding SDK-shipped schema file.
- 4.2. WHEN a collection declares `schema` with a relative file path THEN Core SHALL resolve it relative to the plugin directory.
- 4.3. WHEN a collection's `schema` path does not resolve to an existing file or recognised `cdm:` prefix THEN Core SHALL reject the manifest with a schema-resolution error.
- 4.4. WHEN a collection declares `access = "read"` THEN Core SHALL grant only read operations on that collection.
- 4.5. WHEN a collection declares `access = "write"` THEN Core SHALL grant only write operations on that collection.
- 4.6. WHEN a collection declares `access = "read-write"` THEN Core SHALL grant both read and write operations on that collection.
- 4.7. WHEN a collection omits `schema` or `access` THEN Core SHALL reject the manifest with a missing-field error for the collection.
- 4.8. WHEN a collection declares `extensions` THEN each entry SHALL follow the `ext.<plugin-id>.<field>` naming convention.
- 4.9. WHEN a collection declares `extension_schema` THEN Core SHALL validate that the path resolves to a valid JSON Schema file.
- 4.10. WHEN a collection declares `extension_indexes` THEN Core SHALL pass those fields as index hints to the storage adapter.
- 4.11. WHEN a collection declares `indexes` THEN Core SHALL pass those fields as index hints to the storage adapter.
- 4.12. WHEN a collection declares `strict = true` THEN Core SHALL reject writes with unknown fields via `StorageError::ValidationFailed`.

## Requirement 5 — Event Declaration

**User Story:** As a plugin author, I want to declare events I emit and subscribe to so that the event bus and trigger system wire my plugin correctly.

#### Acceptance Criteria

- 5.1. WHEN `[events.emit]` declares an `events` list THEN Core SHALL register those event names as permitted emissions for the plugin.
- 5.2. WHEN a plugin emits an event not declared in `[events.emit]` THEN Core SHALL reject the emission at runtime.
- 5.3. WHEN event names do not follow the `<plugin-id>.<action>.<outcome>` dot-separated convention THEN Core SHALL reject the manifest with a naming-convention error.
- 5.4. WHEN `[events.subscribe]` declares an `events` list THEN Core SHALL wire the plugin to receive those events via the trigger system.
- 5.5. WHEN `events_emit` capability is not declared but `[events.emit]` contains entries THEN Core SHALL reject the manifest with a capability mismatch error.
- 5.6. WHEN `events_subscribe` capability is not declared but `[events.subscribe]` contains entries THEN Core SHALL reject the manifest with a capability mismatch error.

## Requirement 6 — Configuration Schema

**User Story:** As a plugin author, I want to declare a configuration schema so that Core validates my plugin's runtime config at load time.

#### Acceptance Criteria

- 6.1. WHEN `[config]` declares a `schema` path THEN Core SHALL resolve it relative to the plugin directory and validate it is a valid JSON Schema file.
- 6.2. WHEN the config schema path does not resolve to an existing file THEN Core SHALL reject the manifest with a schema-resolution error.
- 6.3. WHEN the plugin has a `[config]` schema and runtime configuration is provided THEN Core SHALL validate the configuration against the schema at load time.
- 6.4. WHEN runtime configuration fails schema validation THEN Core SHALL reject the plugin with a config-validation error.
- 6.5. WHEN `config_read` capability is not declared but `[config]` section is present THEN Core SHALL reject the manifest with a capability mismatch error.

## Requirement 7 — Trust Model Enforcement

**User Story:** As a user, I want third-party plugin capabilities to require explicit approval so that untrusted plugins cannot access resources without my consent.

#### Acceptance Criteria

- 7.1. WHEN a first-party plugin loads THEN Core SHALL auto-grant all capabilities declared in its manifest.
- 7.2. WHEN a third-party plugin loads THEN Core SHALL check each declared capability against the approval list in Core's configuration file.
- 7.3. WHEN a third-party plugin declares a capability not approved in Core's configuration THEN Core SHALL reject the plugin with an unapproved-capability error.
- 7.4. WHEN all third-party plugin capabilities are approved THEN Core SHALL grant the approved capabilities and load the plugin.

## Requirement 8 — Manifest Validation

**User Story:** As a Core maintainer, I want unknown manifest sections to be rejected so that typos and unsupported fields do not silently pass validation.

#### Acceptance Criteria

- 8.1. WHEN a manifest contains an unknown top-level section THEN Core SHALL reject the manifest with an unknown-section error listing the offending key.
- 8.2. WHEN validation fails for any reason THEN Core SHALL abort plugin loading and log the specific error with the plugin id and field path.
- 8.3. WHEN all validation rules pass THEN Core SHALL mark the plugin as loaded and ready for action invocation.
- 8.4. WHEN a manifest file is missing from a plugin directory THEN Core SHALL skip that directory and log a warning.
- 8.5. WHEN a manifest file contains invalid TOML syntax THEN Core SHALL reject the plugin with a parse error before field-level validation begins.
