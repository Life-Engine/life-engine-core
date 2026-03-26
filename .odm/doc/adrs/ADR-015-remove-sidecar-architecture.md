# ADR-015: Remove Sidecar Architecture — Standalone Core

- **Status:** Accepted
- **Date:** 2026-03-27
- **Supersedes:** ADR-002

## Context

Life Engine Core previously supported a "bundled" deployment mode where a Tauri v2 desktop application spawned Core as a sidecar child process, managed its lifecycle, and communicated over localhost. This added complexity to the codebase (bundled-mode config paths, process management code, Tauri-specific CI) for a deployment path that was not yet validated with users.

Core is better positioned as a standalone server that any client — web, mobile, or desktop — connects to via the REST/GraphQL API. The three remaining deployment modes (standalone binary, Docker container, home server) cover all target use cases.

## Decision

Remove the Tauri sidecar architecture entirely. Core supports three deployment modes: standalone, Docker, and home server. Desktop clients, if needed in the future, will be separate projects that connect to Core as a remote service.

## Consequences

- Simpler Core codebase with no bundled-mode configuration path
- Reduced CI surface (no Tauri build checks)
- `apps/app/` directory removed from the monorepo
- Non-technical users must run Core via Docker or the standalone binary rather than through an embedded desktop app
- Future client applications connect to Core over the network rather than spawning it as a subprocess
