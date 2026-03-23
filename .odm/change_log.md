# Change Log

## 2026-03-23

- WP ph2:2.6 PipelineMessage Envelope: add PipelineMessage struct with MessageMetadata (correlation_id, source, timestamp, auth_context) and TypedPayload enum (Cdm/Custom) to packages/types/src/pipeline.rs. CdmType provides a discriminated union of all 7 canonical collection types plus batch variants (EventBatch, TaskBatch, etc.). SchemaValidated<T> newtype wrapper guarantees JSON Schema validation via jsonschema crate before values enter the pipeline, with Deref for transparent access. Add jsonschema dependency to types crate. Re-export PipelineMessage, MessageMetadata, TypedPayload, CdmType, SchemaValidated, and SchemaValidationError from lib.rs. Fix pre-existing clippy warnings: collapse nested if in events.rs validate_time_range, use derive(Default) with #[default] attribute on TaskPriority::Medium and TaskStatus::Pending instead of manual impls.

- WP ph2:2.5 Notes, Emails, Files, and Credentials Rust Structs: update all four remaining CDM Rust structs in packages/types to match their JSON Schemas. Notes adds NoteFormat enum (Plain/Markdown/Html) and pinned (Option<bool>). Emails restructured with EmailAddress struct (name/address) for from/to/cc/bcc fields, body_text now Option<String>, adds date (DateTime<Utc>), message_id, in_reply_to for threading, read/starred booleans; EmailAttachment updated to filename/mime_type/size_bytes/content_id (removes file_id). Files renames name to filename, size to size_bytes, checksum becomes required String (not Option), adds storage_backend. Credentials replaces issuer/issued_date/expiry_date with name/service/encrypted/expires_at, removes extensions field. Updated all downstream consumers: GraphQL API types and resolvers (GqlEmailParticipant for email addressing), email connector normalizer and tests, filesystem connector normalizer/local/s3, test-fixtures JSON files, test-utils factory functions and assertion macros, validation tests, schema validation integration tests.

- WP ph2:2.4 Contacts Rust Struct with Nested Types: restructure Contact types in packages/types to match the canonical JSON Schema. ContactName replaces display field with prefix/suffix/middle optional fields. EmailAddress renamed to ContactEmail with typed ContactInfoType enum (Home/Work/Other) instead of free-form string. PhoneNumber renamed to ContactPhone with PhoneType enum (Mobile/Home/Work/Fax/Other) and primary boolean. PostalAddress renamed to ContactAddress with region (was state), postal_code (was postcode), and address_type fields. Contact struct renames organisation to organization, adds title, birthday (NaiveDate), photo_url, notes, groups fields. Updated all consumers across the codebase: plugin-sdk-rs re-exports, test-utils factory, validation tests, api-carddav serializer/protocol (with string-to-enum conversion helpers for vCard parsing), connector-contacts google/normalizer (with type mapping from Google API strings to CDM enums), and GraphQL layer (GqlContactName, GqlPhoneNumber, GqlPostalAddress, GqlContact types and mapping code).

- WP ph2:2.3 Events and Tasks Rust Structs: update CalendarEvent and Task structs in packages/types to match JSON Schemas. CalendarEvent gains structured Recurrence type (frequency, interval, until, count, by_day) with from_rrule/to_rrule helpers, Attendee struct (name, email, status with AttendeeStatus enum), Reminder struct (minutes_before, method), EventStatus enum (confirmed/tentative/cancelled), optional end field, all_day boolean, timezone string. Task gains aligned enums: TaskStatus (Pending/InProgress/Completed/Cancelled) and TaskPriority (Low/Medium/High/Urgent) with Default impls, renamed labels to tags, added completed_at, assignee, parent_id fields. Updated all dependent code across connector-calendar (normalizer, google, caldav), api-caldav (serializer, protocol), GraphQL API, test-utils, test-fixtures, plugin-sdk re-exports, validation tests, schema_registry, and quarantine routes.

