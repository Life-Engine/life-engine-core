# Connector Plugins Review

Review date: 2026-03-28

Reviewer scope: `connector-email`, `connector-contacts`, `connector-calendar`, `connector-filesystem`

## Summary

The four connector plugins form a well-structured layer for integrating Life Engine Core with external data sources. Each plugin follows a consistent architecture: configuration + client + normalizer + sync state tracking, with actual network calls gated behind feature flags. The codebase demonstrates strong separation of concerns, consistent credential handling via credential store references, and thorough unit test coverage.

Key strengths:

- Credential security is handled correctly across all plugins: passwords and secrets are never stored in config structs, only credential store key references
- Incremental sync mechanisms are well-implemented (UIDVALIDITY for IMAP, sync-token/ctag/etag for DAV, syncToken for Google APIs)
- Normalizers are robust with good edge-case handling (missing fields, malformed data, encoding)
- Shared `dav-utils` package eliminates duplication between CardDAV and CalDAV connectors
- Integration tests use real services (GreenMail, Radicale, MinIO) with proper isolation

Key concerns:

- The `execute()` method on the `Plugin` trait is a no-op pass-through in all four connectors; it returns the input unchanged
- Pipeline step and transform modules are empty stubs across all four plugins
- No retry/backoff logic in contacts, calendar, or filesystem connectors (only email has it)
- S3 connector stores `secret_access_key` in the struct directly rather than using credential store references
- SMTP transport is created per-send with no connection pooling
- No rate limiting for any external API calls (Google, IMAP, SMTP, CalDAV, CardDAV, S3)

---

## Plugin-by-Plugin Analysis

### connector-email

#### Cargo.toml

- **Path** — `plugins/engine/connector-email/Cargo.toml`
- Clean dependency structure with optional IMAP deps behind `integration` feature
- Uses `cdylib` + `rlib` crate types for WASM plugin support
- `mail-parser = "0.9"` and `lettre = "0.11"` are solid, maintained crates

#### manifest.toml

- **Path** — `plugins/engine/connector-email/manifest.toml`
- Declares three actions: `sync`, `send`, `status`
- Capabilities correctly include storage read/write, HTTP outbound, and credentials read/write
- Config schema specifies `imap_host`, `imap_port`, `smtp_host`, `smtp_port`, `sync_interval_secs`

#### src/lib.rs

- **Path** — `plugins/engine/connector-email/src/lib.rs`
- `EmailConnectorPlugin` implements both `Plugin` (WASM) and `CorePlugin` (native) traits
- Exponential backoff with retry state is well-implemented: 1 min base, doubles per failure, caps at 1 hour
- `on_unload()` correctly resets all state including retry backoff
- Problem: `execute()` at line 210 is a pass-through; "sync" and "send" actions do nothing with the input
- Problem: `id()` is defined on both `Plugin` and `CorePlugin` impls with identical values, which is correct but fragile if they ever diverge
- Test coverage is excellent: 24+ unit tests covering lifecycle, backoff, accessors, WASM trait, and unknown actions

#### src/imap.rs

- **Path** — `plugins/engine/connector-email/src/imap.rs`
- Clean IMAP client design with per-mailbox sync state via `HashMap<String, MailboxSyncState>`
- Incremental sync via UIDVALIDITY + UIDs is correct
- `compute_start_uid()` properly detects UIDVALIDITY changes and triggers full re-sync
- `process_fetched()` is a pure static method, good for testing
- `connect_plain()` exists for testing but has no production guard beyond feature flag
- Problem: No connection timeout configured on TCP connections (line 175)
- Problem: No TLS certificate validation options exposed; relies entirely on system defaults
- Problem: `connect()` uses `async-std` streams inside a tokio runtime; the comment explains this works but it is a fragile cross-runtime dependency

#### src/smtp.rs

- **Path** — `plugins/engine/connector-email/src/smtp.rs`
- `send()` correctly validates recipients are non-empty before building the message
- `build_message()` is exposed as a standalone function for unit testing
- Problem: A new SMTP transport is created on every `send()` call (line 84-95); no connection pooling or reuse
- Problem: No send timeout configured on the SMTP transport
- Problem: No retry logic on SMTP send failure
- Problem: Subject header is logged at info level (line 103) which could leak sensitive information in production logs

#### src/config.rs

