//! iCalendar serialisation: CDM `CalendarEvent` to/from iCalendar VEVENT format.
//!
//! Used by the CalDAV server to serve events in the standard iCalendar
//! format and to parse incoming PUT requests from calendar clients.

use chrono::Utc;
use dav_utils::ical::{escape_ical_value, is_date_only, parse_ical_datetime};
use life_engine_types::CalendarEvent;
use life_engine_types::events::{Attendee, Recurrence};
use uuid::Uuid;

/// Serialise a CDM `CalendarEvent` to iCalendar VCALENDAR/VEVENT format.
///
/// Produces a complete iCalendar document with a single VEVENT,
/// suitable for serving via CalDAV GET responses.
pub fn event_to_ical(event: &CalendarEvent) -> String {
    let uid = &event.source_id;
    let summary = escape_ical_value(&event.title);
    let dtstamp = event.updated_at.format("%Y%m%dT%H%M%SZ").to_string();
    let created = event.created_at.format("%Y%m%dT%H%M%SZ").to_string();
    let last_modified = event.updated_at.format("%Y%m%dT%H%M%SZ").to_string();

    let is_all_day = event.all_day == Some(true);

    let (dtstart, dtend) = if is_all_day {
        let start = event.start.format("%Y%m%d").to_string();
        let end = event.end.unwrap_or(event.start).format("%Y%m%d").to_string();
        (format!("DTSTART;VALUE=DATE:{start}"), format!("DTEND;VALUE=DATE:{end}"))
    } else {
        let start = event.start.format("%Y%m%dT%H%M%SZ").to_string();
        let end = event.end.unwrap_or(event.start).format("%Y%m%dT%H%M%SZ").to_string();
        (format!("DTSTART:{start}"), format!("DTEND:{end}"))
    };

    let mut lines = vec![
        "BEGIN:VCALENDAR".to_string(),
        "VERSION:2.0".to_string(),
        "PRODID:-//Life Engine//CalDAV Server//EN".to_string(),
        "BEGIN:VEVENT".to_string(),
        format!("UID:{uid}"),
        format!("DTSTAMP:{dtstamp}"),
        dtstart,
        dtend,
        format!("SUMMARY:{summary}"),
        format!("CREATED:{created}"),
        format!("LAST-MODIFIED:{last_modified}"),
    ];

    if let Some(ref loc) = event.location {
        lines.push(format!("LOCATION:{}", escape_ical_value(loc)));
    }
    if let Some(ref desc) = event.description {
        lines.push(format!("DESCRIPTION:{}", escape_ical_value(desc)));
    }
    if let Some(ref recurrence) = event.recurrence {
        lines.push(format!("RRULE:{}", recurrence.to_rrule()));
    }
    if let Some(seq) = event.sequence {
        lines.push(format!("SEQUENCE:{seq}"));
    }
    if let Some(ref status) = event.status {
        let status_str = match status {
            life_engine_types::events::EventStatus::Confirmed => "CONFIRMED",
            life_engine_types::events::EventStatus::Tentative => "TENTATIVE",
            life_engine_types::events::EventStatus::Cancelled => "CANCELLED",
        };
        lines.push(format!("STATUS:{status_str}"));
    }
    for attendee in &event.attendees {
        let mut params = Vec::new();
        if let Some(ref name) = attendee.name {
            params.push(format!("CN={name}"));
        }
        if let Some(ref status) = attendee.status {
            let partstat = match status {
                life_engine_types::events::AttendeeStatus::Accepted => "ACCEPTED",
                life_engine_types::events::AttendeeStatus::Declined => "DECLINED",
                life_engine_types::events::AttendeeStatus::Tentative => "TENTATIVE",
                life_engine_types::events::AttendeeStatus::NeedsAction => "NEEDS-ACTION",
            };
            params.push(format!("PARTSTAT={partstat}"));
        }
        if let Some(ref role) = attendee.role {
            let role_str = match role {
                life_engine_types::events::AttendeeRole::Chair => "CHAIR",
                life_engine_types::events::AttendeeRole::ReqParticipant => "REQ-PARTICIPANT",
                life_engine_types::events::AttendeeRole::OptParticipant => "OPT-PARTICIPANT",
                life_engine_types::events::AttendeeRole::NonParticipant => "NON-PARTICIPANT",
            };
            params.push(format!("ROLE={role_str}"));
        }
        if params.is_empty() {
            lines.push(format!("ATTENDEE:mailto:{}", attendee.email));
        } else {
            lines.push(format!("ATTENDEE;{}:mailto:{}", params.join(";"), attendee.email));
        }
    }
    for reminder in &event.reminders {
        let action = match reminder.method {
            life_engine_types::events::ReminderMethod::Email => "EMAIL",
            life_engine_types::events::ReminderMethod::Notification => "DISPLAY",
        };
        lines.push("BEGIN:VALARM".to_string());
        lines.push(format!("ACTION:{action}"));
        lines.push(format!("TRIGGER:-PT{}M", reminder.minutes_before));
        lines.push("DESCRIPTION:Reminder".to_string());
        lines.push("END:VALARM".to_string());
    }

    lines.push("END:VEVENT".to_string());

    if let Some(ref tz) = event.timezone {
        // Insert a minimal VTIMEZONE component before the VEVENT.
        // Full VTIMEZONE with STANDARD/DAYLIGHT sub-components is not
        // yet supported — this placeholder satisfies clients that require
        // the component to be present.
        let vtimezone = vec![
            "BEGIN:VTIMEZONE".to_string(),
            format!("TZID:{tz}"),
            "BEGIN:STANDARD".to_string(),
            "DTSTART:19700101T000000".to_string(),
            "TZOFFSETFROM:+0000".to_string(),
            "TZOFFSETTO:+0000".to_string(),
            "END:STANDARD".to_string(),
            "END:VTIMEZONE".to_string(),
        ];
        // Insert VTIMEZONE right after PRODID (index 3 = BEGIN:VEVENT)
        for (i, line) in vtimezone.into_iter().enumerate() {
            lines.insert(3 + i, line);
        }
    }

    lines.push("END:VCALENDAR".to_string());

    // RFC 5545 §3.1: Lines longer than 75 octets SHOULD be folded with
    // CRLF followed by a single space (linear white space).
    let mut output: String = lines
        .iter()
        .map(|line| fold_line(line))
        .collect::<Vec<_>>()
        .join("\r\n");
    output.push_str("\r\n");
    output
}

