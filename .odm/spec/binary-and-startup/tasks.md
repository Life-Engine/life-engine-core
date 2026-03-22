<!--
domain: binary-and-startup
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Implementation Plan — Core Binary and Startup

## Task Overview

This plan implements Core's binary entry point and 10-step startup sequence. Work begins with configuration parsing and validation, then moves to Argon2id key derivation and SQLCipher database setup. The startup orchestrator ties each step together in order, followed by the graceful shutdown handler, health endpoint, and structured logging integration. These are foundational tasks that all other specs depend on.

**Progress:** 0 / 14 tasks complete

## Steering Document Compliance

- Config precedence: YAML < env vars < CLI args
- Argon2id parameters: 64 MB memory, 3 iterations, 4 parallelism, 32-byte output
- SQLCipher encryption at rest is always enabled
- Localhost-only by default; TLS required for non-localhost bindings
- 10-step startup sequence executed in documented order

## Atomic Task Requirements

- **File Scope:** 1-3 related files maximum
- **Time Boxing:** 15-30 minutes per task
- **Single Purpose:** one testable outcome per task
- **Specific Files:** exact file paths specified
- **Agent-Friendly:** clear input/output, minimal context switching

---

## 1.1 — Configuration Parser
> spec: ./brief.md

- [ ] Define config struct with serde deserialization
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: Define CoreConfig, AuthConfig, StorageConfig, PluginConfig, NetworkConfig structs with serde derives and default values -->
  <!-- requirements: 1.1, 1.4 -->
  <!-- leverage: existing apps/core/src/config.rs -->

- [ ] Implement config loading with env var and CLI overrides
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: Load YAML, apply LIFE_ENGINE_* env var overrides, apply --dot.notation CLI overrides, enforce precedence -->
  <!-- requirements: 1.2, 1.3, 1.5 -->
  <!-- leverage: existing apps/core/src/config.rs -->

- [ ] Implement config validation
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: Validate required fields, value ranges, TLS requirement for non-localhost, and log loaded config at info level -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.4 -->
  <!-- leverage: existing apps/core/src/config.rs -->

- [ ] Add config loading and validation tests
  <!-- file: tests/config/config_test.rs -->
  <!-- purpose: Test YAML loading, env override, CLI override, precedence, missing field rejection, and TLS enforcement -->
  <!-- requirements: 1.1, 1.2, 1.3, 1.5, 2.1, 2.2, 2.3 -->
  <!-- leverage: packages/test-fixtures -->

---

## 1.2 — Argon2id Key Derivation
> spec: ./brief.md

- [ ] Implement Argon2id key derivation function
  <!-- file: apps/core/src/sqlite_storage.rs -->
  <!-- purpose: Derive a 32-byte key from master passphrase using Argon2id with configured parameters (64MB, 3 iter, 4 parallel) -->
  <!-- requirements: 3.1, 3.2 -->
  <!-- leverage: existing apps/core/src/sqlite_storage.rs -->

- [ ] Add key derivation tests
  <!-- file: tests/storage/key_derivation_test.rs -->
  <!-- purpose: Test deterministic key output for same inputs, and verify different passphrases produce different keys -->
  <!-- requirements: 3.1, 3.2 -->
  <!-- leverage: packages/test-utils -->

---

## 1.3 — SQLCipher Database Setup
> spec: ./brief.md

- [ ] Implement encrypted database open and first-launch creation
  <!-- file: apps/core/src/sqlite_storage.rs -->
  <!-- purpose: Open SQLCipher database with derived key, create database file on first launch, apply PRAGMA key, handle decryption failure -->
  <!-- requirements: 4.1, 4.2, 4.4 -->
  <!-- leverage: existing apps/core/src/sqlite_storage.rs -->

- [ ] Implement schema migration runner
  <!-- file: apps/core/src/storage_migration.rs -->
  <!-- purpose: Run pending schema migrations after successful database open; track applied migrations in a migrations table -->
  <!-- requirements: 4.2, 4.3 -->
  <!-- leverage: existing apps/core/src/storage_migration.rs -->

---

## 1.4 — Startup Orchestrator
> spec: ./brief.md

- [ ] Implement 10-step startup sequence in main
  <!-- file: apps/core/src/main.rs -->
  <!-- purpose: Execute all 10 startup steps in order with step-level logging, abort on failure with non-zero exit, log total duration -->
  <!-- requirements: 5.1, 5.2, 5.3, 5.4 -->
  <!-- leverage: existing apps/core/src/main.rs -->

---

## 1.5 — Graceful Shutdown Handler
> spec: ./brief.md

- [ ] Implement SIGTERM shutdown handler
  <!-- file: apps/core/src/shutdown.rs -->
  <!-- purpose: Listen for SIGTERM, stop accepting connections, drain in-flight requests, stop scheduler, call on_unload, close DB, terminate sidecar -->
  <!-- requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6 -->
  <!-- leverage: existing apps/core/src/shutdown.rs -->

- [ ] Add shutdown integration test
  <!-- file: tests/startup/shutdown_test.rs -->
  <!-- purpose: Start Core, send SIGTERM, verify clean exit with zero error output and proper resource cleanup -->
  <!-- requirements: 6.1, 6.6 -->
  <!-- leverage: packages/test-utils -->

---

## 1.6 — Health Endpoint
> spec: ./brief.md

- [ ] Implement health endpoint
  <!-- file: apps/core/src/routes/health.rs -->
  <!-- purpose: GET /api/system/health returns 200 with status/version/uptime JSON; returns 503 on degraded state; no auth required -->
  <!-- requirements: 7.1, 7.2, 7.3 -->
  <!-- leverage: existing apps/core/src/routes/health.rs -->

---

## 1.7 — Structured Logging
> spec: ./brief.md

- [ ] Configure tracing with JSON output and request logging
  <!-- file: apps/core/src/main.rs -->
  <!-- purpose: Initialize tracing subscriber with JSON formatter, configurable log level, and request/response logging layer for method/path/status/duration -->
  <!-- requirements: 8.1, 8.2, 8.3 -->
  <!-- leverage: existing apps/core/src/main.rs -->
