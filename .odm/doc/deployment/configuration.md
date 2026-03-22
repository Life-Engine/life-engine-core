# Configuration Reference

Complete reference for all Life Engine Core configuration options, including the YAML config file, environment variables, and CLI arguments.

## Configuration sources

Life Engine Core loads configuration from three sources in increasing priority order:

1. **YAML file** -- `~/.life-engine/config.yaml` by default, or the path given by `--config`.
2. **Environment variables** -- prefixed with `LIFE_ENGINE_`.
3. **CLI arguments** -- highest priority, override everything.

A value not present at a higher-priority source falls back to the lower source, and ultimately to the built-in default. All fields are optional; a completely absent config file is valid.

## YAML file location

Core looks for `~/.life-engine/config.yaml` at startup. To use a different path:

```bash
life-engine-core --config /etc/life-engine/config.yaml
```

Partial YAML files are valid. Any omitted keys take their default values. A minimal production config that only changes the port:

```yaml
core:
  port: 8080
```

## Full YAML example

```yaml
core:
  host: "127.0.0.1"
  port: 3750
  log_level: "info"
  log_format: "json"
  data_dir: "~/.life-engine/data"

auth:
  provider: "local-token"
  oidc:
    issuer_url: "http://localhost:3751"
    client_id: "life-engine"
    client_secret: "optional-secret"
    jwks_uri: null
    audience: null

storage:
  encryption: true
  argon2:
    memory_mb: 64
    iterations: 3
    parallelism: 4

plugins:
  paths:
    - "/opt/life-engine/plugins"
  auto_enable: false

network:
  tls:
    enabled: false
    cert_path: ""
    key_path: ""
  cors:
    allowed_origins:
      - "http://localhost:1420"
  rate_limit:
    requests_per_minute: 60
```

## Configuration sections

### `core` -- Server settings

- `host` -- The address Core binds to. Default `"127.0.0.1"`. Set to `"0.0.0.0"` to accept connections from all interfaces (required in Docker).
  - Environment variable: `LIFE_ENGINE_CORE_HOST`
  - CLI: `--host`

- `port` -- TCP port. Default `3750`. Must be greater than `0`.
  - Environment variable: `LIFE_ENGINE_CORE_PORT`
  - CLI: `--port`

- `log_level` -- Verbosity. One of `trace`, `debug`, `info`, `warn`, `error`. Default `"info"`.
  - Environment variable: `LIFE_ENGINE_CORE_LOG_LEVEL`
  - CLI: `--log-level`

- `log_format` -- Log output format. `"json"` (structured, for log aggregators) or `"pretty"` (human-readable, for development). Default `"json"`.
  - Environment variable: `LIFE_ENGINE_CORE_LOG_FORMAT`
  - CLI: `--log-format`

- `data_dir` -- Directory for persistent data (SQLite database, plugin state). Default `"~/.life-engine/data"`. Tilde expansion is applied. Must not be empty.
  - Environment variable: `LIFE_ENGINE_CORE_DATA_DIR`
  - CLI: `--data-dir`

### `auth` -- Authentication settings

- `provider` -- Authentication mode. `"local-token"` (default) or `"oidc"`.
  - Environment variable: `LIFE_ENGINE_AUTH_PROVIDER`
  - CLI: none (use YAML or env var)

When `provider` is `"oidc"`, the `auth.oidc` section is required.

### `auth.oidc` -- OIDC settings

These fields configure the connection to Pocket ID or another OIDC-compatible identity provider.

- `issuer_url` -- The OIDC issuer URL. Required for OIDC mode. Example: `"http://localhost:3751"`. Core appends `/.well-known/openid-configuration` to discover endpoints.
  - Environment variable: `LIFE_ENGINE_OIDC_ISSUER_URL`

- `client_id` -- The client ID registered with the identity provider. Required for OIDC mode.
  - Environment variable: `LIFE_ENGINE_OIDC_CLIENT_ID`

- `client_secret` -- The client secret, for confidential clients. Optional.
  - Environment variable: `LIFE_ENGINE_OIDC_CLIENT_SECRET`

- `jwks_uri` -- Custom JWKS endpoint URL. If omitted, derived from `issuer_url`. Optional.

- `audience` -- Expected audience claim value in JWTs. Optional.

