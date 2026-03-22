<!--
domain: binary-and-startup
status: draft
tier: 1
updated: 2026-03-22
-->

# Core Binary and Startup Spec

## Overview

This spec defines the Core binary entry point and its 10-step startup sequence. Core is a self-hosted Rust backend that aggregates personal data, stores it locally in an encrypted SQLite database via SQLCipher, and exposes a REST API. It contains no business logic itself — all features are provided by plugins. The startup sequence handles config loading, database encryption, plugin discovery, and HTTP server binding.

## Goals

- Provide a deterministic 10-step startup sequence that is auditable and debuggable
- Load configuration from YAML with environment variable and CLI argument overrides
- Derive the database encryption key from the master passphrase using Argon2id
- Open an encrypted SQLite database via SQLCipher on every launch
- Shut down gracefully on SIGTERM with ordered cleanup
- Expose a health endpoint for monitoring that requires no authentication

## User Stories

- As a self-hosted user, I want Core to start with a single command so that I do not need to configure external services.
- As a developer, I want config overridable via env vars and CLI args so that I can customize behavior per environment.
- As an operator, I want a health endpoint so that I can monitor whether Core is running and accepting requests.
- As a security-conscious user, I want my database encrypted at rest so that data is protected even if the disk is compromised.

## Functional Requirements

- The system must load config from `~/.life-engine/config.yaml` with env var (`LIFE_ENGINE_*`) and CLI overrides.
- The system must validate config and refuse to start with clear errors for invalid or insecure values.
- The system must derive a 32-byte encryption key from the master passphrase using Argon2id (64 MB, 3 iterations, 4 parallelism).
- The system must open the SQLite database via SQLCipher with the derived key, creating it on first launch.
- The system must execute the 10-step startup sequence in order, logging each step.
- The system must shut down gracefully on SIGTERM with a 5-second timeout.
- The system must expose `GET /api/system/health` without authentication.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
