# Change Log

## 2026-03-22

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
