//! CalDAV and Google Calendar connector plugin for Life Engine Core.
//!
//! This connector plugin implements both `CorePlugin` (for lifecycle
//! management and route registration) and the connector pattern (for
//! syncing calendar events from external CalDAV servers and Google
//! Calendar).
//!
//! # Architecture
//!
//! - `caldav` — CalDAV client with Basic Auth, sync-token/ctag tracking
//! - `google` — Google Calendar API v3 client (stub, behind `integration` feature)
//! - `normalizer` — iCal VEVENT to CDM `CalendarEvent` type conversion

pub mod caldav;
pub mod config;
pub mod error;
pub mod google;
pub mod normalizer;
pub mod steps;
pub mod transform;
pub mod types;

use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use life_engine_plugin_sdk::prelude::*;
use life_engine_plugin_sdk::retry::RetryState;
use life_engine_plugin_sdk::types::Capability;

use crate::caldav::{CalDavClient, CalDavConfig};
use crate::google::{GoogleCalendarClient, GoogleCalendarConfig};

/// The calendar connector plugin.
///
/// Manages CalDAV and Google Calendar clients for syncing calendar
/// events. Credentials are stored in memory (Phase 1); persistent
/// credential storage comes in a later phase.
pub struct CalendarConnectorPlugin {
    /// The CalDAV client, initialised after credentials are provided.
    caldav: Option<CalDavClient>,
    /// The Google Calendar client, initialised after credentials are provided.
    google: Option<GoogleCalendarClient>,
    /// Interval between automatic sync operations.
    sync_interval: Duration,
    /// Timestamp of the last successful sync.
    last_sync: Option<DateTime<Utc>>,
    /// PKCE code verifier for in-flight OAuth2 flows.
    pkce_verifier: Option<String>,
    /// Retry state for exponential backoff on transient sync failures.
    retry_state: RetryState,
}

impl CalendarConnectorPlugin {
    /// Create a new calendar connector plugin with default settings.
    pub fn new() -> Self {
        Self {
            caldav: None,
            google: None,
            sync_interval: Duration::from_secs(300), // 5 minutes
            last_sync: None,
            pkce_verifier: None,
            retry_state: RetryState::with_config(5, 60, 3600).with_jitter(true),
        }
    }

    /// Create a new calendar connector with a custom sync interval.
    pub fn with_sync_interval(sync_interval: Duration) -> Self {
        Self {
            sync_interval,
            ..Self::new()
        }
    }

    /// Configure the CalDAV client with the given settings.
    pub fn configure_caldav(&mut self, config: CalDavConfig) {
        self.caldav = Some(CalDavClient::new(config));
    }

    /// Configure the Google Calendar client with the given settings.
    pub fn configure_google(&mut self, config: GoogleCalendarConfig) {
        self.google = Some(GoogleCalendarClient::new(config));
    }

    /// Returns whether CalDAV is configured.
    pub fn has_caldav(&self) -> bool {
        self.caldav.is_some()
    }

    /// Returns whether Google Calendar is configured.
    pub fn has_google(&self) -> bool {
        self.google.is_some()
    }

    /// Returns the configured sync interval.
    pub fn sync_interval(&self) -> Duration {
        self.sync_interval
    }

    /// Returns the timestamp of the last successful sync.
    pub fn last_sync(&self) -> Option<DateTime<Utc>> {
        self.last_sync
    }

    /// Returns a reference to the CalDAV client, if configured.
    pub fn caldav_client(&self) -> Option<&CalDavClient> {
        self.caldav.as_ref()
    }

    /// Returns a mutable reference to the CalDAV client, if configured.
    pub fn caldav_client_mut(&mut self) -> Option<&mut CalDavClient> {
        self.caldav.as_mut()
    }

    /// Returns a reference to the Google Calendar client, if configured.
    pub fn google_client(&self) -> Option<&GoogleCalendarClient> {
        self.google.as_ref()
    }

    /// Returns a mutable reference to the Google Calendar client, if configured.
    pub fn google_client_mut(&mut self) -> Option<&mut GoogleCalendarClient> {
        self.google.as_mut()
    }

