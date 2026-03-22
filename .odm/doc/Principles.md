---
title: "Life Engine — Design Principles"
tags: [life-engine, architecture, principles, design]
created: 2026-03-21
---

# Design Principles

These principles govern every architectural and implementation decision in Life Engine. They are not aspirational — every design document in this repository must demonstrate how it applies the relevant principles. Every review gate checks for compliance.

For the reasoning behind key technology choices, see the ADRs in `.odm/docs/adrs/`. For how principles are enforced during development, see [[03 - Projects/Life Engine/Planning/Overview#Design Principles]].

## The Principles

### Separation of Concerns

Keep each module, layer, and component responsible for one thing so changes don't ripple unpredictably across the system.

How Life Engine applies this:

- **Core** is an empty orchestrator — it loads plugins, manages storage, and exposes the API. It contains no business logic. All features come from plugins. See [[03 - Projects/Life Engine/Design/Core/Overview]].
- **App Shell** owns data, sync, navigation, and theming. Plugins own UI and user-facing logic. A plugin never touches the database or manages network state. See [[03 - Projects/Life Engine/Design/App/Architecture]].
- **Workflows and Scheduler are independent** — workflows handle request-triggered processing, the scheduler handles time-triggered tasks. They do not share control flow. See [[03 - Projects/Life Engine/Design/Core/Workflow]].
- **SDKs are contracts, not implementations** — `plugin-sdk-rs` and `plugin-sdk-js` define the interface. Host provides the runtime. Plugin bundles carry only types.

### Architecture Decision Records (ADRs)

Document the *why* behind key decisions so future-you (or contributors) don't re-litigate settled questions.

How Life Engine applies this:

- 12 ADRs are published in `.odm/docs/adrs/` during [[03 - Projects/Life Engine/Planning/phases/Phase 0 — Foundation#0.3 — Architecture Decision Records|Phase 0]]. Each follows Context/Decision/Consequences/Alternatives format.
- Any architectural change that introduces a new dependency, replaces a core component, or changes a plugin contract requires a new ADR before implementation.
- ADRs are immutable once accepted. Reversals create a new ADR that supersedes the original. The decision history is never rewritten.

### Fail-Fast with Defined States

Make invalid states unrepresentable so the system surfaces errors immediately rather than silently propagating bad data.

How Life Engine applies this:

- **Schema validation at the boundary** — Bad data is rejected before it hits SQLite. Canonical collections are validated against SDK-defined schemas. Private collections are validated against the JSON Schema declared in the plugin manifest. See [[03 - Projects/Life Engine/Design/Core/Data#Schema Validation]].
- **Workflow validation at creation time** — The workflow engine validates that each step's output shape is compatible with the next step's expected input. Incompatible workflows are rejected at creation time, not at runtime. See [[03 - Projects/Life Engine/Design/Core/Workflow#Data Flow Between Steps]].
- **Plugin loading validation** — The 11-step plugin loading lifecycle validates the manifest, checks capability compatibility, and verifies shared module availability before the plugin ever renders. A plugin with an invalid manifest never enters the running state.
- **Config validation on startup** — Core validates config at startup and rejects insecure settings with clear errors. The system does not start in an ambiguous state.

### Defence in Depth

Every layer (transport, storage, credential, auth) should be independently secure, not relying on upstream layers to compensate.

How Life Engine applies this:

- **Transport** — TLS via `rustls` for all non-localhost connections. See [[03 - Projects/Life Engine/Design/Core/API#Middleware Stack]].
- **Auth** — Every request validated (Pocket ID OIDC or API key). API keys are not a bypass — they go through the same middleware.
- **Storage** — SQLCipher full-database encryption with Argon2id key derivation. See [[03 - Projects/Life Engine/Design/Core/Data#Encryption at Rest]].
- **Credentials** — Individual encryption even within the encrypted database. Refresh tokens encrypted at rest, access tokens in memory only. See [[03 - Projects/Life Engine/Design/Core/Data#Credential Storage]].
- **Plugin isolation** — WASM sandbox (Core plugins) and Shadow DOM (App plugins) provide independent security boundaries. See [[03 - Projects/Life Engine/Design/Core/Plugins#WASM Isolation via Extism]] and [[03 - Projects/Life Engine/Design/App/Architecture/Security Model]].
- **API enforcement** — Rate limiting, CORS, structured error handling, and audit logging are middleware concerns that apply universally — no endpoint can opt out.

### Finish Before Widening

A fully integrated system with fewer features is more valuable than many partially-wired modules sitting unused.

How Life Engine applies this:

- **Phased delivery** — Each phase has measurable exit criteria that must pass before the next phase begins. See [[03 - Projects/Life Engine/Planning/Success Criteria]].
- **Vertical slices** — Phase 1 delivers one complete end-to-end flow (Core + email connector + App shell + email viewer + sidecar mode) rather than scaffolding all connectors at once.
- **Email first** — The email connector is built first because it validates the entire pipeline (auth, fetch, normalise, store, sync). Other connectors wait until Phase 2.
- **Trait-based plugins before WASM** — Phase 1 uses native Rust traits for Core plugins. WASM isolation comes in Phase 4 after the plugin contract is proven through real usage.

### Principle of Least Privilege

Components and plugins should only access what they explicitly declare they need, enforced by the runtime not by convention.

How Life Engine applies this:

- **Deny-by-default capabilities** — Core plugins get no capabilities unless explicitly granted. App plugins get no Shell API methods unless declared in the manifest. See [[03 - Projects/Life Engine/Design/Core/Plugins#Capabilities]] and [[03 - Projects/Life Engine/Design/App/Architecture/Capability System]].
- **Scoped access** — A plugin declaring `storage:read` on `events` cannot read `contacts`. A plugin declaring `http:outbound` for `api.google.com` cannot reach `api.github.com`. The host enforces all scoping at runtime, not by convention.
- **Restricted capabilities** — System-level capabilities (`system:config:write`, `shell:layout`, `plugin:manage`) are limited to first-party plugins via a hardcoded allowlist. Third-party plugins cannot request them. See [[03 - Projects/Life Engine/Design/App/Architecture/Capability System#Restricted Capabilities]].
- **Runtime enforcement** — Even if a manifest were tampered with after install, the shell API proxy re-checks capabilities at runtime. No trust is placed in the static manifest alone.

### Parse, Don't Validate

Use the type system to make invalid data unrepresentable at the boundary so downstream code needs no defensive checks.

How Life Engine applies this:

- **Typed canonical collections** — The SDK defines concrete types for every canonical collection (`Email`, `Event`, `Contact`, etc.). Plugin code works with these types, not raw JSON. See [[03 - Projects/Life Engine/Design/Core/Plugins#Data Model for Plugins]].
- **Schema validation at the boundary** — All data entering the system through the API is validated against JSON Schema before storage. What passes validation is guaranteed to conform. Downstream code trusts the types. See [[03 - Projects/Life Engine/Design/Core/Data#Schema Validation]].
- **Shared types package** — `packages/types/` defines Rust structs with `serde` derives and TypeScript interfaces. One definition, consumed by Core, App, and both SDKs. Invalid data cannot be constructed through the type-safe API.
- **Config validation** — Core config is validated against JSON Schema on load and on patch. The `POST /api/system/config/{section}/validate` endpoint allows pre-validation before applying changes. See [[03 - Projects/Life Engine/Design/Core/API#System Configuration Endpoints]].

### Open/Closed Principle

The shell and engine should be open for extension via plugins but closed to modification when new connectors or features are added.

How Life Engine applies this:

- **Core is a pure orchestrator** — Adding a new feature means adding a new plugin. Core's binary does not change when a connector, processor, or data model is added. See [[03 - Projects/Life Engine/Design/Core/Overview]].
- **StorageAdapter trait** — New storage backends (PostgreSQL, S3) implement the existing trait. No existing code is modified. See [[03 - Projects/Life Engine/Design/Core/Data#Storage Abstraction]].
- **Connectors are regular plugins** — There is no special connector category or trait. A connector is a plugin that declares `http:outbound` and `credentials:read` capabilities. The same plugin interface serves all feature types. See [[03 - Projects/Life Engine/Design/Core/Connectors]].
- **App shell's plugin container** — New App features are plugins loaded into the existing shell container. The shell's architecture does not change when a new plugin is installed.
- **SDK versioning** — New capabilities are expressed as optional traits/interfaces, not by expanding the core interface. Minor versions are additive only. See [[03 - Projects/Life Engine/Design/App/Architecture/Plugin SDK#SDK Versioning]].

### Single Source of Truth

One canonical definition for every type, schema, and capability declaration, consumed everywhere else rather than duplicated.

How Life Engine applies this:

- **`packages/types/`** — Rust structs and TypeScript interfaces for all canonical collections live in one shared package. Core, App, and both SDKs consume these definitions. See [[03 - Projects/Life Engine/Design/MonoRepo Tooling/Repository Structure]].
- **`.odm/docs/schemas/`** — JSON Schema files for canonical collections. Used for validation in Core, App, and plugin manifests. One schema, one location.
- **Canonical collections over private** — The design makes canonical the default and private the exception. Plugins that use canonical collections get automatic interoperability. Private collections exist only for data genuinely unique to a single plugin. See [[03 - Projects/Life Engine/Design/Core/Plugins#Promoting Ecosystem Interoperability]].
- **Plugin manifest as the declaration** — A plugin's capabilities, collections, allowed domains, and settings are all declared in one file (`plugin.json` or the Rust manifest). The runtime reads this single source to configure enforcement.

### Explicit Over Implicit

Connectors and plugins should declare their behaviour, schedules, and requirements in their manifest rather than hiding them in runtime logic.

How Life Engine applies this:

- **Plugin manifests** — Capabilities, collections, allowed domains, shared module dependencies, sidebar configuration, settings schema — all declared upfront in the manifest. The shell reads the manifest before loading any code. See [[03 - Projects/Life Engine/Design/App/Architecture/Plugin Manifest]].
- **Workflow definitions** — Steps, error handling strategies, and trigger routes are explicitly defined in the workflow JSON. No implicit chaining or auto-discovery. See [[03 - Projects/Life Engine/Design/Core/Workflow#Workflow Definitions]].
- **Connector behaviour** — OAuth requirements, sync strategy, rate limits, and allowed domains are part of the connector's plugin manifest. The host knows what a connector will do before it runs.
- **Configuration over convention** — Core's YAML config and environment variables are explicit. Defaults are documented. Network exposure, encryption, and auth all require deliberate configuration changes with startup warnings.

### The Pit of Success

Design your plugin SDK so the easiest path for a plugin author is also the correct, safe, minimal-capability path.

How Life Engine applies this:

- **Canonical collections are the path of least resistance** — Using canonical types requires no schema definition, gives full SDK type support and autocomplete, and provides automatic interoperability with every other plugin. Creating a private collection is more work. See [[03 - Projects/Life Engine/Design/Core/Plugins#Promoting Ecosystem Interoperability]].
- **SDK ships ready-to-use types** — Plugin authors import canonical types directly. No boilerplate, no schema files, no manual validation. See [[03 - Projects/Life Engine/Design/App/Architecture/Plugin SDK]].
- **Scaffolding CLI** — `create-life-engine-plugin` generates a working plugin with correct manifest, correct capability declarations, and a passing test. The generated code already follows all conventions. See [[03 - Projects/Life Engine/Design/App/Architecture/Plugin Scaffolding CLI]].
- **Shell design system** — 17 pre-built Web Components at zero bundle cost. Plugin authors get accessible, themed, responsive UI elements without writing CSS. The easy path produces correct UI. See [[03 - Projects/Life Engine/Design/App/Architecture/Shell Design System]].
- **Shared modules at zero cost** — Lit and React are provided by the host. Plugin authors declare a dependency in the manifest and get framework support without bundling it. No configuration needed.
