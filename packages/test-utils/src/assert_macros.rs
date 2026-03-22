//! Declarative assertion macros for Life Engine tests.
//!
//! Provides reusable assertion macros that eliminate boilerplate across
//! connector and plugin test suites.

/// Assert that a value survives a JSON serialization round-trip.
///
/// Serializes the value to JSON, deserializes it back, and asserts equality.
///
/// # Example
///
/// ```rust
/// use life_engine_test_utils::assert_serialization_roundtrip;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// struct MyType { x: i32 }
///
/// let val = MyType { x: 42 };
/// assert_serialization_roundtrip!(val, MyType);
/// ```
#[macro_export]
macro_rules! assert_serialization_roundtrip {
    ($value:expr, $type:ty) => {{
        let json =
            serde_json::to_string(&$value).expect("serialization should succeed");
        let restored: $type =
            serde_json::from_str(&json).expect("deserialization should succeed");
        assert_eq!(
            $value, restored,
            "value should survive JSON round-trip without data loss"
        );
    }};
}

/// Assert that a plugin's metadata fields match expected values.
///
/// Calls `id()`, `display_name()`, and `version()` on the plugin and
/// asserts each equals the corresponding expected string.
///
/// # Example
///
/// ```rust,ignore
/// use life_engine_test_utils::assert_plugin_metadata;
///
/// let plugin = MyPlugin::new();
/// assert_plugin_metadata!(plugin, "com.example.my-plugin", "My Plugin", "0.1.0");
/// ```
#[macro_export]
macro_rules! assert_plugin_metadata {
    ($plugin:expr, $id:expr, $display_name:expr, $version:expr) => {{
        assert_eq!(
            $plugin.id(),
            $id,
            "plugin id mismatch"
        );
        assert_eq!(
            $plugin.display_name(),
            $display_name,
            "plugin display_name mismatch"
        );
        assert_eq!(
            $plugin.version(),
            $version,
            "plugin version mismatch"
        );
    }};
}

/// Assert that a plugin declares exactly the expected capabilities.
///
/// Checks both the count and the presence of each capability.
///
/// # Example
///
/// ```rust,ignore
/// use life_engine_test_utils::assert_plugin_capabilities;
/// use life_engine_plugin_sdk::types::Capability;
///
/// let plugin = MyPlugin::new();
/// assert_plugin_capabilities!(plugin, [
///     Capability::StorageRead,
///     Capability::StorageWrite,
/// ]);
/// ```
#[macro_export]
macro_rules! assert_plugin_capabilities {
    ($plugin:expr, $expected_caps:expr) => {{
        let caps = $plugin.capabilities();
        let expected: Vec<life_engine_plugin_sdk::types::Capability> =
            $expected_caps.to_vec();
        assert_eq!(
            caps.len(),
            expected.len(),
            "capability count mismatch: got {:?}",
            caps
        );
        for cap in &expected {
            assert!(
                caps.contains(cap),
                "missing expected capability {:?} in {:?}",
                cap,
                caps
            );
        }
    }};
}

/// Assert that a plugin's routes contain exactly the expected paths.
///
/// Verifies the route count and that each expected path is present.
///
/// # Example
///
/// ```rust,ignore
/// use life_engine_test_utils::assert_plugin_routes;
///
/// let plugin = MyPlugin::new();
/// assert_plugin_routes!(plugin, ["/sync", "/status"]);
/// ```
#[macro_export]
macro_rules! assert_plugin_routes {
    ($plugin:expr, $expected_paths:expr) => {{
        let routes = $plugin.routes();
        let expected: Vec<&str> = $expected_paths.to_vec();
        assert_eq!(
            routes.len(),
            expected.len(),
            "route count mismatch: got {:?}",
            routes.iter().map(|r| r.path.as_str()).collect::<Vec<_>>()
        );
        let paths: Vec<&str> = routes.iter().map(|r| r.path.as_str()).collect();
        for expected_path in &expected {
            assert!(
                paths.contains(expected_path),
                "missing expected route '{}' in {:?}",
                expected_path,
                paths
            );
        }
    }};
}