- **Path** — `plugins/engine/connector-email/src/config.rs`
- Simple deserialization config with sensible defaults
- `imap_host` and `smtp_host` are `Option<String>`, good for optional configuration

#### src/types.rs

- **Path** — `plugins/engine/connector-email/src/types.rs`
- Minimal sync state struct with `uid_validity` and `last_uid` as `Option<u32>`
- Duplicates the `MailboxSyncState` concept from `imap.rs` but with slightly different field types (Option vs non-Option); this is confusing

#### src/error.rs

- **Path** — `plugins/engine/connector-email/src/error.rs`
- Well-structured error enum with unique error codes (EMAIL_001 through EMAIL_006)
- Severity assignments are correct: config errors are Fatal, sync/send failures are Retryable

#### src/normalizer.rs

- **Path** — `plugins/engine/connector-email/src/normalizer.rs`
- Comprehensive email normalization from RFC 5322 to CDM `Email` type
- Handles: missing subject, missing date, multi-part bodies, attachments, threading (In-Reply-To + References fallback)
- `extract_body_html()` correctly filters auto-generated HTML from `mail-parser`
- Good: Falls back to `Uuid::new_v4()` when Message-ID is missing
- Good: Sets `created_at` to the email date, `updated_at` to now
- 15 unit tests covering all edge cases

#### src/steps/mod.rs and src/transform/mod.rs

- Both are empty stubs with only module-level doc comments
- Problem: These are declared in `lib.rs` but contain no implementation, meaning the pipeline integration is incomplete

#### src/tests/mod.rs

- Empty stub

#### tests/greenmail_integration.rs

- Comprehensive integration tests against GreenMail Docker container
- Tests: IMAP auth, full sync, incremental sync, attachment handling, SMTP send with round-trip
- Uses `skip_unless_docker!()` macro for CI compatibility
- Good isolation via unique subjects with UUIDs

#### tests/e2e_email_flow.rs

- Thorough offline e2e tests covering the full pipeline: `process_fetched` -> `normalize_message` -> JSON round-trip
- Tests multiple email fixtures: simple, no-subject, attachment, threaded, references-only, no-date, HTML
- Tests incremental sync simulation and UIDVALIDITY change handling
- Tests JSON schema compliance including `skip_serializing_if` behavior

---

### connector-contacts

#### Cargo.toml

- **Path** — `plugins/engine/connector-contacts/Cargo.toml`
- Depends on `dav-utils` shared package for CardDAV operations
- Uses `reqwest` for HTTP, `base64` for auth headers, `mockito` for tests
- No feature-gated dependencies; `reqwest` is always compiled

#### manifest.toml

- **Path** — `plugins/engine/connector-contacts/manifest.toml`
- Declares two actions: `sync`, `status`
- Same capability set as email connector
- Config schema covers both CardDAV and Google Contacts parameters

#### src/lib.rs

- **Path** — `plugins/engine/connector-contacts/src/lib.rs`
- `ContactsConnectorPlugin` manages both CardDAV and Google Contacts clients
- Problem: No retry/backoff state like the email connector has; sync failures are not throttled
- Problem: `execute()` is a pass-through that does nothing with the input
- `on_unload()` correctly clears all clients and sync state
- 20+ unit tests including lifecycle, accessors, Google client state management, WASM trait

#### src/carddav.rs

- **Path** — `plugins/engine/connector-contacts/src/carddav.rs`
- Uses shared `DavSyncState` from `dav-utils` for sync-token/ctag/etag tracking
- `FetchedVCard` implements `DavResource` trait for polymorphic etag filtering
- `filter_changed()` delegates to `dav_utils::etag::filter_changed()` — good reuse
- `auth_header()` delegates to `dav_utils::auth::basic_auth_header()`
- Problem: No HTTP timeout configuration; relies on `reqwest` defaults
- Problem: No connection lifecycle management (no session keep-alive or connection pooling)

#### src/google.rs

- **Path** — `plugins/engine/connector-contacts/src/google.rs`
- Full Google People API v3 client with OAuth2 token refresh, paginated listing, and incremental sync via syncToken
- Token caching with 60-second expiry margin is correct
- Handles HTTP 410 Gone (expired sync token) by clearing state and retrying as full sync
- `normalize_google_person()` maps Google person fields to CDM `Contact` type
- Test-injectable endpoints via `with_endpoints()` constructor
- Problem: `http_client` field is `pub` (line 113) — should be private to prevent external mutation
- Problem: No rate limiting for Google API calls; 429 responses are not handled with backoff
- Problem: `ensure_valid_token()` creates a form POST but does not set a request timeout

