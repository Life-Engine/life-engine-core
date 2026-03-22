//! CalDAV client for connecting to and fetching calendar events from CalDAV servers.
//!
//! Handles connection configuration, sync state tracking via ctag/sync-token,
//! and incremental calendar synchronisation. Actual HTTP requests to CalDAV
//! servers are gated behind the `integration` feature flag.

use dav_utils::etag::DavResource;
use dav_utils::sync_state::DavSyncState;
use life_engine_types::CalendarEvent;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for a CalDAV connection.
///
/// The password is not stored in this config struct. Instead, the
/// `credential_key` names the key under which the password is stored
/// in the credential store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalDavConfig {
    /// The CalDAV server base URL (e.g. `http://localhost:5232`).
    pub server_url: String,
    /// The username for authentication.
    pub username: String,
    /// The key used to look up the password in the credential store.
    #[serde(default = "default_caldav_credential_key")]
    pub credential_key: String,
    /// The calendar path on the server (e.g. `/user/calendar/`).
    pub calendar_path: String,
}

/// Default credential key for CalDAV passwords.
fn default_caldav_credential_key() -> String {
    "caldav_password".to_string()
}

/// Tracks sync state for a calendar collection.
///
/// CalDAV servers expose either a `sync-token` (RFC 6578) or a `ctag`
/// (Apple extension) to detect changes. We store whichever the server
/// provides to enable incremental sync on subsequent fetches.
///
/// This is a type alias for the shared [`DavSyncState`] type.
pub type SyncState = DavSyncState;

/// A fetched calendar resource from a CalDAV server.
#[derive(Debug, Clone)]
pub struct FetchedResource {
    /// The href (path) of the resource on the server.
    pub href: String,
    /// The ETag of the resource.
    pub etag: String,
    /// The raw iCalendar data for this resource.
    pub ical_data: String,
}

impl DavResource for FetchedResource {
    fn href(&self) -> &str {
        &self.href
    }
    fn etag(&self) -> &str {
        &self.etag
    }
}

/// CalDAV client that manages connection configuration and sync state.
///
/// The actual CalDAV HTTP operations (PROPFIND, REPORT, GET) are handled
/// by the integration layer behind the `integration` feature flag. This
/// struct tracks sync tokens and ETags so incremental sync works correctly.
pub struct CalDavClient {
    /// Connection configuration.
    config: CalDavConfig,
    /// Per-calendar sync state.
    sync_states: HashMap<String, SyncState>,
}

impl CalDavClient {
    /// Create a new CalDAV client with the given configuration.
    pub fn new(config: CalDavConfig) -> Self {
        Self {
            config,
            sync_states: HashMap::new(),
        }
    }

    /// Returns the CalDAV configuration.
    pub fn config(&self) -> &CalDavConfig {
        &self.config
    }

    /// Returns the full calendar URL by combining server_url and calendar_path.
    pub fn calendar_url(&self) -> String {
        dav_utils::url::join_dav_url(&self.config.server_url, &self.config.calendar_path)
    }

    /// Returns the current sync state for a calendar path.
    pub fn sync_state(&self, calendar_path: &str) -> Option<&SyncState> {
        self.sync_states.get(calendar_path)
    }

    /// Determine whether a sync should be incremental or full based on
    /// the server's current sync-token or ctag compared to our stored state.
    ///
    /// Returns `(needs_full_sync, previous_sync_token)`.
    pub fn compute_start_sync(
        &self,
        calendar_path: &str,
        server_sync_token: Option<&str>,
        server_ctag: Option<&str>,
    ) -> (bool, Option<String>) {
        match self.sync_states.get(calendar_path) {
            None => {
                // First sync — always full
                (true, None)
            }
            Some(state) => {
                // Check sync-token first
                if let (Some(server_token), Some(stored_token)) =
                    (server_sync_token, &state.sync_token)
                {
                    if server_token == stored_token {
                        // No changes
                        return (false, Some(stored_token.clone()));
                    }
                    // Token changed — incremental sync possible
                    return (false, Some(stored_token.clone()));
                }

                // Fall back to ctag comparison
                if let (Some(server_ct), Some(stored_ct)) =
                    (server_ctag, &state.ctag)
                {
                    if server_ct == stored_ct {
                        // No changes
                        return (false, Some(stored_ct.clone()));
                    }
                    // ctag changed — need to check individual ETags
                    return (true, None);
                }

                // No sync mechanism available — full sync
                (true, None)
            }
        }
    }

