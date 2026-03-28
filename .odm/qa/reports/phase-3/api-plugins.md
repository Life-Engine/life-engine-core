# API Protocol Plugins Review

Review date: 2026-03-28

Plugins reviewed:

- `plugins/engine/api-caldav/` -- CalDAV server API plugin (RFC 4791)
- `plugins/engine/api-carddav/` -- CardDAV server API plugin (RFC 6352)

Shared dependency reviewed:

- `packages/dav-utils/` -- Shared WebDAV/vCard/iCal utilities

---

## Summary

Both plugins are well-structured early implementations that correctly lay the groundwork for CalDAV and CardDAV server functionality. The code is cleanly organized into `serializer`, `protocol`, and `discovery` modules. Serialization round-trips work correctly, XML responses use proper namespace declarations, and shared utility code in `dav-utils` is solid.

However, the plugins are incomplete as CalDAV/CardDAV server implementations. The most critical gap is that the plugin SDK's `HttpMethod` enum lacks PROPFIND and REPORT, which are the two most essential WebDAV/CalDAV/CardDAV methods. Without these, clients cannot discover or sync resources. Several RFC-required behaviors are missing, and there are correctness issues in line folding, ETag generation, and vCard serialization that would cause interoperability problems with real clients.

---

## Plugin-by-Plugin Analysis

### api-caldav

#### Cargo.toml

Well configured. Uses workspace inheritance for common fields. Dependencies are appropriate -- `ical` crate for parsing, `chrono` for dates, `dav-utils` for shared WebDAV logic. No unnecessary dependencies.

#### project.json

Standard Nx project configuration. Build target correctly specifies `wasm32-wasip1` target, consistent with the WASM plugin architecture.

#### src/lib.rs

The plugin struct implements `CorePlugin` correctly with appropriate metadata and capabilities (StorageRead, StorageWrite, Logging).

**Issues found:**

- **Missing PROPFIND route (Critical):** The TODO at line 99 notes that `HttpMethod::Propfind` does not exist in the SDK. PROPFIND is the primary discovery method in CalDAV -- without it, no client can discover the calendar or list events. This is a blocking gap.
- **Missing REPORT route (Critical):** The TODO at line 104 notes the same for REPORT. Calendar-query and calendar-multiget REPORT requests are how CalDAV clients fetch events. Without REPORT, bulk sync is impossible.
- **Missing OPTIONS route (Major):** CalDAV servers must respond to OPTIONS requests with a `DAV: 1, calendar-access` header (RFC 4791 Section 5.1). The plugin does not register an OPTIONS handler. Without this, clients cannot detect CalDAV support.
- **No MKCALENDAR support (Minor):** RFC 4791 defines MKCALENDAR for creating calendar collections. Since this plugin uses a single hardcoded "default" calendar, this is acceptable for v0.1 but should be planned.
- **Well-known endpoint uses GET (Major):** The `.well-known/caldav` route is registered as GET, but RFC 6764 specifies that clients may use any method (commonly PROPFIND) on the well-known URI. The endpoint should handle both GET and PROPFIND, returning a 301 redirect to the principal URL.

Tests are thorough for the functionality that exists: plugin metadata, route registration, lifecycle, and iOS/Thunderbird compatibility checks.

#### src/discovery.rs

Implements RFC 6764 service discovery constants and principal PROPFIND response XML.

**Issues found:**

- **No actual redirect mechanism (Major):** The code defines `CALDAV_PRINCIPAL_URL` and `build_principal_propfind_xml()` but there is no code that actually performs the 301 redirect from `/.well-known/caldav` to the principal URL. The redirect logic must exist somewhere in the request handling chain, but this module only provides constants. If the `.well-known/caldav` route just returns 200 OK instead of 301, clients will not discover the server.
- **Missing current-user-principal property (Major):** RFC 5397 requires a `current-user-principal` property in the PROPFIND response. Many CalDAV clients (Apple Calendar, DAVx5) use this to locate the user's principal URL. The `build_principal_propfind_xml` function omits it.
- **Missing supported-report-set (Minor):** The principal PROPFIND response should advertise which reports are supported (calendar-query, calendar-multiget). Without this, clients must guess.

Tests verify correct constant values and that the XML contains the expected elements.

#### src/protocol.rs

Implements PROPFIND and REPORT XML response building, ETag/CTag generation, and href helpers.

**Issues found:**

