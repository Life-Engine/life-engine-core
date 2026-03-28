//! CardDAV and Google Contacts connector plugin for Life Engine Core.
//!
//! This connector plugin syncs contacts from CardDAV servers (such as
//! Radicale, Nextcloud, iCloud) and Google Contacts into the Life Engine
//! canonical `Contact` data model.
//!
//! # Architecture
//!
//! - `carddav` — CardDAV client with sync-token/ctag incremental sync
//! - `normalizer` — Raw vCard text to CDM `Contact` type conversion
//! - `google` — Google People API client with OAuth2 token management and incremental sync

pub mod carddav;
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
use life_engine_plugin_sdk::types::Capability;

use crate::carddav::{CardDavClient, CardDavConfig};
use crate::google::{GoogleContactsClient, GoogleContactsConfig};

/// The contacts connector plugin.
///
/// Manages CardDAV and Google Contacts clients for syncing contacts.
/// Credentials are stored in memory (Phase 1); persistent credential
/// storage comes in a later phase.
pub struct ContactsConnectorPlugin {
    /// The CardDAV client, initialised after credentials are provided.
    carddav: Option<CardDavClient>,
    /// The Google Contacts client, initialised after credentials are provided.
    google: Option<GoogleContactsClient>,
    /// Interval between automatic sync operations.
    sync_interval: Duration,
    /// Timestamp of the last successful sync.
    last_sync: Option<DateTime<Utc>>,
}

impl ContactsConnectorPlugin {
    /// Create a new contacts connector plugin with default settings.
    pub fn new() -> Self {
        Self {
            carddav: None,
            google: None,
            sync_interval: Duration::from_secs(300), // 5 minutes
            last_sync: None,
        }
    }

    /// Create a new contacts connector with a custom sync interval.
    pub fn with_sync_interval(sync_interval: Duration) -> Self {
        Self {
            sync_interval,
            ..Self::new()
        }
    }

    /// Configure the CardDAV client with the given settings.
    pub fn configure_carddav(&mut self, config: CardDavConfig) {
        self.carddav = Some(CardDavClient::new(config));
    }

    /// Configure the Google Contacts client with the given settings.
    pub fn configure_google(&mut self, config: GoogleContactsConfig) {
        self.google = Some(GoogleContactsClient::new(config));
    }

    /// Returns whether CardDAV is configured.
    pub fn has_carddav(&self) -> bool {
        self.carddav.is_some()
    }

    /// Returns whether Google Contacts is configured.
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

    /// Returns a reference to the CardDAV client, if configured.
    pub fn carddav_client(&self) -> Option<&CardDavClient> {
        self.carddav.as_ref()
    }

    /// Returns a mutable reference to the CardDAV client, if configured.
    pub fn carddav_client_mut(&mut self) -> Option<&mut CardDavClient> {
        self.carddav.as_mut()
    }

    /// Returns a reference to the Google Contacts client, if configured.
    pub fn google_client(&self) -> Option<&GoogleContactsClient> {
        self.google.as_ref()
    }

    /// Returns a mutable reference to the Google Contacts client, if configured.
    pub fn google_client_mut(&mut self) -> Option<&mut GoogleContactsClient> {
        self.google.as_mut()
    }
}

impl Default for ContactsConnectorPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ContactsConnectorPlugin {
    fn id(&self) -> &str {
        "com.life-engine.connector-contacts"
    }

    fn display_name(&self) -> &str {
        "Contacts Connector"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn actions(&self) -> Vec<Action> {
        vec![
            Action::new("sync", "Sync contacts from configured providers"),
            Action::new("status", "Get the current sync status"),
        ]
    }

    fn execute(
        &self,
        action: &str,
        input: PipelineMessage,
    ) -> std::result::Result<PipelineMessage, Box<dyn EngineError>> {
        match action {
            "sync" | "status" => Ok(input),
            other => Err(Box::new(
                crate::error::ContactsConnectorError::UnknownAction(other.to_string()),
            )),
        }
    }
}

life_engine_plugin_sdk::register_plugin!(ContactsConnectorPlugin);

#[async_trait]
impl CorePlugin for ContactsConnectorPlugin {
    fn id(&self) -> &str {
        "com.life-engine.connector-contacts"
    }

    fn display_name(&self) -> &str {
        "Contacts Connector"
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
            "contacts connector plugin loaded"
        );
        Ok(())
    }

