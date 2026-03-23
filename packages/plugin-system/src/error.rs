//! Plugin system error types implementing the EngineError trait.

use life_engine_traits::{EngineError, Severity};
use thiserror::Error;

/// Errors that can occur in the plugin system.
#[derive(Debug, Error)]
pub enum PluginError {
    /// Plugin directory does not exist or is inaccessible.
    #[error("plugin directory not found: {0}")]
    DirectoryNotFound(String),

    /// Failed to read a directory entry during scanning.
    #[error("failed to read directory entry: {0}")]
    DirectoryScanFailed(String),

    /// Plugin manifest is missing or invalid.
    #[error("invalid plugin manifest: {0}")]
    ManifestInvalid(String),

    /// Plugin manifest is missing a required field.
    #[error("manifest missing required field '{field}' for plugin at {path}")]
    ManifestMissingField {
        /// The missing field name.
        field: String,
        /// Path to the manifest file.
        path: String,
    },

    /// WASM binary failed to load.
    #[error("failed to load WASM binary: {0}")]
    WasmLoadFailed(String),

    /// Capability violation at load time (CAP_001).
    #[error("capability violation: {0}")]
    CapabilityViolation(String),

    /// Capability violation at runtime — defence-in-depth check (CAP_002).
    #[error("runtime capability violation: {0}")]
    RuntimeCapabilityViolation(String),

    /// Plugin execution failed.
    #[error("plugin execution failed: {0}")]
    ExecutionFailed(String),

    /// Lifecycle state transition error.
    #[error("lifecycle error: {0}")]
    LifecycleError(String),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl EngineError for PluginError {
    fn code(&self) -> &str {
        match self {
            PluginError::DirectoryNotFound(_) => "PLUGIN_001",
            PluginError::DirectoryScanFailed(_) => "PLUGIN_002",
            PluginError::ManifestInvalid(_) => "PLUGIN_003",
            PluginError::ManifestMissingField { .. } => "PLUGIN_004",
            PluginError::WasmLoadFailed(_) => "PLUGIN_005",
            PluginError::CapabilityViolation(_) => "CAP_001",
            PluginError::RuntimeCapabilityViolation(_) => "CAP_002",
            PluginError::ExecutionFailed(_) => "PLUGIN_007",
            PluginError::LifecycleError(_) => "PLUGIN_008",
            PluginError::Io(_) => "PLUGIN_009",
        }
    }

    fn severity(&self) -> Severity {
        match self {
            PluginError::DirectoryNotFound(_) => Severity::Fatal,
            PluginError::DirectoryScanFailed(_) => Severity::Retryable,
            PluginError::ManifestInvalid(_) => Severity::Fatal,
            PluginError::ManifestMissingField { .. } => Severity::Fatal,
            PluginError::WasmLoadFailed(_) => Severity::Fatal,
            PluginError::CapabilityViolation(_) => Severity::Fatal,
            PluginError::RuntimeCapabilityViolation(_) => Severity::Fatal,
            PluginError::ExecutionFailed(_) => Severity::Retryable,
            PluginError::LifecycleError(_) => Severity::Fatal,
            PluginError::Io(_) => Severity::Retryable,
        }
    }

    fn source_module(&self) -> &str {
        "plugin-system"
    }
}
