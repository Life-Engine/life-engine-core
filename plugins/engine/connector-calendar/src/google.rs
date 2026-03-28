//! Google Calendar connector with OAuth2 PKCE and incremental sync.
//!
//! Provides full OAuth2 PKCE authentication, Google Calendar API v3 client
//! methods, incremental sync via syncToken, and CDM normalization with
//! extensions. Actual HTTP calls are gated behind the `integration`
//! feature flag.

use std::collections::HashMap;

use base64::Engine as _;
use chrono::{DateTime, Duration, Utc};
use life_engine_types::events::{Attendee, Recurrence};
use life_engine_types::CalendarEvent;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Default HTTP request timeout for Google API calls.
#[cfg(feature = "integration")]
const HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Build an HTTP client with the default request timeout.
#[cfg(feature = "integration")]
fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .build()
        .expect("failed to build HTTP client")
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors returned by Google Calendar API operations.
#[derive(Debug, thiserror::Error)]
pub enum GoogleApiError {
    /// The access token has expired and must be refreshed.
    #[error("access token expired")]
    AuthExpired,

    /// Failed to refresh the access token.
    #[error("token refresh failed: {0}")]
    TokenRefreshFailed(String),

    /// Google API returned 429 — rate limited.
    #[error("rate limited, retry after {retry_after_secs:?} seconds")]
    RateLimited {
        /// Seconds to wait before retrying, parsed from the Retry-After header.
        retry_after_secs: Option<u64>,
    },

    /// The sync token has expired (HTTP 410 Gone). A full re-sync is required.
    #[error("sync token expired (410 Gone), full re-sync required")]
    SyncTokenExpired,

    /// The requested resource was not found (HTTP 404).
    #[error("resource not found: {0}")]
    NotFound(String),

    /// The caller lacks permission (HTTP 403).
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// A general API error with status code and body.
    #[error("Google API error {status}: {body}")]
    ApiError {
        /// The HTTP status code.
        status: u16,
        /// The response body.
        body: String,
    },
}

// ---------------------------------------------------------------------------
// OAuth2 PKCE
// ---------------------------------------------------------------------------

/// Default Google OAuth2 authorization endpoint.
pub const DEFAULT_AUTH_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";

/// Default Google OAuth2 token endpoint.
pub const DEFAULT_TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";

/// Google Calendar API read-only scope.
pub const CALENDAR_SCOPE_READONLY: &str = "https://www.googleapis.com/auth/calendar.readonly";

/// Google Calendar API events read-only scope.
pub const CALENDAR_SCOPE_EVENTS_READONLY: &str =
    "https://www.googleapis.com/auth/calendar.events.readonly";

/// Configuration for a Google Calendar connection.
///
/// Uses OAuth 2.0 PKCE flow. Secrets (client_secret, refresh_token) are
/// stored in the credential store, referenced by their key names here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleCalendarConfig {
    /// The OAuth 2.0 client ID.
    pub client_id: String,
    /// The key used to look up the client secret in the credential store.
    #[serde(default = "default_google_cal_client_secret_key")]
    pub client_secret_key: String,
    /// The key used to look up the refresh token in the credential store.
    #[serde(default = "default_google_cal_refresh_token_key")]
    pub refresh_token_key: String,
    /// The OAuth 2.0 redirect URI for the PKCE flow.
    #[serde(default = "default_redirect_uri")]
    pub redirect_uri: String,
    /// The OAuth 2.0 authorization endpoint.
    #[serde(default = "default_auth_endpoint")]
    pub auth_endpoint: String,
    /// The OAuth 2.0 token endpoint.
    #[serde(default = "default_token_endpoint")]
    pub token_endpoint: String,
}

/// Default credential key for Google Calendar client secret.
fn default_google_cal_client_secret_key() -> String {
    "google_cal_client_secret".to_string()
}

/// Default credential key for Google Calendar refresh token.
fn default_google_cal_refresh_token_key() -> String {
    "google_cal_refresh_token".to_string()
}

fn default_redirect_uri() -> String {
    "http://localhost:3750/api/plugins/com.life-engine.connector-calendar/google/callback"
        .to_string()
}

fn default_auth_endpoint() -> String {
    DEFAULT_AUTH_ENDPOINT.to_string()
}

fn default_token_endpoint() -> String {
    DEFAULT_TOKEN_ENDPOINT.to_string()
}

/// OAuth2 token state obtained from the token endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenState {
    /// The access token for API calls.
    pub access_token: String,
    /// The refresh token for obtaining new access tokens.
    pub refresh_token: String,
    /// When the access token expires.
    pub expires_at: DateTime<Utc>,
    /// The granted scopes.
    pub scopes: Vec<String>,
}

impl TokenState {
    /// Returns `true` if the access token is expired or will expire
    /// within the next 60 seconds.
    pub fn is_expired(&self) -> bool {
        Utc::now() + Duration::seconds(60) >= self.expires_at
    }
}

/// Response from the Google OAuth2 token endpoint.
#[cfg(feature = "integration")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    expires_in: i64,
    #[serde(default)]
    scope: Option<String>,
    token_type: String,
}

/// Generate a PKCE code verifier and S256 challenge pair.
///
/// The verifier is a random string of 43-128 URL-safe characters.
/// The challenge is `BASE64URL(SHA256(verifier))`.
pub fn generate_pkce_challenge() -> (String, String) {
    let mut rng = rand::thread_rng();
    let length = rng.gen_range(43..=128);
    let verifier: String = (0..length)
        .map(|_| {
            const CHARSET: &[u8] =
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();

    let challenge = compute_s256_challenge(&verifier);
    (verifier, challenge)
}

/// Compute the S256 PKCE challenge from a verifier.
///
/// `BASE64URL_NO_PAD(SHA256(ascii(verifier)))`
fn compute_s256_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

/// Build the Google OAuth2 authorization URL with PKCE parameters.
///
/// Returns the full URL that the user should be redirected to in their browser.
pub fn build_auth_url(config: &GoogleCalendarConfig, code_challenge: &str, state: &str) -> String {
    let scopes = format!("{CALENDAR_SCOPE_READONLY} {CALENDAR_SCOPE_EVENTS_READONLY}");
    format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent&code_challenge={}&code_challenge_method=S256&state={}",
        config.auth_endpoint,
        urlencoding::encode(&config.client_id),
        urlencoding::encode(&config.redirect_uri),
        urlencoding::encode(&scopes),
        urlencoding::encode(code_challenge),
        urlencoding::encode(state),
    )
}

/// Exchange an authorization code for tokens using the PKCE flow.
///
/// Calls the token endpoint with the authorization code and PKCE verifier.
/// The `client_secret` parameter should be retrieved from the credential
/// store using the config's `client_secret_key`.
#[cfg(feature = "integration")]
pub async fn exchange_code(
    config: &GoogleCalendarConfig,
    auth_code: &str,
    code_verifier: &str,
    client_secret: &str,
) -> Result<TokenState, GoogleApiError> {
    let client = http_client();
    let response = client
        .post(&config.token_endpoint)
        .form(&[
            ("code", auth_code),
            ("client_id", &config.client_id),
            ("client_secret", client_secret),
            ("redirect_uri", &config.redirect_uri),
            ("grant_type", "authorization_code"),
            ("code_verifier", code_verifier),
        ])
        .send()
        .await
        .map_err(|e| GoogleApiError::TokenRefreshFailed(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Err(GoogleApiError::TokenRefreshFailed(format!(
            "token exchange returned {status}: {body}"
        )));
    }

    let token_resp: TokenResponse = response
        .json()
        .await
        .map_err(|e| GoogleApiError::TokenRefreshFailed(e.to_string()))?;

    let scopes = token_resp
        .scope
        .as_deref()
        .unwrap_or("")
        .split_whitespace()
        .map(String::from)
        .collect();

    Ok(TokenState {
        access_token: token_resp.access_token,
        refresh_token: token_resp.refresh_token.unwrap_or_default(),
        expires_at: Utc::now() + Duration::seconds(token_resp.expires_in),
        scopes,
    })
}

/// Refresh an access token using the refresh token.
///
/// The `client_secret` and `refresh_token` parameters should be retrieved
/// from the credential store using the config's key fields.
#[cfg(feature = "integration")]
pub async fn refresh_access_token(
    config: &GoogleCalendarConfig,
    refresh_token: &str,
    client_secret: &str,
) -> Result<TokenState, GoogleApiError> {
    let client = http_client();
    let response = client
        .post(&config.token_endpoint)
        .form(&[
            ("client_id", config.client_id.as_str()),
            ("client_secret", client_secret),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await
        .map_err(|e| GoogleApiError::TokenRefreshFailed(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Err(GoogleApiError::TokenRefreshFailed(format!(
            "refresh returned {status}: {body}"
        )));
    }

    let token_resp: TokenResponse = response
        .json()
        .await
        .map_err(|e| GoogleApiError::TokenRefreshFailed(e.to_string()))?;

    let scopes = token_resp
        .scope
        .as_deref()
        .unwrap_or("")
        .split_whitespace()
        .map(String::from)
        .collect();

    Ok(TokenState {
        access_token: token_resp.access_token,
        refresh_token: token_resp
            .refresh_token
            .unwrap_or_else(|| refresh_token.to_string()),
        expires_at: Utc::now() + Duration::seconds(token_resp.expires_in),
        scopes,
    })
}

// ---------------------------------------------------------------------------
// URL-encoding helper (lightweight, no extra crate)
// ---------------------------------------------------------------------------

mod urlencoding {
    /// Percent-encode a string for use in URL query parameters.
    pub fn encode(input: &str) -> String {
        let mut encoded = String::with_capacity(input.len() * 3);
        for byte in input.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    encoded.push(byte as char);
                }
                _ => {
                    encoded.push('%');
                    encoded.push_str(&format!("{byte:02X}"));
                }
            }
        }
        encoded
    }
}

// ---------------------------------------------------------------------------
// Google Calendar API v3 response types
// ---------------------------------------------------------------------------

/// Response from the Google Calendar `calendarList.list` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleCalendarListResponse {
    /// The list of calendar entries.
    #[serde(default)]
    pub items: Vec<GoogleCalendarListEntry>,
    /// Token for fetching the next page.
    pub next_page_token: Option<String>,
}

/// A single calendar entry from the calendar list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleCalendarListEntry {
    /// The calendar ID.
    pub id: String,
    /// The calendar summary (name).
    pub summary: Option<String>,
    /// The calendar description.
    pub description: Option<String>,
    /// The timezone of the calendar.
    pub time_zone: Option<String>,
    /// The foreground color.
    pub foreground_color: Option<String>,
    /// The background color.
    pub background_color: Option<String>,
    /// Whether this is the primary calendar.
    #[serde(default)]
    pub primary: bool,
    /// The access role the authenticated user has.
    pub access_role: Option<String>,
}

