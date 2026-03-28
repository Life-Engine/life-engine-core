<!--
project: api-plugins
source: .odm/qa/reports/phase-3/api-plugins.md
updated: 2026-03-28
-->

# API Protocol Plugins — QA Remediation Plan

## Plan Overview

This plan addresses the 29 issues identified in the phase-3 QA review of the CalDAV and CardDAV API protocol plugins (`api-caldav`, `api-carddav`) and their shared dependency (`dav-utils`). Work packages are sequenced by priority: critical SDK and data-loss fixes first, then major protocol compliance gaps, then minor completeness improvements.

**Source:** .odm/qa/reports/phase-3/api-plugins.md

**Progress:** 0 / 8 work packages complete

---

## 1.1 — Add Missing HTTP Method Variants to Plugin SDK
> depends: none
> spec: .odm/qa/reports/phase-3/api-plugins.md

- [x] Add `Propfind` variant to `HttpMethod` enum in the plugin SDK [critical]
  <!-- file: plugin-sdk HttpMethod enum -->
  <!-- purpose: Unblock PROPFIND route registration for both CalDAV and CardDAV plugins -->
  <!-- requirements: 1 -->
  <!-- leverage: existing HttpMethod enum -->
- [x] Add `Report` variant to `HttpMethod` enum in the plugin SDK [critical]
  <!-- file: plugin-sdk HttpMethod enum -->
  <!-- purpose: Unblock REPORT route registration for both CalDAV and CardDAV plugins -->
  <!-- requirements: 1 -->
  <!-- leverage: existing HttpMethod enum -->
- [x] Consider adding `Mkcalendar` and `Mkcol` variants for future use [minor]
  <!-- file: plugin-sdk HttpMethod enum -->
  <!-- purpose: Prepare SDK for future WebDAV collection creation methods -->
  <!-- requirements: 27 -->
  <!-- leverage: existing HttpMethod enum -->
- [x] Register PROPFIND routes in `api-caldav/src/lib.rs` [critical]
  <!-- file: plugins/engine/api-caldav/src/lib.rs -->
  <!-- purpose: Enable CalDAV client discovery and collection listing -->
  <!-- requirements: 1 -->
  <!-- leverage: existing route registration pattern -->
- [x] Register REPORT routes in `api-caldav/src/lib.rs` [critical]
  <!-- file: plugins/engine/api-caldav/src/lib.rs -->
  <!-- purpose: Enable CalDAV client bulk sync via calendar-multiget -->
  <!-- requirements: 1 -->
  <!-- leverage: existing route registration pattern -->
- [x] Register PROPFIND routes in `api-carddav/src/lib.rs` [critical]
  <!-- file: plugins/engine/api-carddav/src/lib.rs -->
  <!-- purpose: Enable CardDAV client discovery and collection listing -->
  <!-- requirements: 1 -->
  <!-- leverage: existing route registration pattern -->
- [x] Register REPORT routes in `api-carddav/src/lib.rs` [critical]
  <!-- file: plugins/engine/api-carddav/src/lib.rs -->
  <!-- purpose: Enable CardDAV client bulk sync via addressbook-multiget -->
  <!-- requirements: 1 -->
  <!-- leverage: existing route registration pattern -->

## 1.2 — Fix UTF-8 Line Folding in Serializers
> depends: none
> spec: .odm/qa/reports/phase-3/api-plugins.md

- [x] Rewrite `fold_line` in `api-caldav/src/serializer.rs` to use `char_indices()` instead of byte offsets [critical]
  <!-- file: plugins/engine/api-caldav/src/serializer.rs -->
  <!-- purpose: Prevent silent data corruption of non-ASCII text (names, locations, descriptions) during iCal serialization -->
  <!-- requirements: 2 -->
  <!-- leverage: existing fold_line function at line 68 -->
