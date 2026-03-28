<!--
project: connector-plugins
source: .odm/qa/reports/phase-3/connector-plugins.md
updated: 2026-03-28
-->

# Connector Plugins — QA Remediation Plan

## Plan Overview

This plan addresses the issues identified in the phase-3 QA review of the four connector plugins (`connector-email`, `connector-contacts`, `connector-calendar`, `connector-filesystem`). Work packages are sequenced by priority: critical security and compilation issues first, then major functionality gaps (pipeline integration, retry/backoff, S3 fixes), then minor cleanup.

**Source:** .odm/qa/reports/phase-3/connector-plugins.md

**Progress:** 0 / 10 work packages complete

---

## 1.1 — Fix S3 Credential Storage Pattern
> depends: none
> spec: .odm/qa/reports/phase-3/connector-plugins.md

- [ ] Replace `secret_access_key: String` in `S3Config` with `credential_key: String` referencing the credential store [critical]
  <!-- file: plugins/engine/connector-filesystem/src/s3.rs -->
  <!-- purpose: S3 connector breaks the credential management pattern used by all other connectors; secrets stored directly in config struct -->
  <!-- requirements: 1 -->
  <!-- leverage: credential store key pattern used by email, contacts, calendar connectors -->
- [ ] Update `S3Client` to retrieve the secret from the credential store at connection time [critical]
  <!-- file: plugins/engine/connector-filesystem/src/s3.rs -->
  <!-- purpose: Wire credential store lookup into S3 operations -->
  <!-- requirements: 1 -->
  <!-- leverage: none -->
- [ ] Update S3 integration tests to use credential store pattern [critical]
  <!-- file: plugins/engine/connector-filesystem/tests/s3_integration.rs -->
  <!-- purpose: Tests must reflect new credential handling -->
  <!-- requirements: 1 -->
  <!-- leverage: existing integration test structure -->

## 1.2 — Fix Calendar Connector Compilation and Let-Chains
> depends: none
> spec: .odm/qa/reports/phase-3/connector-plugins.md

- [x] Refactor `handle_event()` in calendar connector to use nested `if let` blocks instead of let-chains syntax [critical]
  <!-- file: plugins/engine/connector-calendar/src/lib.rs -->
  <!-- purpose: let-chains (if let ... && let ...) require nightly or Rust 2024 edition; may not compile on stable -->
  <!-- requirements: 2 -->
  <!-- leverage: existing handle_event at lib.rs:330-331 -->
- [x] Add `urlencoding` as an explicit dependency in calendar connector Cargo.toml [minor]
  <!-- file: plugins/engine/connector-calendar/Cargo.toml -->
  <!-- purpose: build_auth_url uses urlencoding::encode but crate is not declared; relies on transitive dep -->
  <!-- requirements: 11 -->
  <!-- leverage: none -->

## 1.3 — Add Retry and Backoff to All Connectors
> depends: none
> spec: .odm/qa/reports/phase-3/connector-plugins.md

- [ ] Extract `RetryState` pattern from email connector into a shared utility or SDK module [major]
  <!-- file: packages/plugin-sdk or shared utility -->
  <!-- purpose: Avoid duplicating retry logic across 4 connectors -->
  <!-- requirements: 2 -->
  <!-- leverage: existing RetryState implementation in connector-email/src/lib.rs -->
- [ ] Add retry/backoff state to `connector-contacts` plugin [major]
  <!-- file: plugins/engine/connector-contacts/src/lib.rs -->
  <!-- purpose: Sync failures can trigger rapid-fire retries without throttling -->
  <!-- requirements: 2 -->
  <!-- leverage: shared RetryState once extracted -->
- [ ] Add retry/backoff state to `connector-calendar` plugin [major]
  <!-- file: plugins/engine/connector-calendar/src/lib.rs -->
  <!-- purpose: Same unthrottled retry concern -->
  <!-- requirements: 2 -->
  <!-- leverage: shared RetryState once extracted -->
- [ ] Add retry/backoff state to `connector-filesystem` plugin [major]
  <!-- file: plugins/engine/connector-filesystem/src/lib.rs -->
  <!-- purpose: Same unthrottled retry concern -->
  <!-- requirements: 2 -->
  <!-- leverage: shared RetryState once extracted -->

## 1.4 — Implement Rate Limiting for External APIs
> depends: 1.3
> spec: .odm/qa/reports/phase-3/connector-plugins.md