/// Assert that a Basic Authorization header value encodes the expected
/// username and password.
///
/// Decodes the base64 payload after stripping the `Basic ` prefix and
/// asserts the decoded string equals `username:password`.
///
/// # Example
///
/// ```rust
/// use life_engine_test_utils::assert_basic_auth_header;
///
/// let header = "Basic dXNlcjpwYXNz"; // base64("user:pass")
/// assert_basic_auth_header!(header, "user", "pass");
/// ```
#[macro_export]
macro_rules! assert_basic_auth_header {
    ($header_value:expr, $username:expr, $password:expr) => {{
        let header: &str = $header_value.as_ref();
        assert!(
            header.starts_with("Basic "),
            "auth header must start with 'Basic ', got: {}",
            header
        );
        let encoded = header
            .strip_prefix("Basic ")
            .expect("should have 'Basic ' prefix");
        use base64::Engine as _;
        let decoded_bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .expect("auth header should contain valid base64");
        let decoded = String::from_utf8(decoded_bytes)
            .expect("decoded auth header should be valid UTF-8");
        let expected = format!("{}:{}", $username, $password);
        assert_eq!(
            decoded, expected,
            "auth header credentials mismatch"
        );
    }};
}

/// Assert that a sync state is in its initial (empty) condition.
///
/// Checks that `sync_token` and `ctag` are `None` and that `etags` is empty.
/// Works with any type that has fields matching the CalDAV/CardDAV `SyncState` shape.
///
/// # Example
///
/// ```rust,ignore
/// use life_engine_test_utils::assert_sync_state_empty;
///
/// let state = SyncState::default();
/// assert_sync_state_empty!(state);
/// ```
#[macro_export]
macro_rules! assert_sync_state_empty {
    ($state:expr) => {{
        assert!(
            $state.sync_token.is_none(),
            "sync_token should be None, got: {:?}",
            $state.sync_token
        );
        assert!(
            $state.ctag.is_none(),
            "ctag should be None, got: {:?}",
            $state.ctag
        );
        assert!(
            $state.etags.is_empty(),
            "etags should be empty, got {} entries",
            $state.etags.len()
        );
    }};
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestStruct {
        x: i32,
        y: String,
    }

    #[test]
    fn assert_serialization_roundtrip_passes() {
        let val = TestStruct {
            x: 42,
            y: "hello".into(),
        };
        assert_serialization_roundtrip!(val, TestStruct);
    }

    #[test]
    fn assert_basic_auth_header_passes() {
        use base64::Engine as _;
        let creds = "alice:wonderland";
        let encoded =
            base64::engine::general_purpose::STANDARD.encode(creds);
        let header = format!("Basic {}", encoded);
        assert_basic_auth_header!(header, "alice", "wonderland");
    }

    #[test]
    #[should_panic(expected = "auth header must start with 'Basic '")]
    fn assert_basic_auth_header_rejects_non_basic() {
        assert_basic_auth_header!("Bearer token123", "user", "pass");
    }

    #[test]
    fn assert_sync_state_empty_passes_on_default() {
        #[derive(Debug)]
        struct MockSyncState {
            sync_token: Option<String>,
            ctag: Option<String>,
            etags: HashMap<String, String>,
        }

        let state = MockSyncState {
            sync_token: None,
            ctag: None,
            etags: HashMap::new(),
        };
        assert_sync_state_empty!(state);
    }

    #[test]
    #[should_panic(expected = "sync_token should be None")]
    fn assert_sync_state_empty_rejects_non_empty_token() {
        #[derive(Debug)]
        struct MockSyncState {
            sync_token: Option<String>,
            ctag: Option<String>,
            etags: HashMap<String, String>,
        }

        let state = MockSyncState {
            sync_token: Some("abc".into()),
            ctag: None,
            etags: HashMap::new(),
        };
        assert_sync_state_empty!(state);
    }
}
