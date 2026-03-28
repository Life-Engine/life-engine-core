# DAV and Webhooks Review

## Summary

This review covers four packages: `packages/transport-caldav`, `packages/transport-carddav`, `packages/transport-webhook`, and `packages/dav-utils`. It also examines the related `plugins/engine/webhook-sender` plugin and `plugins/engine/api-caldav/src/serializer.rs` since they are the primary consumers of these packages.

The three transport crates (`transport-caldav`, `transport-carddav`, `transport-webhook`) are skeletal scaffolding. They provide config structs, error enums, and `Transport` trait implementations, but contain zero protocol logic. All handler, type, config, and test module files are empty (doc-comment only). The `start()` methods log a message and return `Ok(())` without actually binding a socket or serving requests.

The `dav-utils` crate is the most substantive code reviewed. It provides well-tested shared utilities (URL joining, text processing, ETag filtering, iCal parsing, vCard helpers, XML building, auth headers, sync state). The code quality is generally good but has several protocol compliance gaps and potential issues.

The `webhook-sender` plugin has solid subscription management, delivery logging, and retry tracking, but lacks actual HTTP delivery, HMAC payload signing, and rate limiting.

## File-by-File Analysis

### packages/transport-caldav

- **Cargo.toml** — Depends on `dav-utils`, `life-engine-traits`, `life-engine-types`, `axum`, `serde`, `tokio`, `thiserror`, `async-trait`, `toml`, `tracing`. Dependencies are appropriate for a CalDAV transport, though `axum` is unused.
- **src/lib.rs** — Defines `CaldavTransportConfig` (host/port with defaults 127.0.0.1:5232) and `CaldavTransport` implementing the `Transport` trait. `start()` only logs; it does not bind a socket, create an Axum router, or serve any CalDAV requests. `from_config` deserializes from TOML. No protocol handling exists.
- **src/config.rs** — Empty (doc-comment only).
- **src/handlers/mod.rs** — Empty. No PROPFIND, REPORT, GET, PUT, DELETE, or MKCALENDAR handlers.
- **src/types.rs** — Empty.
- **src/error.rs** — Three error variants: `RequestFailed` (retryable), `BindFailed` (fatal), `InvalidConfig` (fatal). Implements `EngineError` with error codes `TRANSPORT_CALDAV_001` through `003`. Well-structured but insufficient for a real CalDAV transport which needs errors for XML parsing, protocol violations, auth failures, etc.
- **src/tests/mod.rs** — Empty.

### packages/transport-carddav

- **Cargo.toml** — Identical dependency set to `transport-caldav`. Same note: `axum` is declared but unused.
- **src/lib.rs** — Near-identical to CalDAV. Config defaults to `127.0.0.1:5233`. Transport trait implementation is a no-op log-only stub.
- **src/config.rs** — Empty.
- **src/handlers/mod.rs** — Empty. No PROPFIND, REPORT, GET, PUT, DELETE, or MKCOL handlers.
- **src/types.rs** — Empty.
- **src/error.rs** — Mirror of CalDAV errors with `TRANSPORT_CARDDAV_*` codes. Same structural notes apply.
- **src/tests/mod.rs** — Empty.

### packages/transport-webhook

- **Cargo.toml** — Same dependency pattern minus `dav-utils` (correctly excluded since webhooks are not DAV). No HTTP client dependency (`reqwest` or `hyper`) for outbound delivery.
- **src/lib.rs** — Config defaults to `127.0.0.1:3001`. Same no-op `Transport` trait implementation. No webhook receiving or dispatching logic.
- **src/config.rs** — Empty.
- **src/handlers/mod.rs** — Empty. No inbound webhook verification, payload parsing, or signature validation handlers.
- **src/types.rs** — Empty.
- **src/error.rs** — `DeliveryFailed` (retryable), `BindFailed` (fatal), `InvalidConfig` (fatal). Adequate for a stub but missing errors for signature verification, payload validation, timeout, etc.
- **src/tests/mod.rs** — Empty.

### packages/dav-utils