- [x] Rewrite `fold_line` in `api-carddav/src/serializer.rs` to use `char_indices()` instead of byte offsets [critical]
  <!-- file: plugins/engine/api-carddav/src/serializer.rs -->
  <!-- purpose: Prevent silent data corruption of non-ASCII contact names during vCard serialization -->
  <!-- requirements: 2 -->
  <!-- leverage: existing fold_line function -->
- [x] Add tests with multi-byte UTF-8 characters (CJK, emoji, accented names) for both serializers [critical]
  <!-- file: plugins/engine/api-caldav/src/serializer.rs, plugins/engine/api-carddav/src/serializer.rs -->
  <!-- purpose: Verify fold_line never splits multi-byte characters -->
  <!-- requirements: 2 -->
  <!-- leverage: existing test suites -->

## 1.3 — Well-Known Endpoints and OPTIONS Handlers
> depends: 1.1
> spec: .odm/qa/reports/phase-3/api-plugins.md

- [ ] Implement 301 redirect from `/.well-known/caldav` to principal URL in `api-caldav` [major]
  <!-- file: plugins/engine/api-caldav/src/lib.rs, plugins/engine/api-caldav/src/discovery.rs -->
  <!-- purpose: Enable RFC 6764 service discovery — clients must receive a redirect to find the server -->
  <!-- requirements: 3 -->
  <!-- leverage: existing CALDAV_PRINCIPAL_URL constant in discovery.rs -->
- [ ] Implement 301 redirect from `/.well-known/carddav` to principal URL in `api-carddav` [major]
  <!-- file: plugins/engine/api-carddav/src/lib.rs, plugins/engine/api-carddav/src/discovery.rs -->
  <!-- purpose: Enable RFC 6764 service discovery for CardDAV clients -->
  <!-- requirements: 3 -->
  <!-- leverage: existing CARDDAV_PRINCIPAL_URL constant in discovery.rs -->
- [ ] Handle both GET and PROPFIND methods on well-known endpoints [major]
  <!-- file: plugins/engine/api-caldav/src/lib.rs, plugins/engine/api-carddav/src/lib.rs -->
  <!-- purpose: RFC 6764 allows any method on well-known URIs; clients commonly use PROPFIND -->
  <!-- requirements: 3 -->
  <!-- leverage: none -->
- [ ] Add OPTIONS route handler to `api-caldav` returning `DAV: 1, calendar-access` header [major]
  <!-- file: plugins/engine/api-caldav/src/lib.rs -->
  <!-- purpose: CalDAV clients use OPTIONS to detect protocol support (RFC 4791 Section 5.1) -->
  <!-- requirements: 4 -->
  <!-- leverage: none -->
- [ ] Add OPTIONS route handler to `api-carddav` returning `DAV: 1, addressbook` header [major]
  <!-- file: plugins/engine/api-carddav/src/lib.rs -->
  <!-- purpose: CardDAV clients use OPTIONS to detect protocol support (RFC 6352 Section 6.1) -->
  <!-- requirements: 4 -->
  <!-- leverage: none -->

## 1.4 — Discovery and PROPFIND Compliance
> depends: 1.1
> spec: .odm/qa/reports/phase-3/api-plugins.md

- [ ] Add `current-user-principal` property to principal PROPFIND response in `api-caldav/src/discovery.rs` [major]
  <!-- file: plugins/engine/api-caldav/src/discovery.rs -->
  <!-- purpose: Apple Calendar and DAVx5 require this per RFC 5397 for user principal discovery -->
  <!-- requirements: 5 -->
  <!-- leverage: existing build_principal_propfind_xml function -->
- [ ] Add `current-user-principal` property to principal PROPFIND response in `api-carddav/src/discovery.rs` [major]
  <!-- file: plugins/engine/api-carddav/src/discovery.rs -->
  <!-- purpose: Same RFC 5397 requirement for CardDAV -->
  <!-- requirements: 5 -->
  <!-- leverage: existing build_principal_propfind_xml function -->