- **ETag based only on `updated_at` timestamp (Major):** The `generate_etag` function at line 118 generates ETags from `updated_at.format("%Y%m%dT%H%M%SZ")`. This has second-level granularity -- two rapid updates within the same second produce identical ETags, violating RFC 7232 Section 2.3 (ETags must change when the representation changes). A hash-based approach or including a version counter would be more robust.
- **CTag is timestamp-based, same concern (Minor):** `generate_ctag` at line 131 has the same second-granularity problem, though CTags are less critical since they only trigger a full sync, not data loss.
- **Hardcoded collection href (Minor):** `build_propfind_xml` hardcodes the collection URL at line 46 rather than deriving it from the `PropfindResponse` struct. This creates a maintenance risk if the URL pattern changes.
- **No 404 response entries for missing resources (Major):** The `build_report_xml` function at line 92 only handles the success case. Per RFC 4791 Section 7.9, a calendar-multiget REPORT must include `<D:response>` entries with `<D:status>HTTP/1.1 404 Not Found</D:status>` for requested resources that do not exist. Without this, clients cannot distinguish "resource was deleted" from "server error."
- **No Depth header handling (Major):** CalDAV PROPFIND behavior differs based on the `Depth` header (0 returns only the collection, 1 includes children, infinity is discouraged). The code always returns both collection and children, effectively implementing Depth:1 only. Depth:0 requests (common during initial discovery) would return incorrect results.
- **No calendar-query REPORT support (Major):** Only calendar-multiget REPORT is implemented via `build_report_xml`. The calendar-query REPORT (RFC 4791 Section 7.8) -- which allows time-range filtering -- is not present. Many clients use calendar-query for initial sync with a time range.
- **`uid_from_href` does not URL-decode (Minor):** If a UID contains URL-encoded characters (e.g., `%40` for `@`), `uid_from_href` returns the encoded form rather than decoding it first.

Tests cover PROPFIND XML structure, REPORT responses, ETag/CTag generation, and href construction.

#### src/serializer.rs

Implements bidirectional conversion between CDM `CalendarEvent` and iCalendar VEVENT format.

**Issues found:**

- **Line folding splits multi-byte UTF-8 characters (Critical):** The `fold_line` function at line 68 folds on byte boundaries (`bytes[pos..end]`). If a UTF-8 character straddles the fold boundary, `String::from_utf8_lossy` will replace bytes with the replacement character, silently corrupting data. The comment on line 83 acknowledges this: "non-ASCII values may need UTF-8 aware splitting in the future." This is a data-loss bug for any event with non-ASCII text (accented names, CJK characters, emoji) in summary, location, or description fields.
- **Missing VALARM serialization (Minor):** The `CalendarEvent` struct has a `reminders` field, but `event_to_ical` does not serialize reminders as VALARM components. CalDAV clients that set reminders via PUT will have them silently dropped on the next GET.
- **Missing STATUS serialization (Minor):** The `CalendarEvent` struct has a `status` field (Confirmed/Tentative/Cancelled), but `event_to_ical` does not serialize it to the iCal STATUS property.
- **Missing all-day event handling (Major):** The `CalendarEvent` struct has an `all_day` field, but `event_to_ical` always emits `DTSTART` and `DTEND` as DATE-TIME values. For all-day events, RFC 5545 requires `VALUE=DATE` format (e.g., `DTSTART;VALUE=DATE:20260321`). Calendar clients will display all-day events as timed events spanning midnight-to-midnight.
- **Missing SEQUENCE property (Minor):** RFC 5545 Section 3.8.7.4 specifies SEQUENCE for tracking event revisions. Without it, some clients may not properly detect updates.
- **ATTENDEE serialization is minimal (Minor):** The serializer emits `ATTENDEE:mailto:email` but RFC 5545 specifies additional parameters like CN (common name), PARTSTAT (participation status), and ROLE. The parser similarly only extracts the email address.
- **No VTIMEZONE component (Minor):** When `event.timezone` is set, the serializer should include a VTIMEZONE component or use TZID parameters on DTSTART/DTEND. Currently, all times are serialized as UTC regardless of the timezone field.
- **`event_to_ical` does not end with CRLF (Minor):** The final `.join("\r\n")` at line 61 produces CRLF between lines but the output does not end with a trailing CRLF. RFC 5545 Section 3.1 requires content lines to end with CRLF.

Tests are comprehensive: serialization, deserialization, round-trip, error cases, and iOS/RFC 5545 compliance checks.

