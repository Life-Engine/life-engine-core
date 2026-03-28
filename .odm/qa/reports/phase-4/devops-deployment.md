# DevOps and Deployment Review

## Summary

The life-engine-core project provides a comprehensive deployment story spanning Docker containers, bare-metal install via systemd/launchd, and reverse proxy configurations for Caddy and nginx. The primary Dockerfile (`deploy/Dockerfile`) is well-crafted with multi-stage builds, non-root user, Alpine base, and dependency caching. The runtime configuration system (`config.rs`) is thorough with layered loading (TOML file, env vars, CLI args), validation, and security guardrails. However, there are several cross-cutting issues: a stale secondary Dockerfile that will not build, a config format mismatch between Docker Compose mounts and the actual config loader, a Pocket ID port mapping error, missing network isolation in Docker Compose, an empty storage passphrase in the systemd service file, and launchd tilde expansion problems. The new-architecture config module (Phase 9 `startup`) is partially duplicating the legacy config system, creating a transition risk.

## File-by-File Analysis

### deploy/Dockerfile

Multi-stage build using `rust:1.85-alpine` (builder) and `alpine:3.20` (runtime). Installs `musl-dev`, `pkgconf`, `perl`, and `make` in the builder stage for SQLCipher bundled compilation. Copies 17 workspace member Cargo.toml files and the full `plugins/engine/` directory for dependency caching, creates stub source files, runs a warm-up build (swallowing errors with `|| true`), then copies real source and rebuilds.

Strengths:

- Non-root user (`life-engine`) with dedicated group
- Creates `/data`, `/plugins`, `/workflows` directories with correct ownership
- Dockerfile-level `HEALTHCHECK` for orchestrator-independent health monitoring
- `ca-certificates` and `wget` installed for health checks and TLS trust
- `ENTRYPOINT` form prevents shell injection
- Alpine base produces small images (typically under 50 MB)

Issues:

- The warm-up build (`cargo build --release --package life-engine-core 2>/dev/null || true`) suppresses all output including genuine configuration errors. If the workspace Cargo.toml changes shape, this step silently fails and provides no caching benefit while adding build time.
- `COPY plugins/engine/ plugins/engine/` copies the full plugin source (not just Cargo.toml manifests) in the caching layer. Any source change in any plugin invalidates the dependency cache. Should copy only `plugins/engine/*/Cargo.toml` files, then create stubs like the packages.
- `COPY docs/schemas/ docs/schemas/` is copied after the warm-up build but before the final build. If schema files are only needed at runtime, they should be copied in the runtime stage instead to avoid unnecessary rebuilds.
- No `.dockerignore` file found in the repository root. Without one, the full repository (including `target/`, `node_modules/`, `.git/`, docs, etc.) is sent as build context, dramatically slowing builds.
- The `EXPOSE 3750` directive only documents the REST transport. If CalDAV/CardDAV/webhook transports are enabled, their ports are not documented.

### apps/core/Dockerfile

A legacy Dockerfile that only copies 4 of the 28+ workspace members (types, plugin-sdk-rs, test-utils, core). Missing `perl` and `make` packages required for SQLCipher.

Issues:

- **Will not build.** The workspace `Cargo.toml` references all 28 members, but this Dockerfile only provides stubs for 4 packages. Cargo will fail to resolve the workspace.
- Missing packages `perl` and `make` needed for `bundled-sqlcipher` feature.
- Uses `CMD` instead of `ENTRYPOINT` (inconsistent with `deploy/Dockerfile`).
- Does not create `/plugins` or `/workflows` directories.
- `tools/verify-docker-image-size.sh` references this broken Dockerfile at line 30: `docker build -f apps/core/Dockerfile`.

### deploy/docker-compose.yml (standalone deployment)

Single-service compose for the Core container with named volume, resource limits, and health check.

Issues:

