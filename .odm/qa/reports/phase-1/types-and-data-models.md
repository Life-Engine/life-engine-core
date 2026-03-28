# Types and Data Models Review

## Summary

The `packages/types` crate defines the Canonical Data Model (CDM) for the Life Engine ecosystem. It contains 7 CDM entity types (events, tasks, contacts, notes, emails, files, credentials), a pipeline message envelope, storage query/mutation types, extension namespace validation, file helper utilities, schema migration declarations, and new workflow engine contract types (`identity.rs`, `workflow.rs`).

Overall, the crate is well-structured with consistent patterns across all CDM types. Serde annotations are thorough, test coverage is excellent (round-trip, validation, schema-level, and spec-compliance tests), and the new workflow/identity types are cleanly designed. The issues found are mostly minor, with a few notable design concerns.

## File-by-File Analysis

### Cargo.toml

Clean workspace-inherited metadata. Dependencies are appropriate for the crate's scope. The `jsonschema` dependency is used only for `SchemaValidated` in `pipeline.rs` and for integration tests -- this is a heavy dependency for a types crate. Consider whether validation belongs in a separate crate.

### src/lib.rs

- Module declarations cover all 12 submodules correctly
- Re-exports at crate root are comprehensive for CDM types
- Notably, `identity` and `workflow` modules are declared but NOT re-exported at the crate root, requiring consumers to use `life_engine_types::identity::Identity` instead of `life_engine_types::Identity`
- Inline tests cover round-trip serialization for all 7 CDM types
- `FileMetadata` re-exported from `files` module but not `file_helpers` functions -- this is correct since helpers are utilities, not types

### src/pipeline.rs

- `PipelineMessage` envelope is well-designed with metadata + typed payload separation
- `TypedPayload` uses `#[serde(tag = "type", content = "data")]` adjacently-tagged enum -- good choice for extensibility
- `CdmType` uses `#[serde(tag = "collection", content = "value")]` -- consistent tagging strategy
- `Box<CdmType>` in `TypedPayload::Cdm` prevents excessive stack size -- good practice
- `SchemaValidated<T>` newtype with `#[serde(transparent)]` is well-designed, but the `transparent` derive means deserialization bypasses validation. Any JSON can be deserialized into a `SchemaValidated<T>` without validation. This is a significant integrity concern (see Critical issues)
- `SchemaValidationError` implements `Display` and `Error` manually -- correct
- Batch variants (`EventBatch`, `TaskBatch`, etc.) duplicate single variants -- this is acceptable but could grow unwieldy if more CDM types are added
- Test at line 201 uses `matches!()` without `assert!()` wrapping it, so the test always passes regardless of the match result

### src/identity.rs (new, untracked)

- `Identity` struct with `subject`, `issuer`, `claims` -- clean and focused
- `Identity::guest()` factory method is a nice convenience
- `TriggerContext` enum with `Endpoint`, `Event`, `Schedule` variants covers the main trigger types
- Uses `#[serde(tag = "type", rename_all = "snake_case")]` -- internally tagged, consistent with project conventions
- `HashMap<String, serde_json::Value>` for claims is flexible but untyped -- appropriate for this use case
- `PartialEq` derived on both types -- good for testing
- No `Eq` derived on `Identity` -- could be added since all fields support it (String, HashMap)
- No `Hash` on `Identity` -- may be needed if used as map keys

### src/workflow.rs (new, untracked)

- `WorkflowRequest` and `WorkflowResponse` are well-structured protocol-agnostic types
- `WorkflowStatus` enum has exactly 6 variants with HTTP status code mapping -- clean design
- `is_success()` and `http_status_code()` helper methods are useful
- `WorkflowError` has `code`, `message`, `detail` -- standard error structure
- `RequestMeta.request_id` is `String` rather than `Uuid` -- this is intentional based on the test using `"req-001"` format, but inconsistent with `MessageMetadata.correlation_id` which uses `Uuid`
- `ResponseMeta.duration_ms` as `u64` -- correct type for timing
- `WorkflowStatus` derives `Copy` -- good since it's a simple enum
- `WorkflowResponse` does not derive `PartialEq` -- needed only if comparing responses in tests, but `WorkflowRequest` also lacks it; both contain `DateTime<Utc>` which does implement `PartialEq`, so adding it would work

