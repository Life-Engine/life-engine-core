---
title: "ADR-012: CDM Recommended Schemas with JSON Schema Validation"
type: adr
created: 2026-03-28
status: active
---

# ADR-012: CDM Recommended Schemas with JSON Schema Validation

## Status

Accepted

## Context

Life Engine plugins need to share data. A contacts connector that syncs from CardDAV and an email client that displays sender information both need to agree on what a "contact" looks like. Without a shared data model, every plugin pair that exchanges data requires a custom integration — an approach that does not scale to a plugin ecosystem.

At the same time, the system must support plugins that store private data in their own schemas — a plugin-specific cache, an internal state machine, or a data format that no other plugin needs to understand. Forcing all data through a rigid canonical model would prevent this flexibility.

The question is how to balance interoperability (shared schemas that multiple plugins understand) with flexibility (plugins can define their own collections and extend shared ones).

## Decision

Life Engine defines a Canonical Data Model (CDM) — a set of recommended JSON Schemas for common personal data types. The v1 CDM covers: `events` (calendar), `tasks` (to-dos), `contacts` (people), `notes` (freeform text), `emails` (messages), and `credentials` (OAuth tokens, API keys).

CDM schemas are recommendations, not enforced types. There is no hard distinction between "canonical" and "private" collections — there are just collections, some of which follow a recommended schema. Plugins choose whether to adopt CDM schemas in their manifest by referencing `cdm:<name>`. The incentive is interoperability: plugins that adopt the same schema can read each other's data without custom converters.

Validation is opt-in per collection. When a plugin declares a schema for a collection in its manifest, `StorageContext` validates every write against that schema. If no schema is declared, data is stored as-is.

Plugin-scoped collections are namespaced as `{plugin_id}.{collection_name}` to prevent naming collisions. Shared collections use plain names (e.g., `contacts`, `events`).

Extension fields allow plugins to add custom data to shared collections without modifying the base schema. Extension fields are stored under `ext.{plugin_id}.{field_name}` within a document. This avoids field-name collisions between plugins that extend the same collection.

Schemas use the JSON Schema format (draft 2020-12). Plugins can ship custom schema files in a `schemas/` directory alongside their manifest. Schemas can declare index hints (`x-le-index`) that the document storage adapter uses to create database indexes for frequently queried fields.

## Consequences

Positive consequences:

- Plugins that adopt CDM schemas are interoperable by default. A calendar plugin and a scheduling plugin both reading the `events` collection share the same data shape without any integration work.
- Plugins are not forced into the CDM. A plugin that needs private storage simply declares its own collection and optional schema. No bureaucracy, no approval process.
- Extension fields allow plugins to enrich shared data without breaking other plugins. A CRM plugin can add `ext.crm.lead_score` to contacts without affecting the email client that reads the same collection.
- JSON Schema is a widely-adopted standard with tooling in every language. Plugin authors do not need to learn a proprietary schema format.
- Index hints in schemas let plugin authors optimise query performance declaratively, without writing adapter-specific SQL.

Negative consequences:

- CDM schemas are a social contract, not a technical enforcement. If two plugins declare incompatible schemas for the same collection, `StorageContext` validates writes against the first schema registered. The second plugin's writes may fail validation. Conflict resolution for multi-plugin schema ownership is deferred.
- Validation is per-write only. Existing data that predates a schema change is not retroactively validated. Schema evolution requires careful versioning (see [[schema-versioning-rules]]).
- Extension fields add nesting depth to documents. Queries on extension fields (e.g., filtering by `ext.crm.lead_score`) require the adapter to support nested field access, which may not be efficient on all backends.
- The CDM is a v1 snapshot. As the plugin ecosystem grows, the recommended schemas will need governance to evolve without breaking existing plugins. This governance model is not yet defined.