- WP ph2:2.2 Contacts, Notes, Emails, Files, Credentials JSON Schemas: update all 5 remaining canonical collection schemas from JSON Schema Draft-07 to Draft 2020-12. Contacts schema restructured with $defs for ContactName (given/family required, prefix/suffix/middle optional), ContactEmail, ContactPhone, ContactAddress nested types, type enums (home/work/other for emails/addresses, mobile/home/work/fax/other for phones), primary boolean fields, renamed organisation→organization, added title/birthday/photo_url/notes/groups fields. Notes schema adds format enum (plain/markdown/html) and pinned boolean. Emails schema restructures from/to/cc/bcc as EmailAddress objects (name optional, address required), adds date/message_id/in_reply_to for threading, updates attachments to EmailAttachment with size_bytes/content_id, adds read/starred booleans. Files schema renames name→filename and size→size_bytes, adds storage_backend field, adds SHA-256 hex pattern validation on checksum. Credentials schema renames type→credential_type with $defs CredentialType enum, replaces issuer/issued_date/expiry_date with name/service/encrypted/expires_at fields, removes extensions field per spec (claims serves that purpose). All schemas now have $id metadata and uuid format on id fields.

- WP ph2:2.1 Events and Tasks JSON Schemas: upgrade both schemas from JSON Schema Draft-07 to Draft 2020-12 with richer structured types. Events schema now has structured recurrence object (frequency, interval, until, count, by_day), typed attendees with response status enum (accepted/declined/tentative/needs-action), reminders with method enum (notification/email), event status enum (confirmed/tentative/cancelled), timezone field, all_day boolean, and end as optional. Tasks schema updated with corrected status enum (pending/in_progress/completed/cancelled), priority enum (low/medium/high/urgent), both now optional fields. Added completed_at, tags, assignee, parent_id for subtask relationships. Both schemas add $id metadata and uuid format on id fields.

- WP 1.11 Community Plugin Build Verification: verify community plugins build independently from the monorepo with only life-engine-plugin-sdk as a dependency. Created temporary test plugin outside monorepo implementing CorePlugin trait with one action. Compiled successfully to wasm32-wasip1 target producing valid 3.3MB WebAssembly module. Verified .wasm file loadable by Extism runtime. Added serde_json re-export to plugin SDK so plugin authors don't need it as a separate dependency. Cleaned up temporary project after verification. Phase 1 now complete.

- WP 1.9 Directory Layout Verification: verify complete directory structure against ARCHITECTURE.md. All 24 required top-level directories confirmed present (apps/core, 15 package crates, 7 plugin crates, tools/templates/plugin, .odm/doc). All 7 plugin crates have complete standard layout (lib.rs, config.rs, error.rs, steps/, transform/, types.rs, tests/). Add missing tests/ directories with .gitkeep to 5 crates (types, plugin-sdk-rs, dav-utils, test-utils, test-fixtures). Package crate deviations documented and justified by crate purpose. Workspace compiles clean.

- WP 1.8 Plugin Scaffold Template: create tools/templates/plugin/ with full WASM-compatible plugin scaffold (Cargo.toml, manifest.toml, src/lib.rs with CorePlugin impl, config.rs, error.rs, steps/, transform/, types.rs, tests/). Uses __NAME__/__ID__ placeholders for sed substitution. Fix justfile new-plugin recipe to use compatible placeholder syntax instead of broken just brace escaping. Verified: scaffolded test plugin compiles and all 7 template tests pass.

- WP 1.4 Package Crate Scaffolding: add standard internal layout to all 10 package crates (traits, crypto, storage-sqlite, auth, workflow-engine, transport-rest, transport-graphql, transport-caldav, transport-carddav, transport-webhook). Each crate now has the convention: lib.rs with module declarations, config.rs, error.rs with thiserror-based error enums, handlers/mod.rs (or domain-specific modules like storage.rs, transport.rs, plugin.rs for traits; encryption.rs, kdf.rs, hmac.rs for crypto; loader.rs, executor.rs, event_bus.rs, scheduler.rs for workflow-engine), types.rs, and tests/mod.rs. Also marks WP 1.3 (pnpm workspace) as complete since it was already configured. All crates compile.

- WP 1.2 Nx Configuration: add project.json files for 10 packages missing Nx configuration (traits, crypto, storage-sqlite, auth, workflow-engine, transport-rest, transport-graphql, transport-caldav, transport-carddav, transport-webhook) with build, test, and lint targets using cargo commands. Update nx.json to add test→build dependency in targetDefaults and include test files in Rust namedInputs. All 27 projects now discovered by Nx.

