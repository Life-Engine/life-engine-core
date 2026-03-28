<!--
domain: encryption-and-audit
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Implementation Plan — Encryption and Audit

## Task Overview

This plan implements Core's encryption-at-rest and audit logging subsystems. Work begins with the shared crypto crate (`packages/crypto`), then integrates SQLCipher into the storage backend, adds per-record credential encryption, and finally implements audit event emission and retention. Each task targets a narrow slice of functionality that can be tested in isolation.

**Progress:** 0 / 16 tasks complete

## Steering Document Compliance

- AES-256-GCM and Argon2id via `packages/crypto` follows Defence in Depth
- Per-record credential encryption independent of database encryption follows Defence in Depth
- Access tokens in memory only follows Principle of Least Privilege
- Shared crypto crate follows The Pit of Success — one correct API surface
- Audit via event bus follows Single Source of Truth — one event flow for all mutations
- No telemetry or external reporting follows No Lock-In and user sovereignty

## Atomic Task Requirements

- **File Scope:** 1-3 related files maximum
- **Time Boxing:** 15-30 minutes per task
- **Single Purpose:** one testable outcome per task
- **Specific Files:** exact file paths specified
- **Agent-Friendly:** clear input/output, minimal context switching

---

## 1.1 — Crypto Crate Foundation

> spec: ./brief.md

- [ ] Create crypto crate with Argon2id key derivation
  <!-- file: packages/crypto/Cargo.toml, packages/crypto/src/lib.rs -->
  <!-- purpose: Set up crate with argon2 dependency, implement derive_key(passphrase, salt, params) -> [u8; 32] with configurable Argon2Params struct -->
  <!-- requirements: 1.1, 1.2, 1.4, 3.3 -->

- [ ] Implement AES-256-GCM encrypt and decrypt functions
  <!-- file: packages/crypto/src/aes.rs, packages/crypto/src/lib.rs -->
  <!-- purpose: Implement encrypt(plaintext, key) and decrypt(ciphertext, key) using AES-256-GCM with random 96-bit nonce prepended to output -->
  <!-- requirements: 3.1, 3.2 -->

- [ ] Implement HMAC-SHA256 sign and verify functions
  <!-- file: packages/crypto/src/hmac.rs, packages/crypto/src/lib.rs -->
  <!-- purpose: Implement hmac_sign(data, key) -> tag and hmac_verify(data, key, tag) -> bool with constant-time comparison -->
  <!-- requirements: 3.4 -->

## 1.2 — Crypto Crate Tests

> spec: ./brief.md

- [ ] Add unit tests for key derivation
  <!-- file: packages/crypto/src/lib.rs -->
  <!-- purpose: Test that derive_key produces deterministic 32-byte output for same passphrase+salt, different output for different inputs, and respects custom params -->
  <!-- requirements: 1.1, 1.2, 3.3 -->

- [ ] Add unit tests for AES-256-GCM round-trip and tamper detection
  <!-- file: packages/crypto/src/aes.rs -->
  <!-- purpose: Test encrypt-then-decrypt round-trip, verify tampered ciphertext returns error, verify different nonce per call -->
  <!-- requirements: 3.1, 3.2 -->

- [ ] Add unit tests for HMAC sign and verify
  <!-- file: packages/crypto/src/hmac.rs -->
  <!-- purpose: Test sign-then-verify round-trip, verify wrong key or tampered data returns false -->
  <!-- requirements: 3.4 -->

## 2.1 — SQLCipher Integration

> spec: ./brief.md

- [ ] Integrate SQLCipher key application into database open
  <!-- file: packages/storage-sqlite/src/lib.rs -->
  <!-- purpose: On open, derive key via packages/crypto, apply via PRAGMA key, enable WAL mode, verify with sqlite_master query -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.4 -->
  <!-- leverage: packages/crypto derive_key -->

- [ ] Implement salt file management for database encryption
  <!-- file: packages/storage-sqlite/src/salt.rs, packages/storage-sqlite/src/lib.rs -->
  <!-- purpose: Read or create 16-byte random salt file alongside database, used as Argon2id salt input -->
  <!-- requirements: 1.1 -->

- [ ] Add config.toml support for Argon2id parameter overrides
  <!-- file: packages/storage-sqlite/src/config.rs -->
  <!-- purpose: Parse [crypto] section from config.toml to override Argon2id memory, iterations, parallelism defaults for low-resource devices -->
  <!-- requirements: 1.4 -->

## 3.1 — Credential Encryption

> spec: ./brief.md

- [ ] Implement per-record credential encryption and decryption
  <!-- file: packages/storage-sqlite/src/credentials.rs -->
  <!-- purpose: Encrypt credential plaintext with per-record salt via packages/crypto before insert, decrypt on read, never log plaintext -->
  <!-- requirements: 4.1, 4.2, 4.3 -->
  <!-- leverage: packages/crypto encrypt/decrypt -->

- [ ] Implement in-memory OAuth token cache with automatic rotation
  <!-- file: packages/storage-sqlite/src/token_cache.rs -->
  <!-- purpose: Hold access tokens in HashMap (never persisted), encrypt refresh tokens at rest, background task rotates before expiry -->
  <!-- requirements: 5.1, 5.2, 5.3, 5.4 -->
  <!-- leverage: packages/crypto, credential encryption -->

## 4.1 — Audit Event Emission

> spec: ./brief.md

- [ ] Emit storage write audit events from StorageContext
  <!-- file: packages/plugin-sdk/src/storage_context.rs -->
  <!-- purpose: After insert/update/delete, emit system.storage.created/updated/deleted events via event bus with collection, record_id, plugin_id -->
  <!-- requirements: 6.1, 6.2, 6.3, 6.6 -->
  <!-- leverage: existing StorageContext, event bus -->

- [ ] Emit blob storage audit events
  <!-- file: packages/storage-sqlite/src/blob.rs -->
  <!-- purpose: After blob store/delete, emit system.blob.stored/deleted events via event bus with blob_key, plugin_id -->
  <!-- requirements: 6.4, 6.5 -->
  <!-- leverage: event bus -->

## 4.2 — Audit Log Persistence

> spec: ./brief.md

- [ ] Implement AuditLogSubscriber event handler
  <!-- file: packages/storage-sqlite/src/audit.rs -->
  <!-- purpose: Subscribe to all system.* audit event types, persist each to audit_log table with event_type, timestamp, details JSON -->
  <!-- requirements: 7.1, 7.2, 7.3, 7.4, 7.5 -->
  <!-- leverage: event bus subscriber trait, StorageBackend -->

- [ ] Create audit_log table DDL with indexes
  <!-- file: packages/storage-sqlite/src/schema.rs -->
  <!-- purpose: Define CREATE TABLE audit_log with id, event_type, plugin_id, details, created_at and indexes on created_at and event_type -->
  <!-- requirements: 7.1, 8.2 -->
  <!-- leverage: existing schema.rs -->

## 4.3 — Audit Log Retention

> spec: ./brief.md

- [ ] Implement daily audit log retention cleanup
  <!-- file: packages/storage-sqlite/src/audit.rs -->
  <!-- purpose: Delete audit_log entries older than configured retention period (default 90 days), run as daily scheduled task, read audit_retention_days from config.toml -->
  <!-- requirements: 8.1, 8.3, 8.4 -->
  <!-- leverage: AuditLogSubscriber, config -->
