//! CardDAV client for syncing contacts from CardDAV servers.
//!
//! Handles connection configuration and sync state tracking for
//! incremental sync using sync-token or ctag mechanisms.

use dav_utils::etag::DavResource;
use dav_utils::sync_state::DavSyncState;
use serde::{Deserialize, Serialize};

/// Configuration for a CardDAV connection.
///
/// The password is not stored in this config struct. Instead, the
/// `credential_key` names the key under which the password is stored
/// in the credential store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardDavConfig {
    /// The CardDAV server URL (e.g. `https://dav.example.com`).
    pub server_url: String,
    /// The username for authentication.
    pub username: String,
    /// The key used to look up the password in the credential store.
    #[serde(default = "default_carddav_credential_key")]
    pub credential_key: String,
    /// The path to the address book (e.g. `/addressbooks/user/default/`).
    pub addressbook_path: String,
}

/// Default credential key for CardDAV passwords.
fn default_carddav_credential_key() -> String {
    "carddav_password".to_string()
}

/// Tracks sync state for a CardDAV address book.
///
/// This is a type alias for the shared [`DavSyncState`] type.
pub type SyncState = DavSyncState;

/// A fetched vCard resource from CardDAV.
#[derive(Debug, Clone)]
pub struct FetchedVCard {
    /// The href/path of this resource on the server.
    pub href: String,
    /// The ETag for this resource version.
    pub etag: String,
    /// The raw vCard text data.
    pub data: String,
}

impl DavResource for FetchedVCard {
    fn href(&self) -> &str {
        &self.href
    }
    fn etag(&self) -> &str {
        &self.etag
    }
}

/// CardDAV client that manages connection configuration and sync state.
///
/// The actual HTTP/WebDAV session lifecycle is handled by the
/// integration layer. This struct tracks which resources have been
/// synced so incremental sync works correctly.
pub struct CardDavClient {
    /// Connection configuration.
    config: CardDavConfig,
    /// Sync state for the configured address book.
    sync_state: SyncState,
}

impl CardDavClient {
    /// Create a new CardDAV client with the given configuration.
    pub fn new(config: CardDavConfig) -> Self {
        Self {
            config,
            sync_state: SyncState::default(),
        }
    }

    /// Returns the CardDAV configuration.
    pub fn config(&self) -> &CardDavConfig {
        &self.config
    }

    /// Returns the current sync state.
    pub fn sync_state(&self) -> &SyncState {
        &self.sync_state
    }

    /// Build the full URL for the configured address book.
    pub fn addressbook_url(&self) -> String {
        dav_utils::url::join_dav_url(&self.config.server_url, &self.config.addressbook_path)
    }

    /// Build the HTTP Basic Authorization header value.
    ///
    /// The `password` parameter should be retrieved from the credential
    /// store using the config's `credential_key`.
    pub fn auth_header(&self, password: &str) -> String {
        dav_utils::auth::basic_auth_header(&self.config.username, password)
    }

    /// Update the sync token after a successful sync.
    pub fn update_sync_token(&mut self, token: String) {
        self.sync_state.sync_token = Some(token);
    }

    /// Update the ctag after a successful sync.
    pub fn update_ctag(&mut self, ctag: String) {
        self.sync_state.ctag = Some(ctag);
    }

    /// Update the ETag for a specific resource.
    pub fn update_etag(&mut self, href: &str, etag: String) {
        self.sync_state.etags.insert(href.to_string(), etag);
    }

    /// Remove a resource from the tracked ETags (deleted on server).
    pub fn remove_etag(&mut self, href: &str) {
        self.sync_state.etags.remove(href);
    }

    /// Determine which fetched vCards are new or changed compared to
    /// our stored ETags.
    ///
    /// Returns only those vCards whose ETag differs from what we have stored.
    pub fn filter_changed(&self, fetched: &[FetchedVCard]) -> Vec<FetchedVCard> {
        dav_utils::etag::filter_changed(&self.sync_state.etags, fetched)
    }

    /// Reset sync state entirely (e.g. when server indicates full re-sync needed).
    pub fn reset_sync_state(&mut self) {
        self.sync_state = SyncState::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> CardDavConfig {
        CardDavConfig {
            server_url: "https://dav.example.com".into(),
            username: "user@example.com".into(),
            credential_key: "carddav_password".into(),
            addressbook_path: "/addressbooks/user/default/".into(),
        }
    }

    #[test]
    fn carddav_config_serialization() {
        let config = test_config();
        let json = serde_json::to_string(&config).expect("serialize");
        let restored: CardDavConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.server_url, "https://dav.example.com");
        assert_eq!(restored.username, "user@example.com");
        assert_eq!(restored.addressbook_path, "/addressbooks/user/default/");
    }