### src/contacts.rs

- `ContactInfoType` and `PhoneType` enums with `rename_all = "snake_case"` -- correct
- `ContactEmail`, `ContactPhone`, `ContactAddress` use `#[serde(rename = "type")]` to map Rust field names to JSON `"type"` key -- well handled
- All optional fields properly annotated with `skip_serializing_if`
- `ContactAddress` has all fields as `Option<String>` -- allows partial addresses
- `birthday` uses `NaiveDate` -- correct for date-only fields without timezone
- `Contact` has `groups: Vec<String>` but no `#[serde(default)]` annotation -- this means deserialization will fail if `groups` is missing from JSON. However, tests show it works because serde's default behavior for `Vec` when the field is absent depends on the container attributes. Actually, looking more carefully, `groups` has `#[serde(default, skip_serializing_if = "Vec::is_empty")]` -- correct.

### src/credentials.rs

- Intentionally has NO `extensions` field -- documented in doc comment
- `claims: serde_json::Value` is untyped but appropriate since credential claims are type-specific
- `encrypted: Option<bool>` -- tracks whether the credential is stored encrypted
- `CredentialType` has 4 variants covering common credential types
- No `Eq` or `Hash` on `CredentialType` -- could be useful but not critical

### src/emails.rs

- `EmailAddress` with optional `name` and required `address` -- standard
- `EmailAttachment` has `filename`, `mime_type`, `size_bytes`, `content_id` -- covers inline attachments
- `Email.to: Vec<EmailAddress>` is NOT annotated with `#[serde(default)]` -- this means deserialization requires `to` to be present in JSON. This is correct since `to` is a required field
- No `thread_id` field -- tests confirm unknown fields are silently ignored (serde default behavior since `#[serde(deny_unknown_fields)]` is not used)
- No email address validation -- acceptable for a types crate (validation belongs at the boundary)

### src/events.rs

- `Recurrence` with `from_rrule` and `to_rrule` -- good iCal interoperability
- `default_interval()` function for serde default -- correct pattern
- `from_rrule` silently falls back to `interval = 1` on parse failure via `unwrap_or(1)` -- acceptable
- `to_rrule()` omits `INTERVAL` when it equals 1 -- correct behavior for compact RRULE output
- `to_rrule()` omits `UNTIL` if not set -- correct
- `CalendarEvent.validate_time_range()` uses let-chains (Rust nightly/2024 edition feature) -- `if let Some(end) = self.end && self.start >= end` -- verify this compiles on the target edition
- `AttendeeStatus::NeedsAction` uses `#[serde(rename = "needs-action")]` with a hyphen -- inconsistent with the rest of the crate which uses `rename_all = "snake_case"`. This matches iCal conventions but diverges from the project's snake_case JSON convention
- `Attendee::from_email()` factory method -- nice convenience

### src/extensions.rs

- `validate_extension_namespace` correctly enforces single-plugin-write semantics
- Returns `Ok(())` for null and non-object extensions -- correct
- `ExtensionError` has proper `Display` and `Error` implementations
- Test coverage is thorough (valid, cross-namespace, empty, null, nested data)
- No validation of the namespace format itself (e.g., verifying reverse-domain format) -- this could be added but is not critical

### src/file_helpers.rs

- `detect_mime_type` delegates to `mime_guess` with proper fallback
- `compute_sha256` streams in 8KB chunks -- memory efficient
- Returns `sha256:{hex}` format -- good for prefixed checksum identification
- `system_time_to_datetime` falls back to `Utc::now()` on conversion failure -- this silent fallback could mask bugs. A `Result` return or at least logging would be safer
- Doc comments with examples are thorough
- Test coverage includes known hashes, empty files, nonexistent files, and subsecond precision

### src/files.rs

- Simple, clean struct with standard CDM fields
- `checksum: String` is a plain string -- no enforcement of format (e.g., `sha256:` prefix)
- `storage_backend: Option<String>` is untyped -- could be an enum if backend types are known

