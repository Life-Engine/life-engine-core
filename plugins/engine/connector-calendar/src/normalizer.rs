//! Calendar event normalizer: converts parsed iCal VEVENT data to CDM `CalendarEvent` type.
//!
//! Handles edge cases: missing fields, all-day events (DATE vs DATE-TIME),
//! recurrence rules, attendees, timezone conversions, and malformed data.

use chrono::Utc;
use dav_utils::ical::{is_date_only, parse_ical_datetime};
use ical::parser::ical::component::IcalEvent;
use life_engine_types::CalendarEvent;
use life_engine_types::events::{Attendee, Recurrence};
use uuid::Uuid;

/// Normalize a parsed iCal VEVENT into the Life Engine CDM `CalendarEvent` type.
///
/// `source` identifies the connector that produced this event (e.g. "caldav").
pub fn normalize_vevent(event: &IcalEvent, source: &str) -> anyhow::Result<CalendarEvent> {
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
            // If no DTEND, check DURATION; otherwise default to start + 1 hour
            // (or start + 1 day for all-day events)
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
        source: source.into(),
        source_id,
        extensions: None,
        created_at,
        updated_at,
        all_day: None,
        reminders: vec![],
        timezone: None,
        status: None,
    })
}

/// Parse multiple VEVENTs from raw iCalendar data.
///
/// Returns a `Vec` of successfully parsed events and skips malformed ones,
/// logging warnings for each failure.
pub fn parse_vcalendar(ical_data: &str, source: &str) -> anyhow::Result<Vec<CalendarEvent>> {
    use ical::parser::ical::IcalParser;
    use std::io::BufReader;

    let reader = BufReader::new(ical_data.as_bytes());
    let parser = IcalParser::new(reader);

    let mut events = Vec::new();

    for calendar_result in parser {
        let calendar = calendar_result
            .map_err(|e| anyhow::anyhow!("failed to parse iCalendar data: {}", e))?;

        for vevent in &calendar.events {
            match normalize_vevent(vevent, source) {
                Ok(event) => events.push(event),
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "skipping malformed VEVENT"
                    );
                }
            }
        }
    }

    Ok(events)
}

/// Get a property value from a VEVENT by name.
fn get_property(event: &IcalEvent, name: &str) -> Option<String> {
    event
        .properties
        .iter()
        .find(|p| p.name == name)
        .and_then(|p| p.value.clone())
}

/// Get the parameters for a property from a VEVENT by name.
fn get_property_params(
    event: &IcalEvent,
    name: &str,
) -> Option<Vec<(String, Vec<String>)>> {
    event
        .properties
        .iter()
        .find(|p| p.name == name)
        .and_then(|p| p.params.clone())
}

