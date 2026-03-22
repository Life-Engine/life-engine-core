//! Unified DAV sync state tracking.
//!
//! Both CalDAV and CardDAV use the same sync primitives: an optional
//! `sync-token` (RFC 6578), an optional `ctag` (Apple extension), and
//! per-resource ETags. This module provides a single type that both
//! connectors can share.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Tracks sync state for a DAV collection (calendar or address book).
///
/// CalDAV and CardDAV servers expose either a `sync-token` (RFC 6578)
/// or a `ctag` (Apple extension) to detect collection-level changes.
/// Individual resources are tracked by their ETag.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DavSyncState {
    /// The sync-token returned by the server (RFC 6578 WebDAV sync).
    pub sync_token: Option<String>,
    /// The ctag (collection tag) from the server.
    pub ctag: Option<String>,
    /// ETags for individual resources, keyed by href.
    pub etags: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_is_empty() {
        let state = DavSyncState::default();
        assert!(state.sync_token.is_none());
        assert!(state.ctag.is_none());
        assert!(state.etags.is_empty());
    }

    #[test]
    fn state_with_all_fields() {
        let mut etags = HashMap::new();
        etags.insert("/res/1.ics".to_string(), "\"etag-1\"".to_string());
        etags.insert("/res/2.ics".to_string(), "\"etag-2\"".to_string());

        let state = DavSyncState {
            sync_token: Some("sync-abc".into()),
            ctag: Some("ctag-123".into()),
            etags,
        };

        assert_eq!(state.sync_token.as_deref(), Some("sync-abc"));
        assert_eq!(state.ctag.as_deref(), Some("ctag-123"));
        assert_eq!(state.etags.len(), 2);
    }

    #[test]
    fn serialization_roundtrip() {
        let mut etags = HashMap::new();
        etags.insert("/cal/event.ics".into(), "\"e1\"".into());

        let state = DavSyncState {
            sync_token: Some("token-x".into()),
            ctag: Some("ctag-y".into()),
            etags,
        };

        let json = serde_json::to_string(&state).expect("serialize");
        let restored: DavSyncState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(state, restored);
    }

    #[test]
    fn deserialization_from_json() {
        let json = r#"{"sync_token":"tk","ctag":null,"etags":{}}"#;
        let state: DavSyncState = serde_json::from_str(json).expect("deserialize");
        assert_eq!(state.sync_token.as_deref(), Some("tk"));
        assert!(state.ctag.is_none());
        assert!(state.etags.is_empty());
    }

    #[test]
    fn clone_is_independent() {
        let mut state = DavSyncState {
            sync_token: Some("a".into()),
            ctag: None,
            etags: HashMap::new(),
        };
        let cloned = state.clone();
        state.sync_token = Some("b".into());
        assert_eq!(cloned.sync_token.as_deref(), Some("a"));
        assert_eq!(state.sync_token.as_deref(), Some("b"));
    }

    #[test]
    fn partial_eq_works() {
        let a = DavSyncState::default();
        let b = DavSyncState::default();
        assert_eq!(a, b);

        let c = DavSyncState {
            sync_token: Some("x".into()),
            ..Default::default()
        };
        assert_ne!(a, c);
    }

    #[test]
    fn debug_format() {
        let state = DavSyncState::default();
        let debug = format!("{:?}", state);
        assert!(debug.contains("DavSyncState"));
    }
}
