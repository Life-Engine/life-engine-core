<!--
project: life-engine-core
specs: devops-deployment (from QA)
updated: 2026-03-28
-->

# DevOps and Deployment Remediation Plan

## Plan Overview

This plan addresses findings from the phase-4 DevOps and deployment review. Four critical issues block Docker deployment: a broken legacy Dockerfile, a config validation conflict that prevents container startup, a wrong Pocket ID port mapping, and a config format mismatch between Docker mounts and the active config loader. Major issues include broken launchd paths, an insecure systemd passphrase pattern, missing .dockerignore, potential WASM JIT conflicts with systemd hardening, and broken nginx WebSocket handling. The plan is organized into 6 work packages across 3 priority tiers.

**Progress:** 5 / 6 work packages complete

---

## 1.1 — Fix Critical Docker Deployment Blockers
> depends: none
> spec: .odm/qa/reports/phase-4/devops-deployment.md

- [x] Delete or redirect apps/core/Dockerfile to deploy/Dockerfile [critical-fix]
  <!-- file: apps/core/Dockerfile -->
  <!-- purpose: Remove broken legacy Dockerfile that copies only 4 of 28+ workspace members and lacks SQLCipher build deps -->
  <!-- requirements: Problem 1 -->
  <!-- leverage: deploy/Dockerfile is the working multi-stage build -->
- [x] Update tools/verify-docker-image-size.sh to reference deploy/Dockerfile instead of apps/core/Dockerfile [critical-fix]
  <!-- file: tools/verify-docker-image-size.sh -->
  <!-- purpose: Fix image size verification script that references the broken Dockerfile at line 30 -->
  <!-- requirements: Problem 1 -->
  <!-- leverage: existing verification logic -->
- [x] Fix Docker Compose auth/bind conflict: change LIFE_ENGINE_AUTH_PROVIDER or add LIFE_ENGINE_ALLOW_INSECURE_BIND escape hatch [critical-fix]
  <!-- file: deploy/docker-compose.yml -->
  <!-- purpose: Prevent config validation from rejecting local-token on 0.0.0.0 bind, which blocks container startup -->
  <!-- requirements: Problem 2 -->
  <!-- leverage: existing config validation in config.rs -->
- [x] Fix Pocket ID port mapping in full compose from 3751:3751 to 3751:80 [critical-fix]
  <!-- file: deploy/docker-compose.full.yml -->
  <!-- purpose: Pocket ID serves on port 80 internally; current mapping causes health check to always fail, blocking Core startup -->
  <!-- requirements: Problem 3 -->
  <!-- leverage: root docker-compose.yml already uses correct 3751:80 mapping -->
- [x] Resolve config format transition: update main.rs to use startup::load_config() TOML path or change Docker mounts to YAML [critical-fix]
  <!-- file: apps/core/src/main.rs -->
  <!-- purpose: Docker Compose mounts config.toml but the legacy config loader uses serde_yaml, causing parse failure -->
  <!-- requirements: Problem 4 -->
  <!-- leverage: new startup module already implements TOML loading -->

## 1.2 — Fix Service Manager Issues (systemd and launchd)
> depends: none
> spec: .odm/qa/reports/phase-4/devops-deployment.md

- [x] Replace empty LIFE_ENGINE_STORAGE_PASSPHRASE in systemd unit with EnvironmentFile directive [fix]
  <!-- file: deploy/systemd/life-engine-core.service -->
  <!-- purpose: Secrets should load from /etc/life-engine/env with restricted permissions, not be embedded empty in unit file -->
  <!-- requirements: Problem 6 -->
  <!-- leverage: standard systemd EnvironmentFile pattern -->
- [x] Fix launchd plist tilde expansion: install script must replace ~ with actual home directory path [fix]
  <!-- file: deploy/install.sh -->
  <!-- purpose: launchd does not perform shell tilde expansion in ProgramArguments; all paths with ~ will fail -->
  <!-- requirements: Problem 5 -->
  <!-- leverage: install script already has $HOME available -->
- [x] Update launchd plist paths to use absolute path placeholders that install.sh replaces [fix]
  <!-- file: deploy/launchd/com.life-engine.core.plist -->
  <!-- purpose: Replace ~ prefix paths with template markers that install script substitutes at install time -->
  <!-- requirements: Problem 5 -->
  <!-- leverage: none -->