- WP 1.1 Cargo Workspace Configuration: restructure root Cargo.toml workspace members to match new modular architecture. Add 10 new package crates as minimal shells (traits, crypto, storage-sqlite, auth, workflow-engine, transport-rest, transport-graphql, transport-caldav, transport-carddav, transport-webhook). Organize workspace members into logical groups (core binary, foundation, infrastructure, transports, utilities, plugins). Add toml and cron to workspace.dependencies. All crates compile and pass clippy.

- WP 1.6 Justfile Development Commands: add justfile with dev-core (cargo-watch on apps/core and packages with auto-restart), dev-app (vite dev server for Admin UI), and dev-all (runs both concurrently) recipes for one-command development workflow.

- WP 1.5 Plugin Crate Scaffolding: scaffold all 7 first-party plugin crates (connector-email, connector-calendar, connector-contacts, connector-filesystem, webhook-sender, search-indexer, backup) with WASM-compatible standard layout. Add manifest.toml with [plugin], [actions], [capabilities], and [config] sections to each. Set crate-type = ["cdylib", "rlib"] for WASM compilation. Add config.rs, error.rs, steps/mod.rs, transform/mod.rs, types.rs, and tests/mod.rs to each crate. Create search-indexer plugin from scratch with Tantivy dependency. Add thiserror to 4 plugins missing it. All 478 existing tests pass.

- 10-Phase Migration Plan: create comprehensive migration plan in `.odm/planning/plans/` covering the complete transition from the current monolithic architecture to the new modular crate architecture. 10 phase documents (phase-01 through phase-10) with 135 work packages and 172 detailed tasks ordered by dependency graph: monorepo tooling, canonical data models, infrastructure contracts and crypto, plugin SDK, data layer and storage, authentication, workflow engine, plugin system and capabilities, core binary and startup, deployment/migration/plugin conversion.

- Architecture Redesign: add ARCHITECTURE.md as single source of truth defining four-layer modular architecture (transports, workflow engine, plugins, data layer). Rewrite 5 design docs (Overview, Plugins, Workflow, Data, Transports) and mark ADR-011 as superseded (WASM from day one). Delete 5 obsolete docs (Summary, MonoRepo Tooling, Connectors, API). Remove 7 specs no longer aligned (ci-and-cd, connector-architecture, background-scheduler, rest-api, shell-data-api, sync-layer, plugin-loader). Rewrite 11 remaining specs to reflect thin Core orchestrator, WASM plugins via Extism, declarative YAML workflows, configurable transports, StorageBackend trait, and StorageContext query builder.

- WP 2.8 Config Editor — Auth & Storage: add AuthSettingsForm with provider radio selector (local-token/oidc/webauthn), conditional OIDC fields (issuer_url, client_id, client_secret, jwks_uri, audience) and conditional WebAuthn fields (rp_name, rp_id, rp_origin, challenge_ttl_secs). Add StorageSettingsForm with backend radio (sqlite/postgres), encryption toggle, Argon2 inputs (memory_mb, iterations, parallelism), conditional PostgreSQL fields (host, port, dbname, user, password, pool_size, ssl_mode). Wire both forms into ConfigPage with edit buttons on Authentication and Storage sections.

- WP 2.7 Config Editor — Core & Network: add CoreSettingsForm (host, port, log_level, log_format, data_dir inputs with validation) and NetworkSettingsForm (TLS toggle with cert/key path inputs, CORS allowed_origins list editor with add/remove, rate_limit number input). Wire both forms into ConfigPage with per-section edit/view mode toggling. Section component extended with optional Edit button.

- WP 2.5 App Layout & Routing: add react-router-dom, create Layout shell with Sidebar navigation (Dashboard, Configuration, Plugins, System) with active route highlighting, create placeholder page components (DashboardPage, ConfigPage, PluginsPage, SystemPage), wire up BrowserRouter and Routes in App.tsx/main.tsx

- WP 2.4 Shared Config Types & API Client: add TypeScript interfaces in apps/admin/src/types/config.ts mirroring all CoreConfig Rust structs (CoreSettings, AuthSettings, OidcSettings, WebAuthnSettings, StorageSettings, PostgresSettings, Argon2Settings, PluginSettings, NetworkSettings, TlsSettings, CorsSettings, RateLimitSettings) plus SystemInfo, PluginInfo, and HealthStatus types; add fetch-based API client in apps/admin/src/api/client.ts with getConfig(), updateConfig(), getSystemInfo(), getPlugins(), and healthCheck() methods with ApiError class for error handling