- [ ] Implement Depth header handling in PROPFIND for `api-caldav/src/protocol.rs` (Depth:0 and Depth:1) [major]
  <!-- file: plugins/engine/api-caldav/src/protocol.rs -->
  <!-- purpose: Depth:0 requests during initial discovery currently return incorrect results -->
  <!-- requirements: 8 -->
  <!-- leverage: existing build_propfind_xml function -->
- [ ] Implement Depth header handling in PROPFIND for `api-carddav/src/protocol.rs` (Depth:0 and Depth:1) [major]
  <!-- file: plugins/engine/api-carddav/src/protocol.rs -->
  <!-- purpose: Same Depth header compliance for CardDAV -->
  <!-- requirements: 8 -->
  <!-- leverage: existing build_propfind_xml function -->
- [ ] Add `supported-report-set` property to discovery PROPFIND responses in both plugins [minor]
  <!-- file: plugins/engine/api-caldav/src/discovery.rs, plugins/engine/api-carddav/src/discovery.rs -->
  <!-- purpose: Advertise supported reports so clients do not have to guess -->
  <!-- requirements: 25 -->
  <!-- leverage: none -->
- [ ] Add `supported-address-data` property to CardDAV collection PROPFIND response [minor]
  <!-- file: plugins/engine/api-carddav/src/protocol.rs -->
  <!-- purpose: Indicate supported vCard versions per RFC 6352 Section 6.2.2 -->
  <!-- requirements: 24 -->
  <!-- leverage: none -->

## 1.5 — ETag, REPORT, and Protocol Correctness
> depends: 1.1
> spec: .odm/qa/reports/phase-3/api-plugins.md

- [ ] Replace timestamp-based ETag generation with hash-based approach (e.g., hash of id + updated_at + version) in `api-caldav/src/protocol.rs` [major]
  <!-- file: plugins/engine/api-caldav/src/protocol.rs -->
  <!-- purpose: Prevent ETag collisions for rapid updates within the same second (RFC 7232 violation) -->
  <!-- requirements: 6 -->
  <!-- leverage: existing generate_etag function at line 118 -->
- [ ] Replace timestamp-based ETag generation with hash-based approach in `api-carddav/src/protocol.rs` [major]
  <!-- file: plugins/engine/api-carddav/src/protocol.rs -->
  <!-- purpose: Same ETag collision fix for CardDAV -->
  <!-- requirements: 6 -->
  <!-- leverage: existing generate_etag function -->
- [ ] Add 404 response entries in `build_report_xml` for missing resources in `api-caldav/src/protocol.rs` [major]
  <!-- file: plugins/engine/api-caldav/src/protocol.rs -->
  <!-- purpose: RFC 4791 Section 7.9 requires 404 entries so clients can distinguish deletion from error -->
  <!-- requirements: 7 -->
  <!-- leverage: existing build_report_xml function at line 92 -->
- [ ] Add 404 response entries in `build_report_xml` for missing resources in `api-carddav/src/protocol.rs` [major]
  <!-- file: plugins/engine/api-carddav/src/protocol.rs -->
  <!-- purpose: RFC 6352 Section 8.7 same requirement for CardDAV -->
  <!-- requirements: 7 -->
  <!-- leverage: existing build_report_xml function -->
- [ ] Implement calendar-query REPORT support with time-range filtering in `api-caldav/src/protocol.rs` [major]
  <!-- file: plugins/engine/api-caldav/src/protocol.rs -->
  <!-- purpose: RFC 4791 Section 7.8 — many clients use calendar-query for initial sync with time range -->
  <!-- requirements: 9 -->
  <!-- leverage: none -->
- [ ] Implement addressbook-query REPORT support with property filtering in `api-carddav/src/protocol.rs` [major]
  <!-- file: plugins/engine/api-carddav/src/protocol.rs -->
  <!-- purpose: RFC 6352 Section 8.6 — required for efficient search and initial sync -->
  <!-- requirements: 9 -->
  <!-- leverage: none -->
