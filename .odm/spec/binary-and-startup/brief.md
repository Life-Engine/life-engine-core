<!--
domain: binary-and-startup
status: draft
tier: 1
updated: 2026-03-23
-->

# Core Binary and Startup Spec

## Overview

This spec defines the Core binary entry point and its 10-step startup sequence. Core is a thin orchestrator — a self-hosted Rust backend that wires together independent modules but contains no business logic itself. The binary lives in `apps/core/` and consists of three files: `main.rs`, `config.rs`, and `shutdown.rs`. All features are provided by plugins (WASM modules), data flows through declarative workflows (YAML), and protocol handling is delegated to configurable transports.

## Goals

- Provide a deterministic 10-step startup sequence that is auditable and debuggable
- Load application config from TOML with environment variable overrides
- Hand config sections to modules — each module owns its own config parsing
- Derive the database encryption key from the master passphrase using Argon2id
- Initialize the storage backend via the `StorageBackend` trait
- Discover and load plugins from a configured directory (WASM via Extism)
- Load workflow definitions from YAML files in a configured directory
- Start only the transports enabled in config
- Shut down gracefully on SIGTERM with ordered cleanup in reverse startup order

## User Stories

- As a self-hosted user, I want Core to start with a single command so that I do not need to configure external services.
- As a developer, I want config overridable via env vars so that I can customize behavior per environment.
- As an operator, I want a health endpoint so that I can monitor whether Core is running and accepting requests.
- As a security-conscious user, I want my database encrypted at rest so that data is protected even if the disk is compromised.
- As an admin, I want to choose which transports are active so that I only expose the protocols I need.

## Functional Requirements

- The system must load config from `config.toml` with env var (`LIFE_ENGINE_*`) overrides.
- The system must validate config and refuse to start with clear errors for invalid or insecure values.
- The system must hand each config section to the owning module for parsing and validation.
- The system must derive a 32-byte encryption key from the master passphrase using Argon2id (64 MB, 3 iterations, 4 parallelism).
- The system must initialize the storage backend via the `StorageBackend` trait.
- The system must initialize the auth module.
- The system must create a workflow engine and load workflow definitions from YAML files.
- The system must discover and load WASM plugins from the configured plugins directory.
- The system must start only the transports declared in config.
- The system must execute the 10-step startup sequence in order, logging each step.
- The system must shut down gracefully on SIGTERM in reverse startup order with a configurable timeout.
- Health checking is transport-specific — the REST transport provides `GET /api/system/health`.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