- WP 2.3 Admin App Scaffolding: scaffold React 19 + Vite 6 + TypeScript admin app at apps/admin with Tailwind CSS, PostCSS, dev server proxy to Core on port 3750, Nx project.json with dev/build/preview/lint targets, ESLint config, and .gitignore for build artifacts

- WP 2.1 Config API Endpoints: add GET /api/system/config (returns current CoreConfig with secrets redacted) and PUT /api/system/config (accepts partial JSON, merges with current config, validates, persists to config.yaml). Added CoreConfig.to_redacted_json(), merge_partial(), recursive JSON merge, and Arc<RwLock<CoreConfig>> in AppState. 5 new tests covering redaction, auth requirement, update+persist, validation rejection, and PUT auth.

- WP nx:1.5 Task Pipelines & Developer Scripts: configure composite targetDefaults in nx.json for build-ts, test-ts, type-check, and dev targets with correct dependency ordering; add root package.json scripts (dev, build, build:ts, test, test:ts, lint, type-check, affected:test, affected:build) delegating to nx run-many and nx affected; add dev target to core project.json for cargo run workflow

- WP nx:1.3 Rust Project Configurations: add Nx project.json for apps/core (build/test/lint cargo targets), 4 library packages (plugin-sdk-rs, test-utils, test-fixtures, dav-utils), and 9 engine plugins (connector-email, connector-calendar, connector-contacts, connector-filesystem, api-caldav, api-carddav, webhook-receiver, webhook-sender, backup) with wasm32-wasip1 build targets for plugins. All 15 projects visible in Nx graph.

- WP nx:1.2 Nx Installation & Base Config: install Nx and plugins, generate pnpm lockfile, create nx.json with targetDefaults, cache inputs, and named inputs for Rust and TypeScript