#### src/normalizer.rs

- **Path** — `plugins/engine/connector-contacts/src/normalizer.rs`
- Comprehensive vCard parser supporting both 3.0 and 4.0 formats
- Handles: folded lines (space and tab), escaped values, PREF marking, multi-value TYPE params
- `normalize_vcards()` splits multi-vCard responses correctly
- `parse_property_name()` handles parameters without `=` by treating them as TYPE values
- 20+ unit tests covering addresses, organizations, multiple emails, UTF-8, escaped values, partial fields
- Good: Empty address fields are filtered out; unnamed contacts get "(unnamed)" fallback

#### src/config.rs, src/types.rs

- Standard config and sync state structs with sensible defaults
- `ContactsSyncState` tracks both CardDAV and Google sync tokens

#### src/error.rs

- Error codes CONTACTS_001 through CONTACTS_004
- Correct severity assignments

#### src/steps/mod.rs, src/transform/mod.rs, src/tests/mod.rs

- All empty stubs

#### tests/radicale_integration.rs

- Integration tests against Radicale for: PUT/GET round-trip, filter_changed detection, sync state reset
- Proper cleanup via `delete_collection()`
- Good isolation via unique addressbook paths per test

---

### connector-calendar

#### Cargo.toml

- **Path** — `plugins/engine/connector-calendar/Cargo.toml`
- Uses `ical = "0.11"` for iCalendar parsing, `sha2` and `rand` for PKCE
- `reqwest` is optional behind `integration` feature (unlike contacts where it is always compiled)
- Depends on `dav-utils` for shared CalDAV operations

#### manifest.toml

- **Path** — `plugins/engine/connector-calendar/manifest.toml`
- Declares five actions: `sync`, `status`, `calendars`, `google_auth`, `google_callback`
- Most comprehensive action set of all connectors

#### src/lib.rs

- **Path** — `plugins/engine/connector-calendar/src/lib.rs`
- `CalendarConnectorPlugin` manages CalDAV and Google Calendar clients plus PKCE state
- `handle_event()` is the only connector with non-trivial event handling: dispatches `data.created`, `data.updated`, `data.deleted` events for outbound sync
- `google_auth_url()` generates OAuth2 URL with PKCE challenge and stores verifier
- Problem: No retry/backoff state
- Problem: `execute()` is still a pass-through
- Problem: `handle_event()` at line 310 uses `let` chains (`if let ... && let ...`) which require nightly Rust or `#![feature(let_chains)]`; this may not compile on stable
- Problem: `handle_event()` constructs iCal/Google event payloads but never actually sends them (only logs) — outbound sync is incomplete
- 25+ unit tests covering lifecycle, event dispatching, Google OAuth URL generation, PKCE verifier storage

#### src/caldav.rs

- **Path** — `plugins/engine/connector-calendar/src/caldav.rs`
- Per-calendar sync state via `HashMap<String, SyncState>`
- `compute_start_sync()` implements sync-token-first, ctag-fallback logic correctly
- `build_vevent_ical()` converts CDM `CalendarEvent` back to iCalendar format for outbound sync
- `create_event()`, `update_event()`, `delete_event()` are all no-op stubs with `#[allow(dead_code)]`
- Problem: CRUD stubs will silently succeed; callers may not realize outbound writes are no-ops
- Good: `filter_changed()` reuses `dav_utils::etag::filter_changed()`

#### src/google.rs

- **Path** — `plugins/engine/connector-calendar/src/google.rs`
- Full OAuth2 PKCE implementation with `generate_pkce_challenge()` and `compute_s256_challenge()`
- Comprehensive `GoogleApiError` enum with specific variants for 410 (sync token expired), 429 (rate limited), 404, 403
- `GoogleCalendarClient` with token state management, configurable endpoints, sync token tracking
- `build_google_event()` converts CDM `CalendarEvent` to a Google API event struct
- `normalize_google_event()` converts Google API events back to CDM
- Problem: `build_auth_url()` does not URL-encode some parameters correctly; uses `urlencoding::encode()` but the `urlencoding` crate is not in `Cargo.toml` dependencies
- Problem: Rate limiting error variant exists but no automatic retry/backoff on 429 responses
- Problem: `exchange_code()` creates a new `reqwest::Client` per call instead of reusing one

