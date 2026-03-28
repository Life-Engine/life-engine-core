//! iCalendar serialisation: CDM `CalendarEvent` to/from iCalendar VEVENT format.
//!
//! Used by the CalDAV server to serve events in the standard iCalendar
//! format and to parse incoming PUT requests from calendar clients.

use chrono::Utc;
use dav_utils::ical::{is_date_only, parse_ical_datetime};
use life_engine_types::CalendarEvent;
use life_engine_types::events::{Attendee, Recurrence};
use uuid::Uuid;

/// Serialise a CDM `CalendarEvent` to iCalendar VCALENDAR/VEVENT format.
///
/// Produces a complete iCalendar document with a single VEVENT,
/// suitable for serving via CalDAV GET responses.
pub fn event_to_ical(event: &CalendarEvent) -> String {
    let uid = &event.source_id;
    let summary = &event.title;
    let dtstart = event.start.format("%Y%m%dT%H%M%SZ").to_string();
    let dtend = event.end.unwrap_or(event.start).format("%Y%m%dT%H%M%SZ").to_string();
    let dtstamp = event.updated_at.format("%Y%m%dT%H%M%SZ").to_string();
    let created = event.created_at.format("%Y%m%dT%H%M%SZ").to_string();
    let last_modified = event.updated_at.format("%Y%m%dT%H%M%SZ").to_string();

    let mut lines = vec![
        "BEGIN:VCALENDAR".to_string(),
        "VERSION:2.0".to_string(),
        "PRODID:-//Life Engine//CalDAV Server//EN".to_string(),
        "BEGIN:VEVENT".to_string(),
        format!("UID:{uid}"),
        format!("DTSTAMP:{dtstamp}"),
        format!("DTSTART:{dtstart}"),
        format!("DTEND:{dtend}"),
        format!("SUMMARY:{summary}"),
        format!("CREATED:{created}"),
        format!("LAST-MODIFIED:{last_modified}"),
    ];

    if let Some(ref loc) = event.location {
        lines.push(format!("LOCATION:{loc}"));
    }
    if let Some(ref desc) = event.description {
        lines.push(format!("DESCRIPTION:{desc}"));
    }
    if let Some(ref recurrence) = event.recurrence {
        lines.push(format!("RRULE:{}", recurrence.to_rrule()));
    }
    for attendee in &event.attendees {
        lines.push(format!("ATTENDEE:mailto:{}", attendee.email));
    }

    lines.push("END:VEVENT".to_string());
    lines.push("END:VCALENDAR".to_string());

    // RFC 5545 §3.1: Lines longer than 75 octets SHOULD be folded with
    // CRLF followed by a single space (linear white space).
    lines
        .iter()
        .map(|line| fold_line(line))
        .collect::<Vec<_>>()
        .join("\r\n")
}

/// Fold a content line per RFC 5545 §3.1.
///
/// Lines longer than 75 octets are split: the first chunk is 75 octets,
/// continuation chunks are 74 octets (because the leading space counts).
fn fold_line(line: &str) -> String {
    let bytes = line.as_bytes();
    if bytes.len() <= 75 {
        return line.to_string();
    }

    let mut result = String::with_capacity(bytes.len() + bytes.len() / 74 * 3);
    let mut pos = 0;
    let mut first = true;

    while pos < bytes.len() {
        let chunk_len = if first { 75 } else { 74 };
        first = false;

        let end = std::cmp::min(pos + chunk_len, bytes.len());
        // Safety: iCalendar property values should be ASCII-safe for
        // standard properties; non-ASCII values may need UTF-8 aware
        // splitting in the future.
        if !result.is_empty() {
            result.push_str("\r\n ");
        }
        result.push_str(&String::from_utf8_lossy(&bytes[pos..end]));
        pos = end;
    }

    result
}