- [x] Add LimitNOFILE=65536 and CapabilityBoundingSet= directives to systemd unit [fix]
  <!-- file: deploy/systemd/life-engine-core.service -->
  <!-- purpose: Production hardening: raise file descriptor limit and explicitly drop all capabilities -->
  <!-- requirements: Recommendation 11 -->
  <!-- leverage: existing security hardening directives in the unit file -->
- [x] Update install.sh to create /etc/life-engine/ directory and example config file [fix]
  <!-- file: deploy/install.sh -->
  <!-- purpose: systemd ExecStart references --config /etc/life-engine/config.toml but install script never creates this -->
  <!-- requirements: Problem 13 -->
  <!-- leverage: existing install_linux function -->
- [x] Test MemoryDenyWriteExecute=true with WASM plugins loaded; disable if wasmtime JIT requires W^X pages [fix]
  <!-- file: deploy/systemd/life-engine-core.service -->
  <!-- purpose: Verify systemd security hardening does not block wasmtime JIT compilation for plugins -->
  <!-- requirements: Problem 8 -->
  <!-- leverage: none -->

## 2.1 — Fix Reverse Proxy Configurations
> depends: none
> spec: .odm/qa/reports/phase-4/devops-deployment.md

- [x] Fix nginx WebSocket handling: add conditional Connection header based on $http_upgrade variable [fix]
  <!-- file: deploy/nginx/life-engine.conf -->
  <!-- purpose: Current proxy_set_header Connection "" prevents WebSocket upgrades that require Connection: upgrade -->
  <!-- requirements: Problem 9 -->
  <!-- leverage: existing proxy configuration -->
- [x] Update nginx listen directive from deprecated http2 parameter to separate http2 on directive [fix]
  <!-- file: deploy/nginx/life-engine.conf -->
  <!-- purpose: http2 on listen directive is deprecated since nginx 1.25.1 -->
  <!-- requirements: Problem 21 -->
  <!-- leverage: none -->
- [x] Add client_max_body_size directive to nginx config [fix]
  <!-- file: deploy/nginx/life-engine.conf -->
  <!-- purpose: Default 1 MB limit is too small for file uploads, blob storage, or plugin data -->
  <!-- requirements: Nginx issues -->
  <!-- leverage: Caddy config already has 10 MB request body limit -->
- [x] Add security headers to nginx config matching Caddy (HSTS, X-Content-Type-Options, X-Frame-Options) [fix]
  <!-- file: deploy/nginx/life-engine.conf -->
  <!-- purpose: Nginx config lacks security headers that the Caddy config includes, creating inconsistency -->
  <!-- requirements: Nginx issues -->
  <!-- leverage: existing Caddy security headers as reference -->
- [x] Fix Caddyfile env var reference from LE_BEHIND_PROXY to LIFE_ENGINE_BEHIND_PROXY [fix]
  <!-- file: deploy/caddy/Caddyfile -->
  <!-- purpose: Comment references incorrect env var prefix that does not match the LIFE_ENGINE_ convention -->
  <!-- requirements: Problem 12 -->
  <!-- leverage: none -->

## 2.2 — Fix Docker Build and Compose Issues
> depends: 1.1
> spec: .odm/qa/reports/phase-4/devops-deployment.md

- [x] Add .dockerignore file excluding target/, node_modules/, .git/, docs/, reports/, tools/ [fix]
  <!-- file: .dockerignore -->
  <!-- purpose: Without .dockerignore, full repository including multi-GB target/ is sent as build context -->
  <!-- requirements: Problem 7 -->
  <!-- leverage: none -->