- [ ] Derive collection href from `PropfindResponse` struct instead of hardcoding in both plugins [minor]
  <!-- file: plugins/engine/api-caldav/src/protocol.rs, plugins/engine/api-carddav/src/protocol.rs -->
  <!-- purpose: Eliminate maintenance risk if URL patterns change -->
  <!-- requirements: 22 -->
  <!-- leverage: existing PropfindResponse struct -->
- [ ] Add URL-decoding to `uid_from_href` in both plugins [minor]
  <!-- file: plugins/engine/api-caldav/src/protocol.rs, plugins/engine/api-carddav/src/protocol.rs -->
  <!-- purpose: Handle UIDs with encoded characters (e.g., %40 for @) -->
  <!-- requirements: 23 -->
  <!-- leverage: existing uid_from_href function -->

## 1.6 — CalDAV Serializer Completeness
> depends: 1.2
> spec: .odm/qa/reports/phase-3/api-plugins.md

- [ ] Implement all-day event serialization with `VALUE=DATE` format in `event_to_ical` [major]
  <!-- file: plugins/engine/api-caldav/src/serializer.rs -->
  <!-- purpose: All-day events currently render as timed events spanning midnight-to-midnight in clients -->
  <!-- requirements: 10 -->
  <!-- leverage: existing event_to_ical function, all_day field on CalendarEvent -->
- [ ] Add VALARM serialization for reminders [minor]
  <!-- file: plugins/engine/api-caldav/src/serializer.rs -->
  <!-- purpose: Prevent silent loss of reminders on CalDAV round-trip -->
  <!-- requirements: 12 -->
  <!-- leverage: existing reminders field on CalendarEvent -->
- [ ] Add STATUS property serialization [minor]
  <!-- file: plugins/engine/api-caldav/src/serializer.rs -->
  <!-- purpose: Serialize Confirmed/Tentative/Cancelled status for clients -->
  <!-- requirements: 13 -->
  <!-- leverage: existing status field on CalendarEvent -->
- [ ] Add SEQUENCE property serialization [minor]
  <!-- file: plugins/engine/api-caldav/src/serializer.rs -->
  <!-- purpose: Track event revisions per RFC 5545 Section 3.8.7.4 -->
  <!-- requirements: 14 -->
  <!-- leverage: none -->
- [ ] Expand ATTENDEE serialization to include CN, PARTSTAT, and ROLE parameters [minor]
  <!-- file: plugins/engine/api-caldav/src/serializer.rs -->
  <!-- purpose: Provide full attendee information to clients per RFC 5545 -->
  <!-- requirements: 15 -->
  <!-- leverage: existing ATTENDEE emission -->
- [ ] Add VTIMEZONE component when `event.timezone` is set [minor]
  <!-- file: plugins/engine/api-caldav/src/serializer.rs -->
  <!-- purpose: Ensure timezone-aware events serialize correctly instead of forcing UTC -->
  <!-- requirements: 16 -->
  <!-- leverage: existing timezone field on CalendarEvent -->
- [ ] Ensure iCal output ends with trailing CRLF [minor]
  <!-- file: plugins/engine/api-caldav/src/serializer.rs -->
  <!-- purpose: RFC 5545 Section 3.1 compliance -->
  <!-- requirements: 26 -->
  <!-- leverage: existing join("\r\n") at line 61 -->

## 1.7 — CardDAV Serializer Completeness
> depends: 1.2
> spec: .odm/qa/reports/phase-3/api-plugins.md

- [ ] Escape semicolons in ADR property component values in `contact_to_vcard` [major]
  <!-- file: plugins/engine/api-carddav/src/serializer.rs -->
  <!-- purpose: Prevent address corruption when street addresses contain semicolons -->
  <!-- requirements: 11 -->
  <!-- leverage: existing contact_to_vcard at line 113, dav_utils::vcard::escape_value -->
- [ ] Add TITLE property serialization and parsing [minor]
  <!-- file: plugins/engine/api-carddav/src/serializer.rs -->
  <!-- purpose: Preserve contact title field on round-trip -->
  <!-- requirements: 17 -->
  <!-- leverage: existing title field on Contact -->
