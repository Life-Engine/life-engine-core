<!--
domain: binary-and-startup
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Implementation Plan — Core Binary and Startup

## Task Overview

This plan implements Core's thin orchestrator binary and 10-step startup sequence. Core lives in `apps/core/` and consists of three files: `main.rs`, `config.rs`, and `shutdown.rs`. Work begins with TOML configuration loading and validation, then moves to Argon2id key derivation and storage backend initialization via the `StorageBackend` trait. The startup orchestrator ties each step together in dependency order, followed by the graceful shutdown handler and structured logging. The health endpoint is owned by the REST transport crate, not Core.

**Progress:** 0 / 13 tasks complete

## Steering Document Compliance

- Config format: TOML for app settings, YAML for workflow definitions (separate files)
- Config delegation: each module owns its own config section
- Argon2id parameters: 64 MB memory, 3 iterations, 4 parallelism, 32-byte output
- Storage via `StorageBackend` trait — Core does not reference SQLCipher directly
- Transports are configurable — only start what is in config
- Core binary is three files: `main.rs`, `config.rs`, `shutdown.rs`
- Graceful shutdown in reverse startup order
- 10-step startup sequence executed in documented order

## Atomic Task Requirements

- **File Scope:** 1-3 related files maximum
- **Time Boxing:** 15-30 minutes per task
- **Single Purpose:** one testable outcome per task
- **Specific Files:** exact file paths specified
- **Agent-Friendly:** clear input/output, minimal context switching

---

## 1.1 — Configuration Structs

> spec: ./brief.md

- [ ] Define top-level config struct with TOML deserialization
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: Define Config struct with sections for storage, auth, transports, workflows, plugins, and logging. Each section is a raw toml::Value that gets handed to the owning module. Include serde derives and default values. -->
  <!-- requirements: 1.1, 1.3 -->

- [ ] Implement config loading with env var overrides
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: Load config.toml, apply LIFE_ENGINE_* env var overrides using underscore-separated key mapping, enforce precedence (env vars override TOML) -->
  <!-- requirements: 1.2, 1.4 -->

- [ ] Implement top-level config validation and section delegation
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: Validate required top-level fields, then hand each section to the owning module for module-specific validation. Log loaded config (excluding secrets) at info level. -->
  <!-- requirements: 2.1, 2.2, 2.4, 2.5 -->

- [ ] Add config loading and validation tests
  <!-- file: apps/core/tests/config_test.rs -->
  <!-- purpose: Test TOML loading, env override, precedence, missing field rejection, section delegation to modules -->
  <!-- requirements: 1.1, 1.2, 1.4, 2.1, 2.2, 2.5 -->

---

## 1.2 — Key Derivation

> spec: ./brief.md

- [ ] Implement Argon2id key derivation function
  <!-- file: packages/crypto/src/kdf.rs -->
  <!-- purpose: Derive a 32-byte key from master passphrase using Argon2id with configured parameters (64MB, 3 iter, 4 parallel). This lives in the crypto crate, not in Core. -->
  <!-- requirements: 3.1, 3.2 -->

- [ ] Add key derivation tests
  <!-- file: packages/crypto/tests/kdf_test.rs -->
  <!-- purpose: Test deterministic key output for same inputs, verify different passphrases produce different keys -->
  <!-- requirements: 3.1, 3.2 -->

---

## 1.3 — Storage Backend Initialization

> spec: ./brief.md

- [ ] Implement storage backend initialization call in Core
  <!-- file: apps/core/src/main.rs -->
  <!-- purpose: Call StorageBackend::init() with derived key and [storage] config section. Handle initialization errors with clear messages. Core does not know about SQLCipher internals — it uses the trait. -->
  <!-- requirements: 4.1, 4.4 -->

- [ ] Implement SQLCipher storage backend init (in storage crate)
  <!-- file: packages/storage-sqlite/src/lib.rs -->
  <!-- purpose: Implement StorageBackend::init() — open SQLCipher database with derived key, create database file on first launch, run pending schema migrations -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4 -->

---

## 1.4 — Startup Orchestrator

> spec: ./brief.md

- [ ] Implement 10-step startup sequence in main
  <!-- file: apps/core/src/main.rs -->
  <!-- purpose: Execute all 10 startup steps in dependency order with step-level logging (step number, name, duration), abort on failure with non-zero exit, log total startup duration -->
  <!-- requirements: 5.1, 5.2, 5.3, 5.4 -->

---

## 1.5 — Graceful Shutdown Handler

> spec: ./brief.md

- [ ] Implement shutdown handler with reverse-order teardown
  <!-- file: apps/core/src/shutdown.rs -->
  <!-- purpose: Listen for SIGTERM/SIGINT, tear down in reverse startup order: stop transports, unload plugins, stop workflow engine, shut down auth, close storage. Enforce configurable timeout. -->
  <!-- requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7 -->

- [ ] Add shutdown integration test
  <!-- file: apps/core/tests/shutdown_test.rs -->
  <!-- purpose: Start Core, send SIGTERM, verify clean exit with zero error output and proper resource cleanup in reverse order -->
  <!-- requirements: 6.1, 6.7 -->

---

## 1.6 — Structured Logging

> spec: ./brief.md

- [ ] Configure tracing with JSON output and startup step logging
  <!-- file: apps/core/src/main.rs -->
  <!-- purpose: Initialize tracing subscriber with JSON formatter and configurable log level. Log each startup step with number, name, and duration. Request-level logging is delegated to transport crates. -->
  <!-- requirements: 8.1, 8.2, 8.3, 8.4 -->
