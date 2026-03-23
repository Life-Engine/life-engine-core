//! Config read host function for WASM plugins.
//!
//! Allows plugins to read their own config section from the Core config.
//! The host function only returns the calling plugin's own config — never
//! another plugin's config or global config.

use life_engine_traits::Capability;
use tracing::{debug, warn};

use crate::capability::ApprovedCapabilities;
use crate::error::PluginError;

/// Context passed to config host functions, containing the plugin's identity,
/// approved capabilities, and the plugin's config section.
#[derive(Clone, Debug)]
pub struct ConfigHostContext {
    /// The plugin ID making the config call.
    pub plugin_id: String,
    /// The plugin's approved capabilities.
    pub capabilities: ApprovedCapabilities,
    /// The plugin's config section as a JSON value.
    /// `None` means no config section exists for this plugin.
    pub plugin_config: Option<serde_json::Value>,
}

/// Reads the calling plugin's config section.
///
/// Returns the plugin-specific config as JSON bytes. If the plugin has no
/// config section, returns an empty JSON object `{}`.
pub fn host_config_read(ctx: &ConfigHostContext) -> Result<Vec<u8>, PluginError> {
    // Check capability
    if !ctx.capabilities.has(Capability::ConfigRead) {
        warn!(
            plugin_id = %ctx.plugin_id,
            "config:read capability violation"
        );
        return Err(PluginError::RuntimeCapabilityViolation(format!(
            "plugin '{}' lacks capability 'config:read'",
            ctx.plugin_id
        )));
    }

    debug!(
        plugin_id = %ctx.plugin_id,
        "executing config read"
    );

    // Return the plugin's config section, or an empty object if none exists
    let config = ctx
        .plugin_config
        .as_ref()
        .cloned()
        .unwrap_or(serde_json::json!({}));

    serde_json::to_vec(&config).map_err(|e| {
        PluginError::ExecutionFailed(format!(
            "failed to serialize config for plugin '{}': {e}",
            ctx.plugin_id
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashSet;

    // --- Helper functions ---

    fn make_capabilities(caps: &[Capability]) -> ApprovedCapabilities {
        let set: HashSet<Capability> = caps.iter().copied().collect();
        ApprovedCapabilities::new(set)
    }

    fn make_context(
        plugin_id: &str,
        caps: &[Capability],
        config: Option<serde_json::Value>,
    ) -> ConfigHostContext {
        ConfigHostContext {
            plugin_id: plugin_id.to_string(),
            capabilities: make_capabilities(caps),
            plugin_config: config,
        }
    }

    // --- Tests ---

    #[test]
    fn read_returns_plugin_specific_config_section() {
        let config = serde_json::json!({
            "poll_interval": "5m",
            "max_results": 100,
            "enabled": true
        });
        let ctx = make_context("test-plugin", &[Capability::ConfigRead], Some(config.clone()));

        let result = host_config_read(&ctx);

        assert!(result.is_ok(), "config read should succeed: {result:?}");
        let output: serde_json::Value = serde_json::from_slice(&result.unwrap()).unwrap();
        assert_eq!(output, config);
    }

    #[test]
    fn read_without_config_read_capability_returns_error() {
        let ctx = make_context(
            "test-plugin",
            &[Capability::StorageRead], // has storage, not config
            Some(serde_json::json!({"key": "value"})),
        );

        let result = host_config_read(&ctx);

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::RuntimeCapabilityViolation(_)));
        assert!(err.to_string().contains("config:read"));
        assert!(err.to_string().contains("test-plugin"));
    }

    #[test]
    fn nonexistent_config_section_returns_empty_object() {
        let ctx = make_context("test-plugin", &[Capability::ConfigRead], None);

        let result = host_config_read(&ctx);

        assert!(result.is_ok());
        let output: serde_json::Value = serde_json::from_slice(&result.unwrap()).unwrap();
        assert_eq!(output, serde_json::json!({}));
    }

    #[test]
    fn config_preserves_types() {
        let config = serde_json::json!({
            "count": 42,
            "ratio": 3.14,
            "active": true,
            "name": "test",
            "nested": {
                "inner_key": "inner_value",
                "list": [1, 2, 3]
            }
        });
        let ctx = make_context("test-plugin", &[Capability::ConfigRead], Some(config.clone()));

        let result = host_config_read(&ctx).unwrap();
        let output: serde_json::Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(output["count"], 42);
        assert_eq!(output["ratio"], 3.14);
        assert_eq!(output["active"], true);
        assert_eq!(output["name"], "test");
        assert_eq!(output["nested"]["inner_key"], "inner_value");
        assert_eq!(output["nested"]["list"], serde_json::json!([1, 2, 3]));
    }
}
