<!--
project: dav-and-webhooks
source: .odm/qa/reports/phase-2/dav-and-webhooks.md
updated: 2026-03-28
-->

# DAV and Webhooks — QA Remediation Plan

## Plan Overview

This plan addresses all findings from the Phase 2 QA review of the DAV transports (CalDAV, CardDAV), webhook transport, dav-utils, webhook-sender plugin, and api-caldav serializer. The transport crates are currently non-functional stubs with zero protocol logic. The webhook-sender simulates delivery without actual HTTP dispatch or HMAC signing. Several utility-level bugs exist in iCal serialization and DAV XML handling.

Work packages are ordered by dependency: foundational DAV XML parsing (1.1) enables the CalDAV (1.2) and CardDAV (1.3) transports, which can proceed in parallel. Webhook work (1.4, 1.5) is independent. iCal fixes (1.6) are independent. Housekeeping (1.7) can run last.

**Source:** .odm/qa/reports/phase-2/dav-and-webhooks.md

**Progress:** 7 / 7 work packages complete

---

## 1.1 — DAV XML Request Parsing

> depends: none
> spec: .odm/qa/reports/phase-2/dav-and-webhooks.md

- [x] Add `quick-xml` or `roxmltree` dependency to `dav-utils` Cargo.toml [dependency]
  <!-- file: packages/dav-utils/Cargo.toml -->
  <!-- purpose: Enable XML parsing for incoming DAV request bodies -->
  <!-- requirements: M-008 -->
  <!-- leverage: existing dav_xml module for response building -->
- [x] Implement PROPFIND request body parser in `dav-utils` that extracts requested properties [feature]
  <!-- file: packages/dav-utils/src/dav_xml.rs -->
  <!-- purpose: Parse PROPFIND XML to determine which properties the client requests -->
  <!-- requirements: M-008 -->
  <!-- leverage: existing dav_xml module -->
- [x] Implement REPORT request body parser for calendar-query and calendar-multiget [feature]
  <!-- file: packages/dav-utils/src/dav_xml.rs -->
  <!-- purpose: Parse REPORT XML for CalDAV query and multiget operations -->
  <!-- requirements: M-008 -->
  <!-- leverage: existing dav_xml module -->
- [x] Implement REPORT request body parser for addressbook-query and addressbook-multiget [feature]
  <!-- file: packages/dav-utils/src/dav_xml.rs -->
  <!-- purpose: Parse REPORT XML for CardDAV query and multiget operations -->
  <!-- requirements: M-008 -->
  <!-- leverage: existing dav_xml module -->
- [x] Validate and escape `DavProperty::Custom` content or change API to structured data [fix]
  <!-- file: packages/dav-utils/src/dav_xml.rs -->
  <!-- purpose: Prevent XML injection via unescaped custom property content -->
  <!-- requirements: M-007 -->
  <!-- leverage: existing xml_escape function in dav_xml -->
- [x] Fix `open_multistatus` namespace API to accept structured pairs instead of raw string [fix]
  <!-- file: packages/dav-utils/src/dav_xml.rs -->
  <!-- purpose: Eliminate fragile space-prefixed namespace string convention -->
  <!-- requirements: m-003 -->
  <!-- leverage: existing open_multistatus function -->
- [x] Add tests for XML request parsing round-trips and edge cases [test]
  <!-- file: packages/dav-utils/src/dav_xml.rs -->
  <!-- purpose: Verify correct parsing of PROPFIND and REPORT request bodies -->
  <!-- requirements: M-008 -->
  <!-- leverage: existing dav_xml tests -->

## 1.2 — CalDAV Transport Implementation

> depends: 1.1
> spec: .odm/qa/reports/phase-2/dav-and-webhooks.md

- [x] Implement Axum router with CalDAV route registration in `transport-caldav` [feature]
  <!-- file: packages/transport-caldav/src/lib.rs -->
  <!-- purpose: Create HTTP server that actually binds and serves CalDAV requests -->
  <!-- requirements: C-001 -->
  <!-- leverage: axum dependency already declared -->
- [x] Implement PROPFIND handler for calendar discovery (depth 0/1) [feature]
  <!-- file: packages/transport-caldav/src/handlers/mod.rs -->
  <!-- purpose: Enable CalDAV clients to discover calendars and properties -->
  <!-- requirements: M-001 -->
  <!-- leverage: dav-utils dav_xml module for XML response building -->
