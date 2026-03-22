<!--
domain: deployment-modes
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Spec — Deployment Modes

## Contents

- [Purpose](#purpose)
- [Deployment Modes](#deployment-modes)
  - [Bundled with App](#bundled-with-app)
  - [Standalone Binary](#standalone-binary)
  - [Docker Container](#docker-container)
  - [Home Server](#home-server)
- [System Requirements](#system-requirements)
- [Network Configuration](#network-configuration)
- [Acceptance Criteria](#acceptance-criteria)

## Purpose

This spec defines the 4 deployment modes for Core. Each mode targets a different use case and technical comfort level, from zero-configuration desktop usage to self-hosted infrastructure on a home server.

## Deployment Modes

### Bundled with App

This is the recommended default for v1 and the mode most users will experience.

Core runs as a Tauri sidecar subprocess managed entirely by the App. The user never sees a terminal, a config file, or a port number. The App spawns Core on launch and kills it on close. Communication between App and Core happens over `localhost:3750`.

This mode provides a zero-install UX for non-technical users. Download the App, open it, and everything works. No server setup, no Docker, no command line.

Characteristics:

- Core lifecycle is fully managed by the App process
- No user-visible configuration required
- Data is stored in the App's standard data directory (platform-dependent)
- Core is not accessible from other devices on the network
- Updates to Core are bundled with App updates

### Standalone Binary

Core runs as an independent process on any machine. No runtime dependencies — the binary is fully self-contained.

This mode is for technical users who want to run Core on a separate machine (e.g. a home server or VPS) and connect to it from the App on another device.

Configuration is handled via a YAML config file and environment variables. The config file location defaults to `~/.config/life-engine/core.yaml` on Linux/macOS and `%APPDATA%\life-engine\core.yaml` on Windows.

Service management:

- **Linux** — Systemd service unit. Install with `life-engine-core install-service`, manage with `systemctl`.
- **macOS** — Launchd plist. Install with `life-engine-core install-service`, manage with `launchctl`.

The standalone binary is the same binary used in all other deployment modes — the sidecar and Docker image both use it internally.

### Docker Container

Core is available as an official Docker image based on Alpine Linux. The target image size is under 50 MB.

A `docker-compose.yml` file is provided that runs Core alongside Pocket ID (the authentication provider):

```yaml
services:
  core:
    image: ghcr.io/life-engine/core:latest
    ports:
      - "3750:3750"
    volumes:
      - core-data:/data
    environment:
      - LE_AUTH_PROVIDER=pocket-id
      - LE_AUTH_URL=http://pocket-id:3751
      - LE_DATA_DIR=/data

  pocket-id:
    image: ghcr.io/pocket-id/pocket-id:latest
    ports:
      - "3751:3751"
    volumes:
      - auth-data:/data

volumes:
  core-data:
  auth-data:
```

Configuration is entirely through environment variables. Volume mounts ensure data persistence across container restarts and upgrades.

### Home Server

Core runs on consumer hardware — a Raspberry Pi, a NAS, or an old laptop repurposed as a server. This mode uses the standalone binary with ARM64 builds.

Characteristics:

- ARM64 builds are provided for Raspberry Pi 4/5 and similar SBCs
- Core runs comfortably on 128 MB of RAM
- A reverse proxy (Caddy or nginx) is recommended for internet-facing deployments
- TLS is handled by the reverse proxy using Let's Encrypt certificates
- Caddy is the recommended reverse proxy for its automatic HTTPS configuration

Example Caddy configuration for internet-facing deployment:

```text
life-engine.example.com {
    reverse_proxy localhost:3750
}
```

This mode is for users who want full data sovereignty with remote access capability.

## System Requirements

Each deployment mode has the following minimum requirements:

- **Bundled with App** — Same as the App requirements (any system that runs the Tauri desktop application)
- **Standalone binary** — 64-bit processor (x86_64 or ARM64), 128 MB available RAM, 100 MB disk space for the binary and initial data
- **Docker container** — Docker runtime installed, resource allocation depends on container configuration
- **Home server** — ARM64 or x86_64 processor, 128 MB available RAM, 100 MB disk space for the binary and initial data. Raspberry Pi 4 or newer recommended.

## Network Configuration

By default, Core listens on `localhost` only. No ports are exposed to the network. This is the secure default for all deployment modes.

Exposing Core to the network (required for remote App connections) demands explicit configuration:

- **Bind address** — Change from `127.0.0.1` to `0.0.0.0` in config or environment variable
- **TLS** — Required for any non-localhost connection. Handled by Core directly (self-signed or provided certificate) or by a reverse proxy (recommended)
- **Authentication** — Required for any non-localhost connection. Core validates tokens issued by the configured auth provider (Pocket ID)
- **Rate limiting** — Enabled by default when listening on non-localhost addresses. Configurable limits per IP and per authenticated user.
- **Reverse proxy** — Recommended for internet-facing deployments. Handles TLS termination, rate limiting, and provides an additional security layer.

The combination of TLS + authentication + rate limiting is enforced when Core detects a non-localhost bind address. Core refuses to start on a non-localhost address without TLS configured (either directly or via a `LE_BEHIND_PROXY=true` flag indicating a reverse proxy handles TLS).

## Acceptance Criteria

1. All 4 deployment modes work and Core responds to API requests in each
2. The Docker image is under 50 MB
3. The ARM64 binary runs on a Raspberry Pi 4 with 128 MB available RAM
4. A standalone Core instance accepts connections from a remote App with proper authentication and TLS
5. The bundled mode starts and stops Core automatically with the App lifecycle — no user intervention required
