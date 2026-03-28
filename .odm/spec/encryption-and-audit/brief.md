<!--
domain: encryption-and-audit
updated: 2026-03-28
-->

# Encryption and Audit Spec

## Overview

This spec defines Core's encryption-at-rest strategy and audit logging subsystem. All persistent data is encrypted via SQLCipher with an Argon2id-derived key. Credentials receive an additional layer of per-record encryption using `packages/crypto`. Audit events flow through the event bus and are persisted in an `audit_log` table with 90-day retention. No telemetry or external reporting is involved — all audit data stays local.

## Goals

- Full-database encryption via SQLCipher with Argon2id key derivation from a user-provided master passphrase
- Shared crypto crate (`packages/crypto`) providing AES-256-GCM, Argon2id, and HMAC primitives reused across all modules
- Defence-in-depth credential storage with per-record encryption independent of adapter-level encryption
- OAuth token hygiene: refresh tokens encrypted at rest, access tokens held in memory only, automatic rotation before expiry
- Audit events emitted via the event bus on every write operation (`system.storage.created`, `system.storage.updated`, `system.storage.deleted`, `system.blob.stored`, `system.blob.deleted`)
- Security-relevant event logging for auth attempts, credential access, plugin installs, permission changes, and connector auth/revocation
- 90-day audit log retention with daily rotation, encrypted at rest, no external reporting

## User Stories

- As a user, I want my database encrypted at rest so that my data is protected even if my device is stolen or compromised.
- As a user, I want my credentials individually encrypted so that a database-level compromise does not expose all secrets at once.
- As a user, I want security events logged locally so that I can review what happened and when.
- As a module developer, I want a shared crypto crate so that I can use consistent encryption primitives without reimplementing them.
- As a user, I want OAuth tokens handled securely so that refresh tokens are encrypted and access tokens never touch disk.
- As a user, I want audit logs automatically cleaned up so that old entries do not consume storage indefinitely.

## Functional Requirements

- The system must derive the database encryption key from the user's master passphrase using Argon2id (64 MB memory, 3 iterations, 4 parallelism) via `packages/crypto`.
- The system must use SQLCipher to provide transparent full-database encryption for the SQLite storage backend.
- The system must encrypt each credential individually with a key derived from the master passphrase before writing to storage.
- The system must hold OAuth access tokens in memory only and encrypt refresh tokens at rest.
- The system must automatically rotate OAuth tokens before expiry.
- The system must emit audit events via the event bus for all storage write operations and blob operations.
- The system must log security-relevant events including auth attempts, credential access, plugin installs, permission changes, and connector auth/revocation.
- The system must rotate audit logs daily and retain them for 90 days.
- The system must never send audit data externally — all audit data stays local.
- The `packages/crypto` crate must export AES-256-GCM encryption/decryption, Argon2id key derivation, and HMAC utilities.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