/// Parse an iCalendar VCALENDAR string into a CDM `CalendarEvent`.
///
/// Used to process PUT requests from CalDAV clients creating or
/// updating events.
pub fn ical_to_event(ical_data: &str) -> anyhow::Result<CalendarEvent> {
    use ical::parser::ical::IcalParser;
    use std::io::BufReader;

    let reader = BufReader::new(ical_data.as_bytes());
    let parser = IcalParser::new(reader);

    for calendar_result in parser {
        let calendar = calendar_result
            .map_err(|e| anyhow::anyhow!("failed to parse iCalendar data: {}", e))?;

        if let Some(vevent) = calendar.events.first() {
            return parse_vevent(vevent);
        }
    }

    Err(anyhow::anyhow!("no VEVENT found in iCalendar data"))
}

/// Parse a single VEVENT into a CDM `CalendarEvent`.
fn parse_vevent(event: &ical::parser::ical::component::IcalEvent) -> anyhow::Result<CalendarEvent> {
    let title = get_property(event, "SUMMARY")
        .unwrap_or_else(|| "(no title)".to_string());

    let dtstart_raw = get_property(event, "DTSTART")
        .ok_or_else(|| anyhow::anyhow!("VEVENT missing DTSTART"))?;

    let dtstart_params = get_property_params(event, "DTSTART");
    let dtend_params = get_property_params(event, "DTEND");

    let start = parse_ical_datetime(&dtstart_raw, &dtstart_params)?;

    let end = match get_property(event, "DTEND") {
        Some(dtend_raw) => parse_ical_datetime(&dtend_raw, &dtend_params)?,
        None => {
            if is_date_only(&dtstart_raw) {
                start + chrono::Duration::days(1)
            } else {
                start + chrono::Duration::hours(1)
            }
        }
    };

    let recurrence = get_property(event, "RRULE").and_then(|r| Recurrence::from_rrule(&r));
    let location = get_property(event, "LOCATION");
    let description = get_property(event, "DESCRIPTION");
    let attendees = extract_attendees(event);

    let source_id = get_property(event, "UID")
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let created_at = get_property(event, "CREATED")
        .and_then(|v| parse_ical_datetime(&v, &None).ok())
        .unwrap_or_else(Utc::now);

    let updated_at = get_property(event, "LAST-MODIFIED")
        .and_then(|v| parse_ical_datetime(&v, &None).ok())
        .unwrap_or_else(Utc::now);

    Ok(CalendarEvent {
        id: Uuid::new_v4(),
        title,
        start,
        end: Some(end),
        recurrence,
        attendees,
        location,
        description,
        all_day: None,
        reminders: vec![],
        timezone: None,
        status: None,
        source: "caldav-api".into(),
        source_id,
        extensions: None,
        created_at,
        updated_at,
    })
}

fn get_property(
    event: &ical::parser::ical::component::IcalEvent,
    name: &str,
) -> Option<String> {
    event
        .properties
        .iter()
        .find(|p| p.name == name)
        .and_then(|p| p.value.clone())
}

fn get_property_params(
    event: &ical::parser::ical::component::IcalEvent,
    name: &str,
) -> Option<Vec<(String, Vec<String>)>> {
    event
        .properties
        .iter()
        .find(|p| p.name == name)
        .and_then(|p| p.params.clone())
}