#### src/normalizer.rs

- **Path** — `plugins/engine/connector-calendar/src/normalizer.rs`
- Converts parsed `IcalEvent` to CDM `CalendarEvent`
- Handles: missing DTEND (defaults to +1 hour or +1 day for all-day), recurrence rules, attendees with `mailto:` prefix stripping, CREATED/LAST-MODIFIED timestamps
- `parse_vcalendar()` gracefully skips malformed events and logs warnings
- 20+ unit tests covering all edge cases including timezone params, date-only values, multiple events, invalid input
- Uses shared `dav_utils::ical` helpers for date/time parsing

#### src/config.rs, src/types.rs, src/error.rs

- Standard structures following the same pattern as other connectors
- Error codes CALENDAR_001 through CALENDAR_005
- `OAuthFailed` variant is Fatal severity, which is correct

#### src/steps/mod.rs, src/transform/mod.rs, src/tests/mod.rs

- All empty stubs

#### tests/radicale_integration.rs

- Integration tests: PUT event and verify via GET, incremental sync with etag tracking, first sync detection
- Good isolation and cleanup patterns

---

### connector-filesystem

#### Cargo.toml

- **Path** — `plugins/engine/connector-filesystem/Cargo.toml`
- Uses `notify = "7"` for filesystem watching, `sha2` for checksums, `glob` for pattern matching, `mime_guess` for MIME detection
- AWS S3 SDK behind `integration` feature
- `tempfile` dev-dependency for file-based tests

#### manifest.toml

- **Path** — `plugins/engine/connector-filesystem/manifest.toml`
- Declares three actions: `scan`, `status`, `changes`
- Only requests `storage_read` and `storage_write` capabilities (no HTTP outbound or credentials needed for local FS)
- Config schema includes `watch_paths`, include/exclude patterns, `compute_checksums`, `scan_interval_secs`

#### src/lib.rs

- **Path** — `plugins/engine/connector-filesystem/src/lib.rs`
- `FilesystemConnectorPlugin` manages only local filesystem (no S3 at plugin level)
- Problem: S3 client is defined in `s3.rs` but never wired into the plugin struct; only local connector is usable through the plugin
- Problem: `execute()` is a pass-through
- Problem: No `notify` watcher integration; the `notify` crate is listed as a dependency but never used in any source file
- 15+ unit tests covering lifecycle, accessors, WASM trait

#### src/local.rs

- **Path** — `plugins/engine/connector-filesystem/src/local.rs`
- Well-implemented local filesystem scanning with recursive directory traversal
- Glob-based include/exclude with exclude-takes-priority semantics
- SHA-256 checksums via shared `file_helpers::compute_sha256()`
- Change detection: Created, Modified, Deleted detection by comparing indexed state
- `FileChange::Moved` variant exists in the enum but is never generated by `detect_changes()`
- Problem: `scan()` clones `watch_paths` vector on every call (line 164) which is unnecessary
- Problem: No symlink handling; `is_file()` follows symlinks which could lead to infinite loops with circular symlinks
- Problem: No maximum directory depth limit for recursive scanning
- Good: Gracefully handles unreadable files by logging and skipping
- 15+ tests with `tempfile` for real filesystem operations

#### src/s3.rs

- **Path** — `plugins/engine/connector-filesystem/src/s3.rs`
- `S3Config` with custom `Debug` impl that redacts `secret_access_key`
- `#[serde(skip_serializing)]` on `secret_access_key` prevents serialization leaks
- `CloudStorageConnector` trait with `list_objects`, `get_object`, `put_object`, `delete_object`
- `S3Client` implements the trait behind `integration` feature
- Problem: `secret_access_key` is stored directly in the config struct rather than using a credential store key reference like other connectors
- Problem: `build_sdk_client()` is called on every operation; the AWS SDK client is not cached or reused
- Problem: `list_objects()` does not handle pagination; only retrieves the first page (up to 1000 objects)
- Problem: `delete_object()` calls `head_object` then `delete_object` as two separate requests, creating a TOCTOU race condition
- Good: Path-style addressing enabled for MinIO compatibility

#### src/normalizer.rs

- **Path** — `plugins/engine/connector-filesystem/src/normalizer.rs`
- Clean delegation to shared `file_helpers` for MIME detection, SHA-256, and SystemTime conversion
- `source_id` uses the file path, which is reasonable for local files