- WP nx:1.1 Root Package & pnpm Workspace: create root package.json with Nx devDependencies and pnpm-workspace.yaml listing apps/*, packages/*, plugins/* workspace members

## 2026-03-22

- WP 1.4 Low Priority Improvements (75 findings across 7 categories): token expiry validation, tilde expansion via directories crate, manifest segment validation, search field indexing, hex decode error handling, semver pre-release support, LWW version check on delete, TOCTOU fix with compare_exchange, CORS wildcard warnings, secret redaction in S3/WebDAV Debug+Serialize, collection param pattern constraint, blanket allow(dead_code) removal, cfg(test) gating, schema quarantine dedup, dead code removal, SSE serialization warning, search depth limit, HashSet collection dedup, bounded delivery log, test retry loops replacing sleep, skip_unless_docker generalization, workspace dependency alignment, license consistency, RFC 5545/6350 line folding, vCard PREF parsing, credential extensions field, user_id/household_id storage columns

- WP 1.3 Medium Priority Fixes (37 tasks): auth middleware cleanup and O(1) token lookup, WebAuthn IDOR and passkey fixes, config secret redaction and validation, main.rs error propagation and TLS connection cap, SQLite PRAGMA key validation, PostgreSQL TLS default-require, per-collection migration verification, household guest write guard, federation lock recovery and sync_history cap, deterministic identity signatures, HashSet conflict merge, federated create_with_id, search STRING field and bulk indexing and BooleanQuery filtering, WASM client reuse and log truncation, wasm_adapter deduplication, dav-utils escape ordering and CRLF handling, credential Debug redaction and HttpMethod enum and NaiveDate dates, streaming SHA-256, RFC 5321 SMTP parsing, sanitized route error responses, typed StorageError enum, federation HTTP tests, GraphQL Playground gating and N+1 batch fix, user_id injection stripping, household route integration tests, connector lock-free IO, backup URL encoding and XML parsing and manifest stats and schedule validation, CalDAV/CardDAV PROPFIND/REPORT TODOs, BackupPlugin unit tests, Dockerfile HEALTHCHECK and dep caching, Docker Compose cleanup, OpenAPI limit maximum, modern launchctl, scaffold input validation

- WP 1.2 High Priority Fixes (26 tasks): proper HMAC-SHA256, HKDF key derivation, SQL injection and LIKE injection prevention, random Argon2 salts, file-backed storage, WebAuthn passphrase fix, async-safe federation locks, URL encoding, X-Forwarded-For rate limiting, WASM HTTP headers/body/allowlist fixes, XML injection escaping in DAV/CalDAV/CardDAV, iCal TZID timezone conversion via chrono-tz, storage init rate-limiting, federation changes validation, GraphQL N+1 fix, credential security headers, household role persistence, backup path traversal prevention, scaffold template path fix, build verification test update

- WP 1.1 Critical Fixes: replace XOR cipher with AES-256-GCM across core crypto, identity, credential store, and backup modules
- Fix SQL injection via unvalidated sort_by field in SQLite and PostgreSQL storage backends
- Replace expect() panic in GraphQL handler with 503 SERVICE_UNAVAILABLE error response
- Update Dockerfile Rust version from 1.83 to 1.85 for edition 2024 support

- Replace phase-based planning (phase-0 through phase-4) with QA-driven fix plan from full-project audit
- Add full-project QA report (196 findings across 170 files) and 76-task fix plan organized by severity
- Disable dependabot (renamed to .disabled)

- Remove committed test RSA private key from repo; generate test keys at runtime using rsa crate
- Add pre-commit hook and CI secret scan to block private key material from future commits
- Add *.pem, *.key, *.p12, *.pfx, *.jks, *.keystore to .gitignore

- Simplify scaffold-plugin to engine-only, remove unused lit/vanilla templates and tests
- Remove JS/TS and Playwright checks from branch protection (Rust-only repo)
- Remove stale QA report and tasks

- Initial project scaffold: Rust workspace with Core backend, engine plugins, plugin SDK, type definitions, docs, CI/CD, and dev tooling

- Add identity credential system with encrypted store, selective disclosure, W3C VC export, and DID support (WP 4.7)
- Implement IdentityStore with separate encryption key for identity documents (passport, licence, certificates)
- Implement selective disclosure: signed time-limited tokens asserting specific claims without revealing raw documents
- Implement disclosure audit log recording what was disclosed, to whom, and when
- Implement W3C Verifiable Credentials 2.0 format export with JSON-LD context and credential subject
- Implement DID alignment using did:key method for future interoperability
- Add REST API routes at /api/identity/credentials for CRUD, /disclose, /audit, /vc, and /api/identity/did
- Extract shared crypto module (derive_key, xor_encrypt, hmac_sha256) from credential_store and identity modules
- Add 59 tests covering CRUD, encryption, log safety, selective disclosure, audit logging, W3C VC format, and DID

- Add encrypted remote backup plugin with full/incremental backup, restore, retention, and scheduling (WP 4.6)
- Implement backup engine with full and incremental backup creation, Argon2id key derivation, and AES-256-GCM encryption
- Implement three storage backends (local filesystem, S3-compatible, WebDAV) via shared BackupBackend trait
- Implement configurable backup scheduling (daily, weekly, custom cron) with next-run computation
- Implement retention policy enforcement to keep only the N most recent backups
- Implement full and partial restore with SHA-256 integrity verification and checksum validation
- Fix backup ID collision by adding millisecond precision to timestamp-based IDs
- Clean up compiler warnings in lib.rs, crypto.rs, and S3 backend
- Add 60 tests covering crypto, engine, backends, retention, and scheduling modules

- Add hub-to-hub federated sync with mTLS encrypted transport and pull-based protocol (WP 4.5)
- Implement FederationStore with peer CRUD, sync cursor tracking, and sync history
- Implement mTLS client builder (reqwest) and mTLS server config (rustls with client cert verification)
- Implement pull-based sync engine: per-collection change pulling, cursor management, last-write-wins conflict resolution
- Implement federation API routes: POST /api/federation/peers, POST /api/federation/sync, GET /api/federation/status, GET /api/federation/changes/{collection}
- Add FederationStore to AppState and wire federation routes into the main router
- Extract shared sync primitives module (sync_primitives.rs) with ChangeRecord, SyncCursors, and apply_change for DRY across federation and Core-to-App sync
- Add federation protocol specification document (docs/federation-protocol.md)
- Add 32 tests covering peer CRUD, selective sync, mTLS validation, status reporting, change application, cursor tracking, sync history, and serialization

- Add GraphQL API with queries, mutations, subscriptions, and playground (WP 4.4)
- Auto-generate GraphQL types from all 7 canonical CDM schemas (tasks, contacts, events, emails, notes, files, credentials)
- Implement typed query resolvers with filtering, sorting, and pagination via GraphQL arguments
- Implement mutation resolvers for create, update (with optimistic concurrency), and delete operations
- Implement subscription resolvers for real-time record change delivery via message bus
- Implement nested query resolution: event attendees to contacts, email attachments to files
- Serve GraphQL Playground at /api/graphql/playground for interactive exploration
- Add async-graphql and async-graphql-axum dependencies to workspace
- Add 20 tests covering type generation, CRUD operations, subscriptions, nested queries, and filtering

- Add multi-user household support (WP 4.3): per-user data isolation, role-based access control, and shared collection management
- Extend AuthIdentity with user_id, household_id, and HouseholdRole (Admin, Member, Guest)
- Add user_id and household_id fields to Record struct for namespace-level data isolation
- Add HouseholdStore with household creation, member invites (with expiry), role management, and shared collection configuration
- Add pure check_record_access function for DRY permission enforcement across storage, routes, and plugins
- Add household REST API routes: create household, invite member, accept invite, update roles, manage shared collections
- Add 20 Rust tests covering data isolation, shared collections, RBAC, invite flows, cross-household denial, and pure permission checking

- Add WASM plugin runtime with Extism integration (WP 4.1): sandboxed WASM execution with capability-scoped host functions and resource limits
- Add WasmHostBridge with 11 host functions (store_read/write/query/delete, config_get, event_subscribe/emit, log_info/warn/error, http_request) enforcing declared capabilities per plugin
- Add collection scoping: WASM plugins can only access collections declared in their manifest
- Add resource limits: 64MB memory default, 30-second execution timeout, configurable rate limiting on host function calls
- Add HTTP domain scoping: outbound requests restricted to declared domains
- Add WasmPluginAdapter for migrating native CorePlugins through the WASM bridge with identical output verification
- Add WASM guest SDK bindings (wasm_guest module) in plugin-sdk-rs with HostRequest/HostResponse types and resource limit constants
- Add 46 tests covering capability enforcement, collection isolation, rate limiting, domain scoping, and migration output equivalence

- Add webhook receiver and sender plugins (WP 3.6): bidirectional webhook support with HMAC-SHA256 signature verification, configurable JSON path payload mapping, and delivery log with status code tracking
- Add webhook receiver plugin with endpoint registration, signature verification, and payload-to-CDM field mapping
- Add webhook sender plugin with event bus subscription matching, exponential backoff retry (max 5 attempts), and delivery history
- Extract shared retry/backoff module into plugin SDK (DRY with email connector sync logic)
- Add 146 tests across webhook-receiver, webhook-sender, plugin-sdk retry, and connector-email crates

- Add CalDAV/CardDAV server API plugins (WP 3.5): expose events as CalDAV calendars and contacts as CardDAV address books for native app connectivity
- Add CalDAV server plugin with PROPFIND, REPORT, GET, PUT, DELETE protocol handlers and iCalendar VEVENT round-trip serialisation
- Add CardDAV server plugin with PROPFIND, REPORT, GET, PUT, DELETE protocol handlers and vCard 4.0 round-trip serialisation
- Add `.well-known/caldav` and `.well-known/carddav` service discovery endpoints (RFC 6764)
- Add DNS SRV records documentation for custom domain deployments
- Add shared DAV XML multi-status builder and vCard helpers to dav-utils package
- Add 86 unit tests covering protocol compliance, serialisation round-trips, and native client (iOS, Thunderbird) compatibility

- Add PostgreSQL storage plugin (WP 3.4): PgStorage adapter with deadpool-postgres connection pooling, JSONB document storage, and full-text search via tsvector/tsquery
- Add atomic SQLite-to-PostgreSQL migration with record count verification, rollback on failure, and progress callbacks
- Add PostgreSQL configuration to CoreConfig with environment variable overrides
- Add complete PG test suite mirroring all SQLite tests (auto-skipped when PG unavailable)

- Add plugin store registry infrastructure (WP 3.1): registry JSON index, CI validation scripts, and PR template for plugin submissions
- Add plugin submission validation with manifest, size, and capability checks (29 unit tests)

- Add `.odm/` project documentation scaffold with specs, planning, and design docs
- Update `.gitignore` with `.playwright*` and `.notes/` patterns