fn extract_attendees(event: &ical::parser::ical::component::IcalEvent) -> Vec<Attendee> {
    event
        .properties
        .iter()
        .filter(|p| p.name == "ATTENDEE")
        .filter_map(|p| {
            p.value.as_ref().map(|v| {
                let email = v.trim()
                    .strip_prefix("mailto:")
                    .or_else(|| v.trim().strip_prefix("MAILTO:"))
                    .unwrap_or(v.trim())
                    .to_string();
                Attendee::from_email(email)
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample_event() -> CalendarEvent {
        CalendarEvent {
            id: Uuid::new_v4(),
            title: "Team Meeting".into(),
            start: Utc.with_ymd_and_hms(2026, 3, 21, 10, 0, 0).unwrap(),
            end: Some(Utc.with_ymd_and_hms(2026, 3, 21, 11, 0, 0).unwrap()),
            recurrence: Recurrence::from_rrule("FREQ=WEEKLY;BYDAY=MO"),
            attendees: vec![Attendee::from_email("alice@example.com"), Attendee::from_email("bob@example.com")],
            location: Some("Board Room".into()),
            description: Some("Weekly team sync".into()),
            all_day: None,
            reminders: vec![],
            timezone: None,
            status: None,
            source: "local".into(),
            source_id: "evt-round-trip@example.com".into(),
            extensions: None,
            created_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            updated_at: Utc.with_ymd_and_hms(2026, 3, 15, 12, 0, 0).unwrap(),
        }
    }

    // --- Serialisation (CDM -> iCal) ---

    #[test]
    fn event_to_ical_contains_vcalendar_wrapper() {
        let ical = event_to_ical(&sample_event());
        assert!(ical.starts_with("BEGIN:VCALENDAR"));
        assert!(ical.contains("END:VCALENDAR"));
        assert!(ical.contains("VERSION:2.0"));
        assert!(ical.contains("PRODID:"));
    }

    #[test]
    fn event_to_ical_contains_vevent_properties() {
        let ical = event_to_ical(&sample_event());
        assert!(ical.contains("BEGIN:VEVENT"));
        assert!(ical.contains("END:VEVENT"));
        assert!(ical.contains("UID:evt-round-trip@example.com"));
        assert!(ical.contains("SUMMARY:Team Meeting"));
        assert!(ical.contains("DTSTART:20260321T100000Z"));
        assert!(ical.contains("DTEND:20260321T110000Z"));
        assert!(ical.contains("DTSTAMP:"));
        assert!(ical.contains("CREATED:"));
        assert!(ical.contains("LAST-MODIFIED:"));
    }

    #[test]
    fn event_to_ical_includes_optional_properties() {
        let ical = event_to_ical(&sample_event());
        assert!(ical.contains("LOCATION:Board Room"));
        assert!(ical.contains("DESCRIPTION:Weekly team sync"));
        assert!(ical.contains("RRULE:FREQ=WEEKLY;BYDAY=MO"));
        assert!(ical.contains("ATTENDEE:mailto:alice@example.com"));
        assert!(ical.contains("ATTENDEE:mailto:bob@example.com"));
    }

    #[test]
    fn event_to_ical_omits_empty_optionals() {
        let event = CalendarEvent {
            id: Uuid::new_v4(),
            title: "Simple".into(),
            start: Utc.with_ymd_and_hms(2026, 4, 1, 9, 0, 0).unwrap(),
            end: Some(Utc.with_ymd_and_hms(2026, 4, 1, 10, 0, 0).unwrap()),
            recurrence: None,
            attendees: vec![],
            location: None,
            description: None,
            all_day: None,
            reminders: vec![],
            timezone: None,
            status: None,
            source: "local".into(),
            source_id: "simple-001".into(),
            extensions: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let ical = event_to_ical(&event);
        assert!(!ical.contains("LOCATION:"));
        assert!(!ical.contains("DESCRIPTION:"));
        assert!(!ical.contains("RRULE:"));
        assert!(!ical.contains("ATTENDEE:"));
    }

    #[test]
    fn event_to_ical_uses_crlf_line_endings() {
        let ical = event_to_ical(&sample_event());
        assert!(ical.contains("\r\n"));
    }

    // --- Deserialisation (iCal -> CDM) ---

    #[test]
    fn ical_to_event_parses_simple_vevent() {
        let ical = "\
BEGIN:VCALENDAR\r\n\
VERSION:2.0\r\n\
PRODID:-//Test//EN\r\n\
BEGIN:VEVENT\r\n\
UID:parse-001\r\n\
SUMMARY:Parsed Event\r\n\
DTSTART:20260401T090000Z\r\n\
DTEND:20260401T100000Z\r\n\
END:VEVENT\r\n\
END:VCALENDAR\r\n";

        let event = ical_to_event(ical).expect("should parse");
        assert_eq!(event.title, "Parsed Event");
        assert_eq!(event.source_id, "parse-001");
        assert_eq!(event.source, "caldav-api");
    }

    #[test]
    fn ical_to_event_parses_full_vevent() {
        let ical = "\
BEGIN:VCALENDAR\r\n\
VERSION:2.0\r\n\
PRODID:-//Test//EN\r\n\
BEGIN:VEVENT\r\n\
UID:full-001\r\n\
SUMMARY:Full Event\r\n\
DTSTART:20260401T090000Z\r\n\
DTEND:20260401T100000Z\r\n\
LOCATION:Office\r\n\
DESCRIPTION:A full event\r\n\
RRULE:FREQ=DAILY;COUNT=5\r\n\
ATTENDEE:mailto:alice@example.com\r\n\
ATTENDEE:mailto:bob@example.com\r\n\
CREATED:20260101T000000Z\r\n\
LAST-MODIFIED:20260315T120000Z\r\n\
END:VEVENT\r\n\
END:VCALENDAR\r\n";

        let event = ical_to_event(ical).expect("should parse");
        assert_eq!(event.title, "Full Event");
        assert_eq!(event.location.as_deref(), Some("Office"));
        assert_eq!(event.description.as_deref(), Some("A full event"));
        assert_eq!(event.recurrence.as_ref().map(|r| r.to_rrule()).as_deref(), Some("FREQ=DAILY;COUNT=5"));
        assert_eq!(event.attendees.len(), 2);
        assert_eq!(event.attendees[0].email, "alice@example.com");
    }

    #[test]
    fn ical_to_event_errors_on_missing_dtstart() {
        let ical = "\
BEGIN:VCALENDAR\r\n\
VERSION:2.0\r\n\
BEGIN:VEVENT\r\n\
UID:bad-001\r\n\
SUMMARY:No Start\r\n\
END:VEVENT\r\n\
END:VCALENDAR\r\n";

        let result = ical_to_event(ical);
        assert!(result.is_err());
    }

    #[test]
    fn ical_to_event_errors_on_no_vevent() {
        let ical = "\
BEGIN:VCALENDAR\r\n\
VERSION:2.0\r\n\
END:VCALENDAR\r\n";

        let result = ical_to_event(ical);
        assert!(result.is_err());
    }

    // --- Round-trip ---

    #[test]
    fn round_trip_serialisation() {
        let original = sample_event();
        let ical = event_to_ical(&original);
        let restored = ical_to_event(&ical).expect("should round-trip");

        assert_eq!(restored.title, original.title);
        assert_eq!(restored.source_id, original.source_id);
        assert_eq!(restored.start, original.start);
        assert_eq!(restored.end, original.end);
        assert_eq!(restored.location, original.location);
        assert_eq!(restored.description, original.description);
        assert_eq!(restored.recurrence, original.recurrence);
        assert_eq!(restored.attendees, original.attendees);
        assert_eq!(restored.created_at, original.created_at);
        assert_eq!(restored.updated_at, original.updated_at);
    }

    #[test]
    fn round_trip_minimal_event() {
        let event = CalendarEvent {
            id: Uuid::new_v4(),
            title: "Minimal".into(),
            start: Utc.with_ymd_and_hms(2026, 6, 1, 14, 0, 0).unwrap(),
            end: Some(Utc.with_ymd_and_hms(2026, 6, 1, 15, 0, 0).unwrap()),
            recurrence: None,
            attendees: vec![],
            location: None,
            description: None,
            all_day: None,
            reminders: vec![],
            timezone: None,
            status: None,
            source: "local".into(),
            source_id: "min-001".into(),
            extensions: None,
            created_at: Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap(),
            updated_at: Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap(),
        };
        let ical = event_to_ical(&event);
        let restored = ical_to_event(&ical).expect("should round-trip");

        assert_eq!(restored.title, "Minimal");
        assert_eq!(restored.source_id, "min-001");
        assert!(restored.location.is_none());
        assert!(restored.description.is_none());
        assert!(restored.recurrence.is_none());
        assert!(restored.attendees.is_empty());
    }
}