- [x] Implement REPORT handler for calendar-query and calendar-multiget [feature]
  <!-- file: packages/transport-caldav/src/handlers/mod.rs -->
  <!-- purpose: Enable CalDAV clients to query and retrieve calendar resources -->
  <!-- requirements: M-001 -->
  <!-- leverage: dav-utils ical module for parsing -->
- [x] Implement GET/PUT/DELETE handlers for individual calendar resources [feature]
  <!-- file: packages/transport-caldav/src/handlers/mod.rs -->
  <!-- purpose: Enable CRUD operations on individual calendar events -->
  <!-- requirements: M-001 -->
  <!-- leverage: dav-utils etag module for change detection -->
- [x] Implement MKCALENDAR handler [feature]
  <!-- file: packages/transport-caldav/src/handlers/mod.rs -->
  <!-- purpose: Enable creation of new calendar collections -->
  <!-- requirements: M-001 -->
  <!-- leverage: none -->
- [x] Add CalDAV-specific error variants for XML parsing, protocol violations, auth failures [fix]
  <!-- file: packages/transport-caldav/src/error.rs -->
  <!-- purpose: Provide comprehensive error handling for real CalDAV operations -->
  <!-- requirements: C-001 -->
  <!-- leverage: existing error.rs structure -->
- [x] Define CalDAV types in types module [feature]
  <!-- file: packages/transport-caldav/src/types.rs -->
  <!-- purpose: Define request/response types for CalDAV protocol handling -->
  <!-- requirements: M-001 -->
  <!-- leverage: none -->
- [x] Add integration tests for CalDAV transport [test]
  <!-- file: packages/transport-caldav/src/tests/mod.rs -->
  <!-- purpose: Verify CalDAV protocol compliance for core operations -->
  <!-- requirements: m-009 -->
  <!-- leverage: none -->

## 1.3 — CardDAV Transport Implementation

> depends: 1.1
> spec: .odm/qa/reports/phase-2/dav-and-webhooks.md

- [x] Implement Axum router with CardDAV route registration in `transport-carddav` [feature]
  <!-- file: packages/transport-carddav/src/lib.rs -->
  <!-- purpose: Create HTTP server that actually binds and serves CardDAV requests -->
  <!-- requirements: C-001 -->
  <!-- leverage: axum dependency already declared -->
- [x] Implement PROPFIND handler for addressbook discovery (depth 0/1) [feature]
  <!-- file: packages/transport-carddav/src/handlers/mod.rs -->
  <!-- purpose: Enable CardDAV clients to discover addressbooks and properties -->
  <!-- requirements: M-002 -->
  <!-- leverage: dav-utils dav_xml module -->
- [x] Implement REPORT handler for addressbook-query and addressbook-multiget [feature]
  <!-- file: packages/transport-carddav/src/handlers/mod.rs -->
  <!-- purpose: Enable CardDAV clients to query and retrieve contact resources -->
  <!-- requirements: M-002 -->
  <!-- leverage: dav-utils vcard module -->
- [x] Implement GET/PUT/DELETE handlers for individual contact resources [feature]
  <!-- file: packages/transport-carddav/src/handlers/mod.rs -->
  <!-- purpose: Enable CRUD operations on individual contacts -->
  <!-- requirements: M-002 -->
  <!-- leverage: dav-utils etag module -->
- [x] Implement MKCOL handler for addressbook creation [feature]
  <!-- file: packages/transport-carddav/src/handlers/mod.rs -->
  <!-- purpose: Enable creation of new addressbook collections -->
  <!-- requirements: M-002 -->
  <!-- leverage: none -->
- [x] Add CardDAV-specific error variants [fix]
  <!-- file: packages/transport-carddav/src/error.rs -->
  <!-- purpose: Provide comprehensive error handling for real CardDAV operations -->
  <!-- requirements: C-001 -->
  <!-- leverage: existing error.rs structure -->
- [x] Define CardDAV types in types module [feature]
  <!-- file: packages/transport-carddav/src/types.rs -->
  <!-- purpose: Define request/response types for CardDAV protocol handling -->
  <!-- requirements: M-002 -->
  <!-- leverage: none -->
- [x] Add integration tests for CardDAV transport [test]
  <!-- file: packages/transport-carddav/src/tests/mod.rs -->
  <!-- purpose: Verify CardDAV protocol compliance for core operations -->
  <!-- requirements: m-009 -->
  <!-- leverage: none -->

## 1.4 — Webhook Transport: Inbound Handling