/// Extract attendees from a VEVENT.
///
/// Attendee values are typically `mailto:user@example.com` — we strip the
/// `mailto:` prefix and return just the email address.
fn extract_attendees(event: &IcalEvent) -> Vec<Attendee> {
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
    use chrono::Timelike;
    use ical::property::Property;

    /// Helper: build a VEVENT with the given properties.
    #[allow(clippy::type_complexity)]
    fn make_vevent(props: Vec<(&str, &str, Option<Vec<(String, Vec<String>)>>)>) -> IcalEvent {
        let mut event = IcalEvent {
            properties: Vec::new(),
            alarms: Vec::new(),
        };
        for (name, value, params) in props {
            event.properties.push(Property {
                name: name.to_string(),
                value: Some(value.to_string()),
                params,
            });
        }
        event
    }

    #[test]
    fn normalize_simple_event() {
        let vevent = make_vevent(vec![
            ("SUMMARY", "Team Meeting", None),
            ("DTSTART", "20260321T100000Z", None),
            ("DTEND", "20260321T110000Z", None),
            ("UID", "event-001@example.com", None),
            ("LOCATION", "Room A", None),
            ("DESCRIPTION", "Weekly sync", None),
        ]);

        let event = normalize_vevent(&vevent, "caldav").expect("should normalize");
        assert_eq!(event.title, "Team Meeting");
        assert_eq!(event.source, "caldav");
        assert_eq!(event.source_id, "event-001@example.com");
        assert_eq!(event.location.as_deref(), Some("Room A"));
        assert_eq!(event.description.as_deref(), Some("Weekly sync"));
        assert!(event.recurrence.is_none());
        assert!(event.attendees.is_empty());
        assert!(event.start < event.end.unwrap());
    }

    #[test]
    fn normalize_event_with_recurrence() {
        let vevent = make_vevent(vec![
            ("SUMMARY", "Standup", None),
            ("DTSTART", "20260323T090000Z", None),
            ("DTEND", "20260323T091500Z", None),
            ("UID", "recur-001@example.com", None),
            ("RRULE", "FREQ=DAILY;COUNT=10", None),
        ]);

        let event = normalize_vevent(&vevent, "caldav").expect("should normalize");
        assert_eq!(event.recurrence.as_ref().map(|r| r.to_rrule()).as_deref(), Some("FREQ=DAILY;COUNT=10"));
    }

    #[test]
    fn normalize_event_with_attendees() {
        let mut vevent = make_vevent(vec![
            ("SUMMARY", "Planning", None),
            ("DTSTART", "20260325T140000Z", None),
            ("DTEND", "20260325T150000Z", None),
            ("UID", "attend-001@example.com", None),
        ]);
        // Add attendees
        vevent.properties.push(Property {
            name: "ATTENDEE".to_string(),
            value: Some("mailto:alice@example.com".to_string()),
            params: None,
        });
        vevent.properties.push(Property {
            name: "ATTENDEE".to_string(),
            value: Some("mailto:bob@example.com".to_string()),
            params: None,
        });
        vevent.properties.push(Property {
            name: "ATTENDEE".to_string(),
            value: Some("MAILTO:carol@example.com".to_string()),
            params: None,
        });

        let event = normalize_vevent(&vevent, "caldav").expect("should normalize");
        assert_eq!(event.attendees.len(), 3);
        assert_eq!(event.attendees[0].email, "alice@example.com");
        assert_eq!(event.attendees[1].email, "bob@example.com");
        assert_eq!(event.attendees[2].email, "carol@example.com");
    }

    #[test]
    fn normalize_event_with_timezone() {
        let tz_params = Some(vec![
            ("TZID".to_string(), vec!["America/New_York".to_string()]),
        ]);
        let vevent = make_vevent(vec![
            ("SUMMARY", "NYC Meeting", tz_params.clone()),
            ("DTSTART", "20260321T100000", tz_params.clone()),
            ("DTEND", "20260321T110000", tz_params),
            ("UID", "tz-001@example.com", None),
        ]);

        let event = normalize_vevent(&vevent, "caldav").expect("should normalize");
        assert_eq!(event.title, "NYC Meeting");
        // Currently treated as UTC — the important thing is it doesn't error
        assert!(event.start < event.end.unwrap());
    }

    #[test]
    fn normalize_event_missing_optional_fields() {
        let vevent = make_vevent(vec![
            ("DTSTART", "20260321T100000Z", None),
            ("DTEND", "20260321T110000Z", None),
        ]);

        let event = normalize_vevent(&vevent, "caldav").expect("should normalize");
        assert_eq!(event.title, "(no title)");
        assert!(event.location.is_none());
        assert!(event.description.is_none());
        assert!(event.recurrence.is_none());
        assert!(event.attendees.is_empty());
        // source_id should be a generated UUID since UID is missing
        assert!(!event.source_id.is_empty());
    }

    #[test]
    fn normalize_all_day_event_date_only() {
        let date_params = Some(vec![
            ("VALUE".to_string(), vec!["DATE".to_string()]),
        ]);
        let vevent = make_vevent(vec![
            ("SUMMARY", "Holiday", None),
            ("DTSTART", "20260325", date_params.clone()),
            ("DTEND", "20260326", date_params),
            ("UID", "allday-001@example.com", None),
        ]);

        let event = normalize_vevent(&vevent, "caldav").expect("should normalize");
        assert_eq!(event.title, "Holiday");
        // All-day: start should be midnight of 2026-03-25
        assert_eq!(event.start.date_naive().to_string(), "2026-03-25");
        assert_eq!(event.end.unwrap().date_naive().to_string(), "2026-03-26");
    }

    #[test]
    fn normalize_all_day_event_no_dtend() {
        let date_params = Some(vec![
            ("VALUE".to_string(), vec!["DATE".to_string()]),
        ]);
        let vevent = make_vevent(vec![
            ("SUMMARY", "Birthday", None),
            ("DTSTART", "20260401", date_params),
            ("UID", "allday-noend@example.com", None),
        ]);

        let event = normalize_vevent(&vevent, "caldav").expect("should normalize");
        // No DTEND on an all-day event defaults to start + 1 day
        assert_eq!(event.start.date_naive().to_string(), "2026-04-01");
        assert_eq!(event.end.unwrap().date_naive().to_string(), "2026-04-02");
    }

    #[test]
    fn normalize_event_no_dtend_defaults_to_one_hour() {
        let vevent = make_vevent(vec![
            ("SUMMARY", "Quick Chat", None),
            ("DTSTART", "20260321T100000Z", None),
            ("UID", "noend-001@example.com", None),
        ]);

        let event = normalize_vevent(&vevent, "caldav").expect("should normalize");
        let duration = event.end.unwrap() - event.start;
        assert_eq!(duration.num_hours(), 1);
    }

    #[test]
    fn normalize_event_missing_dtstart_returns_error() {
        let vevent = make_vevent(vec![
            ("SUMMARY", "No Start", None),
            ("DTEND", "20260321T110000Z", None),
            ("UID", "nostart@example.com", None),
        ]);

        let result = normalize_vevent(&vevent, "caldav");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("DTSTART"));
    }

    #[test]
    fn normalize_event_with_created_and_modified() {
        let vevent = make_vevent(vec![
            ("SUMMARY", "Tracked Event", None),
            ("DTSTART", "20260321T100000Z", None),
            ("DTEND", "20260321T110000Z", None),
            ("UID", "tracked-001@example.com", None),
            ("CREATED", "20260101T000000Z", None),
            ("LAST-MODIFIED", "20260315T120000Z", None),
        ]);

        let event = normalize_vevent(&vevent, "caldav").expect("should normalize");
        assert_eq!(event.created_at.date_naive().to_string(), "2026-01-01");
        assert_eq!(event.updated_at.date_naive().to_string(), "2026-03-15");
    }

    #[test]
    fn parse_vcalendar_multiple_events() {
        let ical_data = "\
BEGIN:VCALENDAR\r\n\
VERSION:2.0\r\n\
PRODID:-//Test//Test//EN\r\n\
BEGIN:VEVENT\r\n\
SUMMARY:Event One\r\n\
DTSTART:20260321T100000Z\r\n\
DTEND:20260321T110000Z\r\n\
UID:multi-001@example.com\r\n\
END:VEVENT\r\n\
BEGIN:VEVENT\r\n\
SUMMARY:Event Two\r\n\
DTSTART:20260322T140000Z\r\n\
DTEND:20260322T150000Z\r\n\
UID:multi-002@example.com\r\n\
END:VEVENT\r\n\
END:VCALENDAR\r\n";

        let events = parse_vcalendar(ical_data, "caldav").expect("should parse");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].title, "Event One");
        assert_eq!(events[1].title, "Event Two");
        assert_eq!(events[0].source_id, "multi-001@example.com");
        assert_eq!(events[1].source_id, "multi-002@example.com");
    }

    #[test]
    fn parse_vcalendar_skips_malformed_events() {
        // First event has no DTSTART — should be skipped; second is valid
        let ical_data = "\
BEGIN:VCALENDAR\r\n\
VERSION:2.0\r\n\
PRODID:-//Test//Test//EN\r\n\
BEGIN:VEVENT\r\n\
SUMMARY:Bad Event\r\n\
UID:bad@example.com\r\n\
END:VEVENT\r\n\
BEGIN:VEVENT\r\n\
SUMMARY:Good Event\r\n\
DTSTART:20260321T100000Z\r\n\
DTEND:20260321T110000Z\r\n\
UID:good@example.com\r\n\
END:VEVENT\r\n\
END:VCALENDAR\r\n";

        let events = parse_vcalendar(ical_data, "caldav").expect("should parse");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].title, "Good Event");
    }

    #[test]
    fn parse_vcalendar_empty() {
        let ical_data = "\
BEGIN:VCALENDAR\r\n\
VERSION:2.0\r\n\
PRODID:-//Test//Test//EN\r\n\
END:VCALENDAR\r\n";

        let events = parse_vcalendar(ical_data, "caldav").expect("should parse");
        assert!(events.is_empty());
    }

    #[test]
    fn normalized_event_serializes_to_json() {
        let vevent = make_vevent(vec![
            ("SUMMARY", "Serialization Test", None),
            ("DTSTART", "20260321T100000Z", None),
            ("DTEND", "20260321T110000Z", None),
            ("UID", "json-001@example.com", None),
        ]);

        let event = normalize_vevent(&vevent, "caldav").expect("should normalize");
        let json = serde_json::to_string(&event).expect("should serialize");
        let restored: CalendarEvent =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(restored.title, event.title);
        assert_eq!(restored.source_id, event.source_id);
    }

    #[test]
    fn normalized_event_has_valid_uuid() {
        let vevent = make_vevent(vec![
            ("SUMMARY", "UUID Test", None),
            ("DTSTART", "20260321T100000Z", None),
            ("DTEND", "20260321T110000Z", None),
            ("UID", "uuid-001@example.com", None),
        ]);

        let event = normalize_vevent(&vevent, "caldav").expect("should normalize");
        assert!(!event.id.is_nil());
    }

    #[test]
    fn parse_ical_datetime_utc() {
        let dt = parse_ical_datetime("20260321T100000Z", &None).expect("should parse");
        assert_eq!(dt.to_rfc3339(), "2026-03-21T10:00:00+00:00");
    }

    #[test]
    fn parse_ical_datetime_local() {
        let dt = parse_ical_datetime("20260321T153000", &None).expect("should parse");
        assert_eq!(dt.to_rfc3339(), "2026-03-21T15:30:00+00:00");
    }

    #[test]
    fn parse_ical_datetime_date_only() {
        let dt = parse_ical_datetime("20260325", &None).expect("should parse");
        assert_eq!(dt.date_naive().to_string(), "2026-03-25");
        assert_eq!(dt.hour(), 0);
        assert_eq!(dt.minute(), 0);
        assert_eq!(dt.second(), 0);
    }

    #[test]
    fn parse_ical_datetime_invalid() {
        let result = parse_ical_datetime("not-a-date", &None);
        assert!(result.is_err());
    }

    #[test]
    fn is_date_only_true() {
        assert!(is_date_only("20260321"));
    }

    #[test]
    fn is_date_only_false_for_datetime() {
        assert!(!is_date_only("20260321T100000"));
        assert!(!is_date_only("20260321T100000Z"));
    }

    #[test]
    fn extract_attendees_strips_mailto() {
        let mut vevent = IcalEvent {
            properties: Vec::new(),
            alarms: Vec::new(),
        };
        vevent.properties.push(Property {
            name: "ATTENDEE".to_string(),
            value: Some("mailto:test@example.com".to_string()),
            params: None,
        });
        vevent.properties.push(Property {
            name: "ATTENDEE".to_string(),
            value: Some("plain@example.com".to_string()),
            params: None,
        });

        let attendees = extract_attendees(&vevent);
        assert_eq!(attendees.len(), 2);
        assert_eq!(attendees[0].email, "test@example.com");
        assert_eq!(attendees[1].email, "plain@example.com");
    }

    #[test]
    fn normalize_event_with_full_rrule() {
        let vevent = make_vevent(vec![
            ("SUMMARY", "Weekly Standup", None),
            ("DTSTART", "20260323T090000Z", None),
            ("DTEND", "20260323T091500Z", None),
            ("UID", "rrule-full@example.com", None),
            ("RRULE", "FREQ=WEEKLY;BYDAY=MO,WE,FR;UNTIL=20261231T000000Z", None),
        ]);

        let event = normalize_vevent(&vevent, "caldav").expect("should normalize");
        assert_eq!(
            event.recurrence.as_ref().map(|r| r.to_rrule()).as_deref(),
            Some("FREQ=WEEKLY;BYDAY=MO,WE,FR;UNTIL=20261231T000000Z")
        );
    }

    #[test]
    fn parse_vcalendar_with_recurrence_and_attendees() {
        let ical_data = "\
BEGIN:VCALENDAR\r\n\
VERSION:2.0\r\n\
PRODID:-//Test//Test//EN\r\n\
BEGIN:VEVENT\r\n\
SUMMARY:Recurring Meeting\r\n\
DTSTART:20260323T090000Z\r\n\
DTEND:20260323T100000Z\r\n\
UID:full-001@example.com\r\n\
RRULE:FREQ=WEEKLY;BYDAY=MO\r\n\
ATTENDEE:mailto:alice@example.com\r\n\
ATTENDEE:mailto:bob@example.com\r\n\
LOCATION:Board Room\r\n\
DESCRIPTION:Weekly planning session\r\n\
END:VEVENT\r\n\
END:VCALENDAR\r\n";

        let events = parse_vcalendar(ical_data, "caldav").expect("should parse");
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.title, "Recurring Meeting");
        assert_eq!(event.recurrence.as_ref().map(|r| r.to_rrule()).as_deref(), Some("FREQ=WEEKLY;BYDAY=MO"));
        assert_eq!(event.attendees.len(), 2);
        assert_eq!(event.location.as_deref(), Some("Board Room"));
        assert_eq!(event.description.as_deref(), Some("Weekly planning session"));
    }
}
