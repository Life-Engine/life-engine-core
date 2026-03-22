<!--
domain: binary-and-startup
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Requirements Document — Core Binary and Startup

## Introduction

Core is the central backend process of Life Engine. It must start reliably from a single command, load configuration from multiple sources with clear precedence, derive an encryption key for the database, and execute a deterministic 10-step startup sequence. On shutdown, it must clean up all resources in order. This requirements document specifies the precise behavior for each phase of Core's lifecycle.

## Alignment with Product Vision

- **Self-hosted simplicity** — A single binary with zero external runtime dependencies. Config defaults to localhost-only with encrypted storage.
- **Security by default** — Database encryption via SQLCipher is always enabled. Argon2id key derivation protects against brute force passphrase attacks.
- **Offline-capable** — Core starts and operates fully without internet access; only connector syncs require connectivity.
- **Deterministic startup** — Each step is logged and auditable, making debugging straightforward for self-hosted operators.

## Requirements

### Requirement 1 — Config Loading

**User Story:** As a developer, I want configuration loaded from YAML with env var and CLI overrides, so that I can customize Core per environment without modifying files.

#### Acceptance Criteria

- 1.1. WHEN Core starts THEN the system SHALL read `~/.life-engine/config.yaml` as the base configuration.
- 1.2. WHEN environment variables prefixed with `LIFE_ENGINE_` are set THEN the system SHALL apply them as overrides using underscore-separated key mapping (e.g., `LIFE_ENGINE_CORE_PORT` overrides `core.port`).
- 1.3. WHEN CLI arguments are provided in dot notation (e.g., `--core.port=3750`) THEN the system SHALL apply them as the highest-priority overrides.
- 1.4. WHEN the config file does not exist on first launch THEN the system SHALL create a default config file and proceed with defaults.
- 1.5. WHEN the override precedence is evaluated THEN CLI arguments SHALL override env vars, which SHALL override YAML values.

---

### Requirement 2 — Config Validation

**User Story:** As a Core operator, I want invalid config rejected at startup with clear messages, so that misconfigurations are caught immediately.

#### Acceptance Criteria

- 2.1. WHEN a required config field is missing THEN the system SHALL refuse to start and log an error specifying the missing field.
- 2.2. WHEN a config value is out of range or the wrong type THEN the system SHALL refuse to start and log the expected format.
- 2.3. WHEN `core.host` is not `127.0.0.1` and TLS is not enabled THEN the system SHALL refuse to start with a message explaining that TLS is required for non-localhost bindings.
- 2.4. WHEN all config values pass validation THEN the system SHALL log the loaded configuration (excluding secrets) at `info` level.

---

### Requirement 3 — Database Key Derivation

**User Story:** As a security-conscious user, I want the database encryption key derived from my passphrase using Argon2id, so that the raw key is never stored on disk.

#### Acceptance Criteria

- 3.1. WHEN Core starts THEN the system SHALL derive a 32-byte encryption key from the master passphrase using Argon2id with parameters: 64 MB memory, 3 iterations, 4 parallelism.
- 3.2. WHEN the same passphrase and salt are provided THEN the system SHALL produce the same derived key deterministically.
- 3.3. WHEN the master passphrase is incorrect THEN the SQLCipher database SHALL fail to open and the system SHALL log `STORAGE_DECRYPTION_FAILED`.

---

### Requirement 4 — Encrypted Database

**User Story:** As a self-hosted user, I want my database encrypted at rest, so that data is protected even if someone accesses the storage drive.

#### Acceptance Criteria

- 4.1. WHEN Core opens the database THEN the system SHALL use SQLCipher with the Argon2id-derived key.
- 4.2. WHEN the database file does not exist (first launch) THEN the system SHALL create it, apply the encryption key, and run initial schema migrations.
- 4.3. WHEN the database opens successfully THEN the system SHALL run any pending schema migrations before proceeding to the next startup step.
- 4.4. WHEN the database file exists but the key is wrong THEN the system SHALL refuse to start with a clear decryption error.

---

### Requirement 5 — Startup Sequence

**User Story:** As a Core operator, I want the startup sequence to execute in deterministic order with logging, so that I can diagnose issues at any step.

#### Acceptance Criteria

- 5.1. WHEN Core starts THEN the system SHALL execute the 10 startup steps in the documented order: config, validate, derive key, open DB, spawn Pocket ID, load plugins, register routes, load workflows, start scheduler, bind HTTP.
- 5.2. WHEN each step begins THEN the system SHALL log the step number and name at `info` level.
- 5.3. WHEN a step fails THEN the system SHALL log the error and abort startup with a non-zero exit code.
- 5.4. WHEN all 10 steps complete THEN the system SHALL log a startup-complete message with the total duration.

---

### Requirement 6 — Graceful Shutdown

**User Story:** As a Core operator, I want Core to shut down cleanly on SIGTERM, so that in-flight requests complete and data is not corrupted.

#### Acceptance Criteria

- 6.1. WHEN Core receives `SIGTERM` THEN the system SHALL stop accepting new HTTP connections immediately.
- 6.2. WHEN in-flight requests exist THEN the system SHALL wait up to 5 seconds for them to complete.
- 6.3. WHEN the scheduler has running tasks THEN the system SHALL wait for them to finish within the shutdown timeout.
- 6.4. WHEN plugins are loaded THEN the system SHALL call `on_unload` on each plugin during shutdown.
- 6.5. WHEN all cleanup completes THEN the system SHALL close the database connection and terminate the Pocket ID sidecar.
- 6.6. WHEN the 5-second timeout is exceeded THEN the system SHALL force shutdown and log a warning.

---

### Requirement 7 — Health Endpoint

**User Story:** As an operator, I want an unauthenticated health endpoint, so that monitoring tools can check if Core is running.

#### Acceptance Criteria

- 7.1. WHEN a client sends `GET /api/system/health` THEN the system SHALL return HTTP 200 with a JSON body containing `status`, `version`, and `uptime` fields.
- 7.2. WHEN the health endpoint is called THEN the system SHALL NOT require authentication.
- 7.3. WHEN Core is in a degraded state (e.g., database unreachable) THEN the health endpoint SHALL return HTTP 503 with a `status: "degraded"` field.

---

### Requirement 8 — Structured Logging

**User Story:** As a developer, I want structured JSON logs with request metadata, so that I can debug issues efficiently.

#### Acceptance Criteria

- 8.1. WHEN Core logs any event THEN the output SHALL be structured JSON with `timestamp`, `level`, `module`, and `message` fields.
- 8.2. WHEN an HTTP request completes THEN the system SHALL log the method, path, status code, and duration.
- 8.3. WHEN `core.log_level` is configured THEN the system SHALL filter log output to that level and above.
