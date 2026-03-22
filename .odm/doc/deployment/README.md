# Deployment Guide

This directory contains self-hosting documentation for Life Engine Core. Each guide is standalone and can be followed independently.

## Guides

- [Docker Quick Start](docker-quick-start.md) -- Get Core running with Docker or Docker Compose in minutes. Covers the minimal single-service setup, the full stack with Pocket ID (OIDC), building the image manually, and persisting data.

- [Bare-metal Installation](bare-metal.md) -- Build Core from source and install it as a system service on Linux (systemd) or macOS (launchd). Includes the automated install script and manual step-by-step instructions.

- [Reverse Proxy and TLS](reverse-proxy.md) -- Put nginx or Caddy in front of Core for TLS termination with Let's Encrypt. Covers SSE proxy requirements, buffering, and timeouts.

- [Configuration Reference](configuration.md) -- Full reference for every configuration option. Covers the YAML config file, environment variables, CLI arguments, and validation rules.

## Quick links

- Core default bind address: `127.0.0.1:3750`
- Dockerfile: `apps/core/Dockerfile`
- Compose files: `deploy/docker-compose.yml` and `deploy/docker-compose.full.yml`
- Install script: `deploy/install.sh`
- systemd unit: `deploy/systemd/life-engine-core.service`
- launchd plist: `deploy/launchd/com.life-engine.core.plist`
- nginx config: `deploy/nginx/life-engine.conf`
- Caddy config: `deploy/caddy/Caddyfile`
