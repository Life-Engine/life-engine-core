# Life Engine

> A free, open-source personal data sovereignty platform. Standalone self-hosted backend.

- **Core** -- A self-hosted Rust backend that aggregates personal data from external services, stores it locally in an encrypted database, and exposes it through a REST API. Core contains no business logic -- all features come from plugins.

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (1.85+)
- [Node.js](https://nodejs.org/) (22+) with [pnpm](https://pnpm.io/)
- [Docker](https://docs.docker.com/get-docker/) (for dev services)

### Development

```bash
git clone https://github.com/life-engine-org/life-engine.git
cd life-engine

# Install JS dependencies
pnpm install

# Start dev services (Pocket ID, GreenMail, Radicale, MinIO)
docker compose up -d

# Run Core in dev mode
npx nx dev-core

# Run all tests
npx nx cargo-test

# Run all linters
npx nx cargo-lint
```

## Repository Structure

```
life-engine/
  apps/core/              -- Rust Core binary
  packages/types/         -- Shared Rust type definitions
  packages/plugin-sdk-rs/ -- Rust SDK for Core plugin authors
  plugins/engine/         -- First-party Core plugins
  docs/                   -- Documentation, ADRs, JSON schemas
  tools/                  -- Plugin scaffolding templates, dev scripts
  .github/                -- CI/CD workflows, issue/PR templates
```

## Architecture Decision Records

- [ADR-001: Rust for Core](docs/adrs/ADR-001-rust-for-core.md)
- [ADR-002: Tauri v2 for App](docs/adrs/ADR-002-tauri-v2-for-app.md) *(superseded)*
- [ADR-003: Web Components as plugin boundary](docs/adrs/ADR-003-web-components-as-plugin-boundary.md)
- [ADR-004: SQLite + SQLCipher as default storage](docs/adrs/ADR-004-sqlite-sqlcipher-storage.md)
- [ADR-005: axum as HTTP framework](docs/adrs/ADR-005-axum-http-framework.md)
- [ADR-006: Pocket ID for OIDC auth](docs/adrs/ADR-006-pocket-id-oidc-auth.md)
- [ADR-007: Extism for WASM plugin isolation](docs/adrs/ADR-007-extism-wasm-plugins.md)
- [ADR-008: PowerSync for client sync](docs/adrs/ADR-008-powersync-client-sync.md)
- [ADR-009: DCO over CLA](docs/adrs/ADR-009-dco-over-cla.md)
- [ADR-010: Apache 2.0 licence](docs/adrs/ADR-010-apache-2-licence.md)
- [ADR-011: Design for WASM now, implement later](docs/adrs/ADR-011-design-for-wasm-implement-later.md)
- [ADR-012: Lit as recommended plugin framework](docs/adrs/ADR-012-lit-recommended-framework.md)
- [ADR-013: Adoption of 11 governing Design Principles](docs/adrs/ADR-013-design-principles-adoption.md)

## Licence

[Apache 2.0](LICENSE)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, PR process, and DCO sign-off instructions. Project governance is documented in [GOVERNANCE.md](GOVERNANCE.md).

## Security

See [SECURITY.md](SECURITY.md) for reporting vulnerabilities.
