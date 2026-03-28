//! IMAP/SMTP email connector plugin for Life Engine Core.
//!
//! This is the first connector plugin, implementing both `CorePlugin`
//! (for lifecycle management and route registration) and the connector
//! pattern (for syncing email data from external IMAP servers and
//! sending via SMTP).
//!
//! # Architecture
//!
//! - `imap` — IMAP client with TLS, incremental sync via UIDVALIDITY + UIDs
//! - `smtp` — SMTP sending via `lettre`
//! - `normalizer` — Raw RFC 5322 messages to CDM `Email` type conversion

pub mod config;
pub mod error;
pub mod imap;
pub mod normalizer;
pub mod smtp;
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

use crate::imap::{ImapClient, ImapConfig};
use crate::smtp::{SmtpClient, SmtpConfig};

/// The email connector plugin.
///
/// Manages IMAP and SMTP clients for a single email account.
/// Uses the shared `RetryState` for exponential backoff on sync failures:
/// starts at 1 minute, doubles on each failure, caps at 1 hour,
/// and resets on success.
pub struct EmailConnectorPlugin {
    /// The IMAP client, initialised after credentials are provided.
    imap: Option<ImapClient>,
    /// The SMTP client, initialised after credentials are provided.
    smtp: Option<SmtpClient>,
    /// Interval between automatic sync operations.
    sync_interval: Duration,
    /// Timestamp of the last successful sync.
    last_sync: Option<DateTime<Utc>>,
    /// Retry state with exponential backoff (shared with webhook sender).
    retry: RetryState,
    /// Earliest time the next sync retry is allowed.
    next_retry: Option<DateTime<Utc>>,
}

impl EmailConnectorPlugin {
    /// Create a new email connector plugin with default settings.
    pub fn new() -> Self {
        Self {
            imap: None,
            smtp: None,
            sync_interval: Duration::from_secs(300), // 5 minutes
            last_sync: None,
            retry: RetryState::new(),
            next_retry: None,
        }
    }

    /// Create a new email connector with a custom sync interval.
    pub fn with_sync_interval(sync_interval: Duration) -> Self {
        Self {
            sync_interval,
            ..Self::new()
        }
    }

    /// Configure the IMAP client with the given settings.
    pub fn configure_imap(&mut self, config: ImapConfig) {
        self.imap = Some(ImapClient::new(config));
    }

    /// Configure the SMTP client with the given settings.
    pub fn configure_smtp(&mut self, config: SmtpConfig) {
        self.smtp = Some(SmtpClient::new(config));
    }

    /// Returns whether IMAP is configured.
    pub fn has_imap(&self) -> bool {
        self.imap.is_some()
    }

    /// Returns whether SMTP is configured.
    pub fn has_smtp(&self) -> bool {
        self.smtp.is_some()
    }

    /// Returns the configured sync interval.
    pub fn sync_interval(&self) -> Duration {
        self.sync_interval
    }

    /// Returns the timestamp of the last successful sync.
    pub fn last_sync(&self) -> Option<DateTime<Utc>> {
        self.last_sync
    }

    /// Returns a reference to the IMAP client, if configured.
    pub fn imap_client(&self) -> Option<&ImapClient> {
        self.imap.as_ref()
    }

    /// Returns a mutable reference to the IMAP client, if configured.
    pub fn imap_client_mut(&mut self) -> Option<&mut ImapClient> {
        self.imap.as_mut()
    }

    /// Returns a reference to the SMTP client, if configured.
    pub fn smtp_client(&self) -> Option<&SmtpClient> {
        self.smtp.as_ref()
    }

    /// Returns the number of consecutive sync failures.
    pub fn failure_count(&self) -> u32 {
        self.retry.failure_count
    }

    /// Returns the earliest time the next sync retry is allowed.
    pub fn next_retry(&self) -> Option<DateTime<Utc>> {
        self.next_retry
    }

