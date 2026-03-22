use serde::Deserialize;

/// Configuration for the plugin.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginConfig {}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {}
    }
}