> depends: none
> spec: .odm/qa/reports/phase-2/dav-and-webhooks.md

- [x] Implement Axum router with webhook receiving endpoint in `transport-webhook` [feature]
  <!-- file: packages/transport-webhook/src/lib.rs -->
  <!-- purpose: Create HTTP server that binds and receives inbound webhook requests -->
  <!-- requirements: C-001 -->
  <!-- leverage: axum dependency already declared -->
- [x] Implement HMAC signature verification for inbound webhooks [feature]
  <!-- file: packages/transport-webhook/src/handlers/mod.rs -->
  <!-- purpose: Validate webhook payload signatures to prevent spoofing -->
  <!-- requirements: M-003 -->
  <!-- leverage: none -->
- [x] Implement replay protection via timestamp checking [feature]
  <!-- file: packages/transport-webhook/src/handlers/mod.rs -->
  <!-- purpose: Reject stale webhook deliveries to prevent replay attacks -->
  <!-- requirements: M-003 -->
  <!-- leverage: none -->
- [x] Implement idempotency key handling [feature]
  <!-- file: packages/transport-webhook/src/handlers/mod.rs -->
  <!-- purpose: Deduplicate webhook deliveries using idempotency keys -->
  <!-- requirements: M-003 -->
  <!-- leverage: none -->
- [x] Implement content-type validation [feature]
  <!-- file: packages/transport-webhook/src/handlers/mod.rs -->
  <!-- purpose: Reject webhook payloads with unexpected content types -->
  <!-- requirements: M-003 -->
  <!-- leverage: none -->
- [x] Add webhook-specific error variants for signature, timeout, validation [fix]
  <!-- file: packages/transport-webhook/src/error.rs -->
  <!-- purpose: Provide comprehensive error handling for webhook operations -->
  <!-- requirements: C-001 -->
  <!-- leverage: existing error.rs structure -->
- [x] Add integration tests for inbound webhook handling [test]
  <!-- file: packages/transport-webhook/src/tests/mod.rs -->
  <!-- purpose: Verify webhook receiving, signature validation, and replay protection -->
  <!-- requirements: m-009 -->
  <!-- leverage: none -->

## 1.5 — Webhook Sender: HTTP Delivery and Signing

> depends: none
> spec: .odm/qa/reports/phase-2/dav-and-webhooks.md

- [x] Implement `deliver()` method that performs actual HTTP POST via reqwest [feature]
  <!-- file: plugins/engine/webhook-sender/src/lib.rs -->
  <!-- purpose: Replace simulated delivery with real HTTP dispatch to webhook URLs -->
  <!-- requirements: C-002 -->
  <!-- leverage: reqwest already in Cargo.toml -->
- [x] Implement HMAC-SHA256 payload signing when subscription has a secret [feature]
  <!-- file: plugins/engine/webhook-sender/src/lib.rs -->
  <!-- purpose: Cryptographically sign outbound webhook payloads for consumer verification -->
  <!-- requirements: C-003 -->
  <!-- leverage: WebhookSubscription.secret field already exists -->
- [x] Add configurable timeouts (connect, request, total) to webhook delivery [feature]
  <!-- file: plugins/engine/webhook-sender/src/config.rs -->
  <!-- purpose: Prevent hanging connections from exhausting resources during delivery -->
  <!-- requirements: m-006 -->
  <!-- leverage: existing WebhookSenderConfig -->
- [x] Implement per-URL rate limiting with token bucket for outbound delivery [feature]
  <!-- file: plugins/engine/webhook-sender/src/lib.rs -->
  <!-- purpose: Prevent overwhelming webhook endpoints with burst delivery -->
  <!-- requirements: M-004 -->
  <!-- leverage: none -->
- [x] Implement exponential backoff retry for failed deliveries [feature]
  <!-- file: plugins/engine/webhook-sender/src/lib.rs -->
  <!-- purpose: Retry failed webhook deliveries with increasing delays -->
  <!-- requirements: C-002 -->
  <!-- leverage: existing retry tracking in delivery.rs -->
- [x] Replace `Vec::drain` with `VecDeque` in `DeliveryLog` for O(1) eviction [fix]
  <!-- file: plugins/engine/webhook-sender/src/delivery.rs -->
  <!-- purpose: Improve eviction performance from O(n) to O(1) -->
  <!-- requirements: m-010 -->
  <!-- leverage: existing DeliveryLog implementation -->