### api-carddav

#### Cargo.toml

Similar to CalDAV -- clean workspace dependencies. Notably, unlike CalDAV, there is no dedicated vCard parsing crate (equivalent to `ical`). The vCard parser is hand-written in `serializer.rs`. This is acceptable for vCard 4.0 since the format is simpler, but means the parser must be more carefully validated.

#### project.json

Standard Nx project configuration, mirrors CalDAV. Build target correctly specifies `wasm32-wasip1`.

#### src/lib.rs

Mirrors the CalDAV plugin structure. Implements `CorePlugin` with the same capabilities.

**Issues found:**

- **Missing PROPFIND route (Critical):** Same as CalDAV -- `HttpMethod::Propfind` not in SDK.
- **Missing REPORT route (Critical):** Same as CalDAV -- `HttpMethod::Report` not in SDK. Addressbook-query and addressbook-multiget REPORT requests are how CardDAV clients sync contacts.
- **Missing OPTIONS route (Major):** CardDAV servers must respond to OPTIONS with `DAV: 1, addressbook` header (RFC 6352 Section 6.1).
- **Well-known endpoint uses GET (Major):** Same issue as CalDAV. RFC 6764 requires redirect handling for `/.well-known/carddav`.

Tests are equivalent to CalDAV and pass the same structural checks.

#### src/discovery.rs

Implements RFC 6764 service discovery for CardDAV.

**Issues found:**

- **No actual redirect mechanism (Major):** Same as CalDAV -- constants are defined but no redirect logic.
- **Missing current-user-principal property (Major):** Same as CalDAV -- RFC 5397 compliance gap.
- **Missing supported-report-set (Minor):** Same as CalDAV.

#### src/protocol.rs

Mirrors CalDAV protocol module with CardDAV-specific XML namespaces and elements.

**Issues found:**

- **ETag based only on `updated_at` timestamp (Major):** Same second-granularity issue as CalDAV.
- **No 404 response entries in REPORT (Major):** Same as CalDAV -- missing resources get no response entry.
- **No Depth header handling (Major):** Same as CalDAV.
- **No addressbook-query REPORT support (Major):** Only addressbook-multiget is implemented. The addressbook-query REPORT (RFC 6352 Section 8.6) -- which supports property-based filtering -- is not present.
- **Hardcoded collection href (Minor):** Same as CalDAV.
- **`uid_from_href` does not URL-decode (Minor):** Same as CalDAV.
- **Missing `supported-address-data` property (Minor):** The PROPFIND response for the addressbook collection should include `<CR:supported-address-data>` to indicate supported vCard versions (RFC 6352 Section 6.2.2).

Tests mirror CalDAV with appropriate CardDAV adaptations.

#### src/serializer.rs

Implements bidirectional conversion between CDM `Contact` and vCard 4.0 format.

**Issues found:**

- **Line folding splits multi-byte UTF-8 characters (Critical):** Same byte-boundary folding issue as CalDAV. Contact names with non-ASCII characters (extremely common -- accented European names, CJK names, Arabic names) will be corrupted. This is arguably more severe for contacts than events since personal names are frequently non-ASCII.
- **Missing TITLE serialization (Minor):** The `Contact` struct has a `title` field but `contact_to_vcard` does not serialize it. The parser (`vcard_to_contact`) also does not parse the TITLE property.
- **Missing BDAY serialization (Minor):** The `Contact` struct has a `birthday` field but it is not serialized to or parsed from the vCard BDAY property.
- **Missing NOTE serialization (Minor):** The `Contact` struct has a `notes` field but it is not serialized/parsed to/from vCard NOTE.
- **Missing PHOTO serialization (Minor):** The `Contact` struct has a `photo_url` field but no PHOTO property is emitted.
- **Missing categories/groups serialization (Minor):** The `Contact` struct has a `groups` field but no CATEGORIES property is emitted.
- **ADR serialization does not escape values (Major):** In `contact_to_vcard` at line 113, the ADR property components are interpolated directly without escaping. If a street address contains a semicolon (e.g., "Suite 100; Floor 3"), it will break the ADR field delimiter structure and corrupt the address on round-trip.
- **ADR type parameter not serialized (Minor):** The `contact_to_vcard` function at line 107 does not emit a TYPE parameter on ADR properties, even though the CDM has an `address_type` field. Similarly, the parser does not extract the TYPE from ADR.
- **`vcard_to_contact` does not parse REV (Minor):** The parser ignores the REV property, always setting `updated_at` to `Utc::now()`. This means re-importing a vCard loses the original modification timestamp.
- **`vcard_to_contact` does not parse CREATED (Minor):** Similarly, `created_at` is always `Utc::now()`.
- **FN fallback parsing is fragile (Minor):** The FN-to-given/family fallback at line 296 splits on the first space, which fails for names with multiple given names or complex structures (e.g., "Mary Jane Watson" would become given="Mary", family="Jane Watson").
- **`contact_to_vcard` does not end with CRLF (Minor):** Same as CalDAV serializer -- the final `.join("\r\n")` does not append a trailing CRLF.
- **Duplicate escape logic (Minor):** The `escape_vcard_value` function at line 351 duplicates `dav_utils::vcard::escape_value`. The `dav-utils` version should be used instead for consistency.