#### src/config.rs, src/types.rs, src/error.rs

- Standard structures
- `types.rs` defines a separate `FileChange` and `FileChangeType` that duplicates the `FileChange` enum in `local.rs` — two different representations of the same concept
- Error codes FILESYSTEM_001 through FILESYSTEM_004

#### src/steps/mod.rs, src/transform/mod.rs, src/tests/mod.rs

- All empty stubs

#### tests/s3_integration.rs

- Excellent integration tests against MinIO with RAII bucket cleanup via `TestBucket` guard
- Tests: put/get round-trip, list with prefix filtering, delete existing/nonexistent, full lifecycle, sync state tracking
- Handles cleanup even on panic via dedicated OS thread in `Drop` impl
- Tests create unique buckets per test for isolation

---

## Problems Found

### Critical

- **S3 credential storage** — `s3.rs:S3Config.secret_access_key` stores the secret directly in the struct instead of using a credential store key reference. This breaks the credential management pattern used by all other connectors and risks secret leakage through serialization (mitigated by `skip_serializing`, but the value is still in memory and accessible via the `config()` accessor). File: `plugins/engine/connector-filesystem/src/s3.rs:27`

- **Potential nightly-only Rust feature** — `handle_event()` in `connector-calendar/src/lib.rs:331` uses `if let ... && let ...` (let chains), which requires `#![feature(let_chains)]` on Rust editions before 2024. If the project targets stable Rust, this will not compile. File: `plugins/engine/connector-calendar/src/lib.rs:330-331`

### Major

- **Empty pipeline integration** — `steps/mod.rs` and `transform/mod.rs` are empty stubs in all four connectors. The `execute()` method on the `Plugin` trait returns the input unchanged. This means none of the connectors actually perform any work through the WASM pipeline interface. All files: `*/src/steps/mod.rs`, `*/src/transform/mod.rs`

- **No retry/backoff on contacts, calendar, filesystem connectors** — Only the email connector implements exponential backoff via `RetryState`. The contacts, calendar, and filesystem connectors have no failure throttling, meaning sync failures could trigger rapid-fire retries consuming resources. Files: `connector-contacts/src/lib.rs`, `connector-calendar/src/lib.rs`, `connector-filesystem/src/lib.rs`

- **S3 list_objects does not handle pagination** — `list_objects()` uses `list_objects_v2()` but does not check `is_truncated` or use continuation tokens. Buckets with more than 1000 objects will silently return incomplete results. File: `plugins/engine/connector-filesystem/src/s3.rs:170-213`

- **No rate limiting for external API calls** — None of the connectors implement rate limiting or respect rate-limit headers from external services. Google API 429 responses, IMAP connection limits, and CalDAV server throttling are not handled. The `GoogleApiError::RateLimited` variant exists but is never acted upon.

- **S3 SDK client recreated per operation** — `build_sdk_client()` creates a new `aws_sdk_s3::Client` on every API call. The AWS SDK client is designed to be reused with internal connection pooling. File: `plugins/engine/connector-filesystem/src/s3.rs:148-164`

- **SMTP transport created per send** — `SmtpClient::send()` creates a new `AsyncSmtpTransport` for every email sent, establishing a new TLS connection each time. For batch operations this is very inefficient. File: `plugins/engine/connector-email/src/smtp.rs:84-95`

- **Google Contacts client `http_client` is public** — The `http_client` field on `GoogleContactsClient` is `pub`, exposing the internal reqwest client for external mutation. File: `plugins/engine/connector-contacts/src/google.rs:113`

- **Outbound sync in calendar connector is incomplete** — `handle_event()` builds iCal/Google event payloads on outbound events but only logs them. The actual HTTP PUT/POST to push changes to CalDAV/Google servers is not implemented. File: `plugins/engine/connector-calendar/src/lib.rs:310-365`

- **CalDAV CRUD operations are no-op stubs** — `create_event()`, `update_event()`, `delete_event()` in `caldav.rs` all log a warning and return `Ok(())` without performing any operation. Callers have no way to distinguish a stub from a real operation. File: `plugins/engine/connector-calendar/src/caldav.rs:194-211`

### Minor