- [ ] Add BDAY property serialization and parsing [minor]
  <!-- file: plugins/engine/api-carddav/src/serializer.rs -->
  <!-- purpose: Preserve contact birthday on round-trip -->
  <!-- requirements: 17 -->
  <!-- leverage: existing birthday field on Contact -->
- [ ] Add NOTE property serialization and parsing [minor]
  <!-- file: plugins/engine/api-carddav/src/serializer.rs -->
  <!-- purpose: Preserve contact notes on round-trip -->
  <!-- requirements: 17 -->
  <!-- leverage: existing notes field on Contact -->
- [ ] Add PHOTO property serialization [minor]
  <!-- file: plugins/engine/api-carddav/src/serializer.rs -->
  <!-- purpose: Preserve contact photo URL on round-trip -->
  <!-- requirements: 17 -->
  <!-- leverage: existing photo_url field on Contact -->
- [ ] Add CATEGORIES property serialization for groups [minor]
  <!-- file: plugins/engine/api-carddav/src/serializer.rs -->
  <!-- purpose: Preserve contact group membership on round-trip -->
  <!-- requirements: 17 -->
  <!-- leverage: existing groups field on Contact -->
- [ ] Parse REV property in `vcard_to_contact` to preserve modification timestamp [minor]
  <!-- file: plugins/engine/api-carddav/src/serializer.rs -->
  <!-- purpose: Prevent loss of original modification timestamp on vCard import -->
  <!-- requirements: 18 -->
  <!-- leverage: none -->
- [ ] Parse CREATED property in `vcard_to_contact` to preserve creation timestamp [minor]
  <!-- file: plugins/engine/api-carddav/src/serializer.rs -->
  <!-- purpose: Prevent loss of creation timestamp on vCard import -->
  <!-- requirements: 18 -->
  <!-- leverage: none -->
- [ ] Add ADR TYPE parameter serialization and parsing [minor]
  <!-- file: plugins/engine/api-carddav/src/serializer.rs -->
  <!-- purpose: Preserve address type (home, work) on round-trip -->
  <!-- requirements: 19 -->
  <!-- leverage: existing address_type field -->
- [ ] Improve FN-to-name fallback parsing for multi-word names [minor]
  <!-- file: plugins/engine/api-carddav/src/serializer.rs -->
  <!-- purpose: Handle names with multiple given names correctly (e.g., "Mary Jane Watson") -->
  <!-- requirements: 20 -->
  <!-- leverage: existing fallback at line 296 -->
- [ ] Replace duplicate `escape_vcard_value` with `dav_utils::vcard::escape_value` [minor]
  <!-- file: plugins/engine/api-carddav/src/serializer.rs -->
  <!-- purpose: Consolidate duplicate logic and ensure consistent escaping -->
  <!-- requirements: 21 -->
  <!-- leverage: dav_utils::vcard::escape_value -->
- [ ] Ensure vCard output ends with trailing CRLF [minor]
  <!-- file: plugins/engine/api-carddav/src/serializer.rs -->
  <!-- purpose: RFC 6350 compliance -->
  <!-- requirements: 26 -->
  <!-- leverage: existing join("\r\n") -->

## 1.8 — dav-utils Hardening
> depends: none
> spec: .odm/qa/reports/phase-3/api-plugins.md

- [ ] Add warning log when `open_multistatus` drops invalid namespace declarations [minor]
  <!-- file: packages/dav-utils/src/dav_xml.rs -->
  <!-- purpose: Surface namespace validation failures instead of silently swallowing them -->
  <!-- requirements: 28 -->
  <!-- leverage: existing validation at dav_xml.rs:117 -->
- [ ] Fix space handling before extra namespace declarations in `open_multistatus` [minor]
  <!-- file: packages/dav-utils/src/dav_xml.rs -->
  <!-- purpose: Prevent malformed XML when callers omit leading space in namespace strings -->
  <!-- requirements: 29 -->
  <!-- leverage: existing format string at dav_xml.rs:122 -->