### src/migrations.rs

- `CANONICAL_COLLECTIONS` constant with static data -- good for startup validation
- All versions set to 1 -- correct for initial release
- `CanonicalCollection` uses `&'static str` for names -- zero-allocation, appropriate
- `migration_dir` matches collection name in all cases -- possibly redundant but keeps flexibility for future divergence
- `CANONICAL_PLUGIN_ID` is `"core"` -- used to distinguish canonical from plugin collections
- Tests verify uniqueness and positive versions -- good invariant checks

### src/notes.rs

- `NoteFormat` enum with `Plain`, `Markdown`, `Html` -- covers common formats
- Clean, minimal struct

### src/storage.rs

- `StorageQuery` has `limit: Option<u32>` with doc comment saying "capped at 1000" -- the cap is documented but not enforced in the type itself
- `FilterOp` has 5 operators -- covers basic CRUD needs. Missing `GreaterThan`, `LessThan` (only has `Gte`/`Lte`)
- `StorageMutation::Update` has `expected_version: u64` for optimistic concurrency -- good
- `StorageMutation` variants embed `PipelineMessage` which is heavyweight -- each insert/update carries full metadata. This is intentional for audit trail purposes but worth noting

### src/tasks.rs

- `TaskPriority` and `TaskStatus` both derive `Default` -- good for serde defaults
- `#[default]` attribute on enum variants (Rust 1.80+ feature) -- verify compiler support
- `assignee: Option<String>` and `parent_id: Option<Uuid>` enable delegation and subtasks
- Clean, well-documented struct

### tests/cdm_spec_tests.rs

- Comprehensive spec-compliance tests organized by requirement number
- Tests reference a spec document at `.odm/spec/cdm-specification/requirements.md`
- Some tests verify not-yet-implemented features using JSON-level assertions (e.g., `display` field on `ContactName`)
- `Req 4.2` tests expect a `display` field on `ContactName` which does not exist in the struct -- the test passes because serde ignores unknown fields by default, but this indicates a spec-code divergence
- `Req 3.2` references "active" status but code uses `InProgress` -- the test correctly uses `"in_progress"` string
- `Req 3.3` references "none" and "critical" priorities but code uses `Low` and `Urgent` -- test names are misleading (`req3_3_priority_none_deserialises` tests `"low"`, `req3_3_priority_critical_deserialises` tests `"urgent"`)

### tests/schema_validation.rs

- Validates CDM types against JSON Schema definitions in `.odm/doc/schemas/`
- Tests both valid and invalid fixtures per collection
- Extension field tests verify acceptance on all types except credentials
- Required field and enum constraint tests -- thorough
- Depends on fixture files existing at specific paths -- fragile if project structure changes, but uses `CARGO_MANIFEST_DIR` which is reliable

### tests/validation_tests.rs

- Per-type validation covering: required field rejection, optional field defaults, enum serialization/deserialization, skip_serializing_if, serde rename, and unknown field acceptance
- Thorough and systematic coverage
- Tests confirm serde `deny_unknown_fields` is NOT enabled (which is correct for forward compatibility)

### tests/workflow_tests.rs (new, untracked)

- Tests for `WorkflowRequest`, `WorkflowResponse`, `WorkflowStatus`, `WorkflowError`, `Identity`, `TriggerContext`
- Round-trip serialization tests for all types
- Verifies `skip_serializing_if` for empty params, query, errors, traces
- Tests `WorkflowStatus::http_status_code()` for all 6 variants
- Well-organized by spec requirement

## Problems Found

### Critical

- **SchemaValidated bypass via deserialization** (`pipeline.rs:79-81`) -- `SchemaValidated<T>` uses `#[serde(transparent)]` which means any `T` can be deserialized into `SchemaValidated<T>` without schema validation. The newtype guarantee is only enforced through the `new()` constructor. If code deserializes a `PipelineMessage` from JSON (e.g., from a message queue or API), the `SchemaValidated` wrapper provides no actual validation guarantee. Consider implementing a custom `Deserialize` that rejects `SchemaValidated` from untrusted sources, or document this as an explicit invariant.

### Major

