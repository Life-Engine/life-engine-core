<!--
domain: data-layer
status: draft
tier: 1
updated: 2026-03-22
-->

# Data Layer Spec

## Overview

This spec defines the storage model, schema, encryption, data access patterns, and query conventions for Core's data layer. All data is stored in a single SQLite database encrypted with SQLCipher. The data model uses a universal document envelope in a `plugin_data` table — plugins never run DDL statements.

## Goals

- Universal document model — a single `plugin_data` table with a JSON data column serves all plugins without dynamic DDL
- Defence in depth encryption — SQLCipher provides full-database encryption with Argon2id key derivation; credentials receive additional per-record encryption
- Comprehensive audit trail — all security-relevant events are logged to an `audit_log` table with 90-day retention
- Queryable JSON — SQLite's `json_extract` enables filtering, sorting, and pagination over plugin data at personal scale

## User Stories

- As a plugin author, I want to store and retrieve structured data via CRUD operations so that my plugin can persist state without managing its own database.
- As a user, I want my data encrypted at rest so that it is protected even if the device is compromised.
- As a user, I want to export all my data in standard formats so that I am never locked in to Life Engine.
- As a maintainer, I want an audit log of security events so that I can investigate incidents and verify correct system behaviour.

## Functional Requirements

- The system must store all plugin data in a single `plugin_data` table with `id`, `plugin_id`, `collection`, `data` (JSON), `version`, `created_at`, and `updated_at` columns.
- The system must enforce optimistic concurrency via the `version` column, rejecting updates with stale versions.
- The system must validate data against canonical schemas (SDK-defined) and private schemas (manifest-defined) before writes.
- The system must encrypt the database with SQLCipher using a key derived from the user's master passphrase via Argon2id.
- The system must log security events (auth attempts, credential access, plugin installs, permission changes) to the `audit_log` table.
- The system must support query filters (equality, comparison, text search, logical operators), sorting, and pagination.
- The system must support full and per-service data exports in standard formats.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