Tests are thorough: serialization, deserialization, round-trip, error cases, special character escaping, and Thunderbird/iOS compatibility checks.

### dav-utils (shared dependency)

The shared utility package is well-designed and provides correct implementations for:

- **XML escaping** (`dav_xml::xml_escape`) -- handles all five XML special characters
- **Namespace validation** (`dav_xml::validate_namespace_declarations`) -- prevents injection of malformed XML
- **Line unfolding** (`text::unfold_lines`) -- correctly handles CRLF, LF, bare CR, and continuation lines
- **Escape decoding** (`text::decode_escaped_value`) -- single-pass implementation that avoids the double-unescape bug
- **ETag change detection** (`etag::filter_changed`) -- generic trait-based approach
- **iCal datetime parsing** (`ical::parse_ical_datetime`) -- handles DATE, DATE-TIME, UTC, TZID parameters

**Issues found:**

- **`open_multistatus` silently drops invalid namespaces (Minor):** At `dav_xml.rs:117`, invalid namespace declarations are silently replaced with an empty string. This could mask bugs. A warning log would be better (only a debug-level log is mentioned in the doc comment, but no actual log statement exists).
- **`open_multistatus` missing space before extra namespaces (Minor):** At `dav_xml.rs:122`, the format string is `"<D:multistatus xmlns:D=\"DAV:\"{ns}>"`. If `extra_namespaces` does not start with a space, the namespace declarations run together with `xmlns:D="DAV:"`. The callers in the test pass `" xmlns:C=..."` with a leading space, but this is fragile -- the function should ensure a space separator.

---

## Problems Summary

### Critical

1. **CalDAV/CardDAV PROPFIND and REPORT routes cannot be registered** -- The plugin SDK's `HttpMethod` enum lacks `Propfind` and `Report` variants. These two methods are the backbone of CalDAV/CardDAV protocol. Without them, no client can discover collections or sync resources. (Both plugins, `lib.rs`)
2. **Line folding corrupts multi-byte UTF-8 characters** -- Both `api-caldav/src/serializer.rs:fold_line` and `api-carddav/src/serializer.rs:fold_line` fold on byte boundaries, which splits multi-byte characters and replaces them via `from_utf8_lossy`. This silently corrupts any non-ASCII text (names, locations, descriptions). (Both plugins, `serializer.rs`)

### Major

3. **Well-known endpoints do not redirect** -- Both `/.well-known/caldav` and `/.well-known/carddav` are registered as GET routes but must return 301 redirects per RFC 6764. Without a redirect, clients using service discovery will not find the server. (Both plugins, `lib.rs` and `discovery.rs`)
4. **Missing OPTIONS handler with DAV headers** -- CalDAV requires `DAV: 1, calendar-access` and CardDAV requires `DAV: 1, addressbook` in OPTIONS responses for clients to detect protocol support. (Both plugins, `lib.rs`)
5. **Missing current-user-principal property** -- Both principal PROPFIND responses omit `current-user-principal`, which Apple Calendar and DAVx5 require for user discovery per RFC 5397. (Both plugins, `discovery.rs`)
6. **ETag generation has second-level granularity** -- ETags derived from `updated_at.format(...)` can collide for rapid updates within the same second, violating RFC 7232. (Both plugins, `protocol.rs`)
7. **No 404 response entries in REPORT for missing resources** -- Calendar-multiget and addressbook-multiget must return 404 status entries for requested resources that do not exist, per RFC 4791 Section 7.9 and RFC 6352 Section 8.7. (Both plugins, `protocol.rs`)
8. **No Depth header support in PROPFIND** -- The PROPFIND implementation always returns collection + children (Depth:1), but Depth:0 requests during discovery return incorrect results. (Both plugins, `protocol.rs`)
9. **No calendar-query / addressbook-query REPORT** -- Only multiget is implemented. Query reports with filtering are required for efficient initial sync and search. (Both plugins, `protocol.rs`)
10. **All-day events not properly serialized** -- CalDAV events with `all_day: Some(true)` are serialized with DATE-TIME format instead of `VALUE=DATE`, causing calendar clients to display them as timed events. (`api-caldav/src/serializer.rs`)
11. **ADR serialization does not escape semicolons in values** -- Address components are interpolated without escaping, allowing semicolons in street addresses to corrupt the field structure. (`api-carddav/src/serializer.rs`)