### `storage` -- Storage settings

- `encryption` -- Whether to encrypt the SQLite database with SQLCipher using the master passphrase. Default `true`. Set to `false` for development or when running in an already-encrypted volume.
  - Environment variable: `LIFE_ENGINE_STORAGE_ENCRYPTION`

### `storage.argon2` -- Key derivation settings

These control the Argon2 parameters used to derive the SQLCipher encryption key from the master passphrase. Higher values increase security but also increase startup time.

- `memory_mb` -- Memory cost in megabytes. Default `64`.
- `iterations` -- Number of iterations. Default `3`.
- `parallelism` -- Degree of parallelism. Default `4`.

The defaults are tuned for a typical single-user self-hosted machine. Do not lower these values in production without understanding the security trade-offs.

### `plugins` -- Plugin settings

- `paths` -- Array of filesystem paths to scan for plugins. Default empty (no external plugins loaded). Example: `["/opt/life-engine/plugins"]`.
- `auto_enable` -- If `true`, all discovered plugins are loaded automatically. Default `false`.

### `network.tls` -- TLS settings

Core can terminate TLS directly if you do not want to use a reverse proxy. Using a reverse proxy (nginx or Caddy) is generally simpler for Let's Encrypt certificate renewal.

- `enabled` -- Whether to enable TLS. Default `false`.
- `cert_path` -- Path to the PEM certificate file. Required when TLS is enabled.
- `key_path` -- Path to the PEM private key file. Required when TLS is enabled.

Validation rejects a config with `tls.enabled = true` and an empty `cert_path` or `key_path`.

### `network.cors` -- CORS settings

- `allowed_origins` -- Array of origins allowed to make cross-origin requests. Default `["http://localhost:1420"]` (the Tauri dev server). Add your App's origin if deploying to a non-default port or domain.

### `network.rate_limit` -- Rate limiting

- `requests_per_minute` -- Maximum authenticated requests per minute per token. Default `60`. Must be greater than `0`.
  - Environment variable: `LIFE_ENGINE_NETWORK_RATE_LIMIT`

## CLI arguments

CLI arguments override all other configuration sources. The available flags are:

- `--config <path>` -- Path to the YAML config file.
- `--host <addr>` -- Bind address.
- `--port <port>` -- TCP port.
- `--log-level <level>` -- Log level.
- `--log-format <format>` -- Log format.
- `--data-dir <path>` -- Data directory path.

View all options:

```bash
life-engine-core --help
```

## Validation rules

Core validates the configuration on startup and exits with an error message if any rule is violated:

- `port` must not be `0`.
- `log_level` must be one of `trace`, `debug`, `info`, `warn`, `error`.
- `log_format` must be `json` or `pretty`.
- `host` must not be empty.
- `data_dir` must not be empty.
- If `tls.enabled` is `true`, both `cert_path` and `key_path` must be non-empty.
- `rate_limit.requests_per_minute` must be greater than `0`.

## Environment variable quick reference

Every supported environment variable with its corresponding YAML key and default:

- `LIFE_ENGINE_CORE_HOST` -- `core.host` -- `127.0.0.1`
- `LIFE_ENGINE_CORE_PORT` -- `core.port` -- `3750`
- `LIFE_ENGINE_CORE_LOG_LEVEL` -- `core.log_level` -- `info`
- `LIFE_ENGINE_CORE_LOG_FORMAT` -- `core.log_format` -- `json`
- `LIFE_ENGINE_CORE_DATA_DIR` -- `core.data_dir` -- `~/.life-engine/data`
- `LIFE_ENGINE_AUTH_PROVIDER` -- `auth.provider` -- `local-token`
- `LIFE_ENGINE_OIDC_ISSUER_URL` -- `auth.oidc.issuer_url` -- (none)
- `LIFE_ENGINE_OIDC_CLIENT_ID` -- `auth.oidc.client_id` -- (none)
- `LIFE_ENGINE_OIDC_CLIENT_SECRET` -- `auth.oidc.client_secret` -- (none)
- `LIFE_ENGINE_STORAGE_ENCRYPTION` -- `storage.encryption` -- `true`
- `LIFE_ENGINE_NETWORK_RATE_LIMIT` -- `network.rate_limit.requests_per_minute` -- `60`
