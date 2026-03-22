<!--
domain: binary-and-startup
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Requirements Document — Core Binary and Startup

## Introduction

Core is the central binary of Life Engine. It is a thin orchestrator that wires together independent modules — storage, auth, workflow engine, plugins, and transports — but contains no business logic itself. The binary lives in `apps/core/` and consists of three files: `main.rs`, `config.rs`, and `shutdown.rs`. Everything else lives in independent crates under `packages/`.

Core must start reliably from a single command, load configuration from TOML with environment variable overrides, derive an encryption key for the database, initialize modules in dependency order, and execute a deterministic 10-step startup sequence. On shutdown, it must clean up all resources in reverse order.

## Alignment with Product Vision

- **Self-hosted simplicity** — A single binary with zero external runtime dependencies. Config defaults to localhost-only with encrypted storage.
- **Security by default** — Database encryption via SQLCipher is always enabled. Argon2id key derivation protects against brute force passphrase attacks.
- **Offline-capable** — Core starts and operates fully without internet access; only connector syncs require connectivity.
- **Deterministic startup** — Each step is logged and auditable, making debugging straightforward for self-hosted operators.
- **Thin orchestrator** — Core owns wiring only. Business logic lives in plugins, protocol handling in transports, pipeline execution in the workflow engine, and persistence in the storage backend.

## Requirements

### Requirement 1 — Config Loading

**User Story:** As a developer, I want configuration loaded from TOML with env var overrides, so that I can customize Core per environment without modifying files.

#### Acceptance Criteria

- 1.1. WHEN Core starts THEN the system SHALL read `config.toml` as the base configuration.
- 1.2. WHEN environment variables prefixed with `LIFE_ENGINE_` are set THEN the system SHALL apply them as overrides using underscore-separated key mapping (e.g., `LIFE_ENGINE_STORAGE_PATH` overrides `storage.path`).
- 1.3. WHEN the config file does not exist on first launch THEN the system SHALL create a default config file and proceed with defaults.
- 1.4. WHEN the override precedence is evaluated THEN env vars SHALL override TOML values.

---

### Requirement 2 — Config Validation and Delegation

**User Story:** As a Core operator, I want invalid config rejected at startup with clear messages, so that misconfigurations are caught immediately.

#### Acceptance Criteria

- 2.1. WHEN a required config field is missing THEN the system SHALL refuse to start and log an error specifying the missing field.
- 2.2. WHEN a config value is out of range or the wrong type THEN the system SHALL refuse to start and log the expected format.
- 2.3. WHEN a transport is configured with a non-localhost bind address and TLS is not enabled THEN the system SHALL refuse to start with a message explaining that TLS is required for non-localhost bindings.
- 2.4. WHEN all config values pass validation THEN the system SHALL log the loaded configuration (excluding secrets) at `info` level.
- 2.5. WHEN Core reads a config section (e.g., `[storage]`, `[auth]`, `[transports.rest]`) THEN it SHALL hand that section to the owning module for parsing and validation. Core does not interpret module-specific fields.

---

### Requirement 3 — Database Key Derivation

**User Story:** As a security-conscious user, I want the database encryption key derived from my passphrase using Argon2id, so that the raw key is never stored on disk.

#### Acceptance Criteria

- 3.1. WHEN Core starts THEN the system SHALL derive a 32-byte encryption key from the master passphrase using Argon2id with parameters: 64 MB memory, 3 iterations, 4 parallelism.
- 3.2. WHEN the same passphrase and salt are provided THEN the system SHALL produce the same derived key deterministically.
- 3.3. WHEN the master passphrase is incorrect THEN the storage backend SHALL fail to initialize and the system SHALL log `STORAGE_DECRYPTION_FAILED`.

---

### Requirement 4 — Storage Backend Initialization

**User Story:** As a self-hosted user, I want my database encrypted at rest, so that data is protected even if someone accesses the storage drive.

#### Acceptance Criteria

- 4.1. WHEN Core initializes storage THEN the system SHALL call the `StorageBackend` trait's init method with the derived key and the `[storage]` config section.
- 4.2. WHEN the storage file does not exist (first launch) THEN the storage backend SHALL create it, apply the encryption key, and run initial schema migrations.
- 4.3. WHEN the storage opens successfully THEN the storage backend SHALL run any pending schema migrations before returning.
- 4.4. WHEN the storage file exists but the key is wrong THEN the storage backend SHALL return an error and Core SHALL refuse to start with a clear decryption error.

---

### Requirement 5 — Startup Sequence

**User Story:** As a Core operator, I want the startup sequence to execute in deterministic order with logging, so that I can diagnose issues at any step.

#### Acceptance Criteria

- 5.1. WHEN Core starts THEN the system SHALL execute the 10 startup steps in the documented order: load config, validate config, derive database key, initialize storage backend, initialize auth module, create workflow engine, load workflow definitions, discover and load plugins, start active transports, wait for shutdown signal.
- 5.2. WHEN each step begins THEN the system SHALL log the step number and name at `info` level.
- 5.3. WHEN a step fails THEN the system SHALL log the error and abort startup with a non-zero exit code.
- 5.4. WHEN all 10 steps complete THEN the system SHALL log a startup-complete message with the total duration.

---

### Requirement 6 — Graceful Shutdown

**User Story:** As a Core operator, I want Core to shut down cleanly on SIGTERM, so that in-flight requests complete and data is not corrupted.

#### Acceptance Criteria

- 6.1. WHEN Core receives `SIGTERM` THEN the system SHALL begin graceful shutdown in reverse startup order.
- 6.2. WHEN active transports are running THEN the system SHALL stop each transport, ceasing to accept new connections and draining in-flight requests.
- 6.3. WHEN plugins are loaded THEN the system SHALL unload each WASM plugin.
- 6.4. WHEN the workflow engine is running THEN the system SHALL stop the scheduler and wait for running tasks to complete within the shutdown timeout.
- 6.5. WHEN the auth module is initialized THEN the system SHALL shut it down.
- 6.6. WHEN the storage backend is open THEN the system SHALL close it.
- 6.7. WHEN the configurable shutdown timeout is exceeded THEN the system SHALL force shutdown and log a warning.

---

### Requirement 7 — Health Endpoint (Transport-Specific)

**User Story:** As an operator, I want a health endpoint, so that monitoring tools can check if Core is running.

#### Acceptance Criteria

- 7.1. WHEN the REST transport is active and a client sends `GET /api/system/health` THEN the transport SHALL return HTTP 200 with a JSON body containing `status`, `version`, and `uptime` fields.
- 7.2. WHEN the health endpoint is called THEN the transport SHALL NOT require authentication.
- 7.3. WHEN Core is in a degraded state (e.g., storage backend unreachable) THEN the health endpoint SHALL return HTTP 503 with a `status: "degraded"` field.
- 7.4. The health endpoint is owned by the REST transport crate, not by Core. Core provides system status via a shared state object that transports can query.

---

### Requirement 8 — Structured Logging

**User Story:** As a developer, I want structured JSON logs with request metadata, so that I can debug issues efficiently.

#### Acceptance Criteria

- 8.1. WHEN Core logs any event THEN the output SHALL be structured JSON with `timestamp`, `level`, `module`, and `message` fields.
- 8.2. WHEN a startup step begins or completes THEN the system SHALL log the step number, name, and duration.
- 8.3. WHEN `log_level` is configured THEN the system SHALL filter log output to that level and above.
- 8.4. Request-level logging (method, path, status, duration) is the responsibility of each transport crate, not Core.
