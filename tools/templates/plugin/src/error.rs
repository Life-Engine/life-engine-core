use thiserror::Error;

/// Errors specific to this plugin.
#[derive(Debug, Error)]
pub enum PluginError {
    #[error("example error: {0}")]
    Example(String),
}