- **Duplicate type definitions** — `types.rs` in the filesystem connector defines `FileChange` and `FileChangeType` while `local.rs` defines its own `FileChange` enum. Two representations of the same concept exist without cross-references. Files: `connector-filesystem/src/types.rs`, `connector-filesystem/src/local.rs:44-54`

- **Duplicate sync state type** — `connector-email/src/types.rs` defines `SyncState` with `Option<u32>` fields, while `imap.rs` defines `MailboxSyncState` with `u32` fields. Both represent IMAP sync state. Files: `connector-email/src/types.rs`, `connector-email/src/imap.rs:39-45`

- **No connection timeout on IMAP** — `ImapClient::connect()` and `connect_plain()` do not set timeouts on the TCP connection or TLS handshake. A non-responsive server could block indefinitely. File: `plugins/engine/connector-email/src/imap.rs:161-193`

- **No symlink loop protection** — `local.rs` recursive scanning follows symlinks via `is_dir()` and `is_file()` without cycle detection. Circular symlinks would cause infinite recursion. File: `plugins/engine/connector-filesystem/src/local.rs:182-211`

- **No max depth for recursive scanning** — The filesystem connector has no limit on directory recursion depth, which could be problematic on deeply nested filesystems. File: `plugins/engine/connector-filesystem/src/local.rs:182-211`

- **`FileChange::Moved` variant never generated** — The `Moved` variant exists in `local.rs:53` but `detect_changes()` never produces it; file moves appear as a Deleted + Created pair.

- **Subject logged at info level** — `smtp.rs:103` logs the email subject at info level, which could leak sensitive information in production logs.

- **S3 delete has TOCTOU race** — `delete_object()` calls `head_object` to check existence then `delete_object` to delete, but another process could delete the object between the two calls. File: `plugins/engine/connector-filesystem/src/s3.rs:248-271`

- **`notify` dependency unused** — The `notify` crate (v7) is listed in `Cargo.toml` but is never imported or used in any source file. File: `plugins/engine/connector-filesystem/Cargo.toml:28`

- **Unnecessary `watch_paths` clone** — `scan()` clones the `watch_paths` vector on every invocation to work around borrow checker, which is a minor performance concern. File: `plugins/engine/connector-filesystem/src/local.rs:164`

- **`urlencoding` crate may be missing** — `build_auth_url()` in `connector-calendar/src/google.rs` uses `urlencoding::encode()` but `urlencoding` is not listed in `Cargo.toml`. This may come as a transitive dependency but should be declared explicitly. File: `plugins/engine/connector-calendar/src/google.rs:196-203`

---

## Recommendations

1. **Implement pipeline integration** — Fill in `steps/mod.rs` and `transform/mod.rs` with actual pipeline step handlers so that `execute()` performs real work. This is the most impactful gap, as the WASM plugin interface is currently non-functional.

2. **Add retry/backoff to all connectors** — Extract the `RetryState` pattern from the email connector into the SDK or a shared utility and apply it to contacts, calendar, and filesystem connectors.

3. **Fix S3 credential handling** — Change `S3Config` to use a `credential_key: String` pattern (like all other connectors) instead of storing `secret_access_key` directly.

4. **Add connection/request timeouts** — Configure explicit timeouts on IMAP TCP connections, SMTP transports, reqwest clients (CardDAV, Google APIs), and S3 SDK clients.

5. **Implement rate limiting** — Add rate-limiting awareness for Google API calls (respect 429 + Retry-After), IMAP connections (respect server limits), and S3 operations.

6. **Fix S3 pagination** — Add continuation token handling to `list_objects()` to retrieve all objects in a bucket.

7. **Cache reusable clients** — Cache the AWS SDK client in `S3Client` instead of rebuilding it per operation. Consider connection pooling for SMTP transport.

8. **Complete outbound sync** — Implement the actual HTTP calls in `handle_event()` for calendar outbound sync and the CalDAV CRUD stubs.

9. **Resolve type duplication** — Remove or consolidate the duplicate `SyncState`/`FileChange` types that exist across `types.rs` and the main modules.

10. **Add symlink and depth protection** — Add cycle detection and max-depth limits to the filesystem connector's recursive scanning.

11. **Remove unused `notify` dependency** — Either implement filesystem watching using the `notify` crate or remove it from dependencies to reduce build times.

12. **Verify `let_chains` compatibility** — Confirm the project's Rust edition/toolchain supports `let_chains` syntax, or refactor the `handle_event()` method in the calendar connector to use nested `if let` blocks.