/// Response from the Google Calendar `events.list` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleEventsListResponse {
    /// The list of events.
    #[serde(default)]
    pub items: Vec<GoogleEvent>,
    /// Token for fetching the next page of events.
    pub next_page_token: Option<String>,
    /// Sync token for incremental sync on subsequent requests.
    pub next_sync_token: Option<String>,
}

/// A Google Calendar API v3 event.
///
/// This mirrors the relevant parts of the Google Calendar Event resource
/// for conversion to our CDM type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleEvent {
    /// The event ID from Google.
    pub id: String,
    /// The event summary/title.
    pub summary: Option<String>,
    /// The event start time.
    pub start: GoogleDateTime,
    /// The event end time.
    pub end: GoogleDateTime,
    /// Recurrence rules (RRULE strings).
    #[serde(default)]
    pub recurrence: Vec<String>,
    /// The event attendees.
    #[serde(default)]
    pub attendees: Vec<GoogleAttendee>,
    /// The event location.
    pub location: Option<String>,
    /// The event description.
    pub description: Option<String>,
    /// When the event was created.
    pub created: Option<String>,
    /// When the event was last updated.
    pub updated: Option<String>,
    /// The event status (confirmed, tentative, cancelled).
    pub status: Option<String>,
    /// Link to the event in Google Calendar web UI.
    pub html_link: Option<String>,
    /// The event creator.
    pub creator: Option<GoogleEventCreator>,
    /// The event organizer.
    pub organizer: Option<GoogleEventOrganizer>,
    /// A color ID for the event.
    pub color_id: Option<String>,
    /// Reminder overrides for this event.
    pub reminders: Option<serde_json::Value>,
    /// Conference data (Google Meet, etc.).
    pub conference_data: Option<serde_json::Value>,
    /// The ETag for this event resource.
    pub etag: Option<String>,
}

/// The creator of a Google Calendar event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleEventCreator {
    /// The creator's email address.
    pub email: Option<String>,
    /// The creator's display name.
    pub display_name: Option<String>,
    /// Whether this is the authenticated user.
    #[serde(rename = "self", default)]
    pub is_self: bool,
}

/// The organizer of a Google Calendar event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleEventOrganizer {
    /// The organizer's email address.
    pub email: Option<String>,
    /// The organizer's display name.
    pub display_name: Option<String>,
    /// Whether this is the authenticated user.
    #[serde(rename = "self", default)]
    pub is_self: bool,
}

/// A date-time value from the Google Calendar API.
///
/// Google uses either `dateTime` (for timed events) or `date` (for
/// all-day events), never both.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleDateTime {
    /// RFC 3339 date-time for timed events.
    pub date_time: Option<String>,
    /// YYYY-MM-DD date for all-day events.
    pub date: Option<String>,
    /// IANA timezone identifier.
    pub time_zone: Option<String>,
}

/// A Google Calendar attendee.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleAttendee {
    /// The attendee's email address.
    pub email: String,
    /// The attendee's display name.
    pub display_name: Option<String>,
}

// ---------------------------------------------------------------------------
// Sync state
// ---------------------------------------------------------------------------

/// Sync state for a single Google Calendar.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GoogleSyncState {
    /// The sync token from the last events.list response.
    pub sync_token: Option<String>,
    /// Timestamp of the last successful sync.
    pub last_sync: Option<DateTime<Utc>>,
    /// Page token for resuming an interrupted paginated sync.
    pub page_token: Option<String>,
}

// ---------------------------------------------------------------------------
// Google Calendar client
// ---------------------------------------------------------------------------

/// Google Calendar client that manages OAuth configuration, token state,
/// and per-calendar sync state.
pub struct GoogleCalendarClient {
    /// OAuth configuration.
    config: GoogleCalendarConfig,
    /// Current OAuth token state.
    token_state: Option<TokenState>,
    /// Per-calendar sync state, keyed by calendar ID.
    sync_states: HashMap<String, GoogleSyncState>,
}

impl GoogleCalendarClient {
    /// Create a new Google Calendar client with the given configuration.
    pub fn new(config: GoogleCalendarConfig) -> Self {
        Self {
            config,
            token_state: None,
            sync_states: HashMap::new(),
        }
    }

    /// Returns the Google Calendar configuration.
    pub fn config(&self) -> &GoogleCalendarConfig {
        &self.config
    }

    /// Returns the current sync token for a calendar, if available.
    pub fn sync_token(&self) -> Option<&str> {
        // Legacy single-calendar accessor: returns primary calendar sync token
        self.sync_states
            .values()
            .next()
            .and_then(|s| s.sync_token.as_deref())
    }

    /// Returns the sync state for a specific calendar.
    pub fn sync_state(&self, calendar_id: &str) -> Option<&GoogleSyncState> {
        self.sync_states.get(calendar_id)
    }

    /// Returns a mutable reference to the sync states map.
    pub fn sync_states_mut(&mut self) -> &mut HashMap<String, GoogleSyncState> {
        &mut self.sync_states
    }

    /// Update the sync token for a specific calendar after a successful sync.
    pub fn update_sync_token(&mut self, token: String) {
        // Legacy: store under "primary" key
        let state = self.sync_states.entry("primary".to_string()).or_default();
        state.sync_token = Some(token);
        state.last_sync = Some(Utc::now());
    }

    /// Update the sync state for a specific calendar.
    pub fn update_calendar_sync_state(
        &mut self,
        calendar_id: &str,
        sync_token: Option<String>,
        page_token: Option<String>,
    ) {
        let state = self.sync_states.entry(calendar_id.to_string()).or_default();
        state.sync_token = sync_token;
        state.page_token = page_token;
        state.last_sync = Some(Utc::now());
    }

    /// Reset sync state for a calendar (e.g., after a 410 Gone).
    pub fn reset_sync_state(&mut self, calendar_id: &str) {
        self.sync_states.remove(calendar_id);
    }

    /// Set the current token state (e.g., after code exchange or refresh).
    pub fn set_token_state(&mut self, token_state: TokenState) {
        self.token_state = Some(token_state);
    }

    /// Returns the current token state, if available.
    pub fn token_state(&self) -> Option<&TokenState> {
        self.token_state.as_ref()
    }

