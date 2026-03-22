<!--
domain: canonical-data-models
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Spec — Canonical Data Models

## Contents

- [[#Purpose]]
- [[#Principle]]
- [[#Canonical Collections]]
  - [[#Events]]
  - [[#Tasks]]
  - [[#Contacts]]
  - [[#Notes]]
  - [[#Emails]]
  - [[#Files]]
  - [[#Credentials]]
- [[#Extensions Convention]]
- [[#Schema Versioning]]
- [[#JSON Schema Files]]
- [[#Private Collections]]
- [[#Acceptance Criteria]]

## Purpose

This spec defines the 7 canonical collection schemas that form the shared data language of the Life Engine ecosystem. Every connector and plugin that works with these data types uses the same field names, types, and semantics — enabling interoperability without per-integration mapping.

Reference: [[03 - Projects/Life Engine/Design/Core/Data]]

## Principle

Canonical collections are platform-owned and defined in both SDKs (Rust structs in `plugin-sdk-rs`, TypeScript interfaces in `plugin-sdk-js`). They are shared across all plugins and connectors.

Using canonical types is the path of least resistance for plugin authors. A plugin that reads or writes canonical collections needs no schema definition — the types are already available as imports. Plugins only define custom schemas when they need private collections with non-canonical shapes.

## Canonical Collections

### Events

Calendar events with support for recurrence, attendees, and location.

Fields:

- **id** (string, required) — Unique identifier assigned by Core
- **title** (string, required) — Event title
- **start** (string, required) — Start datetime in ISO 8601 format with timezone
- **end** (string, required) — End datetime in ISO 8601 format with timezone
- **recurrence** (string, optional) — Recurrence rule in RRULE format (RFC 5545)
- **attendees** (string array, optional) — List of attendee email addresses or identifiers
- **location** (string, optional) — Free-text location or structured address
- **description** (string, optional) — Event description or notes
- **source** (string, required) — Identifier of the connector or plugin that produced this record
- **source_id** (string, required) — The record's original ID in the source system
- **extensions** (object, optional) — Namespaced object for plugin-specific fields (see [[#Extensions Convention]])

### Tasks

Actionable items with status tracking and priority levels.

Fields:

- **id** (string, required) — Unique identifier assigned by Core
- **title** (string, required) — Task title
- **description** (string, optional) — Detailed description of the task
- **status** (string enum, required) — One of: `pending`, `active`, `completed`, `cancelled`
- **priority** (string enum, required) — One of: `none`, `low`, `medium`, `high`, `critical`
- **due_date** (string, optional) — Due date in ISO 8601 format
- **labels** (string array, optional) — User-defined labels for categorisation
- **source** (string, required) — Identifier of the connector or plugin that produced this record
- **source_id** (string, required) — The record's original ID in the source system
- **extensions** (object, optional) — Namespaced object for plugin-specific fields

### Contacts

People records with structured name, communication channels, and organisation.

Fields:

- **id** (string, required) — Unique identifier assigned by Core
- **name** (object, required) — Structured name with the following sub-fields:
  - **given** (string, required) — Given/first name
  - **family** (string, required) — Family/last name
  - **display** (string, required) — Full display name as the user prefers it
- **emails** (array, optional) — List of email addresses, each an object with:
  - **address** (string, required) — Email address
  - **type** (string, optional) — Label such as `work`, `personal`, `other`
  - **primary** (boolean, optional) — Whether this is the primary email
- **phones** (array, optional) — List of phone numbers, each an object with:
  - **number** (string, required) — Phone number (E.164 format recommended)
  - **type** (string, optional) — Label such as `mobile`, `work`, `home`
- **addresses** (array, optional) — List of postal addresses, each an object with street, city, state, postcode, and country fields
- **organisation** (string, optional) — Company or organisation name
- **source** (string, required) — Identifier of the connector or plugin that produced this record
- **source_id** (string, required) — The record's original ID in the source system
- **extensions** (object, optional) — Namespaced object for plugin-specific fields

### Notes

Text content with optional tagging.

Fields:

- **id** (string, required) — Unique identifier assigned by Core
- **title** (string, required) — Note title
- **body** (string, required) — Note content in markdown or plain text
- **tags** (string array, optional) — Tags for categorisation and search
- **source** (string, required) — Identifier of the connector or plugin that produced this record
- **source_id** (string, required) — The record's original ID in the source system
- **extensions** (object, optional) — Namespaced object for plugin-specific fields

### Emails

Email messages with threading and attachment support.

Fields:

- **id** (string, required) — Unique identifier assigned by Core
- **from** (string, required) — Sender email address
- **to** (string array, required) — Recipient email addresses
- **cc** (string array, optional) — CC recipient email addresses
- **bcc** (string array, optional) — BCC recipient email addresses
- **subject** (string, required) — Email subject line
- **body_text** (string, required) — Plain text body
- **body_html** (string, optional) — HTML body for rich rendering
- **thread_id** (string, optional) — Thread identifier for conversation grouping
- **labels** (string array, optional) — Labels or folder assignments
- **attachments** (array, optional) — List of file references, each an object with:
  - **file_id** (string, required) — Reference to a record in the files collection
  - **filename** (string, required) — Original filename
  - **mime_type** (string, required) — MIME type of the attachment
  - **size** (integer, required) — Size in bytes
- **source** (string, required) — Identifier of the connector or plugin that produced this record
- **source_id** (string, required) — The record's original ID in the source system
- **extensions** (object, optional) — Namespaced object for plugin-specific fields

### Files

File metadata records with integrity verification.

Fields:

- **id** (string, required) — Unique identifier assigned by Core
- **name** (string, required) — Filename including extension
- **mime_type** (string, required) — MIME type (e.g. `application/pdf`, `image/png`)
- **size** (integer, required) — File size in bytes
- **path** (string, required) — Storage path relative to the Core data directory
- **checksum** (string, optional) — SHA-256 hash of the file contents for integrity verification
- **source** (string, required) — Identifier of the connector or plugin that produced this record
- **source_id** (string, required) — The record's original ID in the source system
- **extensions** (object, optional) — Namespaced object for plugin-specific fields

### Credentials

Secure credential storage with typed claims.

Fields:

- **id** (string, required) — Unique identifier assigned by Core
- **type** (string enum, required) — One of: `oauth_token`, `api_key`, `identity_document`, `passkey`
- **issuer** (string, required) — The authority that issued the credential (e.g. `google.com`, `github.com`, `au.gov`)
- **issued_date** (string, required) — Date the credential was issued in ISO 8601 format
- **expiry_date** (string, optional) — Expiration date in ISO 8601 format (null for non-expiring credentials)
- **claims** (object, required) — Type-specific data. The shape depends on the credential type:
  - For `oauth_token`: `access_token`, `refresh_token`, `scopes`, `token_type`
  - For `api_key`: `key`, `prefix`, `permissions`
  - For `identity_document`: `document_type`, `document_number`, `holder_name`, `country`
  - For `passkey`: `credential_id`, `public_key`, `relying_party`, `user_handle`
- **source** (string, required) — Identifier of the connector or plugin that produced this record
- **source_id** (string, required) — The record's original ID in the source system

Note: Credentials do not have an `extensions` field. The `claims` object serves the same purpose by carrying type-specific data.

## Extensions Convention

The `extensions` field is a namespaced object that plugins use to attach plugin-specific data to canonical records without conflicting with other plugins. Each plugin writes under its own reverse-domain namespace.

Example of a canonical task with extensions from two plugins:

```json
{
  "id": "task_abc123",
  "title": "Review pull request",
  "status": "active",
  "priority": "high",
  "source": "com.life-engine.github",
  "source_id": "PR-456",
  "extensions": {
    "com.life-engine.github": {
      "repo": "life-engine/core",
      "pr_number": 456,
      "review_state": "changes_requested"
    },
    "com.example.pomodoro": {
      "estimated_pomodoros": 2,
      "completed_pomodoros": 1
    }
  }
}
```

Rules for extensions:

- Keys must be reverse-domain strings matching the plugin's ID
- Plugins must only write to their own namespace
- Core preserves all extension data during sync and merge operations
- Extensions are optional — omitting the field entirely is valid

## Schema Versioning

Canonical schemas follow additive-only versioning within a major SDK release.

- Adding new optional fields is a non-breaking change and can happen in any minor SDK release
- Removing fields or changing field types is a breaking change and requires a major SDK version bump
- When a major version introduces breaking schema changes, the previous version continues to receive security fixes for 12 months
- Core handles schema migration between versions during the overlap period

## JSON Schema Files

JSON Schema definitions for all 7 canonical collections are published in the monorepo at `.odm/docs/schemas/`. Each collection has its own schema file:

- `.odm/docs/schemas/events.schema.json`
- `.odm/docs/schemas/tasks.schema.json`
- `.odm/docs/schemas/contacts.schema.json`
- `.odm/docs/schemas/notes.schema.json`
- `.odm/docs/schemas/emails.schema.json`
- `.odm/docs/schemas/files.schema.json`
- `.odm/docs/schemas/credentials.schema.json`

These schemas are used for validation in tests, documentation generation, and can be consumed by third-party tools.

## Private Collections

Plugins that need data structures beyond the 7 canonical types can define private collections. Private collections are namespaced to the plugin and follow these conventions:

- The collection name is prefixed with the plugin ID (e.g. `com.life-engine.todos.checklists`)
- The plugin provides a JSON Schema definition in its manifest under the `collections` field
- Core validates records against the provided schema on write
- Other plugins cannot access a plugin's private collections unless the owning plugin exposes them through its API

## Acceptance Criteria

1. All 7 canonical schemas have corresponding Rust structs in `plugin-sdk-rs` with `serde` derives
2. All 7 canonical schemas have corresponding TypeScript interfaces in `plugin-sdk-js`
3. JSON Schema files in `.odm/docs/schemas/` validate successfully against test fixtures for each collection
4. Extension namespaces do not conflict across plugins — Core rejects writes to another plugin's namespace
5. Schema changes within a major version are verified as additive-only in CI
