//! IMAP client for connecting to and fetching emails from IMAP servers.
//!
//! Handles TLS connections, authentication, mailbox listing, and
//! incremental message fetching using UIDVALIDITY + UIDs.
//!
//! The `ImapClient` type manages connection configuration and per-mailbox
//! sync state. Actual IMAP sessions are established on demand via
//! `connect()` and returned as an opaque `ImapSession` wrapper.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for an IMAP connection.
///
/// The password is not stored in this config struct. Instead, the
/// `credential_key` names the key under which the password is stored
/// in the credential store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImapConfig {
    /// The IMAP server hostname.
    pub host: String,
    /// The IMAP server port (typically 993 for TLS, 143 for plain).
    pub port: u16,
    /// The username for authentication.
    pub username: String,
    /// The key used to look up the password in the credential store.
    #[serde(default = "default_imap_credential_key")]
    pub credential_key: String,
    /// Whether to use TLS for the connection.
    pub use_tls: bool,
}

/// Default credential key for IMAP passwords.
fn default_imap_credential_key() -> String {
    "imap_password".to_string()
}

/// Tracks sync state for a single mailbox.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MailboxSyncState {
    /// The UIDVALIDITY value from the server.
    pub uid_validity: u32,
    /// The last UID we successfully synced.
    pub last_uid: u32,
}

/// A fetched message from IMAP with its raw data and UID.
#[derive(Debug, Clone)]
pub struct FetchedMessage {
    /// The IMAP UID for this message.
    pub uid: u32,
    /// The raw RFC 5322 message bytes.
    pub raw: Vec<u8>,
}

/// IMAP client that manages connection configuration and sync state.
///
/// The actual IMAP session lifecycle (connect, fetch, disconnect) is
/// handled by the integration layer. This struct tracks which UIDs
/// have been synced per mailbox so incremental sync works correctly.
pub struct ImapClient {
    /// Connection configuration.
    config: ImapConfig,
    /// Per-mailbox sync state.
    sync_states: HashMap<String, MailboxSyncState>,
}

impl ImapClient {
    /// Create a new IMAP client with the given configuration.
    pub fn new(config: ImapConfig) -> Self {
        Self {
            config,
            sync_states: HashMap::new(),
        }
    }

    /// Returns the IMAP configuration.
    pub fn config(&self) -> &ImapConfig {
        &self.config
    }

    /// Returns the current sync state for a mailbox.
    pub fn sync_state(&self, mailbox: &str) -> Option<&MailboxSyncState> {
        self.sync_states.get(mailbox)
    }

    /// Determine the starting UID for a fetch based on the mailbox's
    /// current UIDVALIDITY and our stored sync state.
    ///
    /// Returns the UID to start fetching from and whether a full
    /// re-sync is needed (UIDVALIDITY mismatch).
    pub fn compute_start_uid(&self, mailbox: &str, uid_validity: u32) -> (u32, bool) {
        match self.sync_states.get(mailbox) {
            Some(state) if state.uid_validity == uid_validity && state.last_uid > 0 => {
                (state.last_uid + 1, false)
            }
            Some(_) => {
                tracing::info!(
                    mailbox = mailbox,
                    "UIDVALIDITY changed, full re-sync required"
                );
                (1, true)
            }
            None => (1, false),
        }
    }

    /// Update the sync state for a mailbox after a successful fetch.
    pub fn update_sync_state(
        &mut self,
        mailbox: &str,
        uid_validity: u32,
        last_uid: u32,
    ) {
        self.sync_states.insert(
            mailbox.to_string(),
            MailboxSyncState {
                uid_validity,
                last_uid,
            },
        );
    }

    /// Process raw fetch responses into `FetchedMessage` values,
    /// filtering out any UIDs below `start_uid`.
    ///
    /// Returns the messages and the maximum UID seen.
    pub fn process_fetched(
        start_uid: u32,
        fetches: &[(u32, Vec<u8>)],
    ) -> (Vec<FetchedMessage>, u32) {
        let mut messages = Vec::new();
        let mut max_uid = start_uid.saturating_sub(1);

        for (uid, body) in fetches {
            if *uid < start_uid {
                continue;
            }
            messages.push(FetchedMessage {
                uid: *uid,
                raw: body.clone(),
            });
            if *uid > max_uid {
                max_uid = *uid;
            }
        }

        (messages, max_uid)
    }