    /// Ensure the access token is valid, refreshing if necessary.
    ///
    /// Returns the current access token or an error if refresh fails.
    /// The `client_secret` parameter should be retrieved from the credential
    /// store using the config's `client_secret_key`.
    #[cfg(feature = "integration")]
    pub async fn ensure_valid_token(
        &mut self,
        client_secret: &str,
    ) -> Result<String, GoogleApiError> {
        match &self.token_state {
            None => Err(GoogleApiError::AuthExpired),
            Some(ts) if ts.is_expired() => {
                let refresh_token = ts.refresh_token.clone();
                let new_state =
                    refresh_access_token(&self.config, &refresh_token, client_secret).await?;
                let access_token = new_state.access_token.clone();
                self.token_state = Some(new_state);
                Ok(access_token)
            }
            Some(ts) => Ok(ts.access_token.clone()),
        }
    }

    /// List all calendars for the authenticated user.
    ///
    /// The `client_secret` parameter should be retrieved from the credential
    /// store using the config's `client_secret_key`.
    #[cfg(feature = "integration")]
    pub async fn list_calendars(
        &mut self,
        client_secret: &str,
    ) -> Result<Vec<GoogleCalendarListEntry>, GoogleApiError> {
        let access_token = self.ensure_valid_token(client_secret).await?;
        let client = http_client();

        let response = client
            .get("https://www.googleapis.com/calendar/v3/users/me/calendarList")
            .bearer_auth(&access_token)
            .send()
            .await
            .map_err(|e| GoogleApiError::ApiError {
                status: 0,
                body: e.to_string(),
            })?;

        map_error_response(&response)?;

        let list: GoogleCalendarListResponse =
            response
                .json()
                .await
                .map_err(|e| GoogleApiError::ApiError {
                    status: 0,
                    body: e.to_string(),
                })?;

        Ok(list.items)
    }

    /// List events for a calendar with optional sync/page tokens and time bounds.
    ///
    /// The `client_secret` parameter should be retrieved from the credential
    /// store using the config's `client_secret_key`.
    #[cfg(feature = "integration")]
    pub async fn list_events(
        &mut self,
        calendar_id: &str,
        sync_token: Option<&str>,
        page_token: Option<&str>,
        time_min: Option<&DateTime<Utc>>,
        time_max: Option<&DateTime<Utc>>,
        client_secret: &str,
    ) -> Result<GoogleEventsListResponse, GoogleApiError> {
        let access_token = self.ensure_valid_token(client_secret).await?;
        let client = http_client();

        let encoded_id = urlencoding::encode(calendar_id);
        let url_str =
            format!("https://www.googleapis.com/calendar/v3/calendars/{encoded_id}/events");
        let mut url = reqwest::Url::parse(&url_str).map_err(|e| GoogleApiError::ApiError {
            status: 0,
            body: e.to_string(),
        })?;

        {
            let mut params = url.query_pairs_mut();
            params.append_pair("singleEvents", "false");
            params.append_pair("maxResults", "250");

            if let Some(token) = sync_token {
                params.append_pair("syncToken", token);
            } else {
                // Only send time bounds when not using syncToken
                if let Some(t_min) = time_min {
                    params.append_pair("timeMin", &t_min.to_rfc3339());
                }
                if let Some(t_max) = time_max {
                    params.append_pair("timeMax", &t_max.to_rfc3339());
                }
            }

            if let Some(token) = page_token {
                params.append_pair("pageToken", token);
            }
        }

        let response = client
            .get(url)
            .bearer_auth(&access_token)
            .send()
            .await
            .map_err(|e| GoogleApiError::ApiError {
                status: 0,
                body: e.to_string(),
            })?;

        map_error_response(&response)?;

        let events_response: GoogleEventsListResponse =
            response
                .json()
                .await
                .map_err(|e| GoogleApiError::ApiError {
                    status: 0,
                    body: e.to_string(),
                })?;

        Ok(events_response)
    }

    /// Get a single event by ID.
    ///
    /// The `client_secret` parameter should be retrieved from the credential
    /// store using the config's `client_secret_key`.
    #[cfg(feature = "integration")]
    pub async fn get_event(
        &mut self,
        calendar_id: &str,
        event_id: &str,
        client_secret: &str,
    ) -> Result<GoogleEvent, GoogleApiError> {
        let access_token = self.ensure_valid_token(client_secret).await?;
        let client = http_client();

        let encoded_cal_id = urlencoding::encode(calendar_id);
        let encoded_evt_id = urlencoding::encode(event_id);
        let url = format!(
            "https://www.googleapis.com/calendar/v3/calendars/{encoded_cal_id}/events/{encoded_evt_id}"
        );

        let response = client
            .get(&url)
            .bearer_auth(&access_token)
            .send()
            .await
            .map_err(|e| GoogleApiError::ApiError {
                status: 0,
                body: e.to_string(),
            })?;

        map_error_response(&response)?;

        let event: GoogleEvent = response
            .json()
            .await
            .map_err(|e| GoogleApiError::ApiError {
                status: 0,
                body: e.to_string(),
            })?;

        Ok(event)
    }

    /// High-level sync orchestrator for a calendar.
    ///
    /// Uses incremental sync via syncToken when available. Falls back
    /// to a full sync on first run or when the sync token expires (410 Gone).
    ///
    /// First sync defaults to events from 30 days ago to 365 days ahead.
    /// The `client_secret` parameter should be retrieved from the credential
    /// store using the config's `client_secret_key`.
    #[cfg(feature = "integration")]
    pub async fn sync_events(
        &mut self,
        calendar_id: &str,
        client_secret: &str,
    ) -> Result<Vec<CalendarEvent>, GoogleApiError> {
        let stored_state = self.sync_states.get(calendar_id).cloned();
        let sync_token = stored_state.as_ref().and_then(|s| s.sync_token.clone());

        let result = self
            .sync_events_inner(calendar_id, sync_token.as_deref(), client_secret)
            .await;

        match result {
            Err(GoogleApiError::SyncTokenExpired) => {
                tracing::warn!(
                    calendar_id = %calendar_id,
                    "sync token expired (410), performing full re-sync"
                );
                self.reset_sync_state(calendar_id);
                self.sync_events_inner(calendar_id, None, client_secret)
                    .await
            }
            other => other,
        }
    }

    /// Inner sync loop: fetches all pages and collects events.
    #[cfg(feature = "integration")]
    async fn sync_events_inner(
        &mut self,
        calendar_id: &str,
        sync_token: Option<&str>,
        client_secret: &str,
    ) -> Result<Vec<CalendarEvent>, GoogleApiError> {
        let mut all_events = Vec::new();
        let mut page_token: Option<String> = None;

        // Time bounds for initial full sync
        let (time_min, time_max) = if sync_token.is_none() {
            let now = Utc::now();
            (
                Some(now - Duration::days(30)),
                Some(now + Duration::days(365)),
            )
        } else {
            (None, None)
        };

        loop {
            let response = self
                .list_events(
                    calendar_id,
                    sync_token,
                    page_token.as_deref(),
                    time_min.as_ref(),
                    time_max.as_ref(),
                    client_secret,
                )
                .await?;

            for google_event in &response.items {
                // Skip cancelled events
                if google_event.status.as_deref() == Some("cancelled") {
                    continue;
                }

                match Self::normalize_google_event(google_event) {
                    Ok(cal_event) => all_events.push(cal_event),
                    Err(e) => {
                        tracing::warn!(
                            event_id = %google_event.id,
                            error = %e,
                            "skipping malformed Google event"
                        );
                    }
                }
            }

            if let Some(next_page) = response.next_page_token {
                page_token = Some(next_page);
            } else {
                // Final page: store the new sync token
                self.update_calendar_sync_state(calendar_id, response.next_sync_token, None);
                break;
            }
        }

        Ok(all_events)
    }

