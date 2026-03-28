<!--
domain: schema-versioning-rules
status: draft
tier: cross-cutting
updated: 2026-03-28
-->

# Schema Versioning Rules Spec

## Overview

This spec defines the versioning, compatibility classification, migration, and deprecation rules that govern all CDM schema changes and plugin private collection schema changes in Life Engine. Schema versions are coupled to SDK semver — there is no separate schema version number. The rules protect the interoperability contract between Core, the two SDKs (`packages/plugin-sdk-rs` and `packages/plugin-sdk-js`), and every plugin in the ecosystem.

Within a major SDK version, CDM schemas follow an additive-only rule: fields may be added (if optional), enum values may be added, new collections may be added, and constraints may be relaxed — but no other changes are permitted. Breaking changes require a major SDK version bump, a 12-month maintenance window for the previous major, and migration transforms shipped alongside the release.

## Goals

- Protect plugin compatibility across minor SDK releases via the additive-only rule
- Classify every possible schema change as breaking or non-breaking with clear, auditable criteria
- Enforce compatibility rules automatically in CI so that breaking changes cannot merge without a major version bump
- Provide a deterministic migration path for breaking changes with idempotent transform scripts
- Give plugin authors a 12-month window to adopt breaking changes before end-of-life
- Require deprecation notices in CHANGELOG, Rust SDK, and TypeScript SDK before any removal
- Apply consistent versioning discipline to plugin private collections

## User Stories

- As an SDK maintainer, I want clear criteria for classifying schema changes so that I can decide whether a proposed change requires a minor or major version bump.
- As a plugin author, I want confidence that upgrading to any minor SDK release will not break my plugin so that I can adopt new SDK versions without risk.
- As a plugin author, I want at least 12 months to migrate to a new major SDK version so that I am not forced into emergency updates.
- As a Core contributor, I want CI to block breaking schema changes that lack a major version bump so that compatibility violations are caught before merge.
- As a Core contributor, I want a standardised migration transform format so that I know exactly how to write and test data migrations.
- As a plugin author, I want deprecation warnings in my build output so that I know which fields or values will be removed in the next major version.

## Functional Requirements

- The system must couple CDM schema versions to SDK semver with no separate schema version number.
- The system must classify schema changes into breaking and non-breaking categories and enforce the additive-only rule within a major version.
- The system must run a schema compatibility check in CI on every PR that modifies schema or type files, blocking merges when breaking changes lack a major version bump.
- The system must support a migration transform format in plugin manifests with idempotent, single-record transform functions.
- The system must run migration transforms on startup after an SDK or plugin upgrade, logging results to the audit log.
- The system must enforce a 12-month maintenance window for the previous major SDK version before end-of-life.
- The system must require deprecation notices in CHANGELOG, Rust SDK attributes, and TypeScript SDK JSDoc annotations before any field or enum value removal.
- The system must publish JSON Schema files at versioned URIs with major-version-prefixed directories and a stable symlink for the current version.
- The system must apply the same compatibility classification and migration rules to plugin private collections, scoped to the plugin's own manifest version.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
