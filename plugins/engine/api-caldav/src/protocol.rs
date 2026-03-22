//! CalDAV protocol handlers for PROPFIND, REPORT, GET, PUT, DELETE.
//!
//! Implements the subset of RFC 4791 (CalDAV) needed for native calendar
//! clients to discover, read, create, update, and delete calendar events.

use chrono::{DateTime, Utc};
use dav_utils::dav_xml::xml_escape;
use life_engine_types::CalendarEvent;

/// Response for a PROPFIND request on the calendar collection.
///
/// Returns metadata about the calendar: display name, supported
/// components, and a CTag for change detection.
#[derive(Debug, Clone)]
pub struct PropfindResponse {
    /// The calendar display name.
    pub display_name: String,
    /// The calendar CTag (change tag) for sync.
    pub ctag: String,
    /// List of calendar resource entries (href + etag).
    pub resources: Vec<ResourceEntry>,
}

/// A single resource entry returned in multi-status responses.
#[derive(Debug, Clone)]
pub struct ResourceEntry {
    /// The href/path of the resource.
    pub href: String,
    /// The ETag of the resource.
    pub etag: String,
    /// The content type (e.g. "text/calendar").
    pub content_type: String,
}

/// Build a PROPFIND multi-status XML response for the calendar collection.
///
/// This is the response to a WebDAV PROPFIND on the calendar URL,
/// returning the list of resources with their ETags.
pub fn build_propfind_xml(response: &PropfindResponse) -> String {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
    xml.push_str("<D:multistatus xmlns:D=\"DAV:\" xmlns:C=\"urn:ietf:params:xml:ns:caldav\" xmlns:CS=\"http://calendarserver.org/ns/\">\r\n");

    // Calendar collection entry
    xml.push_str("  <D:response>\r\n");
    xml.push_str("    <D:href>/api/plugins/com.life-engine.api-caldav/calendars/default/</D:href>\r\n");
    xml.push_str("    <D:propstat>\r\n");
    xml.push_str("      <D:prop>\r\n");
    xml.push_str(&format!(
        "        <D:displayname>{}</D:displayname>\r\n",
        xml_escape(&response.display_name)
    ));
    xml.push_str("        <D:resourcetype><D:collection/><C:calendar/></D:resourcetype>\r\n");
    xml.push_str("        <C:supported-calendar-component-set><C:comp name=\"VEVENT\"/></C:supported-calendar-component-set>\r\n");
    xml.push_str(&format!(
        "        <CS:getctag>{}</CS:getctag>\r\n",
        xml_escape(&response.ctag)
    ));
    xml.push_str("      </D:prop>\r\n");
    xml.push_str("      <D:status>HTTP/1.1 200 OK</D:status>\r\n");
    xml.push_str("    </D:propstat>\r\n");
    xml.push_str("  </D:response>\r\n");

    // Individual resource entries
    for entry in &response.resources {
        xml.push_str("  <D:response>\r\n");
        xml.push_str(&format!("    <D:href>{}</D:href>\r\n", xml_escape(&entry.href)));
        xml.push_str("    <D:propstat>\r\n");
        xml.push_str("      <D:prop>\r\n");
        xml.push_str(&format!(
            "        <D:getetag>{}</D:getetag>\r\n",
            xml_escape(&entry.etag)
        ));
        xml.push_str(&format!(
            "        <D:getcontenttype>{}</D:getcontenttype>\r\n",
            xml_escape(&entry.content_type)
        ));
        xml.push_str("      </D:prop>\r\n");
        xml.push_str("      <D:status>HTTP/1.1 200 OK</D:status>\r\n");
        xml.push_str("    </D:propstat>\r\n");
        xml.push_str("  </D:response>\r\n");
    }

    xml.push_str("</D:multistatus>");
    xml
}

/// Build a calendar-multiget REPORT XML response.
///
/// Returns the full iCalendar data for each requested resource, wrapped
/// in a DAV multi-status response.
pub fn build_report_xml(events: &[(String, String, String)]) -> String {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
    xml.push_str("<D:multistatus xmlns:D=\"DAV:\" xmlns:C=\"urn:ietf:params:xml:ns:caldav\">\r\n");

    for (href, etag, ical_data) in events {
        xml.push_str("  <D:response>\r\n");
        xml.push_str(&format!("    <D:href>{}</D:href>\r\n", xml_escape(href)));
        xml.push_str("    <D:propstat>\r\n");
        xml.push_str("      <D:prop>\r\n");
        xml.push_str(&format!("        <D:getetag>{}</D:getetag>\r\n", xml_escape(etag)));
        xml.push_str(&format!(
            "        <C:calendar-data>{}</C:calendar-data>\r\n",
            xml_escape(ical_data)
        ));
        xml.push_str("      </D:prop>\r\n");
        xml.push_str("      <D:status>HTTP/1.1 200 OK</D:status>\r\n");
        xml.push_str("    </D:propstat>\r\n");
        xml.push_str("  </D:response>\r\n");
    }

    xml.push_str("</D:multistatus>");
    xml
}

