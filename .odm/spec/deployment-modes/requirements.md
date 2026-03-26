<!--
domain: deployment-modes
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Requirements Document — Deployment Modes

## Introduction

Life Engine Core supports three deployment modes to serve users ranging from technical standalone users to self-hosting enthusiasts. Each mode uses the same Core binary but with different lifecycle management, configuration, and network exposure. Core is a thin binary — all deployment modes require the binary plus a `config.toml`, a `plugins/` directory (WASM modules), and a `workflows/` directory (YAML pipeline definitions). This document specifies the requirements for each mode and the network security enforcement that applies across all modes.

## Alignment with Product Vision

- **Defence in Depth** — TLS + authentication + rate limiting are enforced together for any non-localhost exposure; Core refuses to start without them
- **Explicit Over Implicit** — Network exposure requires deliberate configuration changes; the default is localhost-only with no ports exposed
- **Single Source of Truth** — Same binary across all modes, same config format (TOML)
- **The Pit of Success** — The secure default (localhost-only, no TLS needed) is also the easiest to configure

## Requirements

### Requirement 1 — Standalone Binary

**User Story:** As a technical user, I want to run Core as an independent process on any machine, so that I can access it remotely from my devices.

#### Acceptance Criteria

- 1.1. WHEN Core is started in standalone mode THEN the system SHALL read configuration from `~/.config/life-engine/config.toml` (Linux/macOS) or `%APPDATA%\life-engine\config.toml` (Windows), with environment variables overriding file values.
- 1.2. WHEN `life-engine-core install-service` is run on Linux THEN the system SHALL install a systemd service unit that manages Core as a system service.
- 1.3. WHEN `life-engine-core install-service` is run on macOS THEN the system SHALL install a launchd plist that manages Core as a user agent.
- 1.4. WHEN Core is running in standalone mode THEN the system SHALL operate as a fully self-contained binary with no runtime dependencies.
- 1.5. WHEN Core is running in standalone mode THEN the `plugins/` and `workflows/` directories SHALL be configurable via `config.toml`.

### Requirement 2 — Docker Container

**User Story:** As a Docker user, I want to deploy Core with a single docker-compose command, so that I get Core running with persistent storage.

#### Acceptance Criteria

- 2.1. WHEN the Docker image is built THEN it SHALL be based on Alpine Linux and the final image size SHALL be under 50 MB.
- 2.2. WHEN `docker-compose up` is run with the provided compose file THEN the system SHALL start Core with correct volume mounts for data, plugins, and workflows.
- 2.3. WHEN Core runs inside Docker THEN the system SHALL read all configuration from environment variables (no config file required).
- 2.4. WHEN a Docker volume is mounted at `/data` THEN the system SHALL persist the database and all plugin data across container restarts and upgrades.
- 2.5. WHEN the Docker container is stopped and restarted THEN the system SHALL resume with all data intact from the mounted volume.
- 2.6. WHEN Core runs inside Docker THEN WASM plugins SHALL be mounted from a volume or baked into the image at `/plugins`.

### Requirement 3 — Home Server (ARM64)

**User Story:** As a home server user, I want ARM64 builds that run on my Raspberry Pi, so that I can self-host Life Engine with full data sovereignty.

#### Acceptance Criteria

- 3.1. WHEN a release build is produced THEN the system SHALL produce a Core binary compiled for the `aarch64-unknown-linux-gnu` target.
- 3.2. WHEN the ARM64 binary is deployed to a Raspberry Pi 4 THEN Core SHALL start and respond to requests with 128 MB or less of available RAM.
- 3.3. WHEN Core is deployed behind a Caddy reverse proxy THEN the system SHALL respond correctly to proxied requests with TLS terminated by Caddy.
- 3.4. WHEN `LE_BEHIND_PROXY=true` is set THEN the system SHALL accept non-TLS connections on the local interface, trusting the reverse proxy for TLS termination.

### Requirement 4 — Network Security Enforcement

**User Story:** As a user, I want the system to enforce secure defaults when exposed to the network, so that my data is not accidentally served over an insecure connection.

#### Acceptance Criteria

- 4.1. WHEN Core is started with the default configuration THEN the system SHALL bind to `127.0.0.1:3750` and NOT expose any port to the network.
- 4.2. WHEN the bind address is changed to `0.0.0.0` THEN the system SHALL require TLS configuration (certificate path) or the `LE_BEHIND_PROXY=true` flag before starting.
- 4.3. WHEN Core is bound to a non-localhost address without TLS and without `LE_BEHIND_PROXY=true` THEN the system SHALL refuse to start and log an error explaining the security requirement.
- 4.4. WHEN Core is bound to a non-localhost address THEN the system SHALL enable rate limiting with configurable per-IP and per-user limits.
- 4.5. WHEN Core is bound to a non-localhost address THEN the system SHALL require authentication on all transport endpoints.

### Requirement 5 — Configuration Management

**User Story:** As a deployer, I want consistent configuration across all modes, so that I can switch between deployment modes without reconfiguring everything.

#### Acceptance Criteria

- 5.1. WHEN environment variables are set alongside a config file THEN environment variables SHALL take precedence over `config.toml` values.
- 5.2. WHEN Core starts THEN the system SHALL validate the configuration and refuse to start if required values are missing or invalid.
- 5.3. WHEN Core starts THEN the system SHALL log the active deployment mode, bind address, active transports, and whether TLS is enabled.

### Requirement 6 — Transport Configuration

**User Story:** As a deployer, I want to choose which transports are active, so that I only expose the protocols I need.

#### Acceptance Criteria

- 6.1. WHEN `config.toml` declares a `[transports.rest]` section THEN Core SHALL start the REST transport on the configured port.
- 6.2. WHEN a transport section is absent from `config.toml` THEN that transport SHALL NOT be started.
- 6.3. WHEN multiple transports are configured THEN Core SHALL start all of them and share the same auth module across all.
