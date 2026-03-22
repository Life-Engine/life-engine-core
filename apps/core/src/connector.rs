//! Connector trait for external service integration.
//!
//! Connectors bridge Life Engine with external services (email, calendar,
//! contacts, etc.) by implementing a standard sync/disconnect lifecycle.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::storage::StorageAdapter;

/// Credentials for authenticating with an external service.
///
/// Secrets are stored in the credential store rather than inline.
/// The `credential_id` references the key in the credential store
/// where the password or token is stored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorCredentials {
    /// The hostname of the service.
    pub host: String,
    /// The port to connect on.
    pub port: u16,
    /// The username for authentication.
    pub username: String,
    /// The key referencing the password in the credential store.
    pub credential_id: Option<String>,
    /// Whether to use TLS for the connection.
    pub use_tls: bool,
}

/// The result of a sync operation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncResult {
    /// Number of newly created records.
    pub new_records: u64,
    /// Number of updated records.
    pub updated_records: u64,
    /// Number of deleted records.
    pub deleted_records: u64,
}

/// Trait for external service connectors.
///
/// A connector manages the lifecycle of an external service connection:
/// authenticate, sync data into local storage, and disconnect cleanly.
#[async_trait]
pub trait Connector: Send + Sync {
    /// Returns the unique connector identifier.
    fn id(&self) -> &str;

    /// Returns a human-readable name for the connector.
    fn display_name(&self) -> &str;

    /// Returns the CDM collection names this connector can populate.
    fn supported_collections(&self) -> Vec<&str>;

    /// Authenticate with the external service using the provided credentials.
    async fn authenticate(&mut self, credentials: ConnectorCredentials) -> anyhow::Result<()>;

    /// Sync data from the external service into storage.
    ///
    /// If `last_sync` is provided, performs an incremental sync fetching
    /// only changes since that timestamp.
    async fn sync(
        &mut self,
        storage: &dyn StorageAdapter,
        last_sync: Option<DateTime<Utc>>,
    ) -> anyhow::Result<SyncResult>;

    /// Disconnect from the external service and clean up resources.
    async fn disconnect(&mut self) -> anyhow::Result<()>;
}

/// Exponential backoff tracker for sync retries.
///
/// After each failed sync attempt, the delay doubles (with jitter) up to
/// a configurable maximum. A successful sync resets the backoff.
///
/// Default: 30s initial, 15min max, 3 consecutive failures to trigger backoff.
#[derive(Debug, Clone)]
pub struct SyncBackoff {
    /// Base delay in seconds (doubles after each failure).
    pub base_delay_secs: u64,
    /// Maximum delay in seconds (caps the exponential growth).
    pub max_delay_secs: u64,
    /// Number of consecutive failures before backoff activates.
    pub failure_threshold: u32,
    /// Current count of consecutive failures.
    consecutive_failures: u32,
}

impl Default for SyncBackoff {
    fn default() -> Self {
        Self {
            base_delay_secs: 30,
            max_delay_secs: 900, // 15 minutes
            failure_threshold: 3,
            consecutive_failures: 0,
        }
    }
}

impl SyncBackoff {
    /// Create a new backoff tracker with custom parameters.
    pub fn new(base_delay_secs: u64, max_delay_secs: u64, failure_threshold: u32) -> Self {
        Self {
            base_delay_secs,
            max_delay_secs,
            failure_threshold,
            consecutive_failures: 0,
        }
    }

    /// Record a sync failure and return the delay before the next attempt.
    ///
    /// Returns `None` if the failure count is below the threshold (sync
    /// should proceed at normal interval). Returns `Some(delay_secs)` when
    /// backoff is active.
    pub fn record_failure(&mut self) -> Option<u64> {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);

        if self.consecutive_failures < self.failure_threshold {
            return None;
        }