- [x] Fix Dockerfile plugin caching: copy only plugins/engine/*/Cargo.toml instead of full plugin source [fix]
  <!-- file: deploy/Dockerfile -->
  <!-- purpose: Any plugin source change currently invalidates the dependency cache layer -->
  <!-- requirements: Dockerfile issues -->
  <!-- leverage: existing stub pattern used for packages -->
- [x] Fix MinIO test compose console port: change --console-address to :9001 to match port mapping [fix]
  <!-- file: docker-compose.test.yml -->
  <!-- purpose: Console listens on 9101 inside container but mapping points to 9001, making console unreachable -->
  <!-- requirements: Problem 10 -->
  <!-- leverage: dev compose already uses correct console port -->
- [x] Add logging driver configuration to production Docker Compose files [fix]
  <!-- file: deploy/docker-compose.yml -->
  <!-- purpose: Default json-file driver grows unbounded on long-running deployments -->
  <!-- requirements: Problem 17 -->
  <!-- leverage: none -->
- [x] Add CPU limits alongside existing memory limits in Docker Compose [fix]
  <!-- file: deploy/docker-compose.yml -->
  <!-- purpose: Process could consume all available CPU during WASM compilation workloads -->
  <!-- requirements: Problem 18 -->
  <!-- leverage: existing memory limit configuration -->
- [x] Pin Docker image versions in development and test compose files [fix]
  <!-- file: docker-compose.yml -->
  <!-- purpose: :latest tags on Pocket ID, Radicale, MinIO can break without notice -->
  <!-- requirements: Problem 19 -->
  <!-- leverage: none -->
- [x] Add health checks to development compose services [fix]
  <!-- file: docker-compose.yml -->
  <!-- purpose: Integration tests need to wait for service readiness; currently requires manual verification -->
  <!-- requirements: Recommendation 12 -->
  <!-- leverage: existing health check pattern in deploy/docker-compose.yml -->

## 3.1 — Fix Config and Tooling Issues
> depends: 1.1
> spec: .odm/qa/reports/phase-4/devops-deployment.md

- [x] Update default CORS origin from http://localhost:1420 (Tauri) to current architecture default [fix]
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: Stale Tauri dev server port reference; Tauri is no longer used in the project -->
  <!-- requirements: Problem 11 -->
  <!-- leverage: existing default_cors_origins() function -->
- [x] Fix verify-arm64.sh health check to create a valid config file before testing [fix]
  <!-- file: deploy/verify-arm64.sh -->
  <!-- purpose: Script passes --config pointing to non-existent file, causing health check to fail -->
  <!-- requirements: Problem 23 -->
  <!-- leverage: existing verification logic -->
- [x] Fix justfile new-plugin to use cross-platform sed syntax [fix]
  <!-- file: justfile -->
  <!-- purpose: sed -i '' is macOS-only; will fail on Linux/GNU sed -->
  <!-- requirements: Problem 15 -->
  <!-- leverage: none -->
- [x] Remove stale tauri-apps.tauri-vscode extension from devcontainer.json [cleanup]
  <!-- file: .devcontainer/devcontainer.json -->
  <!-- purpose: Tauri extension is irrelevant since Tauri is no longer used -->
  <!-- requirements: Problem 16 -->
  <!-- leverage: none -->
- [x] Add GreenMail ports (3025, 3143, 3993) to devcontainer.json forwardPorts [fix]
  <!-- file: .devcontainer/devcontainer.json -->
  <!-- purpose: Email connector development requires these ports to be forwarded -->
  <!-- requirements: Devcontainer issues -->
  <!-- leverage: existing forwardPorts array -->
- [x] Consolidate duplicate plugin scaffolding: align justfile new-plugin with tools/scripts/scaffold-plugin.sh [cleanup]
  <!-- file: justfile -->
  <!-- purpose: Two scaffolding mechanisms reference different templates and use different approaches -->
  <!-- requirements: Problem 14 -->
  <!-- leverage: tools/templates/plugin/ is the correct template -->

## 3.2 — Config Code Cleanup
> depends: 1.1
> spec: .odm/qa/reports/phase-4/devops-deployment.md

- [x] Deduplicate apply_env_overrides() OIDC and WebAuthn get_or_insert patterns [cleanup]
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: Same default struct construction repeated 3 times for OIDC and 4 times for WebAuthn -->
  <!-- requirements: Problem 24 -->
  <!-- leverage: existing apply_env_overrides() method -->
- [x] Fix PostgreSQL password field to use Option<String> instead of empty string default [fix]
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: Cannot distinguish "not configured" from "intentionally empty"; should use resolve_passphrase() pattern -->
  <!-- requirements: Config issues -->
  <!-- leverage: existing resolve_passphrase() pattern used for storage -->
- [x] Add .env.example guidance for LIFE_ENGINE_STORAGE_PASSPHRASE minimum length requirements [fix]
  <!-- file: deploy/.env.example -->
  <!-- purpose: Empty passphrase with no guidance may silently disable encryption or produce runtime errors -->
  <!-- requirements: .env.example issues -->
  <!-- leverage: none -->