    /// Generate a Google OAuth2 authorization URL with PKCE.
    ///
    /// Stores the PKCE verifier internally for use during the callback.
    /// Returns the URL the user should visit to authorize the application.
    pub fn google_auth_url(&mut self) -> Option<String> {
        let config = self.google.as_ref()?.config().clone();
        let (verifier, challenge) = crate::google::generate_pkce_challenge();
        let state = uuid::Uuid::new_v4().to_string();
        let url = crate::google::build_auth_url(&config, &challenge, &state);
        self.pkce_verifier = Some(verifier);
        Some(url)
    }

    /// Returns the stored PKCE verifier, if an auth flow is in progress.
    pub fn pkce_verifier(&self) -> Option<&str> {
        self.pkce_verifier.as_deref()
    }

    /// Exchange a Google OAuth2 authorization code for tokens.
    ///
    /// Uses the stored PKCE verifier from the auth URL generation step.
    /// On success, stores the token state in the Google client.
    #[cfg(feature = "integration")]
    pub async fn google_exchange_code(
        &mut self,
        auth_code: &str,
        client_secret: &str,
    ) -> anyhow::Result<()> {
        let verifier = self.pkce_verifier.take().ok_or_else(|| {
            anyhow::anyhow!("no PKCE verifier found — call google_auth_url first")
        })?;

        let config = self
            .google
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Google Calendar not configured"))?
            .config()
            .clone();

        let token_state =
            crate::google::exchange_code(&config, auth_code, &verifier, client_secret)
                .await
                .map_err(|e| anyhow::anyhow!("code exchange failed: {e}"))?;

        if let Some(ref mut client) = self.google {
            client.set_token_state(token_state);
        }

        Ok(())
    }

    /// Perform a Google Calendar sync for the given calendar ID.
    ///
    /// Returns the list of synced calendar events.
    ///
    /// The `client_secret` must be retrieved from the credential store
    /// using the config's `client_secret_key`.
    #[cfg(feature = "integration")]
    pub async fn google_sync(
        &mut self,
        calendar_id: &str,
        client_secret: &str,
    ) -> anyhow::Result<Vec<life_engine_types::CalendarEvent>> {
        let client = self
            .google
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Google Calendar not configured"))?;

        let events = client
            .sync_events(calendar_id, client_secret)
            .await
            .map_err(|e| anyhow::anyhow!("Google sync failed: {e}"))?;

        self.last_sync = Some(Utc::now());
        Ok(events)
    }
}

impl Default for CalendarConnectorPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for CalendarConnectorPlugin {
    fn id(&self) -> &str {
        "com.life-engine.connector-calendar"
    }

    fn display_name(&self) -> &str {
        "Calendar Connector"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn actions(&self) -> Vec<Action> {
        vec![
            Action::new("sync", "Sync calendar events from configured providers"),
            Action::new("status", "Get the current sync status"),
            Action::new("calendars", "List available calendars"),
            Action::new("google_auth", "Initiate Google OAuth2 authorization flow"),
            Action::new("google_callback", "Handle Google OAuth2 callback"),
        ]
    }

    fn execute(
        &self,
        action: &str,
        input: PipelineMessage,
    ) -> std::result::Result<PipelineMessage, Box<dyn EngineError>> {
        match action {
            "sync" | "status" | "calendars" | "google_auth" | "google_callback" => Ok(input),
            other => Err(Box::new(
                crate::error::CalendarConnectorError::UnknownAction(other.to_string()),
            )),
        }
    }
}

life_engine_plugin_sdk::register_plugin!(CalendarConnectorPlugin);

#[async_trait]
impl CorePlugin for CalendarConnectorPlugin {
    fn id(&self) -> &str {
        "com.life-engine.connector-calendar"
    }

    fn display_name(&self) -> &str {
        "Calendar Connector"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![
            Capability::StorageRead,
            Capability::StorageWrite,
            Capability::HttpOutbound,
            Capability::CredentialsRead,
            Capability::CredentialsWrite,
        ]
    }

    async fn on_load(&mut self, ctx: &PluginContext) -> Result<()> {
        tracing::info!(
            plugin_id = ctx.plugin_id(),
            "calendar connector plugin loaded"
        );
        Ok(())
    }

    async fn on_unload(&mut self) -> Result<()> {
        self.caldav = None;
        self.google = None;
        self.last_sync = None;
        tracing::info!("calendar connector plugin unloaded");
        Ok(())
    }

