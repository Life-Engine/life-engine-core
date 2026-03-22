<!--
domain: canonical-data-models
status: draft
tier: 1
updated: 2026-03-23
-->

# Canonical Data Models Spec

## Overview

This spec defines the 7 canonical collection schemas that form the shared data language of the Life Engine ecosystem: Events, Tasks, Contacts, Notes, Emails, Files, and Credentials. Every connector and plugin that works with these data types uses the same field names, types, and semantics, enabling interoperability without per-integration mapping. It also defines the `PipelineMessage` envelope — the standard format for all data flowing through workflow pipelines.

Schemas are published as Rust structs in `packages/types` and JSON Schema files in `.odm/doc/schemas/`. The plugin SDK re-exports all types so plugin authors have a single dependency.

## Goals

- Define a stable, shared schema for 7 canonical collections used across all plugins and connectors
- Define the `PipelineMessage` envelope as the standard data format for workflow pipelines
- Publish schemas as Rust structs in `packages/types` and JSON Schema files in `.odm/doc/schemas/`
- Re-export all CDM types and `PipelineMessage` through `packages/plugin-sdk` for plugin author convenience
- Support an extensions convention for plugin-specific fields that avoids namespace conflicts
- Follow additive-only versioning within a major SDK release to maintain backward compatibility
- Allow plugins to define private collections with custom schemas namespaced to their plugin ID

## User Stories

- As a plugin author, I want to import canonical types directly from the plugin SDK so that I do not need to define my own schemas for common data.
- As a connector author, I want a shared schema for contacts so that data from Google and Outlook connectors can be queried uniformly.
- As a plugin author, I want to attach plugin-specific fields to canonical records via namespaced extensions so that my data coexists with other plugins.
- As a plugin author, I want to receive and return `PipelineMessage` envelopes so that my plugin integrates with any workflow without custom wiring.
- As a Core developer, I want JSON Schema files so that record validation and documentation can be generated automatically.

## Functional Requirements

- The system must define Rust structs with `serde` derives for all 7 canonical collections in `packages/types`.
- The system must define `PipelineMessage`, `MessageMetadata`, and `TypedPayload` in `packages/types`.
- The system must publish JSON Schema files for all 7 collections in `.odm/doc/schemas/`.
- The system must re-export all CDM types and `PipelineMessage` from `packages/plugin-sdk`.
- The system must support a namespaced `extensions` object on 6 collections (all except Credentials).
- The system must enforce additive-only schema changes within a major SDK version.
- The system must support private plugin collections namespaced by plugin ID.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
