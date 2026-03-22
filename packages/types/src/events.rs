//! Calendar event canonical data model.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A calendar event in the Life Engine canonical data model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CalendarEvent {
    pub id: Uuid,
    pub title: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurrence: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attendees: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub source: String,
    pub source_id: String,
    /// Plugin-specific extension data, namespaced by plugin ID (reverse-domain format).
    /// Each key is a plugin's manifest `id` (e.g. `com.life-engine.todos`) and each value
    /// is an opaque JSON object owned by that plugin. See ADR-014.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl CalendarEvent {
    /// Validate that the event's time range is consistent (start before end).
    pub fn validate_time_range(&self) -> Result<(), String> {
        if self.start >= self.end {
            return Err(format!(
                "event start ({}) must be before end ({})",
                self.start, self.end
            ));
        }
        Ok(())
    }
}
