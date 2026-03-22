//! Calendar event canonical data model.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Recurrence frequency for repeating events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RecurrenceFrequency {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

/// Recurrence rule for repeating events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Recurrence {
    pub frequency: RecurrenceFrequency,
    #[serde(default = "default_interval")]
    pub interval: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by_day: Option<Vec<String>>,
}

fn default_interval() -> u32 {
    1
}

impl Recurrence {
    /// Parse an iCalendar RRULE string into a `Recurrence`.
    ///
    /// Supports FREQ, INTERVAL, COUNT, UNTIL, and BYDAY parameters.
    /// Returns `None` if the FREQ parameter is missing or unrecognised.
    pub fn from_rrule(rrule: &str) -> Option<Self> {
        // Strip optional "RRULE:" prefix (Google Calendar API includes it)
        let rrule = rrule.strip_prefix("RRULE:").unwrap_or(rrule);

        let mut frequency = None;
        let mut interval = 1u32;
        let mut until = None;
        let mut count = None;
        let mut by_day = None;

        for part in rrule.split(';') {
            if let Some((key, value)) = part.split_once('=') {
                match key {
                    "FREQ" => {
                        frequency = match value {
                            "DAILY" => Some(RecurrenceFrequency::Daily),
                            "WEEKLY" => Some(RecurrenceFrequency::Weekly),
                            "MONTHLY" => Some(RecurrenceFrequency::Monthly),
                            "YEARLY" => Some(RecurrenceFrequency::Yearly),
                            _ => return None,
                        };
                    }
                    "INTERVAL" => {
                        interval = value.parse().unwrap_or(1);
                    }
                    "COUNT" => {
                        count = value.parse().ok();
                    }
                    "UNTIL" => {
                        until = chrono::NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%SZ")
                            .ok()
                            .map(|dt| dt.and_utc());
                    }
                    "BYDAY" => {
                        by_day = Some(value.split(',').map(|s| s.to_string()).collect());
                    }
                    _ => {}
                }
            }
        }

        Some(Recurrence {
            frequency: frequency?,
            interval,
            until,
            count,
            by_day,
        })
    }

    /// Convert this `Recurrence` to an iCalendar RRULE string.
    pub fn to_rrule(&self) -> String {
        let freq = match self.frequency {
            RecurrenceFrequency::Daily => "DAILY",
            RecurrenceFrequency::Weekly => "WEEKLY",
            RecurrenceFrequency::Monthly => "MONTHLY",
            RecurrenceFrequency::Yearly => "YEARLY",
        };
        let mut parts = vec![format!("FREQ={freq}")];
        if self.interval > 1 {
            parts.push(format!("INTERVAL={}", self.interval));
        }
        if let Some(count) = self.count {
            parts.push(format!("COUNT={count}"));
        }
        if let Some(ref days) = self.by_day {
            parts.push(format!("BYDAY={}", days.join(",")));
        }
        if let Some(ref until) = self.until {
            parts.push(format!("UNTIL={}", until.format("%Y%m%dT%H%M%SZ")));
        }
        parts.join(";")
    }
}

/// Attendee response status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AttendeeStatus {
    Accepted,
    Declined,
    Tentative,
    #[serde(rename = "needs-action")]
    NeedsAction,
}

/// An event attendee.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Attendee {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<AttendeeStatus>,
}

impl Attendee {
    /// Create an `Attendee` from just an email address.
    pub fn from_email(email: impl Into<String>) -> Self {
        Self {
            name: None,
            email: email.into(),
            status: None,
        }
    }
}

/// Reminder delivery method.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReminderMethod {
    Notification,
    Email,
}

/// A reminder for an event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Reminder {
    pub minutes_before: u32,
    pub method: ReminderMethod,
}

/// Event confirmation status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EventStatus {
    Confirmed,
    Tentative,
    Cancelled,
}

/// A calendar event in the Life Engine canonical data model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CalendarEvent {
    pub id: Uuid,
    pub title: String,
    pub start: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub all_day: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurrence: Option<Recurrence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attendees: Vec<Attendee>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reminders: Vec<Reminder>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<EventStatus>,
    pub source: String,
    pub source_id: String,
    /// Plugin-specific extension data, namespaced by plugin ID (reverse-domain format).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl CalendarEvent {
    /// Validate that the event's time range is consistent (start before end).
    pub fn validate_time_range(&self) -> Result<(), String> {
        if let Some(end) = self.end {
            if self.start >= end {
                return Err(format!(
                    "event start ({}) must be before end ({})",
                    self.start, end
                ));
            }
        }
        Ok(())
    }
}