    async fn on_unload(&mut self) -> Result<()> {
        self.carddav = None;
        self.google = None;
        self.last_sync = None;
        tracing::info!("contacts connector plugin unloaded");
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
        ]
    }

    async fn handle_event(&self, _event: &CoreEvent) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_id_is_correct() {
        let plugin = ContactsConnectorPlugin::new();
        assert_eq!(CorePlugin::id(&plugin), "com.life-engine.connector-contacts");
    }

    #[test]
    fn plugin_display_name() {
        let plugin = ContactsConnectorPlugin::new();
        assert_eq!(CorePlugin::display_name(&plugin), "Contacts Connector");
    }

    #[test]
    fn plugin_version() {
        let plugin = ContactsConnectorPlugin::new();
        assert_eq!(CorePlugin::version(&plugin), "0.1.0");
    }

    #[test]
    fn plugin_capabilities() {
        use life_engine_test_utils::assert_plugin_capabilities;
        let plugin = ContactsConnectorPlugin::new();
        assert_plugin_capabilities!(plugin, [
            Capability::StorageRead,
            Capability::StorageWrite,
            Capability::HttpOutbound,
            Capability::CredentialsRead,
            Capability::CredentialsWrite,
        ]);
    }

    #[test]
    fn plugin_routes() {
        use life_engine_test_utils::assert_plugin_routes;
        let plugin = ContactsConnectorPlugin::new();
        assert_plugin_routes!(plugin, ["/sync", "/status"]);
    }

    #[tokio::test]
    async fn plugin_lifecycle() {
        let mut plugin = ContactsConnectorPlugin::new();
        assert!(!plugin.has_carddav());
        assert!(!plugin.has_google());

        let ctx = PluginContext::new(CorePlugin::id(&plugin));
        plugin.on_load(&ctx).await.expect("on_load should succeed");

        // Configure clients
        plugin.configure_carddav(CardDavConfig {
            server_url: "https://dav.example.com".into(),
            username: "user@example.com".into(),
            credential_key: "carddav_password".into(),
            addressbook_path: "/addressbooks/user/default/".into(),
        });
        plugin.configure_google(GoogleContactsConfig {
            client_id: "id".into(),
            client_secret_key: "google_contacts_client_secret".into(),
            refresh_token_key: "google_contacts_refresh_token".into(),
        });
        assert!(plugin.has_carddav());
        assert!(plugin.has_google());

        // Unload clears clients
        plugin.on_unload().await.expect("on_unload should succeed");
        assert!(!plugin.has_carddav());
        assert!(!plugin.has_google());
        assert!(plugin.last_sync().is_none());
    }

    #[tokio::test]
    async fn handle_event_returns_ok() {
        let plugin = ContactsConnectorPlugin::new();
        life_engine_test_utils::plugin_test_helpers::test_handle_event_ok(&plugin).await;
    }

    #[test]
    fn default_sync_interval() {
        let plugin = ContactsConnectorPlugin::new();
        assert_eq!(plugin.sync_interval(), Duration::from_secs(300));
    }

    #[test]
    fn custom_sync_interval() {
        let plugin =
            ContactsConnectorPlugin::with_sync_interval(Duration::from_secs(60));
        assert_eq!(plugin.sync_interval(), Duration::from_secs(60));
    }

    #[test]
    fn default_impl() {
        let plugin = ContactsConnectorPlugin::default();
        assert_eq!(CorePlugin::id(&plugin), "com.life-engine.connector-contacts");
    }

    #[test]
    fn no_last_sync_initially() {
        let plugin = ContactsConnectorPlugin::new();
        assert!(plugin.last_sync().is_none());
    }

    #[test]
    fn carddav_client_accessor() {
        let mut plugin = ContactsConnectorPlugin::new();
        assert!(plugin.carddav_client().is_none());

        plugin.configure_carddav(CardDavConfig {
            server_url: "https://dav.example.com".into(),
            username: "user".into(),
            credential_key: "carddav_password".into(),
            addressbook_path: "/addressbooks/user/default/".into(),
        });
        assert!(plugin.carddav_client().is_some());
        assert_eq!(
            plugin.carddav_client().unwrap().config().server_url,
            "https://dav.example.com"
        );
    }

    #[test]
    fn google_client_mut_accessor() {
        let mut plugin = ContactsConnectorPlugin::new();
        assert!(plugin.google_client_mut().is_none());

        plugin.configure_google(GoogleContactsConfig {
            client_id: "id".into(),
            client_secret_key: "google_contacts_client_secret".into(),
            refresh_token_key: "google_contacts_refresh_token".into(),
        });
        assert!(plugin.google_client_mut().is_some());

        // Verify we can mutate through the accessor
        let client = plugin.google_client_mut().unwrap();
        client.update_sync_token("new-token".into());
        assert_eq!(
            plugin.google_client().unwrap().sync_state().sync_token.as_deref(),
            Some("new-token")
        );
    }

    #[test]
    fn google_client_accessor() {
        let mut plugin = ContactsConnectorPlugin::new();
        assert!(plugin.google_client().is_none());

        plugin.configure_google(GoogleContactsConfig {
            client_id: "id".into(),
            client_secret_key: "google_contacts_client_secret".into(),
            refresh_token_key: "google_contacts_refresh_token".into(),
        });
        assert!(plugin.google_client().is_some());
        assert_eq!(plugin.google_client().unwrap().config().client_id, "id");
    }

    // -------------------------------------------------------------------
    // Plugin-level Google configuration tests
    // -------------------------------------------------------------------

    #[test]
    fn configure_google_then_google_client_returns_configured_client() {
        let mut plugin = ContactsConnectorPlugin::new();
        assert!(
            plugin.google_client().is_none(),
            "google_client should be None before configuration"
        );

        plugin.configure_google(GoogleContactsConfig {
            client_id: "configured-id".into(),
            client_secret_key: "configured-secret-key".into(),
            refresh_token_key: "configured-token-key".into(),
        });

        let client = plugin
            .google_client()
            .expect("google_client should be Some after configure_google");
        assert_eq!(client.config().client_id, "configured-id");
        assert_eq!(client.config().client_secret_key, "configured-secret-key");
        assert_eq!(client.config().refresh_token_key, "configured-token-key");
    }

    #[test]
    fn google_client_returns_none_before_configuration() {
        let mut plugin = ContactsConnectorPlugin::new();
        assert!(
            plugin.google_client().is_none(),
            "google_client() should return None on a fresh plugin"
        );
        assert!(
            plugin.google_client_mut().is_none(),
            "google_client_mut() should also return None on a fresh plugin"
        );
    }

    #[tokio::test]
    async fn on_unload_clears_google_client_and_sync_state() {
        let mut plugin = ContactsConnectorPlugin::new();

        // Configure Google client
        plugin.configure_google(GoogleContactsConfig {
            client_id: "id".into(),
            client_secret_key: "google_contacts_client_secret".into(),
            refresh_token_key: "google_contacts_refresh_token".into(),
        });
        assert!(
            plugin.has_google(),
            "plugin should have google after configure_google"
        );

        // Set a sync token via the mutable accessor to verify state is present
        plugin
            .google_client_mut()
            .expect("google_client_mut should be Some")
            .update_sync_token("plugin-level-token".into());
        assert_eq!(
            plugin
                .google_client()
                .expect("google_client should be Some")
                .sync_state()
                .sync_token
                .as_deref(),
            Some("plugin-level-token")
        );

        // Unload should clear everything
        plugin.on_unload().await.expect("on_unload should succeed");
        assert!(
            !plugin.has_google(),
            "google should be cleared after on_unload"
        );
        assert!(
            plugin.google_client().is_none(),
            "google_client() should be None after on_unload"
        );
        assert!(
            plugin.last_sync().is_none(),
            "last_sync should be None after on_unload"
        );
    }

    // --- WASM Plugin trait tests ---

    #[test]
    fn wasm_plugin_id_matches_core() {
        let plugin = ContactsConnectorPlugin::new();
        assert_eq!(Plugin::id(&plugin), CorePlugin::id(&plugin));
    }

    #[test]
    fn wasm_plugin_actions() {
        let plugin = ContactsConnectorPlugin::new();
        let actions = Plugin::actions(&plugin);
        let names: Vec<&str> = actions.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, vec!["sync", "status"]);
    }

    #[test]
    fn wasm_plugin_execute_known_action() {
        use chrono::Utc;
        use uuid::Uuid;

        let plugin = ContactsConnectorPlugin::new();
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
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                }))),
        };
        let result = Plugin::execute(&plugin, "sync", msg);
        assert!(result.is_ok());
    }

    #[test]
    fn wasm_plugin_execute_unknown_action() {
        use chrono::Utc;
        use uuid::Uuid;

        let plugin = ContactsConnectorPlugin::new();
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
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                }))),
        };
        let result = Plugin::execute(&plugin, "nonexistent", msg);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), "CONTACTS_004");
    }
}