- **Config format mismatch.** Mounts `./config.toml:/app/config.toml:ro`, but the legacy `CoreConfig::load()` method calls `load_from_yaml()` which uses `serde_yaml::from_str`. The new `startup::load_config()` does parse TOML, but the legacy code path (still the active one in `main.rs`) does not. If the mounted file contains TOML syntax, parsing will fail.
- `LIFE_ENGINE_STORAGE_PASSPHRASE: "${LIFE_ENGINE_STORAGE_PASSPHRASE}"` — the env var is required for SQLCipher encryption (enabled by default) but there is no `env_file` directive and no documentation that a `.env` file must exist alongside the compose file. If the variable is unset, the container will start with an empty passphrase, which may silently disable encryption or produce a runtime error.
- `LIFE_ENGINE_TRANSPORTS_REST_PORT: "3750"` — this env var exists in the new-architecture config (`startup` module) but is not handled by the legacy `apply_env_overrides()` method. It is silently ignored, and the port is actually set by `LIFE_ENGINE_CORE_PORT`.
- `LIFE_ENGINE_AUTH_PROVIDER: "local-token"` combined with `LIFE_ENGINE_CORE_HOST: "0.0.0.0"` — the validation logic in `config.rs:815` rejects `local-token` on non-localhost bind addresses. This means the container **will refuse to start** with the provided configuration unless the validation is bypassed by the Docker env context. This is a critical startup failure.
- No `networks:` section. The service runs on the default bridge network with no isolation.
- Memory limit of 256M is set, but no CPU limit. On shared hosts, the Rust binary could consume full CPU during compilation-like workloads (plugin WASM compilation).
- No `logging:` driver configuration. Default `json-file` driver will grow unbounded on long-running deployments.

### deploy/docker-compose.full.yml (Core + Pocket ID)

Full-stack deployment with OIDC authentication via Pocket ID.

Issues:

- **Pocket ID port mapping is wrong.** Maps `3751:3751`, but the Pocket ID Docker image serves on port 80 internally. The root development `docker-compose.yml` correctly uses `3751:80`. The health check hits `http://localhost:3751/health` inside the container, but nothing listens on 3751 — it will always fail, preventing the Core service from starting (due to `condition: service_healthy`).
- `LIFE_ENGINE_OIDC_ISSUER_URL: "http://pocket-id:80"` uses the Docker internal hostname, which is correct for container-to-container communication. However, `PUBLIC_APP_URL: "http://localhost:3751"` on the Pocket ID service points to the host-mapped port, creating a mismatch between internal and external URLs that may break OIDC token validation (issuer claim mismatch).
- Uses bind mount `./data:/data` instead of a named volume (unlike the standalone compose). Bind mounts do not benefit from Docker volume drivers and have different permission semantics.
- Same config format mismatch as the standalone compose.
- Same `local-token` / `0.0.0.0` validation conflict does not apply here (uses `oidc`), but the `0.0.0.0` bind still requires `behind_proxy = true` or TLS per the validation logic. Neither is configured.
- No `restart` policy on pocket-id service... actually, there is one at line 50. Correction: restart is set on pocket-id.
- No inter-service network isolation. Both services share the default network with all ports accessible between them.

### deploy/.env.example

Well-documented example environment file covering all major configuration knobs.

Issues:

- `LIFE_ENGINE_STORAGE_PASSPHRASE=` is left empty with no guidance on minimum length or complexity requirements. The Argon2 KDF will accept any passphrase, including empty strings.
- `LIFE_ENGINE_TRANSPORTS_REST_PORT=3750` is documented but not handled by the legacy config loader. Only the new `startup` module processes it.
- Missing `LIFE_ENGINE_BEHIND_PROXY` documentation (commented out at bottom, but the variable name shown is `LIFE_ENGINE_BEHIND_PROXY` while the Caddyfile documentation says `LE_BEHIND_PROXY`). These are inconsistent names.

### deploy/install.sh

Cross-platform installer for bare-metal deployment (Linux systemd, macOS launchd).

Strengths:

- Creates dedicated service user on Linux
- Sets correct ownership and permissions
- Uses `install -m 755` for binary deployment
- Clean error handling with `set -euo pipefail`

Issues:

- The `install_linux` function runs `sudo systemctl enable life-engine-core && sudo systemctl start life-engine-core` immediately. If `LIFE_ENGINE_STORAGE_PASSPHRASE` is not configured in the systemd unit, the service will start and immediately fail. The script should prompt for or validate the passphrase before starting the service.
- No uninstall / rollback capability.
- The macOS install uses `launchctl bootstrap` which is the modern API but does not check if a previous version is already loaded. If the plist is already bootstrapped, this will fail. Should call `launchctl bootout` first if the service exists.
- No check that the binary was built for the correct architecture before installing.

