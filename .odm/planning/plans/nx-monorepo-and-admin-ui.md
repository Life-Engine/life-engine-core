<!--
project: life-engine-core
specs: nx-monorepo, admin-ui
updated: 2026-03-23
-->

# Project Plan — Nx Monorepo & Admin UI

## Plan Overview

This plan covers two coordinated initiatives: converting the existing Cargo workspace into a full Nx monorepo with polyglot orchestration, and building a React-based admin UI for configuring the Life Engine Core service at runtime.

The Nx monorepo conversion (1.x) runs first to establish the build infrastructure — root `package.json`, `pnpm-workspace.yaml`, `nx.json`, and per-project `project.json` files. Once the monorepo foundation is in place, the admin UI (2.x) is scaffolded as a new `apps/admin` Vite + React application within the monorepo. The admin UI depends on new config API endpoints added to Core, which allow reading and writing `config.yaml` with runtime hot-reload for eligible settings.

Specs included:

- Nx Monorepo Conversion — Set up Nx orchestration, project configurations, task pipelines, and developer scripts on top of the existing Cargo workspace
- Admin UI — Build a React + Vite admin dashboard for viewing and editing Core configuration, viewing plugin status, and monitoring system health

Progress: 0 / 14 work packages complete

---

## 1.1 — Root Package & pnpm Workspace

- [x] Create root `package.json` with project metadata and Nx devDependencies
  <!-- file: package.json -->
  <!-- purpose: Create root package.json with name "life-engine-core", private: true, packageManager field for pnpm, and devDependencies for nx and @nx/js -->
  <!-- requirements: monorepo foundation -->
  <!-- leverage: none -->

