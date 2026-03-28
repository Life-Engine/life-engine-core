---
title: Encryption and Audit Specification
type: reference
created: 2026-03-27
updated: 2026-03-28
status: active
tags:
  - life-engine
  - core
  - encryption
  - security
  - audit
---

# Encryption and Audit Specification

Part of [[architecture/core/overview|Core Overview]] · [[architecture/core/README|Core Documentation]]

Technical specification for Core's encryption-at-rest implementation and audit logging. For the conceptual data layer design, see [[data]].

## Encryption at Rest

- **Database** — SQLCipher (transparent, full-database encryption)
- **Key derivation** — Argon2id (64 MB memory, 3 iterations, 4 parallelism). Configurable for low-resource devices.
- **Master passphrase** — User provides at first launch. Derived key unlocks the database.
- **Shared crypto crate** — Encryption primitives (AES-256-GCM, key derivation, HMAC) live in `packages/crypto`, shared across modules.

## Credential Storage

- Each credential encrypted separately with a key derived from the master passphrase
- OAuth refresh tokens encrypted on disk, access tokens in memory only
- Automatic rotation before token expiry
- Defence-in-depth: individual field-level encryption is independent of adapter-level encryption. Credentials are encrypted per-field even when the underlying storage adapter already encrypts at rest.

## Audit Logging

Audit events are emitted via the event bus, not a separate logging system. StorageContext emits the following system events on write operations:

- `system.storage.created` — A record was created in a collection.
- `system.storage.updated` — A record was updated in a collection.
- `system.storage.deleted` — A record was deleted from a collection.
- `system.blob.stored` — A blob was stored in blob storage.
- `system.blob.deleted` — A blob was deleted from blob storage.

Read operations are not audited.

Additional security-relevant events are also logged: auth attempts, credential access, plugin installs, permission changes, and connector auth/revocation.

Audit log retention:

- Rotated daily, retained 90 days, encrypted at rest
- No telemetry or external reporting
