//! Module-internal types for the CalDAV transport crate.

/// Content-Type for iCalendar responses.
pub const CONTENT_TYPE_CALENDAR: &str = "text/calendar; charset=utf-8";

/// Content-Type for WebDAV XML responses.
pub const CONTENT_TYPE_XML: &str = "application/xml; charset=utf-8";

/// A CalDAV calendar collection exposed by this transport.
#[derive(Debug, Clone)]
pub struct CalendarCollection {
    /// Display name shown to CalDAV clients.
    pub display_name: String,
    /// URL path segment for this calendar (e.g. `"default"`).
    pub path: String,
    /// Optional description of the calendar.
    pub description: Option<String>,
    /// Optional hex color for CalDAV clients (e.g. `"#0E61B9FF"`).
    pub color: Option<String>,
}

/// A single calendar resource (event) within a collection.
#[derive(Debug, Clone)]
pub struct CalendarResource {
    /// Unique identifier (used as filename, e.g. `"event-uuid.ics"`).
    pub uid: String,
    /// Full iCalendar data (VCALENDAR with VEVENT).
    pub data: String,
    /// ETag for change detection.
    pub etag: String,
}

/// Depth header values for PROPFIND requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    /// Depth: 0 — properties of the resource itself only.
    Zero,
    /// Depth: 1 — resource and its immediate children.
    One,
    /// Depth: infinity — full recursive tree.
    Infinity,
}

impl Depth {
    /// Parse a Depth header value string.
    pub fn parse(value: &str) -> Self {
        match value.trim() {
            "0" => Depth::Zero,
            "1" => Depth::One,
            _ => Depth::Infinity,
        }
    }
}