- **Silent test pass in pipeline.rs** (`pipeline.rs:201`) -- The `cdm_type_batch_round_trip` test uses `matches!()` without wrapping in `assert!()`, so the result of the match is discarded and the test always passes regardless of the actual variant. Should be `assert!(matches!(restored, CdmType::TaskBatch(v) if v.is_empty()));`.

- **Spec-code divergence for ContactName.display** (`contacts.rs` vs `cdm_spec_tests.rs:587-616`) -- The CDM spec requires a `display` field on `ContactName`, but the struct only has `given`, `family`, `prefix`, `suffix`, `middle`. Spec tests pass due to serde's unknown-field tolerance, masking this gap. Either add `display: String` to `ContactName` or update the spec.

- **identity.rs and workflow.rs not re-exported from lib.rs** (`lib.rs:14,20`) -- The modules are declared as `pub mod identity` and `pub mod workflow` but their types are not re-exported at the crate root. This forces consumers to write `life_engine_types::workflow::WorkflowRequest` instead of `life_engine_types::WorkflowRequest`. All other CDM modules have root re-exports for their primary types.

- **AttendeeStatus::NeedsAction uses hyphenated serde rename** (`events.rs:122-123`) -- `#[serde(rename = "needs-action")]` uses a hyphen while every other enum variant in the crate uses snake_case. This creates an inconsistency in the JSON API surface. While it matches iCal conventions, it breaks the project's uniform `rename_all = "snake_case"` pattern.

### Minor

- **system_time_to_datetime silently falls back to Utc::now()** (`file_helpers.rs:92-97`) -- If the `SystemTime` cannot be converted (pre-epoch), the function returns `Utc::now()` instead of signaling an error. This could mask data integrity issues in file metadata.

- **No Eq derive on Identity** (`identity.rs:14`) -- `Identity` derives `PartialEq` but not `Eq`. Since all fields (`String`, `HashMap<String, Value>`) support `Eq`, it could be added for completeness.

- **RequestMeta.request_id is String, not Uuid** (`workflow.rs:43`) -- `MessageMetadata.correlation_id` in `pipeline.rs` uses `Uuid`, but `RequestMeta.request_id` uses `String`. This inconsistency means the two ID types have different validation guarantees.

- **StorageQuery limit cap not enforced** (`storage.rs:24`) -- Doc comment says limit is "capped at 1000" but there's no enforcement in the type. Consider a newtype or validation.

- **FilterOp missing strict comparison operators** (`storage.rs:42-53`) -- Only `Gte` and `Lte` are available, not `Gt` and `Lt`. This may be intentional but limits query expressiveness.

- **Misleading test names in cdm_spec_tests.rs** -- Test `req3_3_priority_none_deserialises` actually tests `"low"`, and `req3_3_priority_critical_deserialises` tests `"urgent"`. The names reference old spec terminology that doesn't match the implementation.

- **No PartialEq on PipelineMessage or MessageMetadata** (`pipeline.rs:17-18,26-27`) -- These types lack `PartialEq`, preventing direct equality comparison in tests. The pipeline round-trip test at line 162 works around this by comparing individual fields.

- **storage_backend and checksum are untyped strings** (`files.rs:17,15`) -- `storage_backend` could be an enum, and `checksum` could enforce a format prefix. Low priority since validation typically happens elsewhere.

## Recommendations

1. Fix the `matches!()` bug in `pipeline.rs:201` immediately -- it's a test that silently passes.

2. Add re-exports for `identity` and `workflow` types in `lib.rs` to match the pattern used by all other modules.

3. Address the `ContactName.display` spec divergence -- either add the field or update the spec document.

4. Consider implementing custom `Deserialize` for `SchemaValidated` that prevents deserialization from untrusted sources (or at minimum, add prominent documentation about the invariant).

5. Standardize `AttendeeStatus::NeedsAction` to use `"needs_action"` (snake_case) to match all other enum variants in the crate, or document the iCal exception explicitly.

6. Rename misleading spec test functions to match what they actually test.

7. Consider moving `jsonschema` dependency to `[dev-dependencies]` if `SchemaValidated` validation can be feature-gated, to keep the types crate lightweight.