### Minor

12. **Missing VALARM serialization for reminders** (`api-caldav/src/serializer.rs`)
13. **Missing STATUS serialization** (`api-caldav/src/serializer.rs`)
14. **Missing SEQUENCE property** (`api-caldav/src/serializer.rs`)
15. **ATTENDEE parameters are minimal** -- no CN, PARTSTAT, ROLE (`api-caldav/src/serializer.rs`)
16. **No VTIMEZONE component when timezone is set** (`api-caldav/src/serializer.rs`)
17. **Missing TITLE, BDAY, NOTE, PHOTO, CATEGORIES serialization** (`api-carddav/src/serializer.rs`)
18. **REV and CREATED not parsed from vCard** -- timestamps lost on import (`api-carddav/src/serializer.rs`)
19. **ADR type parameter not serialized/parsed** (`api-carddav/src/serializer.rs`)
20. **FN-to-name fallback is fragile** -- splits on first space only (`api-carddav/src/serializer.rs`)
21. **Duplicate `escape_vcard_value` function** -- should use `dav_utils::vcard::escape_value` (`api-carddav/src/serializer.rs`)
22. **Hardcoded collection hrefs in PROPFIND builders** (Both plugins, `protocol.rs`)
23. **`uid_from_href` does not URL-decode** (Both plugins, `protocol.rs`)
24. **Missing `supported-address-data` property** (`api-carddav/src/protocol.rs`)
25. **Missing `supported-report-set` property** (Both plugins, `discovery.rs`)
26. **iCal/vCard output does not end with trailing CRLF** (Both plugins, `serializer.rs`)
27. **Missing MKCALENDAR support** (`api-caldav` -- acceptable for v0.1)
28. **`open_multistatus` silently drops invalid namespaces without logging** (`dav-utils/src/dav_xml.rs`)
29. **`open_multistatus` fragile space handling before namespace declarations** (`dav-utils/src/dav_xml.rs`)

---

## Recommendations

### Immediate priorities (blocking client interop)

1. **Add `Propfind` and `Report` variants to `HttpMethod` in the plugin SDK.** This unblocks both plugins. Consider also adding `Mkcalendar` and `Mkcol` for future use.
2. **Fix `fold_line` in both serializers to respect UTF-8 character boundaries.** Walk by `char_indices()` instead of byte offsets, ensuring fold points never split a character. This is a data-loss bug.
3. **Implement well-known redirect handlers** that return HTTP 301 with `Location` header pointing to the principal URL.
4. **Add OPTIONS route handlers** that return the required DAV compliance headers.

### Short-term improvements (required for real-world client sync)

5. **Add `current-user-principal` to principal PROPFIND responses.**
6. **Use hash-based ETags** (e.g., hash of `id + updated_at + version`) instead of timestamp-only.
7. **Add 404 response entries** in REPORT builders for missing resources.
8. **Implement Depth header handling** in PROPFIND, at minimum supporting Depth:0 and Depth:1.
9. **Add all-day event support** with `VALUE=DATE` format in CalDAV serializer.
10. **Escape ADR components** in CardDAV serializer before concatenation.

### Medium-term improvements (full protocol compliance)

11. Implement calendar-query and addressbook-query REPORT support with time-range and property filtering.
12. Serialize all CDM fields that have iCal/vCard equivalents (reminders, status, title, birthday, notes, photo, groups).
13. Parse REV/CREATED from vCards to preserve timestamps.
14. Add VTIMEZONE support for timezone-aware events.
15. Consolidate duplicate escape/parse logic with `dav-utils`.