/// Fold a content line per RFC 5545 §3.1.
///
/// Lines longer than 75 octets are split: the first chunk is 75 octets,
/// continuation chunks are 74 octets (because the leading space counts).
/// Fold points never split multi-byte UTF-8 characters.
fn fold_line(line: &str) -> String {
    if line.len() <= 75 {
        return line.to_string();
    }

    let mut result = String::with_capacity(line.len() + line.len() / 74 * 3);
    let mut chunk_start = 0;
    let mut chunk_byte_len = 0;
    let mut first = true;

    for (idx, ch) in line.char_indices() {
        let char_len = ch.len_utf8();
        let limit = if first { 75 } else { 74 };

        if chunk_byte_len + char_len > limit {
            if !first {
                result.push_str("\r\n ");
            }
            result.push_str(&line[chunk_start..idx]);
            chunk_start = idx;
            chunk_byte_len = 0;
            first = false;
        }
        chunk_byte_len += char_len;
    }

    if chunk_start < line.len() {
        if !first {
            result.push_str("\r\n ");
        }
        result.push_str(&line[chunk_start..]);
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

    let all_day = if is_date_only(&dtstart_raw) {
        Some(true)
    } else {
        None
    };

    let status = get_property(event, "STATUS").and_then(|s| match s.as_str() {
        "CONFIRMED" => Some(life_engine_types::events::EventStatus::Confirmed),
        "TENTATIVE" => Some(life_engine_types::events::EventStatus::Tentative),
        "CANCELLED" => Some(life_engine_types::events::EventStatus::Cancelled),
        _ => None,
    });

    let reminders = extract_valarms(event);

    let timezone = get_property_params(event, "DTSTART")
        .as_ref()
        .and_then(|params| {
            params.iter().find(|(k, _)| k == "TZID").and_then(|(_, v)| v.first().cloned())
        });

    let sequence = get_property(event, "SEQUENCE").and_then(|s| s.parse::<u32>().ok());

    Ok(CalendarEvent {
        id: Uuid::new_v4(),
        title,
        start,
        end: Some(end),
        recurrence,
        attendees,
        location,
        description,
        all_day,
        reminders,
        timezone,
        status,
        sequence,
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
                let mut name = None;
                let mut status = None;
                let mut role = None;
                if let Some(ref params) = p.params {
                    for (key, values) in params {
                        match key.as_str() {
                            "CN" => name = values.first().cloned(),
                            "PARTSTAT" => {
                                status = values.first().and_then(|s| match s.as_str() {
                                    "ACCEPTED" => Some(life_engine_types::events::AttendeeStatus::Accepted),
                                    "DECLINED" => Some(life_engine_types::events::AttendeeStatus::Declined),
                                    "TENTATIVE" => Some(life_engine_types::events::AttendeeStatus::Tentative),
                                    "NEEDS-ACTION" => Some(life_engine_types::events::AttendeeStatus::NeedsAction),
                                    _ => None,
                                });
                            }
                            "ROLE" => {
                                role = values.first().and_then(|s| match s.as_str() {
                                    "CHAIR" => Some(life_engine_types::events::AttendeeRole::Chair),
                                    "REQ-PARTICIPANT" => Some(life_engine_types::events::AttendeeRole::ReqParticipant),
                                    "OPT-PARTICIPANT" => Some(life_engine_types::events::AttendeeRole::OptParticipant),
                                    "NON-PARTICIPANT" => Some(life_engine_types::events::AttendeeRole::NonParticipant),
                                    _ => None,
                                });
                            }
                            _ => {}
                        }
                    }
                }
                Attendee { name, email, status, role }
            })
        })
        .collect()
}