### deploy/systemd/life-engine-core.service

Systemd service unit with security hardening directives.

Strengths:

- Excellent security hardening: `ProtectSystem=strict`, `ProtectHome=true`, `NoNewPrivileges=true`, `PrivateTmp=true`, `PrivateDevices=true`, `MemoryDenyWriteExecute=true`, `RestrictNamespaces=true`, etc.
- Restart on failure with 5-second delay
- Journal logging for both stdout and stderr
- `After=network-online.target` with `Wants=network-online.target`

Issues:

- **Empty passphrase in unit file.** Line 15: `Environment=LIFE_ENGINE_STORAGE_PASSPHRASE=` — this sets the env var to an empty string, which will either disable encryption or cause a startup error. Should use `EnvironmentFile=-/etc/life-engine/env` to load secrets from a separate file with restricted permissions, rather than embedding (even empty) secrets in the unit file.
- `ExecStart` references `--config /etc/life-engine/config.toml` but `install.sh` does not create this file or the `/etc/life-engine/` directory.
- `MemoryDenyWriteExecute=true` may conflict with WASM plugin execution if the WASM runtime (wasmtime) requires JIT compilation (W^X pages). This should be tested with plugins loaded.
- No `LimitNOFILE=` directive. The default (1024) may be too low if the server handles many concurrent connections or open file handles for plugins/workflows.
- No `CapabilityBoundingSet=` directive to explicitly drop all capabilities.
- Leading spaces on security directives (lines 23-36) — `ProtectSystem=strict` has a leading space. While systemd tolerates this, it is inconsistent formatting that could confuse automated tools.

### deploy/launchd/com.life-engine.core.plist

macOS launchd plist for running Core as a user-level LaunchAgent.

Issues:

- **Tilde (`~`) in ProgramArguments is not expanded by launchd.** Line 13: `~/Library/Application Support/life-engine/config.toml` — launchd does not perform shell tilde expansion in `ProgramArguments` arrays. The path must be absolute (e.g., `/Users/<username>/Library/Application Support/life-engine/config.toml`), or the install script must sed-replace the tilde with `$HOME` at install time. The same issue applies to `WorkingDirectory` (line 22), `StandardOutPath` (line 31), and `StandardErrorPath` (line 34).
- Empty `LIFE_ENGINE_STORAGE_PASSPHRASE` in `EnvironmentVariables` — same concern as the systemd unit. Should use a separate env file or keychain integration.
- No `ThrottleInterval` set. If the process crashes repeatedly, launchd will throttle restarts with a default 10-second interval but will not alert the user.
- The `--config` argument references `config.toml` but the legacy config loader expects YAML. Same config format mismatch issue.

### deploy/caddy/Caddyfile

Reverse proxy configuration for Caddy with automatic TLS.

Strengths:

- Strong security headers (HSTS with preload, X-Content-Type-Options, X-Frame-Options, Referrer-Policy)
- Removes `Server` header
- Request body size limit (10 MB)
- Response compression (gzip)
- Access log with rotation (100 MiB, keep 5)
- CalDAV/CardDAV well-known redirects (commented out with instructions)

Issues:

- Comment on line 9 says `LE_BEHIND_PROXY=true` but the actual env var is `LIFE_ENGINE_BEHIND_PROXY`. The `LE_` prefix does not match the `LIFE_ENGINE_` convention used everywhere else.
- No WebSocket upgrade configuration. If SSE/WebSocket connections are used (the nginx config handles this), Caddy needs `flush_interval -1` or similar streaming configuration.
- No rate limiting at the proxy level. All rate limiting is delegated to the application, which means the Rust process must handle every request including malicious traffic.
- `reverse_proxy localhost:3750` — no health check or failover configuration. If Core is temporarily down, Caddy will return 502 errors immediately.

### deploy/nginx/life-engine.conf

Nginx reverse proxy configuration with TLS.

Strengths:

- HTTP to HTTPS redirect
- TLS 1.2+ with no weak ciphers
- Proxy headers for real IP forwarding (X-Real-IP, X-Forwarded-For, X-Forwarded-Proto)
- SSE/WebSocket support with `proxy_http_version 1.1`, `proxy_buffering off`, `proxy_cache off`
- Long read timeout (86400s / 24 hours) for SSE connections

Issues:

- `listen 443 ssl http2;` — the `http2` parameter on the `listen` directive is deprecated in nginx 1.25.1+. Modern nginx uses `http2 on;` as a separate directive.
- No `client_max_body_size` directive. Defaults to 1 MB, which may be too small for file uploads, blob storage, or plugin data.
- No access logging configuration (unlike the Caddy config).
- No security headers (HSTS, X-Content-Type-Options, X-Frame-Options, CSP). The Caddy config includes these, creating inconsistency between the two proxy options.
- `proxy_set_header Connection "";` — this clears the Connection header for all requests, which is correct for HTTP/1.1 keep-alive but prevents WebSocket upgrades. WebSocket connections require `Connection: upgrade`. This should be conditional based on the `$http_upgrade` variable.
- No `upstream` health check (nginx open-source does not support active health checks, but `max_fails` and `fail_timeout` should be configured on the upstream server directive).

### deploy/verify-arm64.sh

ARM64 build verification script with native and Docker modes.

Strengths:

- Comprehensive verification: cargo check, build, architecture verification, health check, Docker buildx
- Image size check (under 50 MB)
- `.cargo/config.toml` validation for cross-compilation targets
- Clean pass/fail/skip reporting

Issues:

- Health check at line 85 passes `--config "$TEMP_DIR/config.toml"` to the binary but never creates this file. The binary will either fail to start or use defaults.
- The Docker ARM64 test (line 147) checks that `--help` succeeds but does not verify the binary architecture inside the container. Should use `file` command on the binary instead.
- Uses `sleep 2` (line 87) as a hard-coded wait for server startup. A polling loop with timeout would be more reliable across different hardware.

### deploy/arm64-build.md

ARM64 build documentation.

No issues. Clear documentation for Docker multi-arch builds, native cross-compilation, and verification checklist.

### docker-compose.yml (root — development services)

Development-only services: Pocket ID, GreenMail (email), Radicale (CalDAV/CardDAV), MinIO (S3).

Strengths:

- Correct Pocket ID port mapping (`3751:80`)
- Named volumes for all services
- MinIO credentials use env var overrides with sensible defaults

Issues:

- No health checks on any service. Core development will require manual verification that services are ready.
- Pocket ID uses `:latest` tag, which can break without notice. Should pin to a specific version.
- Radicale uses `:latest` tag. Same concern.
- MinIO uses `:latest` tag. Same concern.
- No `networks:` section. All services share the default bridge network.

### docker-compose.test.yml (test services)

Test-specific services with different port mappings to avoid development conflicts.

Issues:

- MinIO test command uses `--console-address ":9101"` but the host mapping is `9101:9001`. The console inside the container is listening on 9101, but the host maps to the container's 9001. This means the console is unreachable. Should be `--console-address ":9001"` (same as the dev compose) with only the host port changed, or the port mapping should be `9101:9101`.
- No health checks.
- Hardcoded `minioadmin/minioadmin` credentials are acceptable for testing but should be documented as non-production.

### .devcontainer/devcontainer.json

VS Code Dev Container using Microsoft's Rust image with Node 20 and Docker-in-Docker.

Issues:

- `tauri-apps.tauri-vscode` extension is included but the project has no Tauri dependency. Leftover from an earlier architecture.
- `postCreateCommand: "cargo check --workspace && pnpm install"` — `cargo check` on the full workspace will take several minutes on first container creation. Consider using `cargo check --package life-engine-core` for faster startup, or make this optional.
- `forwardPorts: [3750, 3751, 5232, 9000, 9001]` does not include GreenMail ports (3025, 3143, 3993) which are needed for email connector development.
- No `features` for SQLite/SQLCipher development tools.
- Docker-in-Docker is included but no Docker Compose feature is specified. Running the development compose from inside the container requires `docker compose` to be available.

### justfile

Development commands for Core, Admin UI, testing, linting, and plugin scaffolding.

Issues:

- `new-plugin` command uses `sed -i ''` which is macOS-only (GNU sed uses `sed -i` without the empty string). Will fail on Linux.
- `dev-core` uses `cargo-watch` but does not declare it as a prerequisite or check for its installation.
- `dev-all` uses backgrounding (`&`) which can leave orphaned processes if one fails. Using a process manager or `parallel` would be more robust.

### apps/core/src/config.rs (runtime configuration)

Layered configuration with defaults, YAML file, environment variables, and CLI arguments. Two co-existing config systems: the legacy `CoreConfig` (YAML-based) and the new `startup` module (TOML-based).

Strengths:

- Sensitive fields redacted in Debug output (passphrase, OIDC secret, PG password)
- Comprehensive validation: port, log level, auth provider, TLS requirements, non-localhost security, CORS
- Security guardrails: refuses `local-token` auth on non-localhost, refuses non-TLS on non-localhost without `behind_proxy`
- Argon2 parameters are configurable for different hardware profiles
- `resolve_passphrase()` prioritizes env var over config file for secrets

Issues:

- **Dual config systems create confusion.** The legacy `CoreConfig::load()` reads YAML; the new `startup::load_config()` reads TOML. Docker Compose mounts `config.toml`, but if the legacy code path is still active in `main.rs`, it will try to parse the file as YAML and fail. The transition state is dangerous.
- The legacy `apply_env_overrides()` method has significant code duplication: the same `get_or_insert(OidcSettings { ... })` pattern with identical defaults is repeated 3 times for OIDC and 4 times for WebAuthn. Each occurrence constructs a full default struct just to insert one field.
- `LIFE_ENGINE_CORE_HOST: "0.0.0.0"` in Docker Compose + `LIFE_ENGINE_AUTH_PROVIDER: "local-token"` triggers the non-localhost rejection in `validate()`. Docker deployments must either use `oidc`/`webauthn` or the validation needs a Docker-aware escape hatch (e.g., `LIFE_ENGINE_ALLOW_INSECURE_BIND=true`).
- Default CORS origin is `http://localhost:1420` (a Tauri dev port), which is stale since Tauri is no longer used. Should default to `http://localhost:3750` or be empty.
- `default_data_dir()` resolves to `~/.life-engine/data` but the Docker compose sets `LIFE_ENGINE_CORE_DATA_DIR=/data`. The `LIFE_ENGINE_STORAGE_PATH` env var is set in Docker but not handled by the legacy `apply_env_overrides()` (only handled by the new `startup` module).
- The PostgreSQL `password` field defaults to an empty string (not `Option<String>`), making it impossible to distinguish "not configured" from "intentionally empty". The `resolve_passphrase()` pattern used for storage should be applied here too.

### .github/ Directory

All workflows and Dependabot are correctly `.disabled` per project rules. Issue templates and PR template exist.

Issues:

- The disabled `ci.yml` references `apps/web/scripts/generate-sdk-docs.mjs`, `apps/web/pnpm-lock.yaml`, and Playwright tests — none of which exist in this repository. If CI is ever re-enabled, it will need significant updates.
- The disabled `release.yml` builds for `x86_64-pc-windows-msvc` but there is no Windows-specific configuration, testing, or documentation in the project.
- The `docker` job in `ci.yml` runs `cargo test --package life-engine-core --test docker_test -- --ignored` but no `docker_test` test file exists.
- Dependabot config is reasonable (weekly for cargo/npm, monthly for GitHub Actions, with grouping).

### tools/verify-docker-image-size.sh

References `apps/core/Dockerfile` (line 30) which is the broken legacy Dockerfile.

### tools/scripts/ci-check.sh

Local CI mirror script with secret scanning, cargo check, clippy, fmt, and test.

Strengths:

- Secret scanning for private keys and sensitive file extensions
- `--quick` mode for pre-commit use
- Clean pass/fail reporting with early exit on failure

Issues:

- Does not run the Docker image size check or verify Docker builds.
- No pnpm/JS checks despite the project having TypeScript components.

## Infrastructure Topology Assessment

The deployment infrastructure supports three deployment models:

- **Docker (recommended)** — `deploy/docker-compose.yml` for standalone, `deploy/docker-compose.full.yml` for Core + OIDC provider. Multi-stage Alpine-based builds produce small images.
- **Bare metal with systemd/launchd** — `deploy/install.sh` installs binary and system service. Systemd unit has strong security hardening.
- **Behind reverse proxy** — Caddy and nginx configurations provided for TLS termination and security headers.