        // Exponential backoff: base * 2^(failures - threshold)
        let exponent = self.consecutive_failures - self.failure_threshold;
        let delay = self.base_delay_secs.saturating_mul(1u64 << exponent.min(10));
        Some(delay.min(self.max_delay_secs))
    }

    /// Record a successful sync, resetting the failure counter.
    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
    }

    /// Returns the current number of consecutive failures.
    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }

    /// Returns `true` if backoff is currently active (failures >= threshold).
    pub fn is_backing_off(&self) -> bool {
        self.consecutive_failures >= self.failure_threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connector_credentials_serialization() {
        let creds = ConnectorCredentials {
            host: "imap.example.com".into(),
            port: 993,
            username: "user@example.com".into(),
            credential_id: Some("imap_password".into()),
            use_tls: true,
        };
        let json = serde_json::to_string(&creds).expect("serialize");
        let restored: ConnectorCredentials =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.host, "imap.example.com");
        assert_eq!(restored.port, 993);
        assert!(restored.use_tls);
        assert_eq!(restored.credential_id.as_deref(), Some("imap_password"));
    }

    #[test]
    fn connector_credentials_without_credential_id() {
        let creds = ConnectorCredentials {
            host: "imap.example.com".into(),
            port: 993,
            username: "user@example.com".into(),
            credential_id: None,
            use_tls: true,
        };
        let json = serde_json::to_string(&creds).expect("serialize");
        let restored: ConnectorCredentials =
            serde_json::from_str(&json).expect("deserialize");
        assert!(restored.credential_id.is_none());
    }

    #[test]
    fn sync_result_default() {
        let result = SyncResult::default();
        assert_eq!(result.new_records, 0);
        assert_eq!(result.updated_records, 0);
        assert_eq!(result.deleted_records, 0);
    }

    #[test]
    fn sync_result_serialization() {
        let result = SyncResult {
            new_records: 10,
            updated_records: 3,
            deleted_records: 1,
        };
        let json = serde_json::to_string(&result).expect("serialize");
        let restored: SyncResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.new_records, 10);
        assert_eq!(restored.updated_records, 3);
        assert_eq!(restored.deleted_records, 1);
    }

    #[test]
    fn backoff_default_values() {
        let b = SyncBackoff::default();
        assert_eq!(b.base_delay_secs, 30);
        assert_eq!(b.max_delay_secs, 900);
        assert_eq!(b.failure_threshold, 3);
        assert_eq!(b.consecutive_failures(), 0);
        assert!(!b.is_backing_off());
    }

    #[test]
    fn backoff_no_delay_below_threshold() {
        let mut b = SyncBackoff::default();
        assert_eq!(b.record_failure(), None);
        assert_eq!(b.record_failure(), None);
        assert_eq!(b.consecutive_failures(), 2);
        assert!(!b.is_backing_off());
    }

    #[test]
    fn backoff_activates_at_threshold() {
        let mut b = SyncBackoff::default();
        b.record_failure(); // 1
        b.record_failure(); // 2
        let delay = b.record_failure(); // 3 = threshold
        assert!(delay.is_some());
        assert_eq!(delay.unwrap(), 30); // base_delay * 2^0
        assert!(b.is_backing_off());
    }

    #[test]
    fn backoff_doubles_each_failure() {
        let mut b = SyncBackoff::new(10, 1000, 1);
        assert_eq!(b.record_failure(), Some(10));  // 10 * 2^0
        assert_eq!(b.record_failure(), Some(20));  // 10 * 2^1
        assert_eq!(b.record_failure(), Some(40));  // 10 * 2^2
        assert_eq!(b.record_failure(), Some(80));  // 10 * 2^3
    }

    #[test]
    fn backoff_caps_at_max() {
        let mut b = SyncBackoff::new(10, 50, 1);
        b.record_failure(); // 10
        b.record_failure(); // 20
        b.record_failure(); // 40
        let delay = b.record_failure(); // would be 80, capped at 50
        assert_eq!(delay, Some(50));
    }

    #[test]
    fn backoff_resets_on_success() {
        let mut b = SyncBackoff::new(10, 1000, 1);
        b.record_failure();
        b.record_failure();
        assert!(b.is_backing_off());

        b.record_success();
        assert_eq!(b.consecutive_failures(), 0);
        assert!(!b.is_backing_off());

        // After reset, first failure below threshold again
        // (threshold=1, so immediately triggers)
        assert_eq!(b.record_failure(), Some(10));
    }

    #[test]
    fn backoff_does_not_overflow() {
        let mut b = SyncBackoff::new(1, u64::MAX, 0);
        for _ in 0..100 {
            let delay = b.record_failure();
            assert!(delay.is_some());
        }
    }
}