    /// Convert a Google Calendar API event to the Life Engine CDM type.
    ///
    /// Populates the `extensions` field with Google-specific metadata under
    /// the `com.life-engine.connector-calendar` namespace.
    pub fn normalize_google_event(event: &GoogleEvent) -> anyhow::Result<CalendarEvent> {
        let title = event
            .summary
            .clone()
            .unwrap_or_else(|| "(no title)".to_string());

        let start = parse_google_datetime(&event.start)?;
        let end = Some(parse_google_datetime(&event.end)?);

        let recurrence = event
            .recurrence
            .iter()
            .find(|r| r.starts_with("RRULE:"))
            .and_then(|r| Recurrence::from_rrule(r));

        let attendees: Vec<Attendee> = event
            .attendees
            .iter()
            .map(|a| Attendee::from_email(a.email.clone()))
            .collect();

        let created_at = event
            .created
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        let updated_at = event
            .updated
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        // Build extensions with Google-specific metadata
        let mut ext = serde_json::Map::new();
        ext.insert(
            "google_event_id".to_string(),
            serde_json::Value::String(event.id.clone()),
        );
        if let Some(ref link) = event.html_link {
            ext.insert(
                "html_link".to_string(),
                serde_json::Value::String(link.clone()),
            );
        }
        if let Some(ref status) = event.status {
            ext.insert(
                "status".to_string(),
                serde_json::Value::String(status.clone()),
            );
        }
        if let Some(ref color_id) = event.color_id {
            ext.insert(
                "color_id".to_string(),
                serde_json::Value::String(color_id.clone()),
            );
        }
        if let Some(ref creator) = event.creator {
            if let Some(ref email) = creator.email {
                ext.insert(
                    "creator_email".to_string(),
                    serde_json::Value::String(email.clone()),
                );
            }
        }
        if let Some(ref organizer) = event.organizer {
            if let Some(ref email) = organizer.email {
                ext.insert(
                    "organizer_email".to_string(),
                    serde_json::Value::String(email.clone()),
                );
            }
        }
        if let Some(ref conf_data) = event.conference_data {
            if let Some(uri) = conf_data
                .get("entryPoints")
                .and_then(|ep| ep.as_array())
                .and_then(|a| a.first())
                .and_then(|first| first.get("uri"))
                .and_then(|u| u.as_str())
            {
                ext.insert(
                    "conference_uri".to_string(),
                    serde_json::Value::String(uri.to_string()),
                );
            }
        }
        if let Some(ref etag) = event.etag {
            ext.insert("etag".to_string(), serde_json::Value::String(etag.clone()));
        }

        let extensions = if ext.is_empty() {
            None
        } else {
            let mut ns = serde_json::Map::new();
            ns.insert(
                "com.life-engine.connector-calendar".to_string(),
                serde_json::Value::Object(ext),
            );
            Some(serde_json::Value::Object(ns))
        };

        Ok(CalendarEvent {
            id: Uuid::new_v4(),
            title,
            start,
            end,
            recurrence,
            attendees,
            location: event.location.clone(),
            description: event.description.clone(),
            source: "google-calendar".into(),
            source_id: event.id.clone(),
            extensions,
            created_at,
            updated_at,
            all_day: None,
            reminders: vec![],
            timezone: None,
            status: None,
            sequence: None,
        })
    }
}

/// Convert a CDM CalendarEvent to a Google Calendar API event.
pub fn build_google_event(event: &CalendarEvent) -> GoogleEvent {
    GoogleEvent {
        id: event.source_id.clone(),
        summary: Some(event.title.clone()),
        start: GoogleDateTime {
            date_time: Some(event.start.to_rfc3339()),
            date: None,
            time_zone: None,
        },
        end: GoogleDateTime {
            date_time: Some(event.end.unwrap_or(event.start).to_rfc3339()),
            date: None,
            time_zone: None,
        },
        recurrence: event
            .recurrence
            .as_ref()
            .map(|r| vec![format!("RRULE:{}", r.to_rrule())])
            .unwrap_or_default(),
        attendees: event
            .attendees
            .iter()
            .map(|a| GoogleAttendee {
                email: a.email.clone(),
                display_name: None,
            })
            .collect(),
        location: event.location.clone(),
        description: event.description.clone(),
        created: Some(event.created_at.to_rfc3339()),
        updated: Some(event.updated_at.to_rfc3339()),
        status: None,
        html_link: None,
        creator: None,
        organizer: None,
        color_id: None,
        reminders: None,
        conference_data: None,
        etag: None,
    }
}

/// Parse a Google Calendar datetime value into `DateTime<Utc>`.
fn parse_google_datetime(gdt: &GoogleDateTime) -> anyhow::Result<DateTime<Utc>> {
    if let Some(ref dt_str) = gdt.date_time {
        let dt = DateTime::parse_from_rfc3339(dt_str)
            .map_err(|e| anyhow::anyhow!("invalid dateTime '{}': {}", dt_str, e))?;
        return Ok(dt.with_timezone(&Utc));
    }

    if let Some(ref date_str) = gdt.date {
        let date = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            .map_err(|e| anyhow::anyhow!("invalid date '{}': {}", date_str, e))?;
        let datetime = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| anyhow::anyhow!("invalid time for date '{}'", date_str))?;
        return Ok(DateTime::from_naive_utc_and_offset(datetime, Utc));
    }

    Err(anyhow::anyhow!(
        "GoogleDateTime has neither dateTime nor date"
    ))
}