- **Cargo.toml** — Dependencies: `anyhow`, `base64` 0.22, `chrono`, `chrono-tz` 0.10, `serde`, `tracing`. Dev: `serde_json`. Lean and appropriate.
- **src/lib.rs** — Module index exposing `auth`, `dav_xml`, `etag`, `ical`, `sync_state`, `text`, `url`, `vcard`. Good doc-comments.
- **src/auth.rs** — `basic_auth_header(username, password)` encodes HTTP Basic auth. Correct implementation using `base64::engine::general_purpose::STANDARD`. Five tests covering roundtrip, special characters, empty values, and unicode. Note: passwords containing colons are encoded correctly (split at first colon is the caller's responsibility during decoding per RFC 7617).
- **src/sync_state.rs** — `DavSyncState` struct with `sync_token`, `ctag`, and `etags` HashMap. Derives `Serialize`/`Deserialize`/`Clone`/`Debug`/`PartialEq`/`Default`. Seven tests. Clean data model, no logic bugs.
- **src/etag.rs** — `DavResource` trait and `filter_changed` function for ETag-based change detection. Correct implementation. Six comprehensive tests. No issues found.
- **src/ical.rs** — `is_date_only` and `parse_ical_datetime` functions. Handles DATE, DATE-TIME, UTC suffix, TZID parameter, VALUE=DATE parameter. Uses `chrono-tz` for timezone conversion. Uses `let-chain` syntax (nightly or edition 2024). Twelve tests. See issues below.
- **src/text.rs** — `unfold_lines`, `decode_escaped_value`, `non_empty`. Line unfolding per RFC 6350/2425. Escape decoding is single-pass (correctly avoids double-unescape bugs). Sixteen tests including regression tests for escape ordering and CRLF normalization. High quality.
- **src/url.rs** — `join_dav_url` with slash normalization preserving `://` scheme. Handles multiple trailing/leading slashes. Ten tests. Clean implementation.
- **src/vcard.rs** — `escape_value`, `parse_property_line`, `has_type_param`, `extract_type`. Sixteen tests including CRLF handling. See issues below.
- **src/dav_xml.rs** — `xml_escape`, `DavResourceEntry`, `DavProperty`, `write_response_entry`, `open_multistatus`, `close_multistatus`, `validate_namespace_declarations`. Twelve tests. See issues below.

### plugins/engine/webhook-sender (Related)

- **Cargo.toml** — Dual crate type (`cdylib` + `rlib`). Has `reqwest` in dependencies for HTTP delivery.
- **src/lib.rs** — `WebhookSenderPlugin` with subscription management, delivery recording, retry tracking. Implements both `Plugin` (WASM) and `CorePlugin` traits. `handle_event` logs matches but does not actually dispatch HTTP requests. 28 tests covering metadata, subscriptions, matching, delivery tracking, retry exhaustion, and reset.
- **src/models.rs** — `WebhookSubscription` with `#[serde(skip_serializing)]` on `secret` (good). Custom `Debug` impl redacts secret. `DeliveryRecord` with success/failure constructors. `DeliveryStatus` enum. Eight tests.
- **src/delivery.rs** — `DeliveryLog` with bounded capacity (10,000 default), eviction of oldest entries, filtering by subscription, recent retrieval. Twelve tests.
- **src/error.rs** — Four error variants with error codes `WEBHOOK_001` through `004`.
- **src/config.rs** — `WebhookSenderConfig` with `max_retries` (default 5) and `max_delivery_log_size` (default 10,000).
- **src/types.rs** — `WebhookSenderStatus` summary struct.
- **src/steps/mod.rs** — Empty.
- **src/transform/mod.rs** — Empty.
- **src/tests/mod.rs** — Empty.

### plugins/engine/api-caldav/src/serializer.rs (Related)

- `event_to_ical` serializes CDM `CalendarEvent` to iCalendar VCALENDAR/VEVENT. Includes RFC 5545 line folding. Uses `ical` crate (v0.11) for parsing via `ical_to_event`. Round-trip tests pass. See issues below.

## Problems Found

### Critical

- **C-001: Transport crates are non-functional stubs** — All three transport crates (`transport-caldav`, `transport-carddav`, `transport-webhook`) implement `Transport::start()` as a no-op that only logs. They do not bind sockets, create HTTP servers, register routes, or serve any requests. The `axum` dependency is declared but completely unused. This means CalDAV/CardDAV/webhook endpoints are not actually available at runtime, despite the transports being "startable". Any code that depends on these transports for protocol access will silently get no functionality.

- **C-002: No actual webhook HTTP delivery** — The webhook-sender plugin records delivery successes/failures in memory but never actually performs an HTTP POST to the webhook URL. `reqwest` is in `Cargo.toml` but is never imported or used in any source file. The `handle_event` method logs that subscriptions match but does not dispatch. This means webhook delivery is entirely simulated.

- **C-003: No HMAC-SHA256 payload signing** — `WebhookSubscription` has a `secret` field documented as "Optional secret for HMAC-SHA256 signing of outgoing payloads" but no signing code exists anywhere. Even when delivery is implemented, there is no cryptographic signing infrastructure. Consumers relying on webhook signature verification will receive unsigned payloads.

### Major

- **M-001: No CalDAV protocol handlers (RFC 4791)** — The CalDAV transport has no PROPFIND handler for calendar discovery, no REPORT handler for calendar-query or calendar-multiget, no GET/PUT/DELETE for individual resources, no MKCALENDAR support, and no WebDAV ACL handling. RFC 4791 compliance is 0%.

- **M-002: No CardDAV protocol handlers (RFC 6352)** — Same as M-001 but for CardDAV. No addressbook-query, addressbook-multiget, or any PROPFIND/REPORT handling. RFC 6352 compliance is 0%.

- **M-003: No inbound webhook verification** — The webhook transport has no handler for receiving inbound webhooks. There is no signature verification (HMAC validation), no replay protection (timestamp checking), no idempotency key handling, and no content-type validation.

- **M-004: No rate limiting for outbound webhooks** — The webhook-sender plugin has no rate limiting for outbound delivery. A burst of events could fire hundreds of concurrent HTTP requests to a single webhook URL, potentially causing target-side rate limiting, IP blocking, or self-inflicted DoS.

- **M-005: iCal serializer does not escape special characters** — In `api-caldav/src/serializer.rs`, `event_to_ical` writes `SUMMARY`, `LOCATION`, and `DESCRIPTION` values directly into iCal output without escaping backslashes, commas, semicolons, or newlines per RFC 5545 section 3.3.11. The `dav_utils::vcard::escape_value` function exists for vCard but there is no equivalent iCal escape function, and the vCard one is not used for iCal output. A calendar event with a title like `"Meeting; Agenda, Notes"` will produce invalid iCalendar output.

- **M-006: iCal line folding is not UTF-8 safe** — `fold_line` in `api-caldav/src/serializer.rs` splits at byte boundaries (75/74 octets) using `&bytes[pos..end]` and `from_utf8_lossy`. If a multi-byte UTF-8 character straddles a fold boundary, the output will contain replacement characters. The code has a comment acknowledging this: "non-ASCII values may need UTF-8 aware splitting in the future."

- **M-007: DavProperty::Custom bypasses XML escaping** — In `dav_xml.rs`, `DavProperty::Custom` inserts its content directly into XML output without any escaping (line 55: `xml.push_str(&format!("        {custom}\r\n"))`). The comment says "assumed to be pre-escaped XML fragments" but there is no validation. If a caller passes unescaped content, the result is malformed or injectable XML.

- **M-008: No XML parsing for DAV requests** — The `dav_xml` module only builds XML responses (output). There is no XML parser for incoming PROPFIND/REPORT request bodies. CalDAV and CardDAV require parsing client XML to determine which properties are requested and what query constraints apply. Without this, even if handlers existed, they could not process standard DAV requests.

- **M-009: vCard `parse_property_line` splits on first colon only** — This causes incorrect parsing for properties whose values contain colons after parameters. For example, `GEO:geo:37.386013,-122.082932` would parse as property `GEO` with value `geo:37.386013,-122.082932`, which is correct. But `NOTE;ENCODING=QUOTED-PRINTABLE:Time: 3:00 PM` would split correctly at the first colon after params. The actual issue is the reverse: the function uses `split_once(':')` which will split `DTSTART;TZID=America/New_York:20260321T100000` correctly, but a property like `X-CUSTOM;VALUE=uri:http://example.com:8080/path` would lose `:8080/path` from the value. However, since `split_once` splits at the first colon, `prop_with_params` would be `X-CUSTOM;VALUE=uri` and `value` would be `http://example.com:8080/path` -- actually this is correct since `split_once` returns everything after the first `:`. Re-reviewing: this is not actually a bug for standard usage.

- **M-010: Missing `Send + Sync` on transport structs** — `CaldavTransport`, `CarddavTransport`, and `WebhookTransport` store only `Clone` config structs, so they are automatically `Send + Sync`. However, once they hold socket handles, connection pools, or state, they may lose this property. The `Transport` trait requires `Send + Sync`, so this is not currently broken but is a design concern for implementation.

### Minor

- **m-001: Duplicate config definitions** — `CaldavTransportConfig` is defined in `lib.rs` while `config.rs` exists as an empty module. The config should live in `config.rs` per the declared module structure. Same applies to CardDAV and Webhook transports.

- **m-002: No TLS configuration** — All three transports default to plaintext (`127.0.0.1` without TLS). There is no TLS configuration in the config structs, no certificate path fields, and no TLS setup code. CalDAV and CardDAV clients commonly require TLS (many clients refuse to send credentials over plaintext).

- **m-003: `open_multistatus` namespace injection** — If `extra_namespaces` passes validation but contains a space before the first `xmlns:`, the output will have `xmlns:D="DAV:" xmlns:C="..."` which is correct. But the format string `"{ns}"` appends directly to the closing quote of `DAV:\"`, so if `ns` is non-empty it must start with a space. The test passes `" xmlns:C=..."` (leading space). This is fragile: callers must know to prefix with a space. A safer API would accept a `&[(&str, &str)]` slice of prefix/URI pairs.

- **m-004: `validate_namespace_declarations` does not handle quoted URIs with spaces** — The function splits on whitespace first, then checks each token. A namespace URI with an encoded space or the unlikely case of `xmlns:C="urn:with space"` would be incorrectly split into multiple tokens and rejected. This is unlikely in practice for DAV namespace URIs but is a limitation.

- **m-005: `DavSyncState.etags` uses `HashMap`** — HashMap has non-deterministic iteration order, which means serialized output order will vary between runs. For sync state comparison via JSON, this works (deserialized comparison is fine) but could cause noisy diffs in logs or debugging.

- **m-006: No connection/request timeout configuration** — The webhook-sender plugin's `reqwest` dependency is unused, but even the config struct has no timeout fields. When HTTP delivery is implemented, it needs connect timeout, request timeout, and total timeout per delivery attempt to prevent hanging connections from exhausting resources.

- **m-007: `event_to_ical` always uses UTC timestamps** — The serializer formats all dates as `YYYYMMDDTHHMMSSZ` (UTC). For all-day events, the RFC 5545 standard expects `VALUE=DATE:YYYYMMDD` without a time component. The `all_day` field exists on `CalendarEvent` but is ignored during serialization. CalDAV clients may display all-day events incorrectly as midnight-to-midnight events.

- **m-008: No Content-Type constants** — None of the transport crates define the required Content-Type headers: `text/calendar; charset=utf-8` for CalDAV, `text/vcard; charset=utf-8` for CardDAV, `application/xml; charset=utf-8` for WebDAV multi-status responses, or `application/json` for webhooks. These will be needed when handlers are implemented.

- **m-009: Empty test modules everywhere** — All three transport crates have empty test files. The `dav-utils` crate has excellent test coverage, but no integration tests exist for the transport layer. The webhook-sender's `steps/`, `transform/`, and `tests/` modules are empty stubs.

- **m-010: `DeliveryLog` uses `Vec::drain` for eviction** — When the log exceeds capacity, `drain(..excess)` shifts all remaining elements left. For 10,000 entries with a single eviction this is O(n). A `VecDeque` would make eviction O(1) at the front.

## Recommendations

1. **Implement CalDAV/CardDAV protocol handlers as the highest priority.** The transport crates are declared in the workspace and importable but provide zero functionality. At minimum, implement PROPFIND (depth 0/1), GET for individual resources, and PUT for creating/updating resources. The `dav-utils` and `api-caldav/serializer` modules provide the building blocks.

2. **Implement actual webhook HTTP delivery.** The `reqwest` dependency is already in `Cargo.toml` for `webhook-sender`. Add `deliver()` method that POSTs to the subscription URL with HMAC-SHA256 signing when a secret is configured, exponential backoff retry, configurable timeouts, and proper error handling.

3. **Add iCal value escaping.** Create `dav_utils::ical::escape_value` analogous to `dav_utils::vcard::escape_value` and use it in the CalDAV serializer for SUMMARY, DESCRIPTION, LOCATION, and all text property values.

4. **Fix UTF-8-safe line folding.** Modify `fold_line` in the CalDAV serializer to find safe split points that do not bisect multi-byte UTF-8 sequences.

5. **Add XML request parsing to dav-utils.** CalDAV and CardDAV both require parsing PROPFIND and REPORT XML request bodies. Consider adding a minimal XML parser (e.g., `quick-xml` or `roxmltree`) to `dav-utils` alongside the existing XML response builder.

6. **Add rate limiting to webhook delivery.** Implement per-URL or per-subscription rate limiting (e.g., token bucket) to prevent overwhelming webhook endpoints.

7. **Handle all-day events properly.** Check `event.all_day` in `event_to_ical` and emit `VALUE=DATE:YYYYMMDD` instead of `YYYYMMDDTHHMMSSZ` for all-day events per RFC 5545.

8. **Move config structs into their config modules.** All three transport crates define config in `lib.rs` while maintaining empty `config.rs` files. Move the structs to be consistent with the module structure.

9. **Add TLS configuration.** Add optional TLS certificate/key path fields to transport configs. CalDAV/CardDAV clients typically require HTTPS.

10. **Make `DavProperty::Custom` safer.** Either validate/escape the custom XML content or change the API to accept structured data instead of raw strings.