    /// Record a sync success: resets failure count and clears backoff.
    pub fn record_sync_success(&mut self) {
        self.retry.record_success();
        self.next_retry = None;
        self.last_sync = Some(Utc::now());
        tracing::debug!("sync succeeded, backoff reset");
    }

    /// Record a sync failure: increments failure count and sets the
    /// next retry time using exponential backoff.
    ///
    /// Backoff starts at 1 minute, doubles on each failure, and caps
    /// at 1 hour.
    pub fn record_sync_failure(&mut self) {
        let backoff = self.retry.record_failure();
        let backoff_chrono = chrono::Duration::from_std(backoff)
            .unwrap_or_else(|_| chrono::Duration::seconds(backoff.as_secs() as i64));
        self.next_retry = Some(Utc::now() + backoff_chrono);
        tracing::warn!(
            failure_count = self.retry.failure_count,
            next_retry_secs = backoff.as_secs(),
            "sync failed, applying exponential backoff"
        );
    }

    /// Check whether a sync attempt is allowed right now.
    ///
    /// Returns `true` if there is no backoff active or the backoff
    /// period has elapsed.
    pub fn can_sync_now(&self) -> bool {
        match self.next_retry {
            Some(retry_at) => Utc::now() >= retry_at,
            None => true,
        }
    }

    /// Compute the backoff duration based on the current failure count.
    ///
    /// Formula: min(BACKOFF_MIN * 2^(failure_count - 1), BACKOFF_MAX)
    pub fn compute_backoff(&self) -> chrono::Duration {
        let backoff = self.retry.compute_backoff();
        chrono::Duration::from_std(backoff)
            .unwrap_or_else(|_| chrono::Duration::seconds(backoff.as_secs() as i64))
    }
}

impl Default for EmailConnectorPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for EmailConnectorPlugin {
    fn id(&self) -> &str {
        "com.life-engine.connector-email"
    }

    fn display_name(&self) -> &str {
        "Email Connector"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn actions(&self) -> Vec<Action> {
        vec![
            Action::new("sync", "Sync emails from the configured IMAP server"),
            Action::new("send", "Send an email via the configured SMTP server"),
            Action::new("status", "Get the current sync status"),
        ]
    }

    fn execute(
        &self,
        action: &str,
        input: PipelineMessage,
    ) -> std::result::Result<PipelineMessage, Box<dyn EngineError>> {
        match action {
            "sync" | "send" | "status" => Ok(input),
            other => Err(Box::new(
                crate::error::EmailConnectorError::UnknownAction(other.to_string()),
            )),
        }
    }
}

life_engine_plugin_sdk::register_plugin!(EmailConnectorPlugin);

#[async_trait]
impl CorePlugin for EmailConnectorPlugin {
    fn id(&self) -> &str {
        "com.life-engine.connector-email"
    }

    fn display_name(&self) -> &str {
        "Email Connector"
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
            "email connector plugin loaded"
        );
        Ok(())
    }

    async fn on_unload(&mut self) -> Result<()> {
        // Clear clients and backoff state on unload
        self.imap = None;
        self.smtp = None;
        self.last_sync = None;
        self.retry = RetryState::new();
        self.next_retry = None;
        tracing::info!("email connector plugin unloaded");
        Ok(())
    }

