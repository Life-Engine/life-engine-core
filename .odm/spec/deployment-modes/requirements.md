<!--
domain: deployment-modes
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Requirements Document — Deployment Modes

## Introduction

Life Engine Core supports four deployment modes to serve users ranging from non-technical desktop users to self-hosting enthusiasts. Each mode uses the same Core binary but with different lifecycle management, configuration, and network exposure. This document specifies the requirements for each mode and the network security enforcement that applies across all modes.

## Alignment with Product Vision

- **Defence in Depth** — TLS + authentication + rate limiting are enforced together for any non-localhost exposure; Core refuses to start without them
- **Explicit Over Implicit** — Network exposure requires deliberate configuration changes; the default is localhost-only with no ports exposed
- **Finish Before Widening** — Bundled mode is the v1 default; standalone, Docker, and home server modes extend from the same binary
- **The Pit of Success** — The secure default (localhost-only, no TLS needed) is also the easiest to configure

## Requirements

### Requirement 1 — Bundled Mode (Tauri Sidecar)

**User Story:** As a non-technical user, I want Core to start and stop automatically with the desktop App, so that I never need to manage a server.

#### Acceptance Criteria

- 1.1. WHEN the Tauri App launches THEN the system SHALL spawn the Core binary as a sidecar subprocess listening on `localhost:3750`.
- 1.2. WHEN the Tauri App is closed THEN the system SHALL send a graceful shutdown signal to Core and wait up to 5 seconds before force-killing the process.
- 1.3. WHEN Core is running in bundled mode THEN the system SHALL store data in the platform-standard App data directory (e.g. `~/Library/Application Support/life-engine/` on macOS).
- 1.4. WHEN Core is running in bundled mode THEN the system SHALL bind to `127.0.0.1` only, rejecting any attempt to bind to a non-localhost address.
- 1.5. WHEN the App is updated THEN the bundled Core binary SHALL be updated alongside it as part of the same installer.

### Requirement 2 — Standalone Binary

**User Story:** As a technical user, I want to run Core as an independent process on any machine, so that I can access it remotely from my devices.

#### Acceptance Criteria

- 2.1. WHEN Core is started in standalone mode THEN the system SHALL read configuration from `~/.config/life-engine/core.yaml` (Linux/macOS) or `%APPDATA%\life-engine\core.yaml` (Windows), with environment variables overriding file values.
- 2.2. WHEN `life-engine-core install-service` is run on Linux THEN the system SHALL install a systemd service unit that manages Core as a system service.
- 2.3. WHEN `life-engine-core install-service` is run on macOS THEN the system SHALL install a launchd plist that manages Core as a user agent.
- 2.4. WHEN Core is running in standalone mode THEN the system SHALL operate as a fully self-contained binary with no runtime dependencies.

### Requirement 3 — Docker Container

**User Story:** As a Docker user, I want to deploy Core with a single docker-compose command, so that I get Core and authentication running with persistent storage.

#### Acceptance Criteria

- 3.1. WHEN the Docker image is built THEN it SHALL be based on Alpine Linux and the final image size SHALL be under 50 MB.
- 3.2. WHEN `docker-compose up` is run with the provided compose file THEN the system SHALL start both Core and Pocket ID with correct networking between them.
- 3.3. WHEN Core runs inside Docker THEN the system SHALL read all configuration from environment variables (no config file required).
- 3.4. WHEN a Docker volume is mounted at `/data` THEN the system SHALL persist the database and all plugin data across container restarts and upgrades.
- 3.5. WHEN the Docker container is stopped and restarted THEN the system SHALL resume with all data intact from the mounted volume.

### Requirement 4 — Home Server (ARM64)

**User Story:** As a home server user, I want ARM64 builds that run on my Raspberry Pi, so that I can self-host Life Engine with full data sovereignty.

#### Acceptance Criteria

- 4.1. WHEN the release pipeline runs THEN the system SHALL produce a Core binary compiled for the `aarch64-unknown-linux-gnu` target.
- 4.2. WHEN the ARM64 binary is deployed to a Raspberry Pi 4 THEN Core SHALL start and respond to API requests with 128 MB or less of available RAM.
- 4.3. WHEN Core is deployed behind a Caddy reverse proxy THEN the system SHALL respond correctly to proxied requests with TLS terminated by Caddy.
- 4.4. WHEN `LE_BEHIND_PROXY=true` is set THEN the system SHALL accept non-TLS connections on the local interface, trusting the reverse proxy for TLS termination.

### Requirement 5 — Network Security Enforcement

**User Story:** As a user, I want the system to enforce secure defaults when exposed to the network, so that my data is not accidentally served over an insecure connection.

#### Acceptance Criteria

- 5.1. WHEN Core is started with the default configuration THEN the system SHALL bind to `127.0.0.1:3750` and NOT expose any port to the network.
- 5.2. WHEN the bind address is changed to `0.0.0.0` THEN the system SHALL require TLS configuration (certificate path) or the `LE_BEHIND_PROXY=true` flag before starting.
- 5.3. WHEN Core is bound to a non-localhost address without TLS and without `LE_BEHIND_PROXY=true` THEN the system SHALL refuse to start and log an error explaining the security requirement.
- 5.4. WHEN Core is bound to a non-localhost address THEN the system SHALL enable rate limiting with configurable per-IP and per-user limits.
- 5.5. WHEN Core is bound to a non-localhost address THEN the system SHALL require authentication (Pocket ID OIDC tokens) on all API endpoints.

### Requirement 6 — Configuration Management

**User Story:** As a deployer, I want consistent configuration across all modes, so that I can switch between deployment modes without reconfiguring everything.

#### Acceptance Criteria

- 6.1. WHEN environment variables are set alongside a config file THEN environment variables SHALL take precedence over file values.
- 6.2. WHEN Core starts THEN the system SHALL validate the configuration and refuse to start if required values are missing or invalid.
- 6.3. WHEN Core starts THEN the system SHALL log the active deployment mode, bind address, and whether TLS is enabled.