    /// Connect and authenticate with the IMAP server.
    ///
    /// This is the integration entry point that establishes a real
    /// network connection. The `password` parameter should be retrieved
    /// from the credential store using the config's `credential_key`.
    ///
    /// Note: The `async-imap` crate uses `futures_io` traits, not
    /// `tokio::io`. This method uses `async-std` compatible streams
    /// internally.
    #[cfg(feature = "integration")]
    pub async fn connect(
        &self,
        password: &str,
    ) -> anyhow::Result<async_imap::Session<async_native_tls::TlsStream<async_std::net::TcpStream>>>
    {
        use anyhow::Context;
        use async_std::net::TcpStream;

        tracing::debug!(
            host = %self.config.host,
            port = %self.config.port,
            "connecting to IMAP server"
        );

        let connect_timeout = std::time::Duration::from_secs(30);

        let tcp = tokio::time::timeout(
            connect_timeout,
            TcpStream::connect((&*self.config.host, self.config.port)),
        )
        .await
        .map_err(|_| anyhow::anyhow!("IMAP connection timed out after {connect_timeout:?}"))?
        .context("failed to connect to IMAP server")?;

        let tls = async_native_tls::TlsConnector::new();
        let tls_stream = tokio::time::timeout(connect_timeout, tls.connect(&self.config.host, tcp))
            .await
            .map_err(|_| anyhow::anyhow!("IMAP TLS handshake timed out after {connect_timeout:?}"))?
            .context("TLS handshake failed")?;

        let client = async_imap::Client::new(tls_stream);
        let session = client
            .login(&self.config.username, password)
            .await
            .map_err(|e| anyhow::anyhow!("IMAP login failed: {}", e.0))?;

        tracing::info!(host = %self.config.host, "IMAP session established");
        Ok(session)
    }

