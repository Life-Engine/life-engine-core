<!--
domain: deployment-modes
status: draft
tier: 1
updated: 2026-03-23
-->

# Deployment Modes Spec

## Overview

This spec defines the 4 deployment modes for Core. Each mode targets a different use case and technical comfort level, from zero-configuration desktop usage (bundled with the Tauri App) to self-hosted infrastructure on a Raspberry Pi home server. Core is a thin binary — deployment involves the binary, a `config.toml`, a `plugins/` directory containing WASM modules, and a `workflows/` directory containing pipeline definitions.

## Goals

- Zero-config desktop — the bundled mode manages Core as a Tauri sidecar with no user-visible configuration
- Standalone flexibility — a single self-contained binary runs on any 64-bit system with systemd/launchd service management
- Container deployment — an official Docker image under 50 MB runs Core with persistent volumes
- Home server sovereignty — ARM64 builds run on Raspberry Pi with Caddy reverse proxy for internet-facing TLS

## User Stories

- As a non-technical user, I want Core to start and stop automatically with the App so that I never interact with a server directly.
- As a technical user, I want to run Core as a standalone binary on my VPS so that I can access it remotely from any device.
- As a Docker user, I want a docker-compose file that runs Core so that I can deploy with a single command.
- As a home server user, I want ARM64 builds so that I can run Life Engine on my Raspberry Pi with full data sovereignty.

## Functional Requirements

- The system must run Core as a Tauri sidecar in bundled mode, starting on App launch and stopping on App close.
- The system must provide a standalone binary that runs independently on Linux, macOS, and Windows with TOML/env config.
- The system must provide a Docker image under 50 MB based on Alpine Linux with volume persistence.
- The system must provide ARM64 binary builds that run on Raspberry Pi 4 with 128 MB available RAM.
- The system must enforce TLS and authentication for any non-localhost bind address.
- The system must provide a Caddy reverse proxy configuration for internet-facing deployments.
- The system must include the `plugins/` directory and `workflows/` directory in all deployment artifacts.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
