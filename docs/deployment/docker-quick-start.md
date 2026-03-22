# Docker Quick Start

Deploy Life Engine Core using Docker or Docker Compose. The repository ships a multi-stage Dockerfile that produces a minimal Alpine-based image, plus two Compose files for common setups.

## Prerequisites

- Docker Engine 20.10 or later
- Docker Compose v2 (the `docker compose` plugin)
- Git (to clone the repository)

## Clone the repository

```bash
git clone https://github.com/life-engine-org/life-engine.git
cd life-engine
```

## The Dockerfile

`apps/core/Dockerfile` uses a two-stage build:

- **Stage 1** (`builder`) -- Compiles the release binary from `rust:1.83-alpine` using `musl-dev` for a fully static binary.
- **Stage 2** -- Copies only the binary into a clean `alpine:3.20` runtime image. The final image includes `ca-certificates` and `wget` (for health checks), runs as a non-root `life-engine` user, and exposes port `3750`.

The data directory `/data` is created inside the image and owned by the service user. Mount a named volume or host directory there to persist data across container restarts.

The image sets one default environment variable:

- `LIFE_ENGINE_CORE_HOST=0.0.0.0` -- Binds to all interfaces inside the container.

## Option 1 -- Minimal single-service setup

`deploy/docker-compose.yml` runs Core alone with the `local-token` auth provider. This is the fastest way to get started.

Start it from the `deploy/` directory:

```bash
cd deploy
docker compose up -d
```

This builds the image from `apps/core/Dockerfile` and starts a single Core container with these environment variables:

- `LIFE_ENGINE_CORE_HOST` = `0.0.0.0`
- `LIFE_ENGINE_CORE_PORT` = `3750`
- `LIFE_ENGINE_CORE_LOG_LEVEL` = `info`
- `LIFE_ENGINE_CORE_DATA_DIR` = `/data`
- `LIFE_ENGINE_AUTH_PROVIDER` = `local-token`

A named volume `core-data` is mounted at `/data` for persistence. The container restarts automatically unless you explicitly stop it.

Verify it is healthy:

```bash
curl http://localhost:3750/api/system/health
```

Generate your first authentication token:

```bash
curl -s -X POST \
  -H "Content-Type: application/json" \
  -d '{"passphrase": "your-master-passphrase"}' \
  http://localhost:3750/api/auth/token
```

Store the returned token securely. It is shown only once.

## Option 2 -- Full stack with Pocket ID (OIDC)

`deploy/docker-compose.full.yml` runs Core alongside Pocket ID for OIDC authentication. Core waits for the Pocket ID health check to pass before starting.

Start the full stack:

```bash
cd deploy
docker compose -f docker-compose.full.yml up -d
```

This starts two services:

- **core** -- Life Engine Core, configured with `LIFE_ENGINE_AUTH_PROVIDER=oidc` and `LIFE_ENGINE_OIDC_ISSUER_URL=http://pocket-id:3751`.
- **pocket-id** -- The Pocket ID identity provider (`stonith404/pocket-id:latest`) on port `3751`.

Once Pocket ID is healthy, Core starts and connects to it automatically. Log in via `POST /api/auth/login` with the credentials configured in Pocket ID.

## Building the image manually

To build the image directly without Compose, run from the repository root:

```bash
docker build \
  -f apps/core/Dockerfile \
  -t life-engine-core:latest \
  .
```

Run the image:

```bash
docker run -d \
  --name life-engine-core \
  -p 3750:3750 \
  -e LIFE_ENGINE_CORE_DATA_DIR=/data \
  -v life-engine-data:/data \
  life-engine-core:latest
```

## Persisting data

The Core SQLite database lives in the `LIFE_ENGINE_CORE_DATA_DIR` path (`/data` inside the container). Always mount a named volume or host path there. If you remove the container without preserving the volume, all data is lost.

Example with a host path instead of a named volume:

```bash
docker run -d \
  --name life-engine-core \
  -p 3750:3750 \
  -e LIFE_ENGINE_CORE_DATA_DIR=/data \
  -v /opt/life-engine/data:/data \
  life-engine-core:latest
```

## Key environment variables

The most important variables for a Docker deployment are:

- `LIFE_ENGINE_CORE_HOST` -- Set to `0.0.0.0` inside containers so the port is reachable from outside. The default inside the image is already `0.0.0.0`.
- `LIFE_ENGINE_CORE_PORT` -- Port to bind on. Default `3750`.
- `LIFE_ENGINE_CORE_DATA_DIR` -- Path to the data directory. Must match the volume mount path.
- `LIFE_ENGINE_AUTH_PROVIDER` -- `local-token` or `oidc`.
- `LIFE_ENGINE_CORE_LOG_LEVEL` -- `trace`, `debug`, `info`, `warn`, or `error`.
- `LIFE_ENGINE_CORE_LOG_FORMAT` -- `json` (default) or `pretty`.

See [configuration.md](configuration.md) for the complete reference.

## Health check

Both Compose files include a health check that polls the health endpoint:

```bash
wget --spider -q http://localhost:3750/api/system/health
```

The minimal Compose file checks every 30 seconds with 3 retries. The Pocket ID service checks every 10 seconds with 5 retries.

## Reverse proxy

For production, place nginx or Caddy in front of Core to handle TLS termination. The nginx config at `deploy/nginx/life-engine.conf` and the Caddy config at `deploy/caddy/Caddyfile` work without modification for a Docker-based setup where the proxy runs on the host and Core runs in a container with port `3750` published.

See [reverse-proxy.md](reverse-proxy.md) for full details.
