<!--
domain: plugin-manifest
status: draft
tier: 1
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Plugin Manifest Tasks

**Progress:** 0 / 18 tasks complete

## 1.1 — Manifest Data Structures

- [ ] Define `PluginManifest`, `PluginIdentity`, `ActionDecl` structs with serde derives
  <!-- files: crates/le-plugin/src/manifest.rs -->
  <!-- purpose: Create the top-level manifest struct and identity/action types with serde deserialization -->
  <!-- requirements: 1.1, 1.8, 2.1, 8.1 -->

- [ ] Define `CapabilityDecl`, `StorageOp` enum, and deny-by-default defaults
  <!-- files: crates/le-plugin/src/manifest.rs -->
  <!-- purpose: Capability struct with Default impl that denies all capabilities -->
  <!-- requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7 -->

- [ ] Define `CollectionDecl`, `CollectionAccess` enum, `EventDecl`, `EventList`, `ConfigDecl` structs
  <!-- files: crates/le-plugin/src/manifest.rs -->
  <!-- purpose: Collection, event, and config declaration types -->
  <!-- requirements: 4.1, 4.4, 4.5, 4.6, 5.1, 5.4, 6.1 -->

## 1.2 — Validation Error Types

- [ ] Define `ManifestError` enum with structured error variants
  <!-- files: crates/le-plugin/src/manifest_error.rs -->
  <!-- purpose: Error types for all validation failure modes: MissingField, InvalidFormat, SchemaNotFound, DuplicateId, UnknownSection, CapabilityMismatch, ExportMismatch, UnapprovedCapability, ParseError -->
  <!-- requirements: 8.2, 8.5 -->

- [ ] Implement Display and error reporting with plugin_id and field_path context
  <!-- files: crates/le-plugin/src/manifest_error.rs -->
  <!-- purpose: Human-readable error messages with structured context for logging -->
  <!-- requirements: 8.2 -->

## 1.3 — Identity Validation

- [ ] Implement kebab-case regex validation for plugin `id`
  <!-- files: crates/le-plugin/src/manifest_validate.rs -->
  <!-- purpose: Validate id matches ^[a-z][a-z0-9]*(-[a-z0-9]+)*$ -->
  <!-- requirements: 1.5 -->

- [ ] Implement semver format validation for plugin `version`
  <!-- files: crates/le-plugin/src/manifest_validate.rs -->
  <!-- purpose: Validate version matches major.minor.patch format -->
  <!-- requirements: 1.6 -->

- [ ] Implement required-field presence checks for `id`, `name`, `version`
  <!-- files: crates/le-plugin/src/manifest_validate.rs -->
  <!-- purpose: Reject manifests with missing or empty required identity fields -->
  <!-- requirements: 1.2, 1.3, 1.4 -->

## 1.4 — Action Validation

- [ ] Implement minimum-one-action check and action description validation
  <!-- files: crates/le-plugin/src/manifest_validate.rs -->
  <!-- purpose: Ensure at least one action exists and each has a description -->
  <!-- requirements: 2.2, 2.3 -->

- [ ] Implement WASM export verification after module instantiation
  <!-- files: crates/le-plugin/src/manifest_validate.rs, crates/le-plugin/src/loader.rs -->
  <!-- purpose: After instantiation, check each declared action name maps to an exported WASM function -->
  <!-- requirements: 2.6 -->

## 1.5 — Capability Cross-Check

- [ ] Implement capability-section cross-check for events and config
  <!-- files: crates/le-plugin/src/manifest_validate.rs -->
  <!-- purpose: Reject manifests where events.emit is declared without events_emit capability, events.subscribe without events_subscribe, or config without config_read -->
  <!-- requirements: 5.5, 5.6, 6.5 -->

## 1.6 — Collection Validation

- [ ] Implement schema path resolution for `cdm:` prefix and relative paths
  <!-- files: crates/le-plugin/src/manifest_validate.rs -->
  <!-- purpose: Resolve cdm: prefixes to SDK schema files and relative paths to plugin directory -->
  <!-- requirements: 4.1, 4.2, 4.3 -->

- [ ] Implement extension field namespace validation
  <!-- files: crates/le-plugin/src/manifest_validate.rs -->
  <!-- purpose: Verify each extension field matches ext.<plugin-id>.<field> with the declaring plugin's id -->
  <!-- requirements: 4.8 -->

- [ ] Implement extension_schema and collection schema file existence checks
  <!-- files: crates/le-plugin/src/manifest_validate.rs -->
  <!-- purpose: Verify schema files exist at declared paths -->
  <!-- requirements: 4.9, 6.1, 6.2 -->

## 1.7 — Event Validation

- [ ] Implement event name format validation and plugin-id prefix check
  <!-- files: crates/le-plugin/src/manifest_validate.rs -->
  <!-- purpose: Validate dot-separated convention and emit events start with the plugin's id -->
  <!-- requirements: 5.3 -->

## 1.8 — Trust Model

- [ ] Implement first-party auto-grant and third-party approval checking
  <!-- files: crates/le-plugin/src/trust.rs -->
  <!-- purpose: Auto-grant for first-party plugins; check approval list for third-party plugins -->
  <!-- requirements: 7.1, 7.2, 7.3, 7.4 -->

## 1.9 — Orchestration

- [ ] Implement the full validation pipeline in load order
  <!-- files: crates/le-plugin/src/loader.rs -->
  <!-- purpose: Run all validation steps in sequence: parse, identity, duplicate, actions, capability cross-check, collections, events, config, trust, WASM exports -->
  <!-- requirements: 8.3, 8.4 -->

- [ ] Implement duplicate plugin id detection in the plugin registry
  <!-- files: crates/le-plugin/src/loader.rs, crates/le-plugin/src/registry.rs -->
  <!-- purpose: Reject a plugin if its id is already registered -->
  <!-- requirements: 1.7 -->

## 1.10 — Tests

- [ ] Write unit tests for manifest parsing, validation, and error cases
  <!-- files: crates/le-plugin/src/manifest_test.rs -->
  <!-- purpose: Cover happy-path parsing, missing fields, invalid formats, unknown sections, capability cross-checks, collection validation, event naming, trust model scenarios -->
  <!-- requirements: 1.1-1.8, 2.1-2.6, 3.1-3.8, 4.1-4.12, 5.1-5.6, 6.1-6.5, 7.1-7.4, 8.1-8.5 -->