    #[test]
    fn carddav_client_construction() {
        let client = CardDavClient::new(test_config());
        assert_eq!(client.config().server_url, "https://dav.example.com");
        assert!(client.sync_state().sync_token.is_none());
        assert!(client.sync_state().ctag.is_none());
        assert!(client.sync_state().etags.is_empty());
    }

    #[test]
    fn addressbook_url_construction() {
        let client = CardDavClient::new(test_config());
        assert_eq!(
            client.addressbook_url(),
            "https://dav.example.com/addressbooks/user/default/"
        );
    }

    #[test]
    fn addressbook_url_normalizes_slashes() {
        let config = CardDavConfig {
            server_url: "https://dav.example.com/".into(),
            addressbook_path: "/addressbooks/user/default/".into(),
            ..test_config()
        };
        let client = CardDavClient::new(config);
        assert_eq!(
            client.addressbook_url(),
            "https://dav.example.com/addressbooks/user/default/"
        );
    }

    #[test]
    fn auth_header_is_valid_basic() {
        let client = CardDavClient::new(test_config());
        let header = client.auth_header("secret");
        assert!(header.starts_with("Basic "));

        // Decode and verify
        use base64::Engine;
        let encoded = header.strip_prefix("Basic ").unwrap();
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .expect("valid base64");
        let creds = String::from_utf8(decoded).expect("valid utf8");
        assert_eq!(creds, "user@example.com:secret");
    }

    #[test]
    fn sync_state_management() {
        let mut client = CardDavClient::new(test_config());

        // Update sync token
        client.update_sync_token("token-123".into());
        assert_eq!(
            client.sync_state().sync_token.as_deref(),
            Some("token-123")
        );

        // Update ctag
        client.update_ctag("ctag-456".into());
        assert_eq!(client.sync_state().ctag.as_deref(), Some("ctag-456"));

        // Update ETags
        client.update_etag("/contact/1.vcf", "etag-aaa".into());
        client.update_etag("/contact/2.vcf", "etag-bbb".into());
        assert_eq!(client.sync_state().etags.len(), 2);

        // Remove an ETag
        client.remove_etag("/contact/1.vcf");
        assert_eq!(client.sync_state().etags.len(), 1);
        assert!(!client.sync_state().etags.contains_key("/contact/1.vcf"));
    }

    #[test]
    fn sync_state_reset() {
        let mut client = CardDavClient::new(test_config());
        client.update_sync_token("token-123".into());
        client.update_ctag("ctag-456".into());
        client.update_etag("/contact/1.vcf", "etag-aaa".into());

        client.reset_sync_state();
        assert!(client.sync_state().sync_token.is_none());
        assert!(client.sync_state().ctag.is_none());
        assert!(client.sync_state().etags.is_empty());
    }

    #[test]
    fn sync_state_serialization() {
        let mut state = SyncState {
            sync_token: Some("token-123".into()),
            ctag: Some("ctag-456".into()),
            ..Default::default()
        };
        state.etags.insert("/contact/1.vcf".into(), "etag-aaa".into());

        let json = serde_json::to_string(&state).expect("serialize");
        let restored: SyncState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.sync_token.as_deref(), Some("token-123"));
        assert_eq!(restored.ctag.as_deref(), Some("ctag-456"));
        assert_eq!(restored.etags.len(), 1);
    }

    #[test]
    fn filter_changed_detects_new_resources() {
        let client = CardDavClient::new(test_config());
        let fetched = vec![FetchedVCard {
            href: "/contact/new.vcf".into(),
            etag: "etag-new".into(),
            data: "BEGIN:VCARD\nEND:VCARD".into(),
        }];
        let changed = client.filter_changed(&fetched);
        assert_eq!(changed.len(), 1);
    }

    #[test]
    fn filter_changed_detects_modified_resources() {
        let mut client = CardDavClient::new(test_config());
        client.update_etag("/contact/1.vcf", "etag-old".into());

        let fetched = vec![FetchedVCard {
            href: "/contact/1.vcf".into(),
            etag: "etag-new".into(),
            data: "BEGIN:VCARD\nEND:VCARD".into(),
        }];
        let changed = client.filter_changed(&fetched);
        assert_eq!(changed.len(), 1);
    }

    #[test]
    fn filter_changed_skips_unchanged_resources() {
        let mut client = CardDavClient::new(test_config());
        client.update_etag("/contact/1.vcf", "etag-same".into());

        let fetched = vec![FetchedVCard {
            href: "/contact/1.vcf".into(),
            etag: "etag-same".into(),
            data: "BEGIN:VCARD\nEND:VCARD".into(),
        }];
        let changed = client.filter_changed(&fetched);
        assert!(changed.is_empty());
    }
}