/// Generate an ETag for a calendar event based on its updated_at timestamp.
pub fn generate_etag(event: &CalendarEvent) -> String {
    format!(
        "\"{}\"",
        event
            .updated_at
            .format("%Y%m%dT%H%M%SZ")
    )
}

/// Generate a CTag for the entire calendar collection.
///
/// The CTag changes whenever any event in the collection changes,
/// allowing clients to quickly detect whether a sync is needed.
pub fn generate_ctag(last_modified: DateTime<Utc>) -> String {
    format!(
        "life-engine-{}",
        last_modified.format("%Y%m%dT%H%M%SZ")
    )
}

/// Build the href path for a calendar event resource.
pub fn event_href(source_id: &str) -> String {
    format!(
        "/api/plugins/com.life-engine.api-caldav/calendars/default/{}.ics",
        source_id
    )
}

/// Parse a UID from a resource href path.
///
/// Extracts the filename stem from a path like
/// `/api/plugins/com.life-engine.api-caldav/calendars/default/uid-123.ics`
pub fn uid_from_href(href: &str) -> Option<&str> {
    let filename = href.rsplit('/').next()?;
    filename.strip_suffix(".ics")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample_event() -> CalendarEvent {
        CalendarEvent {
            id: uuid::Uuid::new_v4(),
            title: "Test Event".into(),
            start: Utc.with_ymd_and_hms(2026, 3, 21, 10, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 3, 21, 11, 0, 0).unwrap(),
            recurrence: None,
            attendees: vec![],
            location: Some("Room A".into()),
            description: Some("A test event".into()),
            source: "local".into(),
            source_id: "evt-001".into(),
            extensions: None,
            created_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            updated_at: Utc.with_ymd_and_hms(2026, 3, 15, 12, 0, 0).unwrap(),
        }
    }

    // --- PROPFIND tests ---

    #[test]
    fn propfind_xml_contains_calendar_metadata() {
        let response = PropfindResponse {
            display_name: "My Calendar".into(),
            ctag: "ctag-123".into(),
            resources: vec![],
        };
        let xml = build_propfind_xml(&response);
        assert!(xml.contains("<D:multistatus"));
        assert!(xml.contains("<D:displayname>My Calendar</D:displayname>"));
        assert!(xml.contains("<C:calendar/>"));
        assert!(xml.contains("<CS:getctag>ctag-123</CS:getctag>"));
        assert!(xml.contains("VEVENT"));
    }

    #[test]
    fn propfind_xml_includes_resource_entries() {
        let response = PropfindResponse {
            display_name: "Calendar".into(),
            ctag: "ctag-abc".into(),
            resources: vec![
                ResourceEntry {
                    href: "/api/plugins/com.life-engine.api-caldav/calendars/default/evt-001.ics".into(),
                    etag: "\"etag-1\"".into(),
                    content_type: "text/calendar".into(),
                },
                ResourceEntry {
                    href: "/api/plugins/com.life-engine.api-caldav/calendars/default/evt-002.ics".into(),
                    etag: "\"etag-2\"".into(),
                    content_type: "text/calendar".into(),
                },
            ],
        };
        let xml = build_propfind_xml(&response);
        assert!(xml.contains("evt-001.ics"));
        assert!(xml.contains("evt-002.ics"));
        assert!(xml.contains("<D:getetag>&quot;etag-1&quot;</D:getetag>"));
        assert!(xml.contains("<D:getetag>&quot;etag-2&quot;</D:getetag>"));
        assert!(xml.contains("text/calendar"));
    }

    #[test]
    fn propfind_xml_is_well_formed() {
        let response = PropfindResponse {
            display_name: "Cal".into(),
            ctag: "ct".into(),
            resources: vec![],
        };
        let xml = build_propfind_xml(&response);
        assert!(xml.starts_with("<?xml version=\"1.0\""));
        assert!(xml.contains("</D:multistatus>"));
    }

    // --- REPORT tests ---

    #[test]
    fn report_xml_contains_calendar_data() {
        let events = vec![
            (
                "/cal/evt-001.ics".into(),
                "\"etag-1\"".into(),
                "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:Test\r\nEND:VEVENT\r\nEND:VCALENDAR".into(),
            ),
        ];
        let xml = build_report_xml(&events);
        assert!(xml.contains("<C:calendar-data>"));
        assert!(xml.contains("BEGIN:VCALENDAR"));
        assert!(xml.contains("SUMMARY:Test"));
        assert!(xml.contains("<D:getetag>&quot;etag-1&quot;</D:getetag>"));
    }

    #[test]
    fn report_xml_multiple_events() {
        let events = vec![
            ("/cal/a.ics".into(), "\"ea\"".into(), "ical-a".into()),
            ("/cal/b.ics".into(), "\"eb\"".into(), "ical-b".into()),
        ];
        let xml = build_report_xml(&events);
        assert!(xml.contains("/cal/a.ics"));
        assert!(xml.contains("/cal/b.ics"));
        assert!(xml.contains("ical-a"));
        assert!(xml.contains("ical-b"));
    }

    #[test]
    fn report_xml_empty_collection() {
        let events: Vec<(String, String, String)> = vec![];
        let xml = build_report_xml(&events);
        assert!(xml.contains("<D:multistatus"));
        assert!(xml.contains("</D:multistatus>"));
        // No response entries
        assert!(!xml.contains("<D:response>"));
    }

    // --- ETag/CTag tests ---

    #[test]
    fn generate_etag_from_event() {
        let event = sample_event();
        let etag = generate_etag(&event);
        assert!(etag.starts_with('"'));
        assert!(etag.ends_with('"'));
        assert!(etag.contains("20260315T120000Z"));
    }

    #[test]
    fn generate_ctag_from_timestamp() {
        let ts = Utc.with_ymd_and_hms(2026, 3, 20, 14, 30, 0).unwrap();
        let ctag = generate_ctag(ts);
        assert_eq!(ctag, "life-engine-20260320T143000Z");
    }

    // --- Href/UID helpers ---

    #[test]
    fn event_href_construction() {
        let href = event_href("evt-123");
        assert_eq!(
            href,
            "/api/plugins/com.life-engine.api-caldav/calendars/default/evt-123.ics"
        );
    }

    #[test]
    fn uid_from_href_extracts_uid() {
        let href = "/api/plugins/com.life-engine.api-caldav/calendars/default/evt-123.ics";
        assert_eq!(uid_from_href(href), Some("evt-123"));
    }

    #[test]
    fn uid_from_href_returns_none_for_non_ics() {
        assert_eq!(uid_from_href("/some/path/without/extension"), None);
    }

    #[test]
    fn uid_from_href_handles_complex_uids() {
        let href = "/cal/event-001@example.com.ics";
        assert_eq!(uid_from_href(href), Some("event-001@example.com"));
    }

    // --- GET operation tests ---

    #[test]
    fn get_event_returns_ical_format() {
        let event = sample_event();
        let ical = crate::serializer::event_to_ical(&event);
        assert!(ical.contains("BEGIN:VCALENDAR"));
        assert!(ical.contains("BEGIN:VEVENT"));
        assert!(ical.contains("SUMMARY:Test Event"));
        assert!(ical.contains("END:VEVENT"));
        assert!(ical.contains("END:VCALENDAR"));
    }

    // --- PUT operation tests ---

    #[test]
    fn put_event_parses_ical_to_cdm() {
        let ical = "\
BEGIN:VCALENDAR\r\n\
VERSION:2.0\r\n\
PRODID:-//Test//Test//EN\r\n\
BEGIN:VEVENT\r\n\
UID:new-evt-001\r\n\
SUMMARY:New Event\r\n\
DTSTART:20260401T090000Z\r\n\
DTEND:20260401T100000Z\r\n\
LOCATION:Office\r\n\
END:VEVENT\r\n\
END:VCALENDAR\r\n";
        let event = crate::serializer::ical_to_event(ical).expect("should parse");
        assert_eq!(event.title, "New Event");
        assert_eq!(event.source_id, "new-evt-001");
        assert_eq!(event.location.as_deref(), Some("Office"));
    }

    // --- DELETE operation tests ---

    #[test]
    fn delete_event_identifies_by_uid() {
        let uid = uid_from_href(
            "/api/plugins/com.life-engine.api-caldav/calendars/default/evt-to-delete.ics",
        );
        assert_eq!(uid, Some("evt-to-delete"));
    }

    // --- iOS Calendar compatibility tests ---

    #[test]
    fn ios_calendar_propfind_response_has_required_elements() {
        // iOS Calendar requires: displayname, resourcetype with calendar,
        // supported-calendar-component-set, and getctag
        let response = PropfindResponse {
            display_name: "Life Engine Calendar".into(),
            ctag: "ctag-ios-test".into(),
            resources: vec![ResourceEntry {
                href: "/api/plugins/com.life-engine.api-caldav/calendars/default/evt-001.ics"
                    .into(),
                etag: "\"etag-1\"".into(),
                content_type: "text/calendar".into(),
            }],
        };
        let xml = build_propfind_xml(&response);

        // Required DAV namespace declarations
        assert!(xml.contains("xmlns:D=\"DAV:\""));
        assert!(xml.contains("xmlns:C=\"urn:ietf:params:xml:ns:caldav\""));
        assert!(xml.contains("xmlns:CS=\"http://calendarserver.org/ns/\""));

        // Required properties for iOS Calendar
        assert!(xml.contains("<D:displayname>"));
        assert!(xml.contains("<D:resourcetype>"));
        assert!(xml.contains("<C:calendar/>"));
        assert!(xml.contains("<C:supported-calendar-component-set>"));
        assert!(xml.contains("<C:comp name=\"VEVENT\"/>"));
        assert!(xml.contains("<CS:getctag>"));

        // Individual resources must have getetag and getcontenttype
        assert!(xml.contains("<D:getetag>"));
        assert!(xml.contains("<D:getcontenttype>text/calendar</D:getcontenttype>"));

        // Status lines
        assert!(xml.contains("HTTP/1.1 200 OK"));
    }

    #[test]
    fn ios_calendar_ical_output_is_rfc5545_compliant() {
        // iOS Calendar requires RFC 5545 compliant iCalendar output
        let event = sample_event();
        let ical = crate::serializer::event_to_ical(&event);

        // Required iCal properties for iOS
        assert!(ical.contains("VERSION:2.0"));
        assert!(ical.contains("PRODID:"));
        assert!(ical.contains("UID:"));
        assert!(ical.contains("DTSTAMP:"));
        assert!(ical.contains("DTSTART:"));
        assert!(ical.contains("DTEND:"));
        assert!(ical.contains("SUMMARY:"));

        // Must use CRLF line endings
        assert!(ical.contains("\r\n"));

        // Timestamps must be in UTC format (YYYYMMDDTHHMMSSZ)
        let dtstart_line = ical
            .lines()
            .find(|l| l.starts_with("DTSTART:"))
            .expect("should have DTSTART");
        let dtstart_value = dtstart_line.strip_prefix("DTSTART:").unwrap();
        assert!(
            dtstart_value.ends_with('Z'),
            "DTSTART must be UTC (end with Z)"
        );
        assert_eq!(dtstart_value.len(), 16, "DTSTART format: YYYYMMDDTHHMMSSz");
    }

    #[test]
    fn ios_calendar_report_contains_calendar_data() {
        // iOS uses calendar-multiget REPORT to fetch event data
        let ical = crate::serializer::event_to_ical(&sample_event());
        let events = vec![(
            event_href("evt-001"),
            generate_etag(&sample_event()),
            ical,
        )];
        let xml = build_report_xml(&events);

        assert!(xml.contains("xmlns:C=\"urn:ietf:params:xml:ns:caldav\""));
        assert!(xml.contains("<C:calendar-data>"));
        assert!(xml.contains("BEGIN:VCALENDAR"));
        assert!(xml.contains("BEGIN:VEVENT"));
    }

    // --- Thunderbird compatibility tests ---

    #[test]
    fn thunderbird_propfind_response_compatible() {
        // Thunderbird requires similar elements to iOS but also checks
        // for proper D:collection in resourcetype
        let response = PropfindResponse {
            display_name: "Calendar".into(),
            ctag: "ctag-tb".into(),
            resources: vec![],
        };
        let xml = build_propfind_xml(&response);

        assert!(xml.contains("<D:collection/>"));
        assert!(xml.contains("<C:calendar/>"));
        assert!(xml.contains("<D:displayname>Calendar</D:displayname>"));
    }
}
