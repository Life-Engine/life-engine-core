<!--
domain: deployment-modes
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Implementation Plan — Deployment Modes

## Task Overview

This plan implements the three deployment modes for Core. Work is structured from infrastructure outward: Docker image first (validates the binary in isolation), then systemd/launchd service management, Caddy reverse proxy config, and network security enforcement. Each task produces a testable artifact.

**Progress:** 0 / 11 tasks complete

## Steering Document Compliance

- Localhost-only default follows Defence in Depth — network exposure is opt-in with mandatory TLS
- Configuration via TOML + env vars follows Explicit Over Implicit — no hidden defaults
- Same binary across all modes follows Single Source of Truth
- Deployment includes plugins/ directory (WASM) and workflows/ directory (YAML)
- Transport sections in config.toml determine which transports are active

## Atomic Task Requirements

- **File Scope:** 1-3 related files maximum
- **Time Boxing:** 15-30 minutes per task
- **Single Purpose:** one testable outcome per task
- **Specific Files:** exact file paths specified
- **Agent-Friendly:** clear input/output, minimal context switching

---

## 1.1 — Docker Image
> spec: ./brief.md

- [ ] Create multi-stage Dockerfile for Core based on Alpine Linux
  <!-- file: deploy/Dockerfile -->
  <!-- purpose: Build Core binary in a Rust builder stage and produce a minimal Alpine-based image under 50 MB; include /plugins and /workflows directories -->
  <!-- requirements: 3.1, 3.6 -->
  <!-- leverage: existing deploy/ directory -->

- [ ] Create docker-compose.yml with volume mounts and environment configuration
  <!-- file: deploy/docker-compose.yml -->
  <!-- purpose: Configure Core service with persistent volumes for data, plugins, and workflows; use environment-based TOML config -->
  <!-- requirements: 3.2, 3.3, 3.4, 3.5 -->
  <!-- leverage: existing deploy/docker-compose.yml -->

---

## 1.2 — Standalone Binary Configuration
> spec: ./brief.md

- [ ] Implement TOML config file loading with environment variable overrides
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: Load config.toml from platform config directory, merge with LE_* environment variables; parse transport sections to determine active transports -->
  <!-- requirements: 2.1, 2.5, 6.1, 6.2, 7.1, 7.2, 7.3 -->
  <!-- leverage: existing apps/core/src/config.rs -->

- [ ] Add startup logging for deployment mode, bind address, transports, and TLS status
  <!-- file: apps/core/src/main.rs -->
  <!-- purpose: Log active configuration on startup including which transports are enabled and plugin directory path -->
  <!-- requirements: 6.3 -->
  <!-- leverage: existing apps/core/src/main.rs -->

---

## 2.1 — Service Management
> spec: ./brief.md

- [ ] Create systemd service unit for Linux
  <!-- file: deploy/systemd/life-engine-core.service -->
  <!-- purpose: Define systemd unit that runs Core as a system service with restart-on-failure -->
  <!-- requirements: 2.2 -->
  <!-- leverage: existing deploy/systemd/ directory -->

- [ ] Create launchd plist for macOS
  <!-- file: deploy/launchd/com.life-engine.core.plist -->
  <!-- purpose: Define launchd plist that runs Core as a user agent with KeepAlive -->
  <!-- requirements: 2.3 -->
  <!-- leverage: existing deploy/launchd/ directory -->

- [ ] Implement `install-service` CLI subcommand
  <!-- file: apps/core/src/main.rs -->
  <!-- purpose: Add CLI subcommand that copies the appropriate service file and enables the service -->
  <!-- requirements: 2.2, 2.3, 2.4 -->
  <!-- leverage: existing main.rs CLI argument parsing -->

---

## 2.2 — Reverse Proxy Configuration
> spec: ./brief.md

- [ ] Create Caddy reverse proxy configuration for internet-facing deployment
  <!-- file: deploy/caddy/Caddyfile -->
  <!-- purpose: Configure Caddy to reverse-proxy to Core with automatic HTTPS via Let's Encrypt -->
  <!-- requirements: 4.3 -->
  <!-- leverage: existing deploy/caddy/ directory -->

- [ ] Add LE_BEHIND_PROXY flag support to Core startup
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: Skip TLS requirement when behind a reverse proxy, trust X-Forwarded-For headers -->
  <!-- requirements: 4.4, 5.2 -->
  <!-- leverage: existing apps/core/src/config.rs -->

---

## 2.3 — ARM64 Build Verification
> spec: ./brief.md

- [ ] Verify ARM64 binary builds and runs on Raspberry Pi 4
  <!-- file: Cargo.toml -->
  <!-- purpose: Confirm build produces aarch64-unknown-linux-gnu binary; verify it starts and responds to requests with 128 MB available RAM -->
  <!-- requirements: 4.1, 4.2 -->
  <!-- leverage: existing Cargo workspace configuration -->

---

## 3.1 — Network Security Enforcement
> spec: ./brief.md

- [ ] Implement non-localhost startup validation (TLS + auth required)
  <!-- file: apps/core/src/main.rs -->
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: Refuse to start on 0.0.0.0 without TLS config or LE_BEHIND_PROXY; enable rate limiting; require auth on all transport endpoints -->
  <!-- requirements: 5.1, 5.2, 5.3, 5.4, 5.5 -->
  <!-- leverage: packages/auth for auth enforcement -->