The topology has a flat architecture: Core is a single binary serving all transports (REST, GraphQL, CalDAV, CardDAV, webhook). There is no microservice decomposition, no service mesh, no sidecar pattern. This is appropriate for the project's self-hosted, single-user/household design.

Missing from the topology:

- **No backup/restore tooling for the deployment.** The SQLCipher database in the named volume has no automated backup mechanism.
- **No monitoring/alerting integration.** The health check endpoint exists, but there is no Prometheus metrics endpoint, no structured log shipping configuration, and no alerting documentation.
- **No update/migration path.** There is no mechanism for rolling updates, database migration on upgrade, or rollback.

## Problems Found

### Critical

1. **`apps/core/Dockerfile` will not build.** It copies only 4 of 28+ workspace members and lacks `perl`/`make` for SQLCipher. `tools/verify-docker-image-size.sh` references this broken Dockerfile, so the image size verification is non-functional.

2. **Docker Compose `local-token` + `0.0.0.0` bind causes startup rejection.** `deploy/docker-compose.yml` sets `LIFE_ENGINE_AUTH_PROVIDER=local-token` and `LIFE_ENGINE_CORE_HOST=0.0.0.0`. The config validation at `config.rs:815` explicitly rejects `local-token` on non-localhost addresses. The container will refuse to start.

3. **Pocket ID port mapping wrong in full compose.** `deploy/docker-compose.full.yml` maps `3751:3751` but Pocket ID serves on port 80 internally. The health check will always fail, preventing Core from starting due to `depends_on: condition: service_healthy`.

4. **Config format mismatch between Docker mounts and config loader.** Docker Compose mounts `config.toml` but the active (legacy) config loader uses `serde_yaml`. The new startup module reads TOML, but the legacy code path is still active. The application will fail to parse the config file.

### Major

5. **Launchd plist uses tilde (`~`) which is not expanded.** All paths in `deploy/launchd/com.life-engine.core.plist` use `~` prefix, but launchd does not perform tilde expansion. The service will fail to find its config file, log directory, and working directory on macOS.

6. **Systemd unit has empty `LIFE_ENGINE_STORAGE_PASSPHRASE`.** Secrets should be loaded from an `EnvironmentFile` with restricted permissions, not embedded (even empty) in the unit file. An empty passphrase may silently disable encryption.

7. **Missing `.dockerignore` file.** Without it, the full repository (including `target/`, `node_modules/`, `.git/`) is sent as Docker build context. This can add gigabytes to the context transfer and dramatically slow builds.

8. **`MemoryDenyWriteExecute=true` may block WASM JIT.** The systemd security hardening directive prevents creating executable memory regions. Wasmtime's default compilation mode requires W^X pages for JIT compilation. This needs testing with plugins loaded.

9. **Nginx WebSocket handling is broken.** `proxy_set_header Connection "";` clears the Connection header for all requests, which prevents WebSocket upgrades that require `Connection: upgrade`. Should be conditional on `$http_upgrade`.

10. **MinIO test compose has console port mismatch.** `docker-compose.test.yml` runs MinIO with `--console-address ":9101"` but maps host port `9101` to container port `9001`. The console listens on 9101 inside the container but the mapping points to 9001 — the console is unreachable.

### Minor

11. **Stale CORS default.** `default_cors_origins()` returns `http://localhost:1420` (a Tauri dev server port). Tauri is no longer used in the project.

12. **Caddyfile references `LE_BEHIND_PROXY` env var.** The actual env var used by the application is `LIFE_ENGINE_BEHIND_PROXY`. The `LE_` prefix does not match the convention used everywhere else.

13. **`deploy/install.sh` does not create `/etc/life-engine/` or `config.toml`.** The systemd service's `ExecStart` references `--config /etc/life-engine/config.toml` but the install script does not create this directory or file.

14. **Duplicate plugin scaffolding mechanisms.** `justfile` `new-plugin` and `tools/scripts/scaffold-plugin.sh` do the same thing but reference different templates and use different approaches.