/// Map an HTTP response status to a `GoogleApiError` if it indicates failure.
///
/// Returns `Ok(())` for success responses, allowing the caller to proceed
/// with parsing the body.
#[cfg(feature = "integration")]
fn map_error_response(response: &reqwest::Response) -> Result<(), GoogleApiError> {
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }

    match status.as_u16() {
        401 => Err(GoogleApiError::AuthExpired),
        403 => Err(GoogleApiError::PermissionDenied("forbidden".to_string())),
        404 => Err(GoogleApiError::NotFound(response.url().to_string())),
        410 => Err(GoogleApiError::SyncTokenExpired),
        429 => {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok());
            Err(GoogleApiError::RateLimited {
                retry_after_secs: retry_after,
            })
        }
        _ => Err(GoogleApiError::ApiError {
            status: status.as_u16(),
            body: format!("HTTP {}", status),
        }),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> GoogleCalendarConfig {
        GoogleCalendarConfig {
            client_id: "test-client-id".into(),
            client_secret_key: "google_cal_client_secret".into(),
            refresh_token_key: "google_cal_refresh_token".into(),
            redirect_uri: "http://localhost:3750/callback".into(),
            auth_endpoint: DEFAULT_AUTH_ENDPOINT.to_string(),
            token_endpoint: DEFAULT_TOKEN_ENDPOINT.to_string(),
        }
    }

    #[test]
    fn google_config_serialization() {
        let config = test_config();
        let json = serde_json::to_string(&config).expect("serialize");
        let restored: GoogleCalendarConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.client_id, "test-client-id");
        assert_eq!(restored.client_secret_key, "google_cal_client_secret");
        assert_eq!(restored.refresh_token_key, "google_cal_refresh_token");
    }

    #[test]
    fn google_config_default_endpoints() {
        let json = r#"{
            "client_id": "id"
        }"#;
        let config: GoogleCalendarConfig = serde_json::from_str(json).expect("deserialize");
        assert_eq!(config.auth_endpoint, DEFAULT_AUTH_ENDPOINT);
        assert_eq!(config.token_endpoint, DEFAULT_TOKEN_ENDPOINT);
        assert!(!config.redirect_uri.is_empty());
        assert_eq!(config.client_secret_key, "google_cal_client_secret");
        assert_eq!(config.refresh_token_key, "google_cal_refresh_token");
    }

    #[test]
    fn google_client_construction() {
        let client = GoogleCalendarClient::new(test_config());
        assert_eq!(client.config().client_id, "test-client-id");
        assert!(client.sync_token().is_none());
    }

    #[test]
    fn google_client_sync_token() {
        let mut client = GoogleCalendarClient::new(test_config());
        assert!(client.sync_token().is_none());

        client.update_sync_token("next-page-token-123".into());
        assert_eq!(client.sync_token(), Some("next-page-token-123"));
    }

    #[test]
    fn google_client_calendar_sync_state() {
        let mut client = GoogleCalendarClient::new(test_config());
        assert!(client.sync_state("cal-1").is_none());

        client.update_calendar_sync_state("cal-1", Some("sync-abc".to_string()), None);
        let state = client.sync_state("cal-1").expect("should have state");
        assert_eq!(state.sync_token.as_deref(), Some("sync-abc"));
        assert!(state.last_sync.is_some());
        assert!(state.page_token.is_none());
    }

    #[test]
    fn google_client_reset_sync_state() {
        let mut client = GoogleCalendarClient::new(test_config());
        client.update_calendar_sync_state("cal-1", Some("sync-abc".to_string()), None);
        assert!(client.sync_state("cal-1").is_some());

        client.reset_sync_state("cal-1");
        assert!(client.sync_state("cal-1").is_none());
    }

    #[test]
    fn google_sync_state_serialization() {
        let state = GoogleSyncState {
            sync_token: Some("token-123".into()),
            last_sync: Some(Utc::now()),
            page_token: None,
        };
        let json = serde_json::to_string(&state).expect("serialize");
        let restored: GoogleSyncState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.sync_token.as_deref(), Some("token-123"));
    }

    // ---- PKCE tests ----

    #[test]
    fn pkce_challenge_length() {
        let (verifier, challenge) = generate_pkce_challenge();
        assert!(
            verifier.len() >= 43 && verifier.len() <= 128,
            "verifier length {} not in range 43..=128",
            verifier.len()
        );
        assert!(!challenge.is_empty(), "challenge should not be empty");
    }

    #[test]
    fn pkce_challenge_is_deterministic_for_verifier() {
        let verifier = "test-verifier-string-that-is-long-enough-43chars";
        let c1 = compute_s256_challenge(verifier);
        let c2 = compute_s256_challenge(verifier);
        assert_eq!(c1, c2);
    }

    #[test]
    fn pkce_challenge_is_url_safe_base64() {
        let (_, challenge) = generate_pkce_challenge();
        // URL-safe base64 no-pad: only [A-Za-z0-9_-]
        for ch in challenge.chars() {
            assert!(
                ch.is_ascii_alphanumeric() || ch == '_' || ch == '-',
                "unexpected character in challenge: {ch}"
            );
        }
    }

    #[test]
    fn pkce_verifier_is_url_safe() {
        let (verifier, _) = generate_pkce_challenge();
        for ch in verifier.chars() {
            assert!(
                ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' || ch == '~',
                "unexpected character in verifier: {ch}"
            );
        }
    }

    #[test]
    fn pkce_s256_known_value() {
        // RFC 7636 Appendix B test vector
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = compute_s256_challenge(verifier);
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    // ---- Auth URL tests ----

    #[test]
    fn build_auth_url_contains_required_params() {
        let config = test_config();
        let (_, challenge) = generate_pkce_challenge();
        let url = build_auth_url(&config, &challenge, "state-123");

        assert!(url.starts_with(DEFAULT_AUTH_ENDPOINT));
        assert!(url.contains("client_id="));
        assert!(url.contains("redirect_uri="));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("access_type=offline"));
        assert!(url.contains("prompt=consent"));
        assert!(url.contains("code_challenge="));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("state=state-123"));
        assert!(url.contains("scope="));
    }

    #[test]
    fn build_auth_url_encodes_special_characters() {
        let mut config = test_config();
        config.client_id = "client id with spaces".into();
        let url = build_auth_url(&config, "challenge", "state");
        // spaces should be percent-encoded
        assert!(url.contains("client%20id%20with%20spaces"));
        assert!(!url.contains("client id with spaces"));
    }

    // ---- Token state tests ----

    #[test]
    fn token_state_expired() {
        let ts = TokenState {
            access_token: "token".into(),
            refresh_token: "refresh".into(),
            expires_at: Utc::now() - Duration::seconds(10),
            scopes: vec![],
        };
        assert!(ts.is_expired());
    }

    #[test]
    fn token_state_not_expired() {
        let ts = TokenState {
            access_token: "token".into(),
            refresh_token: "refresh".into(),
            expires_at: Utc::now() + Duration::seconds(3600),
            scopes: vec![],
        };
        assert!(!ts.is_expired());
    }

    #[test]
    fn token_state_within_buffer() {
        // Expires in 30 seconds — should be considered expired (60s buffer)
        let ts = TokenState {
            access_token: "token".into(),
            refresh_token: "refresh".into(),
            expires_at: Utc::now() + Duration::seconds(30),
            scopes: vec![],
        };
        assert!(ts.is_expired());
    }

    #[test]
    fn client_set_token_state() {
        let mut client = GoogleCalendarClient::new(test_config());
        assert!(client.token_state().is_none());

        let ts = TokenState {
            access_token: "access".into(),
            refresh_token: "refresh".into(),
            expires_at: Utc::now() + Duration::seconds(3600),
            scopes: vec!["scope1".into()],
        };
        client.set_token_state(ts);
        assert!(client.token_state().is_some());
        assert_eq!(
            client.token_state().map(|t| t.access_token.as_str()),
            Some("access")
        );
    }

    // ---- Normalize tests ----

    #[test]
    fn normalize_google_event_timed() {
        let event = GoogleEvent {
            id: "google-event-001".into(),
            summary: Some("Google Meeting".into()),
            start: GoogleDateTime {
                date_time: Some("2026-03-21T10:00:00Z".into()),
                date: None,
                time_zone: None,
            },
            end: GoogleDateTime {
                date_time: Some("2026-03-21T11:00:00Z".into()),
                date: None,
                time_zone: None,
            },
            recurrence: vec![],
            attendees: vec![GoogleAttendee {
                email: "alice@example.com".into(),
                display_name: Some("Alice".into()),
            }],
            location: Some("Room B".into()),
            description: Some("Quarterly review".into()),
            created: Some("2026-03-01T00:00:00Z".into()),
            updated: Some("2026-03-15T12:00:00Z".into()),
            status: Some("confirmed".into()),
            html_link: Some("https://calendar.google.com/event?eid=abc".into()),
            creator: Some(GoogleEventCreator {
                email: Some("creator@example.com".into()),
                display_name: None,
                is_self: true,
            }),
            organizer: Some(GoogleEventOrganizer {
                email: Some("organizer@example.com".into()),
                display_name: None,
                is_self: false,
            }),
            color_id: Some("5".into()),
            reminders: None,
            conference_data: None,
            etag: Some("\"etag-123\"".into()),
        };

        let cal_event =
            GoogleCalendarClient::normalize_google_event(&event).expect("should normalize");
        assert_eq!(cal_event.title, "Google Meeting");
        assert_eq!(cal_event.source, "google-calendar");
        assert_eq!(cal_event.source_id, "google-event-001");
        assert_eq!(cal_event.attendees[0].email, "alice@example.com");
        assert_eq!(cal_event.location.as_deref(), Some("Room B"));
        assert_eq!(cal_event.description.as_deref(), Some("Quarterly review"));
        assert!(cal_event.recurrence.is_none());
        assert!(cal_event.start < cal_event.end.unwrap());

        // Verify extensions
        let extensions = cal_event
            .extensions
            .as_ref()
            .expect("should have extensions");
        let ns = extensions
            .get("com.life-engine.connector-calendar")
            .expect("should have namespace");
        assert_eq!(
            ns.get("google_event_id").and_then(|v| v.as_str()),
            Some("google-event-001")
        );
        assert_eq!(
            ns.get("html_link").and_then(|v| v.as_str()),
            Some("https://calendar.google.com/event?eid=abc")
        );
        assert_eq!(ns.get("status").and_then(|v| v.as_str()), Some("confirmed"));
        assert_eq!(ns.get("color_id").and_then(|v| v.as_str()), Some("5"));
        assert_eq!(
            ns.get("creator_email").and_then(|v| v.as_str()),
            Some("creator@example.com")
        );
        assert_eq!(
            ns.get("organizer_email").and_then(|v| v.as_str()),
            Some("organizer@example.com")
        );
        assert_eq!(
            ns.get("etag").and_then(|v| v.as_str()),
            Some("\"etag-123\"")
        );
    }

    #[test]
    fn normalize_google_event_all_day() {
        let event = GoogleEvent {
            id: "google-allday-001".into(),
            summary: Some("Company Holiday".into()),
            start: GoogleDateTime {
                date_time: None,
                date: Some("2026-03-25".into()),
                time_zone: None,
            },
            end: GoogleDateTime {
                date_time: None,
                date: Some("2026-03-26".into()),
                time_zone: None,
            },
            recurrence: vec![],
            attendees: vec![],
            location: None,
            description: None,
            created: None,
            updated: None,
            status: None,
            html_link: None,
            creator: None,
            organizer: None,
            color_id: None,
            reminders: None,
            conference_data: None,
            etag: None,
        };

        let cal_event =
            GoogleCalendarClient::normalize_google_event(&event).expect("should normalize");
        assert_eq!(cal_event.title, "Company Holiday");
        assert_eq!(cal_event.start.date_naive().to_string(), "2026-03-25");
        assert_eq!(
            cal_event.end.unwrap().date_naive().to_string(),
            "2026-03-26"
        );
    }

    #[test]
    fn normalize_google_event_with_recurrence() {
        let event = GoogleEvent {
            id: "google-recur-001".into(),
            summary: Some("Daily Standup".into()),
            start: GoogleDateTime {
                date_time: Some("2026-03-23T09:00:00Z".into()),
                date: None,
                time_zone: None,
            },
            end: GoogleDateTime {
                date_time: Some("2026-03-23T09:15:00Z".into()),
                date: None,
                time_zone: None,
            },
            recurrence: vec!["RRULE:FREQ=DAILY;COUNT=30".into()],
            attendees: vec![],
            location: None,
            description: None,
            created: None,
            updated: None,
            status: None,
            html_link: None,
            creator: None,
            organizer: None,
            color_id: None,
            reminders: None,
            conference_data: None,
            etag: None,
        };

        let cal_event =
            GoogleCalendarClient::normalize_google_event(&event).expect("should normalize");
        assert_eq!(
            cal_event
                .recurrence
                .as_ref()
                .map(|r| r.to_rrule())
                .as_deref(),
            Some("FREQ=DAILY;COUNT=30")
        );
    }

    #[test]
    fn normalize_google_event_no_title() {
        let event = GoogleEvent {
            id: "google-notitle-001".into(),
            summary: None,
            start: GoogleDateTime {
                date_time: Some("2026-03-21T10:00:00Z".into()),
                date: None,
                time_zone: None,
            },
            end: GoogleDateTime {
                date_time: Some("2026-03-21T11:00:00Z".into()),
                date: None,
                time_zone: None,
            },
            recurrence: vec![],
            attendees: vec![],
            location: None,
            description: None,
            created: None,
            updated: None,
            status: None,
            html_link: None,
            creator: None,
            organizer: None,
            color_id: None,
            reminders: None,
            conference_data: None,
            etag: None,
        };

        let cal_event =
            GoogleCalendarClient::normalize_google_event(&event).expect("should normalize");
        assert_eq!(cal_event.title, "(no title)");
    }

    #[test]
    fn normalize_google_event_with_conference_data() {
        let event = GoogleEvent {
            id: "google-conf-001".into(),
            summary: Some("Meet Call".into()),
            start: GoogleDateTime {
                date_time: Some("2026-03-21T10:00:00Z".into()),
                date: None,
                time_zone: None,
            },
            end: GoogleDateTime {
                date_time: Some("2026-03-21T11:00:00Z".into()),
                date: None,
                time_zone: None,
            },
            recurrence: vec![],
            attendees: vec![],
            location: None,
            description: None,
            created: None,
            updated: None,
            status: None,
            html_link: None,
            creator: None,
            organizer: None,
            color_id: None,
            reminders: None,
            conference_data: Some(serde_json::json!({
                "entryPoints": [
                    { "entryPointType": "video", "uri": "https://meet.google.com/abc-def-ghi" }
                ]
            })),
            etag: None,
        };

        let cal_event =
            GoogleCalendarClient::normalize_google_event(&event).expect("should normalize");
        let ext = cal_event.extensions.as_ref().expect("extensions");
        let ns = ext
            .get("com.life-engine.connector-calendar")
            .expect("namespace");
        assert_eq!(
            ns.get("conference_uri").and_then(|v| v.as_str()),
            Some("https://meet.google.com/abc-def-ghi")
        );
    }

    #[test]
    fn normalize_google_event_minimal_has_extensions() {
        // Even a minimal event should get google_event_id in extensions
        let event = GoogleEvent {
            id: "min-001".into(),
            summary: None,
            start: GoogleDateTime {
                date_time: Some("2026-03-21T10:00:00Z".into()),
                date: None,
                time_zone: None,
            },
            end: GoogleDateTime {
                date_time: Some("2026-03-21T11:00:00Z".into()),
                date: None,
                time_zone: None,
            },
            recurrence: vec![],
            attendees: vec![],
            location: None,
            description: None,
            created: None,
            updated: None,
            status: None,
            html_link: None,
            creator: None,
            organizer: None,
            color_id: None,
            reminders: None,
            conference_data: None,
            etag: None,
        };

        let cal_event =
            GoogleCalendarClient::normalize_google_event(&event).expect("should normalize");
        // google_event_id is always set, so extensions should be Some
        let ext = cal_event.extensions.as_ref().expect("extensions");
        let ns = ext
            .get("com.life-engine.connector-calendar")
            .expect("namespace");
        assert_eq!(
            ns.get("google_event_id").and_then(|v| v.as_str()),
            Some("min-001")
        );
    }

    #[test]
    fn parse_google_datetime_rfc3339() {
        let gdt = GoogleDateTime {
            date_time: Some("2026-03-21T10:00:00Z".into()),
            date: None,
            time_zone: None,
        };
        let dt = parse_google_datetime(&gdt).expect("should parse");
        assert_eq!(dt.to_rfc3339(), "2026-03-21T10:00:00+00:00");
    }

    #[test]
    fn parse_google_datetime_date_only() {
        let gdt = GoogleDateTime {
            date_time: None,
            date: Some("2026-03-25".into()),
            time_zone: None,
        };
        let dt = parse_google_datetime(&gdt).expect("should parse");
        assert_eq!(dt.date_naive().to_string(), "2026-03-25");
    }

    #[test]
    fn parse_google_datetime_neither_errors() {
        let gdt = GoogleDateTime {
            date_time: None,
            date: None,
            time_zone: None,
        };
        let result = parse_google_datetime(&gdt);
        assert!(result.is_err());
    }

    #[test]
    fn build_google_event_produces_correct_structure() {
        use chrono::{TimeZone, Utc};
        let event = CalendarEvent {
            id: Uuid::new_v4(),
            title: "CDM Event".into(),
            start: Utc.with_ymd_and_hms(2026, 3, 21, 10, 0, 0).unwrap(),
            end: Some(Utc.with_ymd_and_hms(2026, 3, 21, 11, 0, 0).unwrap()),
            recurrence: Recurrence::from_rrule("FREQ=DAILY;COUNT=5"),
            attendees: vec![Attendee::from_email("bob@example.com")],
            location: Some("Office".into()),
            description: Some("A meeting".into()),
            source: "google-calendar".into(),
            source_id: "google-123".into(),
            extensions: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            all_day: None,
            reminders: vec![],
            timezone: None,
            status: None,
        };
        let ge = build_google_event(&event);
        assert_eq!(ge.id, "google-123");
        assert_eq!(ge.summary.as_deref(), Some("CDM Event"));
        assert!(ge.start.date_time.is_some());
        assert!(ge.end.date_time.is_some());
        assert_eq!(ge.recurrence, vec!["RRULE:FREQ=DAILY;COUNT=5"]);
        assert_eq!(ge.attendees.len(), 1);
        assert_eq!(ge.attendees[0].email, "bob@example.com");
        assert_eq!(ge.location.as_deref(), Some("Office"));
        assert_eq!(ge.description.as_deref(), Some("A meeting"));
    }

    #[test]
    fn google_event_serialization() {
        let event = GoogleEvent {
            id: "test-id".into(),
            summary: Some("Test".into()),
            start: GoogleDateTime {
                date_time: Some("2026-03-21T10:00:00Z".into()),
                date: None,
                time_zone: None,
            },
            end: GoogleDateTime {
                date_time: Some("2026-03-21T11:00:00Z".into()),
                date: None,
                time_zone: None,
            },
            recurrence: vec![],
            attendees: vec![],
            location: None,
            description: None,
            created: None,
            updated: None,
            status: None,
            html_link: None,
            creator: None,
            organizer: None,
            color_id: None,
            reminders: None,
            conference_data: None,
            etag: None,
        };

        let json = serde_json::to_string(&event).expect("serialize");
        let restored: GoogleEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.id, "test-id");
        assert_eq!(restored.summary.as_deref(), Some("Test"));
    }

    #[test]
    fn google_event_camel_case_serialization() {
        let event = GoogleEvent {
            id: "camel-test".into(),
            summary: Some("CamelCase".into()),
            start: GoogleDateTime {
                date_time: Some("2026-03-21T10:00:00Z".into()),
                date: None,
                time_zone: None,
            },
            end: GoogleDateTime {
                date_time: Some("2026-03-21T11:00:00Z".into()),
                date: None,
                time_zone: None,
            },
            recurrence: vec![],
            attendees: vec![],
            location: None,
            description: None,
            created: None,
            updated: None,
            status: Some("confirmed".into()),
            html_link: Some("https://example.com".into()),
            creator: Some(GoogleEventCreator {
                email: Some("creator@test.com".into()),
                display_name: None,
                is_self: false,
            }),
            organizer: None,
            color_id: Some("1".into()),
            reminders: None,
            conference_data: None,
            etag: Some("\"etag-abc\"".into()),
        };

        let json = serde_json::to_string(&event).expect("serialize");
        // Verify camelCase keys in JSON
        assert!(
            json.contains("\"htmlLink\""),
            "should use camelCase htmlLink"
        );
        assert!(json.contains("\"colorId\""), "should use camelCase colorId");
        assert!(
            json.contains("\"conferenceData\"") || !json.contains("conference_data"),
            "should use camelCase conferenceData or omit"
        );
    }

    #[test]
    fn google_event_deserialize_from_api_format() {
        let json = r#"{
            "id": "api-event-1",
            "summary": "API Event",
            "start": { "dateTime": "2026-03-21T10:00:00Z" },
            "end": { "dateTime": "2026-03-21T11:00:00Z" },
            "status": "confirmed",
            "htmlLink": "https://calendar.google.com/event?eid=abc",
            "creator": { "email": "user@example.com", "self": true },
            "organizer": { "email": "org@example.com" },
            "colorId": "5",
            "etag": "\"etag-from-api\""
        }"#;

        let event: GoogleEvent = serde_json::from_str(json).expect("deserialize");
        assert_eq!(event.id, "api-event-1");
        assert_eq!(event.status.as_deref(), Some("confirmed"));
        assert_eq!(
            event.html_link.as_deref(),
            Some("https://calendar.google.com/event?eid=abc")
        );
        assert!(event.creator.as_ref().expect("creator").is_self);
        assert_eq!(
            event.organizer.as_ref().and_then(|o| o.email.as_deref()),
            Some("org@example.com")
        );
        assert_eq!(event.color_id.as_deref(), Some("5"));
        assert_eq!(event.etag.as_deref(), Some("\"etag-from-api\""));
    }

    #[test]
    fn google_calendar_list_response_deserialization() {
        let json = r#"{
            "items": [
                {
                    "id": "primary",
                    "summary": "My Calendar",
                    "timeZone": "America/New_York",
                    "primary": true,
                    "accessRole": "owner"
                }
            ]
        }"#;

        let response: GoogleCalendarListResponse = serde_json::from_str(json).expect("deserialize");
        assert_eq!(response.items.len(), 1);
        assert_eq!(response.items[0].id, "primary");
        assert_eq!(response.items[0].summary.as_deref(), Some("My Calendar"));
        assert!(response.items[0].primary);
        assert_eq!(response.items[0].access_role.as_deref(), Some("owner"));
    }

    #[test]
    fn google_events_list_response_deserialization() {
        let json = r#"{
            "items": [
                {
                    "id": "evt-1",
                    "summary": "Test Event",
                    "start": { "dateTime": "2026-03-21T10:00:00Z" },
                    "end": { "dateTime": "2026-03-21T11:00:00Z" }
                }
            ],
            "nextSyncToken": "sync-token-xyz"
        }"#;

        let response: GoogleEventsListResponse = serde_json::from_str(json).expect("deserialize");
        assert_eq!(response.items.len(), 1);
        assert_eq!(response.items[0].id, "evt-1");
        assert_eq!(response.next_sync_token.as_deref(), Some("sync-token-xyz"));
        assert!(response.next_page_token.is_none());
    }

    #[test]
    fn google_events_list_response_with_pagination() {
        let json = r#"{
            "items": [],
            "nextPageToken": "page-2"
        }"#;

        let response: GoogleEventsListResponse = serde_json::from_str(json).expect("deserialize");
        assert!(response.items.is_empty());
        assert_eq!(response.next_page_token.as_deref(), Some("page-2"));
        assert!(response.next_sync_token.is_none());
    }

    // ---- Error type tests ----

    #[test]
    fn google_api_error_display() {
        let err = GoogleApiError::AuthExpired;
        assert_eq!(err.to_string(), "access token expired");

        let err = GoogleApiError::RateLimited {
            retry_after_secs: Some(30),
        };
        assert!(err.to_string().contains("rate limited"));

        let err = GoogleApiError::SyncTokenExpired;
        assert!(err.to_string().contains("410 Gone"));

        let err = GoogleApiError::NotFound("test-url".into());
        assert!(err.to_string().contains("not found"));

        let err = GoogleApiError::PermissionDenied("test".into());
        assert!(err.to_string().contains("permission denied"));

        let err = GoogleApiError::ApiError {
            status: 500,
            body: "server error".into(),
        };
        assert!(err.to_string().contains("500"));
    }

    #[test]
    fn google_api_error_token_refresh_failed() {
        let err = GoogleApiError::TokenRefreshFailed("network error".into());
        assert!(err.to_string().contains("token refresh failed"));
        assert!(err.to_string().contains("network error"));
    }

    // ---- URL encoding tests ----

    #[test]
    fn urlencoding_basic() {
        assert_eq!(urlencoding::encode("hello"), "hello");
        assert_eq!(urlencoding::encode("hello world"), "hello%20world");
        assert_eq!(urlencoding::encode("a@b.com"), "a%40b.com");
    }

    #[test]
    fn urlencoding_preserves_unreserved() {
        let unreserved = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
        assert_eq!(urlencoding::encode(unreserved), unreserved);
    }

    // ---- Timezone offset and parse tests ----

    #[test]
    fn normalize_google_event_timezone_offset() {
        let event = minimal_google_event("tz-offset-001");
        let event = GoogleEvent {
            start: GoogleDateTime {
                date_time: Some("2026-03-21T10:00:00+05:30".into()),
                date: None,
                time_zone: None,
            },
            end: GoogleDateTime {
                date_time: Some("2026-03-21T11:00:00+05:30".into()),
                date: None,
                time_zone: None,
            },
            ..event
        };

        let cal_event =
            GoogleCalendarClient::normalize_google_event(&event).expect("should normalize");
        // +05:30 means UTC is 5h30m earlier
        assert_eq!(cal_event.start.to_rfc3339(), "2026-03-21T04:30:00+00:00");
        assert_eq!(
            cal_event.end.unwrap().to_rfc3339(),
            "2026-03-21T05:30:00+00:00"
        );
    }

    #[test]
    fn normalize_google_event_multiple_recurrence_rules() {
        let event = GoogleEvent {
            recurrence: vec![
                "RRULE:FREQ=WEEKLY;BYDAY=MO".into(),
                "EXDATE:20260401T100000Z".into(),
                "RDATE:20260415T100000Z".into(),
            ],
            ..minimal_google_event("multi-recur-001")
        };

        let cal_event =
            GoogleCalendarClient::normalize_google_event(&event).expect("should normalize");
        // Only the RRULE component is parsed into Recurrence; EXDATE/RDATE are separate iCal properties
        assert_eq!(
            cal_event
                .recurrence
                .as_ref()
                .map(|r| r.to_rrule())
                .as_deref(),
            Some("FREQ=WEEKLY;BYDAY=MO")
        );
    }

    #[test]
    fn normalize_google_event_multiple_attendees() {
        let event = GoogleEvent {
            attendees: vec![
                GoogleAttendee {
                    email: "alice@example.com".into(),
                    display_name: Some("Alice Smith".into()),
                },
                GoogleAttendee {
                    email: "bob@example.com".into(),
                    display_name: None,
                },
                GoogleAttendee {
                    email: "carol@example.com".into(),
                    display_name: Some("Carol".into()),
                },
                GoogleAttendee {
                    email: "dave@example.com".into(),
                    display_name: None,
                },
            ],
            ..minimal_google_event("multi-att-001")
        };

        let cal_event =
            GoogleCalendarClient::normalize_google_event(&event).expect("should normalize");
        assert_eq!(cal_event.attendees.len(), 4);
        assert_eq!(cal_event.attendees[0].email, "alice@example.com");
        assert_eq!(cal_event.attendees[1].email, "bob@example.com");
        assert_eq!(cal_event.attendees[2].email, "carol@example.com");
        assert_eq!(cal_event.attendees[3].email, "dave@example.com");
    }

    #[test]
    fn normalize_google_event_empty_strings() {
        let event = GoogleEvent {
            summary: Some(String::new()),
            location: Some(String::new()),
            description: Some(String::new()),
            ..minimal_google_event("empty-str-001")
        };

        let cal_event =
            GoogleCalendarClient::normalize_google_event(&event).expect("should normalize");
        // Empty string summary is preserved (not replaced with "(no title)")
        assert_eq!(cal_event.title, "");
        assert_eq!(cal_event.location.as_deref(), Some(""));
        assert_eq!(cal_event.description.as_deref(), Some(""));
    }

    #[test]
    fn normalize_google_event_malformed_start_time() {
        let event = GoogleEvent {
            start: GoogleDateTime {
                date_time: Some("not-a-valid-datetime".into()),
                date: None,
                time_zone: None,
            },
            ..minimal_google_event("bad-start-001")
        };

        let result = GoogleCalendarClient::normalize_google_event(&event);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("not-a-valid-datetime"));
    }

    #[test]
    fn normalize_google_event_malformed_end_time() {
        let event = GoogleEvent {
            end: GoogleDateTime {
                date_time: Some("also-not-valid".into()),
                date: None,
                time_zone: None,
            },
            ..minimal_google_event("bad-end-001")
        };

        let result = GoogleCalendarClient::normalize_google_event(&event);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("also-not-valid"));
    }

    #[test]
    fn normalize_google_event_neither_datetime_nor_date() {
        let event = GoogleEvent {
            start: GoogleDateTime {
                date_time: None,
                date: None,
                time_zone: None,
            },
            ..minimal_google_event("no-dt-001")
        };

        let result = GoogleCalendarClient::normalize_google_event(&event);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("neither"),
            "error should be descriptive, got: {err_msg}"
        );
    }

    #[test]
    fn parse_google_datetime_timezone_offset() {
        let gdt = GoogleDateTime {
            date_time: Some("2026-06-15T14:30:00+05:30".into()),
            date: None,
            time_zone: None,
        };
        let dt = parse_google_datetime(&gdt).expect("should parse");
        assert_eq!(dt.to_rfc3339(), "2026-06-15T09:00:00+00:00");
    }

    #[test]
    fn parse_google_datetime_negative_offset() {
        let gdt = GoogleDateTime {
            date_time: Some("2026-12-01T09:00:00-08:00".into()),
            date: None,
            time_zone: None,
        };
        let dt = parse_google_datetime(&gdt).expect("should parse");
        // -08:00 means UTC is 8h later: 09:00 PST = 17:00 UTC
        assert_eq!(dt.to_rfc3339(), "2026-12-01T17:00:00+00:00");
    }

    #[test]
    fn parse_google_datetime_malformed() {
        let gdt = GoogleDateTime {
            date_time: Some("garbage-not-a-date".into()),
            date: None,
            time_zone: None,
        };
        let result = parse_google_datetime(&gdt);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("garbage-not-a-date"));
    }

    // ---- JSON deserialization tests ----

    #[test]
    fn google_event_deserialization_from_api_json() {
        let json = r#"{
            "id": "full-api-event-123",
            "summary": "Team Standup",
            "start": {
                "dateTime": "2026-04-01T09:00:00-07:00",
                "timeZone": "America/Los_Angeles"
            },
            "end": {
                "dateTime": "2026-04-01T09:30:00-07:00",
                "timeZone": "America/Los_Angeles"
            },
            "recurrence": ["RRULE:FREQ=DAILY;COUNT=5"],
            "attendees": [
                { "email": "alice@corp.com", "displayName": "Alice" },
                { "email": "bob@corp.com" }
            ],
            "location": "Conference Room A",
            "description": "Daily standup meeting",
            "created": "2026-03-01T00:00:00.000Z",
            "updated": "2026-03-28T12:00:00.000Z",
            "status": "confirmed",
            "htmlLink": "https://calendar.google.com/event?eid=xyz",
            "creator": { "email": "alice@corp.com", "displayName": "Alice", "self": true },
            "organizer": { "email": "alice@corp.com", "displayName": "Alice", "self": true },
            "colorId": "9",
            "etag": "\"3456789012345000\""
        }"#;

        let event: GoogleEvent = serde_json::from_str(json).expect("deserialize");
        assert_eq!(event.id, "full-api-event-123");
        assert_eq!(event.summary.as_deref(), Some("Team Standup"));
        assert_eq!(
            event.start.date_time.as_deref(),
            Some("2026-04-01T09:00:00-07:00")
        );
        assert_eq!(
            event.start.time_zone.as_deref(),
            Some("America/Los_Angeles")
        );
        assert_eq!(event.recurrence, vec!["RRULE:FREQ=DAILY;COUNT=5"]);
        assert_eq!(event.attendees.len(), 2);
        assert_eq!(event.attendees[0].email, "alice@corp.com");
        assert_eq!(event.attendees[0].display_name.as_deref(), Some("Alice"));
        assert_eq!(event.attendees[1].email, "bob@corp.com");
        assert!(event.attendees[1].display_name.is_none());
        assert_eq!(event.location.as_deref(), Some("Conference Room A"));
        assert_eq!(event.description.as_deref(), Some("Daily standup meeting"));
        assert_eq!(event.status.as_deref(), Some("confirmed"));
        assert_eq!(
            event.html_link.as_deref(),
            Some("https://calendar.google.com/event?eid=xyz")
        );
        assert_eq!(event.color_id.as_deref(), Some("9"));
        assert!(event.creator.as_ref().unwrap().is_self);
    }

    #[test]
    fn google_event_deserialization_missing_optional_fields() {
        let json = r#"{
            "id": "minimal-api-001",
            "start": { "date": "2026-05-01" },
            "end": { "date": "2026-05-02" }
        }"#;

        let event: GoogleEvent = serde_json::from_str(json).expect("deserialize");
        assert_eq!(event.id, "minimal-api-001");
        assert!(event.summary.is_none());
        assert!(event.recurrence.is_empty());
        assert!(event.attendees.is_empty());
        assert!(event.location.is_none());
        assert!(event.description.is_none());
        assert!(event.created.is_none());
        assert!(event.updated.is_none());
        assert!(event.status.is_none());
        assert!(event.html_link.is_none());
        assert!(event.creator.is_none());
        assert!(event.organizer.is_none());
        assert!(event.color_id.is_none());
        assert!(event.reminders.is_none());
        assert!(event.conference_data.is_none());
        assert!(event.etag.is_none());
    }

    #[test]
    fn google_attendee_deserialization() {
        let json = r#"{ "email": "test@example.com", "displayName": "Test User" }"#;
        let attendee: GoogleAttendee = serde_json::from_str(json).expect("deserialize");
        assert_eq!(attendee.email, "test@example.com");
        assert_eq!(attendee.display_name.as_deref(), Some("Test User"));
    }

    #[test]
    fn google_datetime_deserialization_timed() {
        let json = r#"{ "dateTime": "2026-07-01T15:00:00+02:00", "timeZone": "Europe/Berlin" }"#;
        let gdt: GoogleDateTime = serde_json::from_str(json).expect("deserialize");
        assert_eq!(gdt.date_time.as_deref(), Some("2026-07-01T15:00:00+02:00"));
        assert_eq!(gdt.time_zone.as_deref(), Some("Europe/Berlin"));
        assert!(gdt.date.is_none());
    }

    #[test]
    fn google_datetime_deserialization_all_day() {
        let json = r#"{ "date": "2026-12-25" }"#;
        let gdt: GoogleDateTime = serde_json::from_str(json).expect("deserialize");
        assert_eq!(gdt.date.as_deref(), Some("2026-12-25"));
        assert!(gdt.date_time.is_none());
        assert!(gdt.time_zone.is_none());
    }

    // ---- Source metadata and ID uniqueness tests ----

    #[test]
    fn normalize_preserves_source_metadata() {
        let event = minimal_google_event("source-meta-001");
        let cal_event =
            GoogleCalendarClient::normalize_google_event(&event).expect("should normalize");
        assert_eq!(cal_event.source, "google-calendar");
        assert_eq!(cal_event.source_id, "source-meta-001");
    }

    #[test]
    fn normalize_generates_unique_ids() {
        let event = minimal_google_event("same-id-001");
        let cal_event_1 =
            GoogleCalendarClient::normalize_google_event(&event).expect("first normalize");
        let cal_event_2 =
            GoogleCalendarClient::normalize_google_event(&event).expect("second normalize");
        assert_ne!(
            cal_event_1.id, cal_event_2.id,
            "each normalization should produce a unique UUID"
        );
        // But source_id should be the same
        assert_eq!(cal_event_1.source_id, cal_event_2.source_id);
    }

    /// Build a minimal valid GoogleEvent for use in tests.
    fn minimal_google_event(id: &str) -> GoogleEvent {
        GoogleEvent {
            id: id.into(),
            summary: Some("Test Event".into()),
            start: GoogleDateTime {
                date_time: Some("2026-03-21T10:00:00Z".into()),
                date: None,
                time_zone: None,
            },
            end: GoogleDateTime {
                date_time: Some("2026-03-21T11:00:00Z".into()),
                date: None,
                time_zone: None,
            },
            recurrence: vec![],
            attendees: vec![],
            location: None,
            description: None,
            created: None,
            updated: None,
            status: None,
            html_link: None,
            creator: None,
            organizer: None,
            color_id: None,
            reminders: None,
            conference_data: None,
            etag: None,
        }
    }

    // ---- Creator/Organizer type tests ----

    #[test]
    fn google_event_creator_serialization() {
        let creator = GoogleEventCreator {
            email: Some("user@example.com".into()),
            display_name: Some("User".into()),
            is_self: true,
        };
        let json = serde_json::to_string(&creator).expect("serialize");
        let restored: GoogleEventCreator = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.email.as_deref(), Some("user@example.com"));
        assert!(restored.is_self);
    }

    #[test]
    fn google_event_organizer_serialization() {
        let organizer = GoogleEventOrganizer {
            email: Some("org@example.com".into()),
            display_name: None,
            is_self: false,
        };
        let json = serde_json::to_string(&organizer).expect("serialize");
        let restored: GoogleEventOrganizer = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.email.as_deref(), Some("org@example.com"));
        assert!(!restored.is_self);
    }
}
