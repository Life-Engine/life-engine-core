<!--
domain: cdm-specification
updated: 2026-03-28
-->

# CDM Specification Spec

## Overview

This spec defines the Canonical Data Model (CDM) for the Life Engine ecosystem. The CDM provides 6 recommended collection schemas (Events, Tasks, Contacts, Notes, Emails, and Credentials) that form the common language shared by all connectors and plugins. Every connector normalises external data into these types, and every plugin reads and writes through them.

There is no hard distinction between "canonical" and "private" collections. There are just collections, some of which follow a CDM recommended schema. The CDM also defines a namespaced extensions convention (`ext`) for plugin-specific data, a plugin-scoped collections convention for custom data types, and implementor guidance for connector authors.

Type definitions live in three locations: Rust structs in `packages/types/src/` (authoritative source), TypeScript interfaces in `packages/plugin-sdk-js/src/index.ts`, and JSON Schemas in `docs/schemas/`. The Rust structs are the single source of truth; TypeScript interfaces and JSON Schemas are derived from them and must stay in sync.

## Goals

- Define stable, shared schemas for 6 CDM recommended collections used across all plugins and connectors
- Publish schemas as Rust structs in `packages/types`, TypeScript interfaces in `packages/plugin-sdk-js`, and JSON Schema files in `docs/schemas/`
- Enforce common fields (`id`, `source`, `source_id`, `created_at`, `updated_at`, `ext`) across all collections
- Support a namespaced extensions convention (`ext.{plugin_id}.{field_name}`) for plugin-specific data
- Allow plugins to define private collections namespaced by plugin ID
- Follow additive-only versioning within a major SDK release
- Provide clear implementor guidance for connector authors mapping external data to CDM types

## User Stories

- As a connector author, I want shared schemas for common data types so that data from different external systems can be queried uniformly.
- As a plugin author, I want to attach plugin-specific fields to CDM records via namespaced extensions so that my data coexists with other plugins without conflicts.
- As a plugin author, I want to define private collections for data that does not fit the 6 CDM types so that I can store plugin-specific structured data.
- As a Core developer, I want JSON Schema files so that record validation and documentation can be generated automatically.
- As a connector author, I want clear guidance on mapping external data to CDM fields so that I preserve round-trip fidelity during sync.

## Functional Requirements

- The system must define Rust structs with `serde` derives for all 6 CDM recommended collections in `packages/types`.
- The system must publish JSON Schema files for all 6 collections in `docs/schemas/`.
- The system must publish TypeScript interfaces for all 6 collections in `packages/plugin-sdk-js`.
- The system must enforce common fields on all collections: `id`, `source`, `source_id`, `created_at`, `updated_at`.
- The system must support a namespaced `ext` field on 5 collections (all except Credentials).
- The system must support plugin-scoped collections namespaced by plugin ID.
- The system must enforce additive-only schema changes within a major SDK version.
- The system must preserve all extension namespaces during writes (merge, not replace).

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