    fn routes(&self) -> Vec<PluginRoute> {
        vec![
            PluginRoute {
                method: HttpMethod::Post,
                path: "/sync".into(),
            },
            PluginRoute {
                method: HttpMethod::Get,
                path: "/status".into(),
            },
            PluginRoute {
                method: HttpMethod::Get,
                path: "/calendars".into(),
            },
            PluginRoute {
                method: HttpMethod::Get,
                path: "/google/auth".into(),
            },
            PluginRoute {
                method: HttpMethod::Get,
                path: "/google/callback".into(),
            },
        ]
    }

    async fn handle_event(&self, event: &CoreEvent) -> Result<()> {
        // Only handle events for the 'events' collection
        let collection = event
            .payload
            .get("collection")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if collection != "events" {
            return Ok(());
        }

        match event.event_type.as_str() {
            "data.created" | "data.updated" => {
                if let Some(data) = event.payload.get("data") {
                    let source = data
                        .get("source")
                        .and_then(|v| v.as_str())
                        .unwrap_or("local");
                    tracing::info!(
                        event_type = %event.event_type,
                        source = %source,
                        "outbound sync: forwarding event to provider"
                    );
                    match source {
                        "caldav" => {
                            if let Some(ref _client) = self.caldav {
                                if let Ok(cal_event) =
                                    serde_json::from_value::<life_engine_types::CalendarEvent>(
                                        data.clone(),
                                    )
                                {
                                    let ical = crate::caldav::build_vevent_ical(&cal_event);
                                    tracing::debug!(
                                        ical_len = ical.len(),
                                        "built VEVENT for CalDAV push"
                                    );
                                }
                            }
                        }
                        "google-calendar" => {
                            if let Some(ref _client) = self.google {
                                if let Ok(cal_event) =
                                    serde_json::from_value::<life_engine_types::CalendarEvent>(
                                        data.clone(),
                                    )
                                {
                                    let google_event =
                                        crate::google::build_google_event(&cal_event);
                                    tracing::debug!(google_event_id = %google_event.id, "built Google event for push");
                                }
                            }
                        }
                        _ => {
                            tracing::debug!(source = %source, "no outbound provider for source");
                        }
                    }
                }
            }
            "data.deleted" => {
                if let Some(id) = event.payload.get("id").and_then(|v| v.as_str()) {
                    let source = event
                        .payload
                        .get("source")
                        .and_then(|v| v.as_str())
                        .unwrap_or("local");
                    tracing::info!(
                        event_type = %event.event_type,
                        id = %id,
                        source = %source,
                        "outbound sync: deleting event from provider"
                    );
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_id_is_correct() {
        let plugin = CalendarConnectorPlugin::new();
        assert_eq!(
            CorePlugin::id(&plugin),
            "com.life-engine.connector-calendar"
        );
    }

    #[test]
    fn plugin_display_name() {
        let plugin = CalendarConnectorPlugin::new();
        assert_eq!(CorePlugin::display_name(&plugin), "Calendar Connector");
    }

    #[test]
    fn plugin_version() {
        let plugin = CalendarConnectorPlugin::new();
        assert_eq!(CorePlugin::version(&plugin), "0.1.0");
    }

    #[test]
    fn plugin_capabilities() {
        let plugin = CalendarConnectorPlugin::new();
        use life_engine_test_utils::assert_plugin_capabilities;
        assert_plugin_capabilities!(
            plugin,
            [
                Capability::StorageRead,
                Capability::StorageWrite,
                Capability::HttpOutbound,
                Capability::CredentialsRead,
                Capability::CredentialsWrite,
            ]
        );
    }

    #[test]
    fn plugin_routes() {
        use life_engine_test_utils::assert_plugin_routes;
        let plugin = CalendarConnectorPlugin::new();
        assert_plugin_routes!(
            plugin,
            [
                "/sync",
                "/status",
                "/calendars",
                "/google/auth",
                "/google/callback"
            ]
        );
    }

    #[tokio::test]
    async fn plugin_lifecycle() {
        let mut plugin = CalendarConnectorPlugin::new();
        assert!(!plugin.has_caldav());
        assert!(!plugin.has_google());

        let ctx = PluginContext::new(CorePlugin::id(&plugin));
        plugin.on_load(&ctx).await.expect("on_load should succeed");

        // Configure clients
        plugin.configure_caldav(CalDavConfig {
            server_url: "http://localhost:5232".into(),
            username: "user".into(),
            credential_key: "caldav_password".into(),
            calendar_path: "/user/calendar/".into(),
        });
        plugin.configure_google(GoogleCalendarConfig {
            client_id: "client-id".into(),
            client_secret_key: "google_cal_client_secret".into(),
            refresh_token_key: "google_cal_refresh_token".into(),
            redirect_uri: "http://localhost:3750/callback".into(),
            auth_endpoint: crate::google::DEFAULT_AUTH_ENDPOINT.to_string(),
            token_endpoint: crate::google::DEFAULT_TOKEN_ENDPOINT.to_string(),
        });
        assert!(plugin.has_caldav());
        assert!(plugin.has_google());

        // Unload clears clients
        plugin.on_unload().await.expect("on_unload should succeed");
        assert!(!plugin.has_caldav());
        assert!(!plugin.has_google());
        assert!(plugin.last_sync().is_none());
    }

    #[tokio::test]
    async fn handle_event_returns_ok() {
        let plugin = CalendarConnectorPlugin::new();
        life_engine_test_utils::plugin_test_helpers::test_handle_event_ok(&plugin).await;
    }

    #[test]
    fn default_sync_interval() {
        let plugin = CalendarConnectorPlugin::new();
        assert_eq!(plugin.sync_interval(), Duration::from_secs(300));
    }

    #[test]
    fn custom_sync_interval() {
        let plugin = CalendarConnectorPlugin::with_sync_interval(Duration::from_secs(60));
        assert_eq!(plugin.sync_interval(), Duration::from_secs(60));
    }

    #[test]
    fn default_impl() {
        let plugin = CalendarConnectorPlugin::default();
        assert_eq!(
            CorePlugin::id(&plugin),
            "com.life-engine.connector-calendar"
        );
    }

    #[tokio::test]
    async fn handle_event_dispatches_data_created() {
        let plugin = CalendarConnectorPlugin::new();
        let event = CoreEvent {
            event_type: "data.created".into(),
            payload: serde_json::json!({
                "collection": "events",
                "data": { "source": "caldav", "title": "Test" }
            }),
            source_plugin: "core".into(),
            timestamp: Utc::now(),
        };
        let result = plugin.handle_event(&event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn handle_event_dispatches_data_updated() {
        let plugin = CalendarConnectorPlugin::new();
        let event = CoreEvent {
            event_type: "data.updated".into(),
            payload: serde_json::json!({
                "collection": "events",
                "data": { "source": "google-calendar", "title": "Updated" }
            }),
            source_plugin: "core".into(),
            timestamp: Utc::now(),
        };
        let result = plugin.handle_event(&event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn handle_event_dispatches_data_deleted() {
        let plugin = CalendarConnectorPlugin::new();
        let event = CoreEvent {
            event_type: "data.deleted".into(),
            payload: serde_json::json!({
                "collection": "events",
                "id": "evt-123",
                "source": "caldav"
            }),
            source_plugin: "core".into(),
            timestamp: Utc::now(),
        };
        let result = plugin.handle_event(&event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn handle_event_ignores_non_event_collections() {
        let plugin = CalendarConnectorPlugin::new();
        let event = CoreEvent {
            event_type: "data.created".into(),
            payload: serde_json::json!({
                "collection": "tasks",
                "data": { "title": "A task" }
            }),
            source_plugin: "core".into(),
            timestamp: Utc::now(),
        };
        let result = plugin.handle_event(&event).await;
        assert!(result.is_ok());
    }

    #[test]
    fn no_last_sync_initially() {
        let plugin = CalendarConnectorPlugin::new();
        assert!(plugin.last_sync().is_none());
    }

    #[test]
    fn caldav_client_accessor() {
        let mut plugin = CalendarConnectorPlugin::new();
        assert!(plugin.caldav_client().is_none());

        plugin.configure_caldav(CalDavConfig {
            server_url: "http://localhost:5232".into(),
            username: "user".into(),
            credential_key: "caldav_password".into(),
            calendar_path: "/user/cal/".into(),
        });
        assert!(plugin.caldav_client().is_some());
        assert_eq!(
            plugin.caldav_client().unwrap().config().server_url,
            "http://localhost:5232"
        );
    }

    #[test]
    fn google_client_accessor() {
        let mut plugin = CalendarConnectorPlugin::new();
        assert!(plugin.google_client().is_none());

        plugin.configure_google(GoogleCalendarConfig {
            client_id: "id".into(),
            client_secret_key: "google_cal_client_secret".into(),
            refresh_token_key: "google_cal_refresh_token".into(),
            redirect_uri: "http://localhost:3750/callback".into(),
            auth_endpoint: crate::google::DEFAULT_AUTH_ENDPOINT.to_string(),
            token_endpoint: crate::google::DEFAULT_TOKEN_ENDPOINT.to_string(),
        });
        assert!(plugin.google_client().is_some());
        assert_eq!(plugin.google_client().unwrap().config().client_id, "id");
    }

    #[test]
    fn google_client_mut_accessor() {
        let mut plugin = CalendarConnectorPlugin::new();
        plugin.configure_google(GoogleCalendarConfig {
            client_id: "id".into(),
            client_secret_key: "google_cal_client_secret".into(),
            refresh_token_key: "google_cal_refresh_token".into(),
            redirect_uri: "http://localhost:3750/callback".into(),
            auth_endpoint: crate::google::DEFAULT_AUTH_ENDPOINT.to_string(),
            token_endpoint: crate::google::DEFAULT_TOKEN_ENDPOINT.to_string(),
        });

        // Use mutable accessor to update sync token
        let client = plugin
            .google_client_mut()
            .expect("should have google client");
        client.update_sync_token("new-sync-token-456".into());

        // Verify via immutable accessor
        let client = plugin.google_client().expect("should have google client");
        assert_eq!(client.sync_token(), Some("new-sync-token-456"));
    }

    #[test]
    fn configure_google_replaces_existing() {
        let mut plugin = CalendarConnectorPlugin::new();

        // First configuration
        plugin.configure_google(GoogleCalendarConfig {
            client_id: "first-id".into(),
            client_secret_key: "google_cal_client_secret".into(),
            refresh_token_key: "google_cal_refresh_token".into(),
            redirect_uri: "http://localhost:3750/callback".into(),
            auth_endpoint: crate::google::DEFAULT_AUTH_ENDPOINT.to_string(),
            token_endpoint: crate::google::DEFAULT_TOKEN_ENDPOINT.to_string(),
        });
        assert_eq!(
            plugin.google_client().unwrap().config().client_id,
            "first-id"
        );

        // Second configuration replaces the first
        plugin.configure_google(GoogleCalendarConfig {
            client_id: "second-id".into(),
            client_secret_key: "google_cal_client_secret_2".into(),
            refresh_token_key: "google_cal_refresh_token_2".into(),
            redirect_uri: "http://localhost:3750/callback".into(),
            auth_endpoint: crate::google::DEFAULT_AUTH_ENDPOINT.to_string(),
            token_endpoint: crate::google::DEFAULT_TOKEN_ENDPOINT.to_string(),
        });
        assert_eq!(
            plugin.google_client().unwrap().config().client_id,
            "second-id"
        );
    }

    #[tokio::test]
    async fn plugin_unload_clears_google_sync_token() {
        let mut plugin = CalendarConnectorPlugin::new();
        plugin.configure_google(GoogleCalendarConfig {
            client_id: "id".into(),
            client_secret_key: "google_cal_client_secret".into(),
            refresh_token_key: "google_cal_refresh_token".into(),
            redirect_uri: "http://localhost:3750/callback".into(),
            auth_endpoint: crate::google::DEFAULT_AUTH_ENDPOINT.to_string(),
            token_endpoint: crate::google::DEFAULT_TOKEN_ENDPOINT.to_string(),
        });

        // Set a sync token
        plugin
            .google_client_mut()
            .unwrap()
            .update_sync_token("sync-123".into());
        assert!(plugin.google_client().unwrap().sync_token().is_some());

        // Unload should clear the client entirely
        plugin.on_unload().await.expect("on_unload should succeed");
        assert!(plugin.google_client().is_none());
    }

    #[test]
    fn google_and_caldav_independent() {
        let mut plugin = CalendarConnectorPlugin::new();

        // Configure only Google
        plugin.configure_google(GoogleCalendarConfig {
            client_id: "id".into(),
            client_secret_key: "google_cal_client_secret".into(),
            refresh_token_key: "google_cal_refresh_token".into(),
            redirect_uri: "http://localhost:3750/callback".into(),
            auth_endpoint: crate::google::DEFAULT_AUTH_ENDPOINT.to_string(),
            token_endpoint: crate::google::DEFAULT_TOKEN_ENDPOINT.to_string(),
        });

        assert!(plugin.has_google(), "Google should be configured");
        assert!(!plugin.has_caldav(), "CalDAV should not be configured");
        assert!(plugin.google_client().is_some());
        assert!(plugin.caldav_client().is_none());
    }

    #[test]
    fn google_auth_url_generation() {
        let mut plugin = CalendarConnectorPlugin::new();

        // Without Google configured, returns None
        assert!(plugin.google_auth_url().is_none());
        assert!(plugin.pkce_verifier().is_none());

        // Configure Google
        plugin.configure_google(GoogleCalendarConfig {
            client_id: "test-client".into(),
            client_secret_key: "google_cal_client_secret".into(),
            refresh_token_key: "google_cal_refresh_token".into(),
            redirect_uri: "http://localhost:3750/callback".into(),
            auth_endpoint: crate::google::DEFAULT_AUTH_ENDPOINT.to_string(),
            token_endpoint: crate::google::DEFAULT_TOKEN_ENDPOINT.to_string(),
        });

        let url = plugin.google_auth_url().expect("should generate URL");
        assert!(url.starts_with(crate::google::DEFAULT_AUTH_ENDPOINT));
        assert!(url.contains("client_id="));
        assert!(url.contains("code_challenge="));
        assert!(url.contains("code_challenge_method=S256"));

        // PKCE verifier should be stored
        assert!(plugin.pkce_verifier().is_some());
        let verifier = plugin.pkce_verifier().unwrap();
        assert!(verifier.len() >= 43 && verifier.len() <= 128);
    }

    // --- WASM Plugin trait tests ---

    #[test]
    fn wasm_plugin_id_matches_core() {
        let plugin = CalendarConnectorPlugin::new();
        assert_eq!(Plugin::id(&plugin), CorePlugin::id(&plugin));
    }

    #[test]
    fn wasm_plugin_actions() {
        let plugin = CalendarConnectorPlugin::new();
        let actions = Plugin::actions(&plugin);
        let names: Vec<&str> = actions.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "sync",
                "status",
                "calendars",
                "google_auth",
                "google_callback"
            ]
        );
    }

    #[test]
    fn wasm_plugin_execute_known_action() {
        use chrono::Utc;
        use uuid::Uuid;

        let plugin = CalendarConnectorPlugin::new();
        let msg = PipelineMessage {
            metadata: MessageMetadata {
                correlation_id: Uuid::new_v4(),
                source: "test".into(),
                timestamp: Utc::now(),
                auth_context: None,
                warnings: vec![],
            },
            payload: TypedPayload::Cdm(Box::new(CdmType::Task(life_engine_plugin_sdk::Task {
                id: uuid::Uuid::new_v4(),
                title: "test".into(),
                description: None,
                status: life_engine_plugin_sdk::TaskStatus::Pending,
                priority: life_engine_plugin_sdk::TaskPriority::Medium,
                due_date: None,
                completed_at: None,
                tags: vec![],
                assignee: None,
                parent_id: None,
                source: "test".into(),
                source_id: "t-1".into(),
                extensions: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }))),
        };
        let result = Plugin::execute(&plugin, "sync", msg);
        assert!(result.is_ok());
    }

    #[test]
    fn wasm_plugin_execute_unknown_action() {
        use chrono::Utc;
        use uuid::Uuid;

        let plugin = CalendarConnectorPlugin::new();
        let msg = PipelineMessage {
            metadata: MessageMetadata {
                correlation_id: Uuid::new_v4(),
                source: "test".into(),
                timestamp: Utc::now(),
                auth_context: None,
                warnings: vec![],
            },
            payload: TypedPayload::Cdm(Box::new(CdmType::Task(life_engine_plugin_sdk::Task {
                id: uuid::Uuid::new_v4(),
                title: "test".into(),
                description: None,
                status: life_engine_plugin_sdk::TaskStatus::Pending,
                priority: life_engine_plugin_sdk::TaskPriority::Medium,
                due_date: None,
                completed_at: None,
                tags: vec![],
                assignee: None,
                parent_id: None,
                source: "test".into(),
                source_id: "t-1".into(),
                extensions: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }))),
        };
        let result = Plugin::execute(&plugin, "nonexistent", msg);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), "CALENDAR_005");
    }
}