- [x] Create `pnpm-workspace.yaml` listing all workspace members
  <!-- file: pnpm-workspace.yaml -->
  <!-- purpose: List apps/*, packages/*, and plugins/* as pnpm workspace members so pnpm resolves workspace:* dependencies -->
  <!-- requirements: pnpm workspace protocol -->
  <!-- leverage: existing packages/types/package.json -->

---

## 1.2 — Nx Installation & Base Config
> depends: 1.1

- [x] Install Nx and plugins, generate lockfile
  <!-- file: package.json, pnpm-lock.yaml -->
  <!-- purpose: Run pnpm install to generate lockfile with nx, @nx/js, @nx/vite, @nx/react as devDependencies -->
  <!-- requirements: Nx available in workspace -->
  <!-- leverage: package.json from WP 1.1 -->

- [x] Create `nx.json` with target defaults, cache inputs, and named inputs
  <!-- file: nx.json -->
  <!-- purpose: Define targetDefaults for build (dependsOn: ^build), test, lint with cache settings; set namedInputs for Rust and TypeScript -->
  <!-- requirements: task caching, affected detection -->
  <!-- leverage: none -->

---

## 1.3 — Rust Project Configurations
> depends: 1.2

- [x] Create `project.json` for `apps/core` with cargo targets
  <!-- file: apps/core/project.json -->
  <!-- purpose: Define build (cargo build -p life-engine-core), test (cargo test -p life-engine-core), and lint (cargo clippy -p life-engine-core) executor targets -->
  <!-- requirements: Nx orchestrates Rust builds -->
  <!-- leverage: apps/core/Cargo.toml -->

- [x] Create `project.json` for each Rust library package
  <!-- file: packages/plugin-sdk-rs/project.json, packages/test-utils/project.json, packages/test-fixtures/project.json, packages/dav-utils/project.json -->
  <!-- purpose: Define build/test/lint targets for each Rust library crate -->
  <!-- requirements: Nx project detection for libraries -->
  <!-- leverage: respective Cargo.toml files -->

- [x] Create `project.json` for each engine plugin
  <!-- file: plugins/engine/connector-email/project.json, plugins/engine/connector-calendar/project.json, plugins/engine/connector-contacts/project.json, plugins/engine/connector-filesystem/project.json, plugins/engine/api-caldav/project.json, plugins/engine/api-carddav/project.json, plugins/engine/webhook-receiver/project.json, plugins/engine/webhook-sender/project.json, plugins/engine/backup/project.json -->
  <!-- purpose: Define build/test targets for each WASM plugin crate -->
  <!-- requirements: Nx project detection for plugins -->
  <!-- leverage: respective Cargo.toml files -->

---

## 1.4 — TypeScript Project Configuration
> depends: 1.2

- [x] Create `project.json` for `packages/types` with TypeScript targets
  <!-- file: packages/types/project.json -->
  <!-- purpose: Define build (tsc) and test (vitest) targets for the dual Rust/TypeScript types package -->
  <!-- requirements: TypeScript compilation in Nx pipeline -->
  <!-- leverage: packages/types/package.json, packages/types/tsconfig.json -->

---

## 1.5 — Task Pipelines & Developer Scripts
> depends: 1.3, 1.4

- [x] Configure composite task pipelines in `nx.json`
  <!-- file: nx.json -->
  <!-- purpose: Add pipeline tasks for dev, build-all, test-all, lint-all that orchestrate across Rust and TypeScript projects with correct dependency ordering -->
  <!-- requirements: unified developer commands -->
  <!-- leverage: nx.json from WP 1.2 -->

- [x] Add root `package.json` scripts for common workflows
  <!-- file: package.json -->
  <!-- purpose: Add scripts: dev, build, test, lint, affected:test, affected:build that delegate to npx nx run-many / nx affected -->
  <!-- requirements: one-command operations -->
  <!-- leverage: package.json from WP 1.1 -->

---

## 1.6 — Gitignore & Verification
> depends: 1.5

- [ ] Update `.gitignore` for Nx cache and node_modules
  <!-- file: .gitignore -->
  <!-- purpose: Add .nx/, node_modules/, dist/ entries to .gitignore if not already present -->
  <!-- requirements: clean git state -->
  <!-- leverage: existing .gitignore -->

- [ ] Verify Nx affected detection and caching work correctly
  <!-- file: (verification — no file output) -->
  <!-- purpose: Run npx nx affected:test and confirm only changed projects are tested; verify cache hits on re-runs -->
  <!-- requirements: affected detection works end-to-end -->
  <!-- leverage: full Nx setup from WPs 1.1–1.5 -->

---

## 2.1 — Config API Endpoints
> depends: 1.3

- [ ] Add GET `/api/system/config` endpoint to Core
  <!-- file: apps/core/src/routes/system.rs -->
  <!-- purpose: Add handler that serializes current CoreConfig to JSON with sensitive fields (passwords, secrets) redacted; requires auth -->
  <!-- requirements: admin UI reads current config -->
  <!-- leverage: CoreConfig struct, existing system routes, Debug impl redaction pattern -->

- [ ] Add PUT `/api/system/config` endpoint to Core
  <!-- file: apps/core/src/routes/system.rs -->
  <!-- purpose: Add handler that accepts partial config JSON, validates against CoreConfig constraints, merges with current config, and writes to config.yaml -->
  <!-- requirements: admin UI writes config changes -->
  <!-- leverage: CoreConfig, serde_yaml, config.rs validation -->

---

## 2.2 — Config Reload & API Tests
> depends: 2.1

- [ ] Add runtime config reload capability
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: Add reload method that re-reads config.yaml and applies hot-reloadable settings (log_level, rate_limit, cors) without server restart -->
  <!-- requirements: config changes take effect without downtime -->
  <!-- leverage: CoreConfig::load, tracing EnvFilter reload -->

- [ ] Wire config reload into PUT endpoint and add route registration
  <!-- file: apps/core/src/routes/system.rs, apps/core/src/main.rs -->
  <!-- purpose: Call config reload after PUT writes config.yaml; register GET/PUT /api/system/config routes in the router -->
  <!-- requirements: end-to-end config update flow -->
  <!-- leverage: system routes, main.rs router setup -->

- [ ] Add tests for config API endpoints
  <!-- file: apps/core/src/routes/system.rs -->
  <!-- purpose: Add tests for GET config (redaction, auth required), PUT config (validation, write, reload), and error cases -->
  <!-- requirements: endpoint correctness -->
  <!-- leverage: existing test_helpers module -->

---

## 2.3 — Admin App Scaffolding
> depends: 1.2

- [ ] Create React + Vite + TypeScript app at `apps/admin`
  <!-- file: apps/admin/package.json, apps/admin/vite.config.ts, apps/admin/tsconfig.json, apps/admin/tsconfig.node.json, apps/admin/index.html -->
  <!-- purpose: Scaffold a new Vite + React + TypeScript application with dev server proxy to Core on port 3750 -->
  <!-- requirements: admin UI foundation -->
  <!-- leverage: none -->

- [ ] Set up Tailwind CSS in admin app
  <!-- file: apps/admin/src/index.css, apps/admin/postcss.config.js, apps/admin/tailwind.config.ts -->
  <!-- purpose: Install and configure Tailwind CSS for utility-first styling with sensible defaults -->
  <!-- requirements: consistent styling -->
  <!-- leverage: none -->

- [ ] Create Nx `project.json` for admin app
  <!-- file: apps/admin/project.json -->
  <!-- purpose: Define dev (vite dev), build (vite build), preview (vite preview), and lint (eslint) targets -->
  <!-- requirements: admin app participates in Nx orchestration -->
  <!-- leverage: nx.json pipeline config from WP 1.2 -->

---

## 2.4 — Shared Config Types & API Client
> depends: 2.2, 2.3

- [ ] Create TypeScript interfaces matching CoreConfig structs
  <!-- file: apps/admin/src/types/config.ts -->
  <!-- purpose: Define TypeScript interfaces for CoreConfig, CoreSettings, AuthSettings, StorageSettings, PluginSettings, NetworkSettings and their nested types -->
  <!-- requirements: type-safe config editing -->
  <!-- leverage: apps/core/src/config.rs struct definitions -->

- [ ] Create API client module for Core communication
  <!-- file: apps/admin/src/api/client.ts -->
  <!-- purpose: Create fetch-based API client with getConfig(), updateConfig(), getSystemInfo(), getPlugins(), healthCheck() methods with error handling -->
  <!-- requirements: admin UI talks to Core API -->
  <!-- leverage: TypeScript config types -->

---

## 2.5 — App Layout & Routing
> depends: 2.3

- [ ] Create admin layout with sidebar navigation
  <!-- file: apps/admin/src/components/Layout.tsx, apps/admin/src/components/Sidebar.tsx -->
  <!-- purpose: Create app shell with sidebar listing pages: Dashboard, Configuration, Plugins, System; highlight active route -->
  <!-- requirements: navigable admin interface -->
  <!-- leverage: Tailwind CSS -->

- [ ] Set up React Router with page routes
  <!-- file: apps/admin/src/App.tsx, apps/admin/src/main.tsx -->
  <!-- purpose: Configure react-router-dom with routes for / (dashboard), /config (config editor), /plugins (plugin list), /system (system status) -->
  <!-- requirements: client-side routing -->
  <!-- leverage: Layout component -->

---

## 2.6 — Config Overview Page
> depends: 2.4, 2.5

- [ ] Create config overview page showing current settings
  <!-- file: apps/admin/src/pages/ConfigPage.tsx -->
  <!-- purpose: Fetch and display current config organized into collapsible sections (Core, Auth, Storage, Plugins, Network) with edit buttons per section -->
  <!-- requirements: admin sees current config at a glance -->
  <!-- leverage: API client, config types -->

---

## 2.7 — Config Editor — Core & Network
> depends: 2.6

- [ ] Create Core settings editor form
  <!-- file: apps/admin/src/components/config/CoreSettingsForm.tsx -->
  <!-- purpose: Form with text inputs for host and data_dir, number input for port, select dropdowns for log_level and log_format, with save/cancel buttons and validation -->
  <!-- requirements: edit core server settings -->
  <!-- leverage: config types, API client updateConfig() -->

- [ ] Create Network settings editor form
  <!-- file: apps/admin/src/components/config/NetworkSettingsForm.tsx -->
  <!-- purpose: Form with TLS enabled toggle and file path inputs, CORS allowed_origins list editor (add/remove), rate_limit number input, with save/cancel buttons -->
  <!-- requirements: edit network/TLS/CORS/rate-limit settings -->
  <!-- leverage: config types, API client updateConfig() -->

---

## 2.8 — Config Editor — Auth & Storage
> depends: 2.6

- [ ] Create Auth settings editor form
  <!-- file: apps/admin/src/components/config/AuthSettingsForm.tsx -->
  <!-- purpose: Form with provider radio selector (local-token/oidc/webauthn), conditional OIDC fields (issuer_url, client_id, client_secret), conditional WebAuthn fields (rp_name, rp_id, rp_origin), with save/cancel buttons -->
  <!-- requirements: edit authentication settings -->
  <!-- leverage: config types, API client updateConfig() -->

- [ ] Create Storage settings editor form
  <!-- file: apps/admin/src/components/config/StorageSettingsForm.tsx -->
  <!-- purpose: Form with backend radio (sqlite/postgres), encryption toggle, Argon2 number inputs, conditional PostgreSQL fields (host, port, dbname, user, password, pool_size, ssl_mode), with save/cancel buttons -->
  <!-- requirements: edit storage settings -->
  <!-- leverage: config types, API client updateConfig() -->

---

## 2.9 — Plugins & System Pages
> depends: 2.4, 2.5

- [ ] Create Plugins page showing loaded plugins and plugin settings
  <!-- file: apps/admin/src/pages/PluginsPage.tsx -->
  <!-- purpose: Display plugin list with id, name, version, status badges; include plugin settings form (paths list editor, auto_enable toggle) -->
  <!-- requirements: admin manages plugins -->
  <!-- leverage: API client getPlugins(), config types PluginSettings -->

- [ ] Create System status page with health and info
  <!-- file: apps/admin/src/pages/SystemPage.tsx -->
  <!-- purpose: Display system version, uptime (formatted), storage backend, plugins loaded count, health check status with auto-refresh -->
  <!-- requirements: admin monitors system health -->
  <!-- leverage: API client getSystemInfo(), healthCheck() -->
