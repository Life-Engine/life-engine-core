<!--
domain: deployment-modes
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Spec — Deployment Modes

## Purpose

This spec defines the 3 deployment modes for Core. Each mode targets a different use case and technical comfort level, from a standalone binary to self-hosted infrastructure on a home server. Core is a thin binary — all deployment modes share the same binary, `config.toml`, `plugins/` directory (WASM modules), and `workflows/` directory (YAML pipeline definitions).

## Deployment Artifacts

Every deployment, regardless of mode, requires these components:

- **Core binary** — The thin orchestrator (`apps/core/`)
- **`config.toml`** — Application configuration (storage, auth, transports, plugin activation)
- **`plugins/` directory** — WASM modules, each in its own subdirectory with `plugin.wasm` and `manifest.toml`
- **`workflows/` directory** — YAML workflow definitions for pipeline execution

## Deployment Modes

### Standalone Binary

Core runs as an independent process on any machine. No runtime dependencies — the binary is fully self-contained.

This mode is for technical users who want to run Core on a separate machine (e.g. a home server or VPS) and connect to it from the App on another device.

Configuration is handled via `config.toml` and environment variables. The config file location defaults to `~/.config/life-engine/config.toml` on Linux/macOS and `%APPDATA%\life-engine\config.toml` on Windows.

Service management:

- **Linux** — Systemd service unit. Install with `life-engine-core install-service`, manage with `systemctl`.
- **macOS** — Launchd plist. Install with `life-engine-core install-service`, manage with `launchctl`.

The standalone binary is the same binary used in all other deployment modes — the Docker image uses it internally.

Transport configuration example in `config.toml`:

```toml
[transports.rest]
port = 3750

[transports.graphql]
port = 3751

[plugins]
path = "./plugins/"

[workflows]
path = "./workflows/"
```

### Docker Container

Core is available as an official Docker image based on Alpine Linux. The target image size is under 50 MB.

A `docker-compose.yml` file is provided:

```yaml
services:
  core:
    image: ghcr.io/life-engine/core:latest
    ports:
      - "3750:3750"
    volumes:
      - core-data:/data
      - ./plugins:/plugins
      - ./workflows:/workflows
    environment:
      - LE_AUTH_PROVIDER=pocket-id
      - LE_AUTH_ISSUER=https://auth.local
      - LE_DATA_DIR=/data
      - LE_PLUGINS_PATH=/plugins
      - LE_WORKFLOWS_PATH=/workflows

volumes:
  core-data:
```

Configuration is entirely through environment variables. Volume mounts ensure data persistence across container restarts and upgrades. WASM plugins and workflow definitions are mounted from the host or baked into the image.

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

- **Standalone binary** — 64-bit processor (x86_64 or ARM64), 128 MB available RAM, 100 MB disk space for the binary and initial data
- **Docker container** — Docker runtime installed, resource allocation depends on container configuration
- **Home server** — ARM64 or x86_64 processor, 128 MB available RAM, 100 MB disk space for the binary and initial data. Raspberry Pi 4 or newer recommended.

## Network Configuration

By default, Core listens on `localhost` only. No ports are exposed to the network. This is the secure default for all deployment modes.

Exposing Core to the network (required for remote App connections) demands explicit configuration:

- **Bind address** — Change from `127.0.0.1` to `0.0.0.0` in `config.toml` or via `LE_BIND_ADDRESS` environment variable
- **TLS** — Required for any non-localhost connection. Handled by Core directly (self-signed or provided certificate) or by a reverse proxy (recommended)
- **Authentication** — Required for any non-localhost connection. Core validates tokens via the auth module (Pocket ID OIDC)
- **Rate limiting** — Enabled by default when listening on non-localhost addresses. Configurable limits per IP and per authenticated user.
- **Reverse proxy** — Recommended for internet-facing deployments. Handles TLS termination, rate limiting, and provides an additional security layer.

The combination of TLS + authentication + rate limiting is enforced when Core detects a non-localhost bind address. Core refuses to start on a non-localhost address without TLS configured (either directly or via a `LE_BEHIND_PROXY=true` flag indicating a reverse proxy handles TLS).

## Transport Configuration

Transports are configurable in `config.toml`. Only transports with a declared section are started. All active transports share the same auth module.

```toml
[transports.rest]
port = 3750

[transports.caldav]
port = 5232
```

If no transport sections are declared, Core starts with REST on the default port as a fallback.