- [x] Add tests for HTTP delivery, HMAC signing, rate limiting, and retry [test]
  <!-- file: plugins/engine/webhook-sender/src/tests/mod.rs -->
  <!-- purpose: Verify real webhook delivery, signing, and rate limiting behavior -->
  <!-- requirements: m-009 -->
  <!-- leverage: existing 28 unit tests -->

## 1.6 — iCal Serializer Fixes

> depends: none
> spec: .odm/qa/reports/phase-2/dav-and-webhooks.md

- [x] Create `escape_ical_value` function in `dav-utils` for RFC 5545 text escaping [fix]
  <!-- file: packages/dav-utils/src/ical.rs -->
  <!-- purpose: Escape backslashes, commas, semicolons, and newlines per RFC 5545 section 3.3.11 -->
  <!-- requirements: M-005 -->
  <!-- leverage: existing vcard::escape_value as reference -->
- [x] Apply iCal escaping to SUMMARY, DESCRIPTION, LOCATION in `event_to_ical` [fix]
  <!-- file: plugins/engine/api-caldav/src/serializer.rs -->
  <!-- purpose: Prevent invalid iCalendar output from special characters in event fields -->
  <!-- requirements: M-005 -->
  <!-- leverage: new escape_ical_value function -->
- [x] Fix `fold_line` to find safe UTF-8 split points [fix]
  <!-- file: plugins/engine/api-caldav/src/serializer.rs -->
  <!-- purpose: Prevent replacement characters from splitting multi-byte UTF-8 sequences -->
  <!-- requirements: M-006 -->
  <!-- leverage: existing fold_line implementation -->
- [x] Handle all-day events with `VALUE=DATE:YYYYMMDD` format [fix]
  <!-- file: plugins/engine/api-caldav/src/serializer.rs -->
  <!-- purpose: Emit correct date-only format for all-day events per RFC 5545 -->
  <!-- requirements: m-007 -->
  <!-- leverage: existing all_day field on CalendarEvent -->
- [x] Add tests for escaping, UTF-8 folding, and all-day event serialization [test]
  <!-- file: plugins/engine/api-caldav/src/serializer.rs -->
  <!-- purpose: Verify correct iCal output for edge cases -->
  <!-- requirements: M-005, M-006, m-007 -->
  <!-- leverage: existing round-trip tests -->

## 1.7 — Housekeeping and Hardening

> depends: 1.2, 1.3, 1.4
> spec: .odm/qa/reports/phase-2/dav-and-webhooks.md

- [x] Move config structs from `lib.rs` into `config.rs` for all three transport crates [refactor]
  <!-- file: packages/transport-caldav/src/config.rs, packages/transport-carddav/src/config.rs, packages/transport-webhook/src/config.rs -->
  <!-- purpose: Align config definitions with declared module structure -->
  <!-- requirements: m-001 -->
  <!-- leverage: existing config.rs empty modules -->
- [x] Add optional TLS certificate/key path fields to transport configs [feature]
  <!-- file: packages/transport-caldav/src/config.rs, packages/transport-carddav/src/config.rs, packages/transport-webhook/src/config.rs -->
  <!-- purpose: Enable HTTPS for transports that commonly require TLS -->
  <!-- requirements: m-002 -->
  <!-- leverage: existing config structs -->
- [x] Define Content-Type constants for each transport [feature]
  <!-- file: packages/transport-caldav/src/types.rs, packages/transport-carddav/src/types.rs, packages/transport-webhook/src/types.rs -->
  <!-- purpose: Provide correct Content-Type headers for protocol responses -->
  <!-- requirements: m-008 -->
  <!-- leverage: none -->
- [x] Replace `HashMap` with `BTreeMap` in `DavSyncState.etags` for deterministic ordering [fix]
  <!-- file: packages/dav-utils/src/sync_state.rs -->
  <!-- purpose: Ensure deterministic serialization for debugging and log diffing -->
  <!-- requirements: m-005 -->
  <!-- leverage: existing DavSyncState struct -->
- [x] Ensure transport structs maintain `Send + Sync` after adding runtime state [design]
  <!-- file: packages/transport-caldav/src/lib.rs, packages/transport-carddav/src/lib.rs, packages/transport-webhook/src/lib.rs -->
  <!-- purpose: Prevent Send+Sync violations when adding socket handles and connection pools -->
  <!-- requirements: M-010 -->
  <!-- leverage: existing Transport trait bounds -->