15. **`justfile` `new-plugin` uses macOS-only `sed -i ''`.** Will fail on Linux/GNU sed.

16. **`.devcontainer/devcontainer.json` includes stale Tauri extension.** The `tauri-apps.tauri-vscode` extension is irrelevant since Tauri is no longer used.

17. **Docker Compose files lack logging driver configuration.** Default `json-file` driver will grow unbounded on long-running deployments.

18. **No CPU limits in Docker Compose.** Only memory limits (256M) are set. The process could consume all available CPU.

19. **Development compose uses `:latest` tags for all services.** Pocket ID, Radicale, and MinIO images are unpinned and can break without notice.

20. **`LIFE_ENGINE_TRANSPORTS_REST_PORT` is set in Docker Compose but not handled by the legacy config.** Only the new `startup` module processes this env var. In the current code, it is silently ignored.

21. **Nginx config uses deprecated `http2` on `listen` directive.** Since nginx 1.25.1, `http2` should be specified as a separate `http2 on;` directive.

22. **Dual config system transition risk.** The `config.rs` file contains both the legacy YAML-based `CoreConfig` and the new TOML-based `startup::CoreConfig`. Docker Compose targets the new TOML format, but the legacy path may still be active in `main.rs`. This dual state creates confusion about which config system is authoritative.

23. **verify-arm64.sh health check creates non-existent config path.** Line 85 passes `--config "$TEMP_DIR/config.toml"` but never creates the file.

24. **Config env var `apply_env_overrides()` has significant code duplication.** The `get_or_insert` pattern for OIDC (3 times) and WebAuthn (4 times) repeats identical default struct construction.

## Recommendations

1. **Delete `apps/core/Dockerfile`** or redirect it to `deploy/Dockerfile`. Update `tools/verify-docker-image-size.sh` to reference `deploy/Dockerfile`.

2. **Fix the Docker Compose auth/bind conflict.** Either change `LIFE_ENGINE_AUTH_PROVIDER` to `oidc` in the standalone compose (with a note to configure OIDC), or add a `LIFE_ENGINE_ALLOW_INSECURE_BIND` escape hatch for container environments where `0.0.0.0` is safe because Docker network isolation provides the security boundary.

3. **Fix Pocket ID port mapping** in `deploy/docker-compose.full.yml` to `3751:80`.

4. **Resolve the config format transition.** Either update `main.rs` to use the new `startup::load_config()` TOML path, or change Docker Compose mounts to reference `config.yaml`. The dual system should be resolved before any production deployment.

5. **Fix launchd plist paths.** The install script should replace `~` with the actual home directory path at install time using `sed` or a template engine.

6. **Use `EnvironmentFile` in the systemd unit** for secrets: `EnvironmentFile=-/etc/life-engine/env`. Document that this file should be created with `chmod 600` and contain `LIFE_ENGINE_STORAGE_PASSPHRASE=<value>`.

7. **Add a `.dockerignore` file** at the repository root excluding `target/`, `node_modules/`, `.git/`, `docs/`, `reports/`, `tools/`, and other non-build directories.

8. **Test `MemoryDenyWriteExecute` with WASM plugins.** If wasmtime JIT requires W^X, either use wasmtime's Cranelift AOT compilation mode or remove the `MemoryDenyWriteExecute=true` directive.

9. **Fix nginx WebSocket handling.** Add conditional Connection header:
   ```
   proxy_set_header Upgrade $http_upgrade;
   proxy_set_header Connection $connection_upgrade;
   ```
   With a `map` block for `$connection_upgrade`.

10. **Fix MinIO test console port.** Change `docker-compose.test.yml` to use `--console-address ":9001"` to match the container port mapping.

11. **Add `LimitNOFILE=65536` and `CapabilityBoundingSet=`** to the systemd unit for production hardening.

12. **Add health checks to development compose services** so integration tests can wait for readiness.

13. **Pin Docker image versions** in the development and test compose files. Use specific tags (e.g., `minio/minio:RELEASE.2024-01-01T00-00-00Z`) instead of `:latest`.

14. **Update the default CORS origin** from `http://localhost:1420` (Tauri) to something relevant to the current architecture.

15. **Add logging driver configuration** to production Docker Compose files (e.g., `json-file` with `max-size` and `max-file`, or `local` driver).