    /// Update the sync state for a calendar after a successful sync.
    pub fn update_sync_state(
        &mut self,
        calendar_path: &str,
        sync_token: Option<String>,
        ctag: Option<String>,
        etags: HashMap<String, String>,
    ) {
        self.sync_states.insert(
            calendar_path.to_string(),
            SyncState {
                sync_token,
                ctag,
                etags,
            },
        );
    }

    /// Filter fetched resources to find only those that are new or changed
    /// compared to the stored ETags.
    ///
    /// Returns the resources that need processing and the updated ETag map.
    pub fn filter_changed(
        &self,
        calendar_path: &str,
        fetched: &[FetchedResource],
    ) -> Vec<FetchedResource> {
        let stored_etags = self
            .sync_states
            .get(calendar_path)
            .map(|s| &s.etags)
            .cloned()
            .unwrap_or_default();

        dav_utils::etag::filter_changed(&stored_etags, fetched)
    }

    /// Build the HTTP Basic Auth header value for CalDAV requests.
    ///
    /// The `password` parameter should be retrieved from the credential
    /// store using the config's `credential_key`.
    pub fn auth_header(&self, password: &str) -> String {
        dav_utils::auth::basic_auth_header(&self.config.username, password)
    }

    /// Create an event on the CalDAV server (stub, gated behind `integration` feature).
    #[allow(dead_code)]
    pub fn create_event(&self, _event: &CalendarEvent) -> anyhow::Result<()> {
        tracing::info!("CalDAV create_event stub called");
        Ok(())
    }

    /// Update an event on the CalDAV server (stub, gated behind `integration` feature).
    #[allow(dead_code)]
    pub fn update_event(&self, _event: &CalendarEvent) -> anyhow::Result<()> {
        tracing::info!("CalDAV update_event stub called");
        Ok(())
    }

    /// Delete an event on the CalDAV server (stub, gated behind `integration` feature).
    #[allow(dead_code)]
    pub fn delete_event(&self, _event_id: &str) -> anyhow::Result<()> {
        tracing::info!("CalDAV delete_event stub called");
        Ok(())
    }
}