- [ ] Handle Google API 429 responses with Retry-After backoff in `connector-contacts/src/google.rs` [major]
  <!-- file: plugins/engine/connector-contacts/src/google.rs -->
  <!-- purpose: GoogleApiError::RateLimited variant exists but is never acted upon -->
  <!-- requirements: 5 -->
  <!-- leverage: existing error variant -->
- [ ] Handle Google API 429 responses with Retry-After backoff in `connector-calendar/src/google.rs` [major]
  <!-- file: plugins/engine/connector-calendar/src/google.rs -->
  <!-- purpose: Same rate limiting gap for Google Calendar API -->
  <!-- requirements: 5 -->
  <!-- leverage: existing GoogleApiError::RateLimited variant -->
- [ ] Add connection-rate awareness for IMAP to avoid server-imposed limits [minor]
  <!-- file: plugins/engine/connector-email/src/imap.rs -->
  <!-- purpose: Rapid reconnection could hit IMAP server connection limits -->
  <!-- requirements: 5 -->
  <!-- leverage: none -->

## 1.5 — Fix S3 Client Caching and Pagination
> depends: none
> spec: .odm/qa/reports/phase-3/connector-plugins.md

- [ ] Cache the AWS SDK client in `S3Client` struct, create once in constructor [major]
  <!-- file: plugins/engine/connector-filesystem/src/s3.rs -->
  <!-- purpose: build_sdk_client() is called per operation, wasting connection pooling and TLS setup -->
  <!-- requirements: 6 -->
  <!-- leverage: existing build_sdk_client at s3.rs:148-164 -->
- [ ] Add S3 list pagination using continuation tokens until `is_truncated` is false [major]
  <!-- file: plugins/engine/connector-filesystem/src/s3.rs -->
  <!-- purpose: Only first 1000 objects returned; larger buckets silently truncated -->
  <!-- requirements: 4 -->
  <!-- leverage: existing list_objects at s3.rs:170-213 -->
- [ ] Remove unnecessary `head_object` check before `delete_object` [minor]
  <!-- file: plugins/engine/connector-filesystem/src/s3.rs -->
  <!-- purpose: TOCTOU race; S3 delete on non-existent key is a no-op -->
  <!-- requirements: 8 -->
  <!-- leverage: existing delete_object -->

## 1.6 — Add Connection and Request Timeouts
> depends: none
> spec: .odm/qa/reports/phase-3/connector-plugins.md

- [ ] Add TCP connection timeout to IMAP `connect()` and `connect_plain()` [minor]
  <!-- file: plugins/engine/connector-email/src/imap.rs -->
  <!-- purpose: Non-responsive server blocks indefinitely with no timeout -->
  <!-- requirements: 3 -->
  <!-- leverage: existing connect methods at imap.rs:161-193 -->
- [ ] Add send timeout to SMTP transport configuration [minor]
  <!-- file: plugins/engine/connector-email/src/smtp.rs -->
  <!-- purpose: No timeout on SMTP transport -->
  <!-- requirements: 3 -->
  <!-- leverage: existing transport creation at smtp.rs:84-95 -->
- [ ] Add HTTP request timeout to CardDAV reqwest client in contacts connector [minor]
  <!-- file: plugins/engine/connector-contacts/src/carddav.rs -->
  <!-- purpose: Relies on reqwest defaults with no explicit timeout -->
  <!-- requirements: 3 -->
  <!-- leverage: none -->
- [ ] Add HTTP request timeout to Google API clients (contacts and calendar) [minor]
  <!-- file: plugins/engine/connector-contacts/src/google.rs, plugins/engine/connector-calendar/src/google.rs -->
  <!-- purpose: ensure_valid_token and API calls have no timeout -->
  <!-- requirements: 3 -->
  <!-- leverage: none -->

## 1.7 — SMTP Connection Pooling
> depends: none
> spec: .odm/qa/reports/phase-3/connector-plugins.md

- [ ] Cache SMTP transport in `SmtpClient` struct instead of creating per-send [major]
  <!-- file: plugins/engine/connector-email/src/smtp.rs -->
  <!-- purpose: New TLS connection per email sent is very inefficient for batch operations -->
  <!-- requirements: 7 -->
  <!-- leverage: existing send at smtp.rs:84-95 -->