    fn routes(&self) -> Vec<PluginRoute> {
        vec![
            PluginRoute {
                method: HttpMethod::Post,
                path: "/sync".into(),
            },
            PluginRoute {
                method: HttpMethod::Post,
                path: "/send".into(),
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
        let plugin = EmailConnectorPlugin::new();
        assert_eq!(CorePlugin::id(&plugin), "com.life-engine.connector-email");
    }

    #[test]
    fn plugin_display_name() {
        let plugin = EmailConnectorPlugin::new();
        assert_eq!(CorePlugin::display_name(&plugin), "Email Connector");
    }

    #[test]
    fn plugin_version() {
        let plugin = EmailConnectorPlugin::new();
        assert_eq!(CorePlugin::version(&plugin), "0.1.0");
    }

    #[test]
    fn plugin_capabilities() {
        use life_engine_test_utils::assert_plugin_capabilities;
        let plugin = EmailConnectorPlugin::new();
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
        let plugin = EmailConnectorPlugin::new();
        assert_plugin_routes!(plugin, ["/sync", "/send", "/status"]);
    }

    #[tokio::test]
    async fn plugin_lifecycle() {
        let mut plugin = EmailConnectorPlugin::new();
        assert!(!plugin.has_imap());
        assert!(!plugin.has_smtp());

        let ctx = PluginContext::new(CorePlugin::id(&plugin));
        plugin.on_load(&ctx).await.expect("on_load should succeed");

        // Configure clients
        plugin.configure_imap(ImapConfig {
            host: "imap.example.com".into(),
            port: 993,
            username: "user@example.com".into(),
            credential_key: "imap_password".into(),
            use_tls: true,
        });
        plugin.configure_smtp(SmtpConfig {
            host: "smtp.example.com".into(),
            port: 587,
            username: "user@example.com".into(),
            credential_key: "smtp_password".into(),
            use_tls: true,
        });
        assert!(plugin.has_imap());
        assert!(plugin.has_smtp());

        // Simulate a failure to set backoff state
        plugin.record_sync_failure();
        assert_eq!(plugin.failure_count(), 1);

        // Unload clears clients and backoff state
        plugin.on_unload().await.expect("on_unload should succeed");
        assert!(!plugin.has_imap());
        assert!(!plugin.has_smtp());
        assert!(plugin.last_sync().is_none());
        assert_eq!(plugin.failure_count(), 0);
        assert!(plugin.next_retry().is_none());
    }

    #[tokio::test]
    async fn handle_event_returns_ok() {
        let plugin = EmailConnectorPlugin::new();
        life_engine_test_utils::plugin_test_helpers::test_handle_event_ok(&plugin).await;
    }

    #[test]
    fn default_sync_interval() {
        let plugin = EmailConnectorPlugin::new();
        assert_eq!(plugin.sync_interval(), Duration::from_secs(300));
    }

    #[test]
    fn custom_sync_interval() {
        let plugin = EmailConnectorPlugin::with_sync_interval(Duration::from_secs(60));
        assert_eq!(plugin.sync_interval(), Duration::from_secs(60));
    }

    #[test]
    fn default_impl() {
        let plugin = EmailConnectorPlugin::default();
        assert_eq!(CorePlugin::id(&plugin), "com.life-engine.connector-email");
    }

    #[test]
    fn no_last_sync_initially() {
        let plugin = EmailConnectorPlugin::new();
        assert!(plugin.last_sync().is_none());
    }

    #[test]
    fn initial_failure_count_is_zero() {
        let plugin = EmailConnectorPlugin::new();
        assert_eq!(plugin.failure_count(), 0);
        assert!(plugin.next_retry().is_none());
        assert!(plugin.can_sync_now());
    }

    #[test]
    fn backoff_doubles_on_each_failure() {
        let mut plugin = EmailConnectorPlugin::new();

        // First failure: 60 seconds
        plugin.record_sync_failure();
        assert_eq!(plugin.failure_count(), 1);
        assert_eq!(plugin.compute_backoff().num_seconds(), 60);

        // Second failure: 120 seconds
        plugin.record_sync_failure();
        assert_eq!(plugin.failure_count(), 2);
        assert_eq!(plugin.compute_backoff().num_seconds(), 120);

        // Third failure: 240 seconds
        plugin.record_sync_failure();
        assert_eq!(plugin.failure_count(), 3);
        assert_eq!(plugin.compute_backoff().num_seconds(), 240);
    }

    #[test]
    fn backoff_caps_at_one_hour() {
        let mut plugin = EmailConnectorPlugin::new();

        // Simulate many failures
        for _ in 0..20 {
            plugin.record_sync_failure();
        }

        let backoff = plugin.compute_backoff();
        assert_eq!(
            backoff.num_seconds(),
            3600,
            "backoff should cap at 3600 seconds (1 hour)"
        );
    }

    #[test]
    fn backoff_resets_on_success() {
        let mut plugin = EmailConnectorPlugin::new();

        plugin.record_sync_failure();
        plugin.record_sync_failure();
        assert_eq!(plugin.failure_count(), 2);
        assert!(plugin.next_retry().is_some());

        plugin.record_sync_success();
        assert_eq!(plugin.failure_count(), 0);
        assert!(plugin.next_retry().is_none());
        assert!(plugin.last_sync().is_some());
        assert!(plugin.can_sync_now());
    }

    #[test]
    fn compute_backoff_zero_failures_returns_zero() {
        let plugin = EmailConnectorPlugin::new();
        assert_eq!(plugin.compute_backoff().num_seconds(), 0);
    }

    #[test]
    fn can_sync_now_true_initially() {
        let plugin = EmailConnectorPlugin::new();
        assert!(plugin.can_sync_now());
    }

    #[test]
    fn can_sync_now_false_during_backoff() {
        let mut plugin = EmailConnectorPlugin::new();
        plugin.record_sync_failure();
        // next_retry is set to ~60 seconds from now, so we should not be able to sync
        assert!(!plugin.can_sync_now());
    }

    #[test]
    fn imap_client_accessor() {
        let mut plugin = EmailConnectorPlugin::new();
        assert!(plugin.imap_client().is_none());

        plugin.configure_imap(ImapConfig {
            host: "imap.example.com".into(),
            port: 993,
            username: "user".into(),
            credential_key: "imap_password".into(),
            use_tls: true,
        });
        assert!(plugin.imap_client().is_some());
        assert_eq!(
            plugin.imap_client().unwrap().config().host,
            "imap.example.com"
        );
    }

    #[test]
    fn smtp_client_accessor() {
        let mut plugin = EmailConnectorPlugin::new();
        assert!(plugin.smtp_client().is_none());

        plugin.configure_smtp(SmtpConfig {
            host: "smtp.example.com".into(),
            port: 587,
            username: "user".into(),
            credential_key: "smtp_password".into(),
            use_tls: true,
        });
        assert!(plugin.smtp_client().is_some());
        assert_eq!(
            plugin.smtp_client().unwrap().config().host,
            "smtp.example.com"
        );
    }

    // --- WASM Plugin trait tests ---

    #[test]
    fn wasm_plugin_id_matches_core() {
        let plugin = EmailConnectorPlugin::new();
        assert_eq!(Plugin::id(&plugin), CorePlugin::id(&plugin));
    }

    #[test]
    fn wasm_plugin_actions() {
        let plugin = EmailConnectorPlugin::new();
        let actions = Plugin::actions(&plugin);
        let names: Vec<&str> = actions.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, vec!["sync", "send", "status"]);
    }

    #[test]
    fn wasm_plugin_execute_known_action() {
        use chrono::Utc;
        use uuid::Uuid;

        let plugin = EmailConnectorPlugin::new();
        let msg = PipelineMessage {
            metadata: MessageMetadata {
                correlation_id: Uuid::new_v4(),
                source: "test".into(),
                timestamp: Utc::now(),
                auth_context: None,
                warnings: vec![],
            },
            payload: TypedPayload::Cdm(Box::new(CdmType::Task(life_engine_plugin_sdk::Task {
                    id: Uuid::new_v4(),
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

        let plugin = EmailConnectorPlugin::new();
        let msg = PipelineMessage {
            metadata: MessageMetadata {
                correlation_id: Uuid::new_v4(),
                source: "test".into(),
                timestamp: Utc::now(),
                auth_context: None,
                warnings: vec![],
            },
            payload: TypedPayload::Cdm(Box::new(CdmType::Task(life_engine_plugin_sdk::Task {
                    id: Uuid::new_v4(),
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
        assert_eq!(err.code(), "EMAIL_005");
        assert!(err.severity().is_fatal());
    }
}