/// Convert a CDM CalendarEvent back to iCalendar VEVENT format.
pub fn build_vevent_ical(event: &CalendarEvent) -> String {
    let uid = &event.source_id;
    let summary = &event.title;
    let dtstart = event.start.format("%Y%m%dT%H%M%SZ").to_string();
    let dtend = event.end.format("%Y%m%dT%H%M%SZ").to_string();

    let mut lines = vec![
        "BEGIN:VCALENDAR".to_string(),
        "VERSION:2.0".to_string(),
        "PRODID:-//Life Engine//Calendar//EN".to_string(),
        "BEGIN:VEVENT".to_string(),
        format!("UID:{uid}"),
        format!("SUMMARY:{summary}"),
        format!("DTSTART:{dtstart}"),
        format!("DTEND:{dtend}"),
    ];

    if let Some(ref loc) = event.location {
        lines.push(format!("LOCATION:{loc}"));
    }
    if let Some(ref desc) = event.description {
        lines.push(format!("DESCRIPTION:{desc}"));
    }
    if let Some(ref rrule) = event.recurrence {
        lines.push(format!("RRULE:{rrule}"));
    }
    for attendee in &event.attendees {
        lines.push(format!("ATTENDEE:mailto:{attendee}"));
    }

    lines.push("END:VEVENT".to_string());
    lines.push("END:VCALENDAR".to_string());
    lines.join("\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> CalDavConfig {
        CalDavConfig {
            server_url: "http://localhost:5232".into(),
            username: "testuser".into(),
            credential_key: "caldav_password".into(),
            calendar_path: "/testuser/calendar/".into(),
        }
    }

    #[test]
    fn caldav_config_serialization() {
        let config = test_config();
        let json = serde_json::to_string(&config).expect("serialize");
        let restored: CalDavConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.server_url, "http://localhost:5232");
        assert_eq!(restored.username, "testuser");
        assert_eq!(restored.calendar_path, "/testuser/calendar/");
    }

    #[test]
    fn caldav_client_construction() {
        let client = CalDavClient::new(test_config());
        assert_eq!(client.config().server_url, "http://localhost:5232");
        assert!(client.sync_state("/testuser/calendar/").is_none());
    }

    #[test]
    fn calendar_url_construction() {
        let client = CalDavClient::new(test_config());
        assert_eq!(
            client.calendar_url(),
            "http://localhost:5232/testuser/calendar/"
        );
    }

    #[test]
    fn calendar_url_trims_slashes() {
        let config = CalDavConfig {
            server_url: "http://localhost:5232/".into(),
            username: "user".into(),
            credential_key: "caldav_password".into(),
            calendar_path: "/user/cal/".into(),
        };
        let client = CalDavClient::new(config);
        assert_eq!(client.calendar_url(), "http://localhost:5232/user/cal/");
    }

    #[test]
    fn sync_state_default() {
        let state = SyncState::default();
        assert!(state.sync_token.is_none());
        assert!(state.ctag.is_none());
        assert!(state.etags.is_empty());
    }

    #[test]
    fn sync_state_serialization() {
        let mut etags = HashMap::new();
        etags.insert("/cal/event1.ics".into(), "\"etag-1\"".into());
        let state = SyncState {
            sync_token: Some("sync-token-abc".into()),
            ctag: Some("ctag-123".into()),
            etags,
        };
        let json = serde_json::to_string(&state).expect("serialize");
        let restored: SyncState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.sync_token.as_deref(), Some("sync-token-abc"));
        assert_eq!(restored.ctag.as_deref(), Some("ctag-123"));
        assert_eq!(restored.etags.len(), 1);
    }

    #[test]
    fn compute_start_sync_first_sync() {
        let client = CalDavClient::new(test_config());
        let (needs_full, prev_token) =
            client.compute_start_sync("/testuser/calendar/", None, None);
        assert!(needs_full);
        assert!(prev_token.is_none());
    }

    #[test]
    fn compute_start_sync_token_unchanged() {
        let mut client = CalDavClient::new(test_config());
        client.update_sync_state(
            "/testuser/calendar/",
            Some("token-1".into()),
            None,
            HashMap::new(),
        );

        let (needs_full, prev_token) =
            client.compute_start_sync("/testuser/calendar/", Some("token-1"), None);
        assert!(!needs_full);
        assert_eq!(prev_token.as_deref(), Some("token-1"));
    }

    #[test]
    fn compute_start_sync_token_changed() {
        let mut client = CalDavClient::new(test_config());
        client.update_sync_state(
            "/testuser/calendar/",
            Some("token-1".into()),
            None,
            HashMap::new(),
        );

        let (needs_full, prev_token) =
            client.compute_start_sync("/testuser/calendar/", Some("token-2"), None);
        assert!(!needs_full);
        assert_eq!(prev_token.as_deref(), Some("token-1"));
    }

    #[test]
    fn compute_start_sync_ctag_unchanged() {
        let mut client = CalDavClient::new(test_config());
        client.update_sync_state(
            "/testuser/calendar/",
            None,
            Some("ctag-abc".into()),
            HashMap::new(),
        );

        let (needs_full, _) =
            client.compute_start_sync("/testuser/calendar/", None, Some("ctag-abc"));
        assert!(!needs_full);
    }

    #[test]
    fn compute_start_sync_ctag_changed() {
        let mut client = CalDavClient::new(test_config());
        client.update_sync_state(
            "/testuser/calendar/",
            None,
            Some("ctag-old".into()),
            HashMap::new(),
        );

        let (needs_full, _) =
            client.compute_start_sync("/testuser/calendar/", None, Some("ctag-new"));
        assert!(needs_full);
    }

    #[test]
    fn update_sync_state() {
        let mut client = CalDavClient::new(test_config());
        assert!(client.sync_state("/testuser/calendar/").is_none());

        let mut etags = HashMap::new();
        etags.insert("/cal/e1.ics".into(), "\"etag-1\"".into());
        client.update_sync_state(
            "/testuser/calendar/",
            Some("token-1".into()),
            Some("ctag-1".into()),
            etags,
        );

        let state = client
            .sync_state("/testuser/calendar/")
            .expect("should have state");
        assert_eq!(state.sync_token.as_deref(), Some("token-1"));
        assert_eq!(state.ctag.as_deref(), Some("ctag-1"));
        assert_eq!(state.etags.len(), 1);
    }

    #[test]
    fn filter_changed_new_resources() {
        let client = CalDavClient::new(test_config());
        let fetched = vec![
            FetchedResource {
                href: "/cal/event1.ics".into(),
                etag: "\"etag-1\"".into(),
                ical_data: "BEGIN:VCALENDAR...".into(),
            },
        ];

        let changed = client.filter_changed("/testuser/calendar/", &fetched);
        assert_eq!(changed.len(), 1);
    }

    #[test]
    fn filter_changed_unchanged_resources() {
        let mut client = CalDavClient::new(test_config());
        let mut etags = HashMap::new();
        etags.insert("/cal/event1.ics".into(), "\"etag-1\"".into());
        client.update_sync_state("/testuser/calendar/", None, None, etags);

        let fetched = vec![
            FetchedResource {
                href: "/cal/event1.ics".into(),
                etag: "\"etag-1\"".into(),
                ical_data: "BEGIN:VCALENDAR...".into(),
            },
        ];

        let changed = client.filter_changed("/testuser/calendar/", &fetched);
        assert!(changed.is_empty());
    }

    #[test]
    fn filter_changed_modified_resources() {
        let mut client = CalDavClient::new(test_config());
        let mut etags = HashMap::new();
        etags.insert("/cal/event1.ics".into(), "\"etag-old\"".into());
        client.update_sync_state("/testuser/calendar/", None, None, etags);

        let fetched = vec![
            FetchedResource {
                href: "/cal/event1.ics".into(),
                etag: "\"etag-new\"".into(),
                ical_data: "BEGIN:VCALENDAR...".into(),
            },
        ];

        let changed = client.filter_changed("/testuser/calendar/", &fetched);
        assert_eq!(changed.len(), 1);
    }

    #[test]
    fn filter_changed_mixed() {
        let mut client = CalDavClient::new(test_config());
        let mut etags = HashMap::new();
        etags.insert("/cal/unchanged.ics".into(), "\"etag-same\"".into());
        etags.insert("/cal/modified.ics".into(), "\"etag-old\"".into());
        client.update_sync_state("/testuser/calendar/", None, None, etags);

        let fetched = vec![
            FetchedResource {
                href: "/cal/unchanged.ics".into(),
                etag: "\"etag-same\"".into(),
                ical_data: "unchanged data".into(),
            },
            FetchedResource {
                href: "/cal/modified.ics".into(),
                etag: "\"etag-new\"".into(),
                ical_data: "modified data".into(),
            },
            FetchedResource {
                href: "/cal/new.ics".into(),
                etag: "\"etag-brand-new\"".into(),
                ical_data: "new data".into(),
            },
        ];

        let changed = client.filter_changed("/testuser/calendar/", &fetched);
        assert_eq!(changed.len(), 2);
        let hrefs: Vec<&str> = changed.iter().map(|r| r.href.as_str()).collect();
        assert!(hrefs.contains(&"/cal/modified.ics"));
        assert!(hrefs.contains(&"/cal/new.ics"));
        assert!(!hrefs.contains(&"/cal/unchanged.ics"));
    }

    #[test]
    fn auth_header_basic() {
        let client = CalDavClient::new(test_config());
        let header = client.auth_header("testpass");
        assert!(header.starts_with("Basic "));

        // Decode and verify
        use base64::Engine;
        let encoded = header.strip_prefix("Basic ").unwrap();
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .expect("valid base64");
        let creds = String::from_utf8(decoded).expect("valid utf8");
        assert_eq!(creds, "testuser:testpass");
    }

    #[test]
    fn build_vevent_ical_produces_valid_output() {
        use chrono::{TimeZone, Utc};
        let event = CalendarEvent {
            id: uuid::Uuid::new_v4(),
            title: "Team Sync".into(),
            start: Utc.with_ymd_and_hms(2026, 3, 21, 10, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 3, 21, 11, 0, 0).unwrap(),
            recurrence: Some("FREQ=WEEKLY;BYDAY=MO".into()),
            attendees: vec!["alice@example.com".into()],
            location: Some("Room A".into()),
            description: Some("Weekly sync".into()),
            source: "caldav".into(),
            source_id: "uid-001@example.com".into(),
            extensions: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let ical = build_vevent_ical(&event);
        assert!(ical.contains("BEGIN:VCALENDAR"));
        assert!(ical.contains("BEGIN:VEVENT"));
        assert!(ical.contains("SUMMARY:Team Sync"));
        assert!(ical.contains("DTSTART:20260321T100000Z"));
        assert!(ical.contains("DTEND:20260321T110000Z"));
        assert!(ical.contains("UID:uid-001@example.com"));
        assert!(ical.contains("LOCATION:Room A"));
        assert!(ical.contains("DESCRIPTION:Weekly sync"));
        assert!(ical.contains("RRULE:FREQ=WEEKLY;BYDAY=MO"));
        assert!(ical.contains("ATTENDEE:mailto:alice@example.com"));
        assert!(ical.contains("END:VEVENT"));
        assert!(ical.contains("END:VCALENDAR"));
    }

    #[test]
    fn fetched_resource_stores_data() {
        let resource = FetchedResource {
            href: "/cal/test.ics".into(),
            etag: "\"etag-test\"".into(),
            ical_data: "BEGIN:VCALENDAR\nEND:VCALENDAR".into(),
        };
        assert_eq!(resource.href, "/cal/test.ics");
        assert_eq!(resource.etag, "\"etag-test\"");
        assert!(!resource.ical_data.is_empty());
    }
}