- [ ] Add retry logic for SMTP send failures [minor]
  <!-- file: plugins/engine/connector-email/src/smtp.rs -->
  <!-- purpose: No retry on transient SMTP failures -->
  <!-- requirements: 7 -->
  <!-- leverage: shared RetryState from WP 1.3 -->
- [ ] Reduce log level for email subject from info to debug [minor]
  <!-- file: plugins/engine/connector-email/src/smtp.rs -->
  <!-- purpose: Info-level subject logging could leak sensitive information in production -->
  <!-- requirements: 7 -->
  <!-- leverage: existing log at smtp.rs:103 -->

## 1.8 — Complete Calendar Outbound Sync
> depends: 1.2
> spec: .odm/qa/reports/phase-3/connector-plugins.md

- [ ] Implement actual HTTP PUT/POST/DELETE in CalDAV CRUD stubs (`create_event`, `update_event`, `delete_event`) [major]
  <!-- file: plugins/engine/connector-calendar/src/caldav.rs -->
  <!-- purpose: CRUD methods are no-op stubs that silently succeed; callers cannot distinguish real from fake ops -->
  <!-- requirements: 8 -->
  <!-- leverage: existing stubs at caldav.rs:194-211 -->
- [ ] Wire `handle_event()` to actually send outbound sync requests instead of only logging [major]
  <!-- file: plugins/engine/connector-calendar/src/lib.rs -->
  <!-- purpose: Event handler builds payloads but never sends them -->
  <!-- requirements: 8 -->
  <!-- leverage: existing handle_event at lib.rs:310-365 -->
- [ ] Make `http_client` field private on `GoogleContactsClient` [minor]
  <!-- file: plugins/engine/connector-contacts/src/google.rs -->
  <!-- purpose: pub field exposes internal reqwest client for external mutation -->
  <!-- requirements: 6 -->
  <!-- leverage: existing field at google.rs:113 -->

## 1.9 — Filesystem Connector Hardening
> depends: none
> spec: .odm/qa/reports/phase-3/connector-plugins.md

- [ ] Add symlink cycle detection to recursive directory scanning [minor]
  <!-- file: plugins/engine/connector-filesystem/src/local.rs -->
  <!-- purpose: Circular symlinks would cause infinite recursion -->
  <!-- requirements: 4 -->
  <!-- leverage: existing scan at local.rs:182-211 -->
- [ ] Add configurable max depth limit for recursive scanning [minor]
  <!-- file: plugins/engine/connector-filesystem/src/local.rs -->
  <!-- purpose: No limit on recursion depth; problematic on deeply nested filesystems -->
  <!-- requirements: 5 -->
  <!-- leverage: existing recursive scan -->
- [ ] Remove unused `notify` crate dependency or implement filesystem watching [minor]
  <!-- file: plugins/engine/connector-filesystem/Cargo.toml -->
  <!-- purpose: Crate listed but never imported; adds to build time -->
  <!-- requirements: 9 -->
  <!-- leverage: none -->
- [ ] Remove unnecessary `watch_paths` clone in `scan()` [minor]
  <!-- file: plugins/engine/connector-filesystem/src/local.rs -->
  <!-- purpose: Clones vector every call to work around borrow checker -->
  <!-- requirements: 10 -->
  <!-- leverage: existing scan at local.rs:164 -->

## 1.10 — Type Consolidation and Cleanup
> depends: none
> spec: .odm/qa/reports/phase-3/connector-plugins.md

- [ ] Consolidate duplicate `SyncState` / `MailboxSyncState` types in email connector [minor]
  <!-- file: plugins/engine/connector-email/src/types.rs, plugins/engine/connector-email/src/imap.rs -->
  <!-- purpose: Two representations of IMAP sync state with different field types (Option vs non-Option) -->
  <!-- requirements: 2 -->
  <!-- leverage: existing types -->
- [ ] Consolidate duplicate `FileChange` / `FileChangeType` types in filesystem connector [minor]
  <!-- file: plugins/engine/connector-filesystem/src/types.rs, plugins/engine/connector-filesystem/src/local.rs -->
  <!-- purpose: Two representations of the same concept without cross-references -->
  <!-- requirements: 1 -->
  <!-- leverage: existing types -->
- [ ] Remove or implement `FileChange::Moved` variant [minor]
  <!-- file: plugins/engine/connector-filesystem/src/local.rs -->
  <!-- purpose: Variant exists but detect_changes never produces it -->
  <!-- requirements: 6 -->
  <!-- leverage: none -->