    /// Connect and authenticate with the IMAP server over a plain TCP
    /// connection (no TLS).
    ///
    /// Used for testing against GreenMail's non-TLS IMAP port.
    /// Should NOT be used in production. The `password` parameter should
    /// be retrieved from the credential store using the config's `credential_key`.
    #[cfg(feature = "integration")]
    pub async fn connect_plain(
        &self,
        password: &str,
    ) -> anyhow::Result<async_imap::Session<async_std::net::TcpStream>> {
        use anyhow::Context;
        use async_std::net::TcpStream;

        tracing::debug!(
            host = %self.config.host,
            port = %self.config.port,
            "connecting to IMAP server (plain, no TLS)"
        );

        let connect_timeout = std::time::Duration::from_secs(30);

        let tcp = tokio::time::timeout(
            connect_timeout,
            TcpStream::connect((&*self.config.host, self.config.port)),
        )
        .await
        .map_err(|_| anyhow::anyhow!("IMAP connection timed out after {connect_timeout:?}"))?
        .context("failed to connect to IMAP server")?;

        let client = async_imap::Client::new(tcp);
        let session = client
            .login(&self.config.username, password)
            .await
            .map_err(|e| anyhow::anyhow!("IMAP login failed: {}", e.0))?;

        tracing::info!(host = %self.config.host, "IMAP session established (plain)");
        Ok(session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imap_config_serialization() {
        let config = ImapConfig {
            host: "imap.example.com".into(),
            port: 993,
            username: "user@example.com".into(),
            credential_key: "imap_password".into(),
            use_tls: true,
        };
        let json = serde_json::to_string(&config).expect("serialize");
        let restored: ImapConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.host, "imap.example.com");
        assert_eq!(restored.port, 993);
        assert!(restored.use_tls);
    }

    #[test]
    fn imap_client_construction() {
        let config = ImapConfig {
            host: "imap.example.com".into(),
            port: 993,
            username: "user@example.com".into(),
            credential_key: "imap_password".into(),
            use_tls: true,
        };
        let client = ImapClient::new(config);
        assert_eq!(client.config().host, "imap.example.com");
        assert!(client.sync_state("INBOX").is_none());
    }

    #[test]
    fn mailbox_sync_state_default() {
        let state = MailboxSyncState::default();
        assert_eq!(state.uid_validity, 0);
        assert_eq!(state.last_uid, 0);
    }

    #[test]
    fn fetched_message_stores_raw_data() {
        let msg = FetchedMessage {
            uid: 42,
            raw: b"From: test@example.com\r\n\r\nBody".to_vec(),
        };
        assert_eq!(msg.uid, 42);
        assert!(!msg.raw.is_empty());
    }

    #[test]
    fn compute_start_uid_first_sync() {
        let client = ImapClient::new(ImapConfig {
            host: "imap.example.com".into(),
            port: 993,
            username: "user".into(),
            credential_key: "imap_password".into(),
            use_tls: true,
        });
        let (start, resync) = client.compute_start_uid("INBOX", 12345);
        assert_eq!(start, 1);
        assert!(!resync);
    }

    #[test]
    fn compute_start_uid_incremental() {
        let mut client = ImapClient::new(ImapConfig {
            host: "imap.example.com".into(),
            port: 993,
            username: "user".into(),
            credential_key: "imap_password".into(),
            use_tls: true,
        });
        client.update_sync_state("INBOX", 12345, 100);

        let (start, resync) = client.compute_start_uid("INBOX", 12345);
        assert_eq!(start, 101);
        assert!(!resync);
    }

    #[test]
    fn compute_start_uid_validity_changed() {
        let mut client = ImapClient::new(ImapConfig {
            host: "imap.example.com".into(),
            port: 993,
            username: "user".into(),
            credential_key: "imap_password".into(),
            use_tls: true,
        });
        client.update_sync_state("INBOX", 12345, 100);

        let (start, resync) = client.compute_start_uid("INBOX", 99999);
        assert_eq!(start, 1);
        assert!(resync);
    }

    #[test]
    fn update_sync_state() {
        let mut client = ImapClient::new(ImapConfig {
            host: "imap.example.com".into(),
            port: 993,
            username: "user".into(),
            credential_key: "imap_password".into(),
            use_tls: true,
        });
        assert!(client.sync_state("INBOX").is_none());

        client.update_sync_state("INBOX", 12345, 50);
        let state = client.sync_state("INBOX").expect("should have state");
        assert_eq!(state.uid_validity, 12345);
        assert_eq!(state.last_uid, 50);
    }

    #[test]
    fn process_fetched_filters_old_uids() {
        let fetches = vec![
            (5u32, b"old message".to_vec()),
            (10, b"new message 1".to_vec()),
            (15, b"new message 2".to_vec()),
        ];
        let (messages, max_uid) = ImapClient::process_fetched(10, &fetches);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].uid, 10);
        assert_eq!(messages[1].uid, 15);
        assert_eq!(max_uid, 15);
    }

    #[test]
    fn process_fetched_empty() {
        let fetches: Vec<(u32, Vec<u8>)> = vec![];
        let (messages, max_uid) = ImapClient::process_fetched(1, &fetches);
        assert!(messages.is_empty());
        assert_eq!(max_uid, 0);
    }

    #[test]
    fn process_fetched_all_below_start() {
        let fetches = vec![
            (1u32, b"msg1".to_vec()),
            (2, b"msg2".to_vec()),
        ];
        let (messages, max_uid) = ImapClient::process_fetched(10, &fetches);
        assert!(messages.is_empty());
        assert_eq!(max_uid, 9);
    }

    #[test]
    fn sync_state_serialization() {
        let state = MailboxSyncState {
            uid_validity: 12345,
            last_uid: 100,
        };
        let json = serde_json::to_string(&state).expect("serialize");
        let restored: MailboxSyncState =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.uid_validity, 12345);
        assert_eq!(restored.last_uid, 100);
    }
}
