# Architecture Decision Records

This directory contains the Architecture Decision Records (ADRs) for Life Engine. Each ADR documents a key architectural or technology decision: its context, what was decided, the consequences, and the alternatives that were considered and rejected.

ADRs are immutable once accepted. If a decision is reversed or superseded, a new ADR is written that explicitly supersedes the original. The history of decisions is never rewritten.

Any change that introduces a new dependency, replaces a core component, or alters a plugin contract requires a new ADR before implementation begins. This is enforced as a review gate.

## Index

- [ADR-001](ADR-001-rust-for-core.md) — Rust for the Core backend (axum, tokio, rusqlite)
- [ADR-002](ADR-002-tauri-v2-for-app.md) — Tauri v2 for the desktop and mobile client
- [ADR-003](ADR-003-web-components-as-plugin-boundary.md) — Web Components with closed Shadow DOM as the App plugin isolation boundary
- [ADR-004](ADR-004-sqlite-sqlcipher-storage.md) — SQLite and SQLCipher as the default encrypted storage backend
- [ADR-005](ADR-005-axum-http-framework.md) — axum as the HTTP framework for Core's REST API
- [ADR-006](ADR-006-pocket-id-oidc-auth.md) — Pocket ID as the self-hosted OIDC identity provider
- [ADR-007](ADR-007-extism-wasm-plugins.md) — Extism as the WASM runtime for Core plugin isolation
- [ADR-008](ADR-008-powersync-client-sync.md) — PowerSync for local-first client-server synchronisation
- [ADR-009](ADR-009-dco-over-cla.md) — Developer Certificate of Origin over Contributor Licence Agreement
- [ADR-010](ADR-010-apache-2-licence.md) — Apache 2.0 as the project licence
- [ADR-011](ADR-011-design-for-wasm-implement-later.md) — Design for WASM plugin isolation now, implement in Phase 4
- [ADR-012](ADR-012-lit-recommended-framework.md) — Lit as the recommended framework for App plugin authors
- [ADR-013](ADR-013-design-principles-adoption.md) — Adoption of 11 governing design principles
- [ADR-014](ADR-014-extensions-namespacing-convention.md) — Extensions namespacing convention for plugin-specific CDM fields

## Format

Each ADR follows this structure:

```
# ADR-NNN: Title

## Status
Accepted | Superseded by ADR-NNN

## Context
What is the problem or decision that needs to be made?

## Decision
What was decided?

## Consequences
What are the positive and negative results?

## Alternatives Considered
What other options were evaluated and why were they rejected?
```

## Contributing

To propose a new ADR, open a pull request with a new file following the naming convention `ADR-NNN-short-description.md`. ADRs are discussed as part of the pull request review before being accepted.