fn extract_valarms(event: &ical::parser::ical::component::IcalEvent) -> Vec<life_engine_types::events::Reminder> {
    use life_engine_types::events::{Reminder, ReminderMethod};

    event.alarms.iter().filter_map(|alarm| {
        let action = alarm.properties.iter()
            .find(|p| p.name == "ACTION")
            .and_then(|p| p.value.as_ref())
            .map(|v| v.to_uppercase());
        let trigger = alarm.properties.iter()
            .find(|p| p.name == "TRIGGER")
            .and_then(|p| p.value.as_ref());

        let minutes_before = trigger.and_then(|t| parse_trigger_minutes(t))?;
        let method = match action.as_deref() {
            Some("EMAIL") => ReminderMethod::Email,
            _ => ReminderMethod::Notification,
        };
        Some(Reminder { minutes_before, method })
    }).collect()
}

fn parse_trigger_minutes(trigger: &str) -> Option<u32> {
    // Parse duration like "-PT15M", "-PT1H", "-PT1H30M", "-P1D"
    let s = trigger.strip_prefix('-')?;
    let s = s.strip_prefix('P')?;
    let mut total_minutes = 0u32;

    if let Some(rest) = s.strip_prefix('T') {
        let mut num_buf = String::new();
        for ch in rest.chars() {
            if ch.is_ascii_digit() {
                num_buf.push(ch);
            } else {
                let n: u32 = num_buf.parse().ok()?;
                num_buf.clear();
                match ch {
                    'H' => total_minutes += n * 60,
                    'M' => total_minutes += n,
                    'S' => total_minutes += n / 60,
                    _ => {}
                }
            }
        }
    } else {
        // Handle "-P1D" etc.
        let mut num_buf = String::new();
        for ch in s.chars() {
            if ch.is_ascii_digit() {
                num_buf.push(ch);
            } else if ch == 'D' {
                let n: u32 = num_buf.parse().ok()?;
                num_buf.clear();
                total_minutes += n * 24 * 60;
            } else if ch == 'T' {
                break;
            }
        }
    }

    if total_minutes > 0 { Some(total_minutes) } else { None }
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
            sequence: None,
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
            sequence: None,
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
            sequence: None,
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

    #[test]
    fn fold_line_preserves_multibyte_utf8() {
        // CJK characters are 3 bytes each. Build a line that would split
        // a character if folding on byte boundaries.
        let cjk = "会".repeat(30); // 90 bytes, 30 chars
        let line = format!("SUMMARY:{cjk}");
        let folded = fold_line(&line);

        // Verify no replacement characters appear (from_utf8_lossy would insert U+FFFD)
        assert!(
            !folded.contains('\u{FFFD}'),
            "fold_line must not split multi-byte characters"
        );

        // Verify the unfolded content round-trips correctly
        let unfolded: String = folded
            .replace("\r\n ", "")
            .to_string();
        assert_eq!(unfolded, line);
    }

    #[test]
    fn fold_line_preserves_emoji() {
        // Emoji are 4 bytes each
        let emoji_line = format!("SUMMARY:{}", "🎉".repeat(20)); // 8 + 80 = 88 bytes
        let folded = fold_line(&emoji_line);

        assert!(
            !folded.contains('\u{FFFD}'),
            "fold_line must not split emoji characters"
        );

        let unfolded: String = folded.replace("\r\n ", "");
        assert_eq!(unfolded, emoji_line);
    }

    #[test]
    fn fold_line_preserves_accented_names() {
        // Mix of 1-byte ASCII and 2-byte accented characters
        let name = "Ñoño García López Ñoño García López Ñoño García López Ñoño García López";
        let line = format!("SUMMARY:{name}");
        let folded = fold_line(&line);

        assert!(
            !folded.contains('\u{FFFD}'),
            "fold_line must not split accented characters"
        );

        let unfolded: String = folded.replace("\r\n ", "");
        assert_eq!(unfolded, line);
    }
}
