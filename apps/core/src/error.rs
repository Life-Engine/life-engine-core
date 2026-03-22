//! Core error types for the Life Engine backend.
//!
//! Uses `thiserror` for structured, typed errors. Application-level
//! code propagates errors with `anyhow`.

use thiserror::Error;

/// Errors that can occur in the Core binary.
#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum CoreError {
    /// Configuration file could not be read or parsed.
    #[error("config error: {0}")]
    Config(String),

    /// A plugin failed during its lifecycle.
    #[error("plugin error (plugin={plugin_id}): {message}")]
    Plugin {
        /// The ID of the plugin that failed.
        plugin_id: String,
        /// A description of what went wrong.
        message: String,
    },

    /// The storage subsystem encountered an error.
    #[error("storage error: {0}")]
    Storage(String),

    /// A database rekey (passphrase change) operation failed.
    #[error("rekey error: {0}")]
    Rekey(String),

    /// TLS configuration or certificate loading failed.
    #[error("TLS error: {0}")]
    Tls(String),

    /// The message bus encountered an error.
    #[error("message bus error: {0}")]
    MessageBus(String),

    /// A federation sync operation failed.
    #[error("federation error: {0}")]
    Federation(String),

    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_error_display() {
        let err = CoreError::Config("invalid port".into());
        assert_eq!(err.to_string(), "config error: invalid port");
    }

    #[test]
    fn plugin_error_display() {
        let err = CoreError::Plugin {
            plugin_id: "com.test.bad".into(),
            message: "on_load failed".into(),
        };
        assert_eq!(
            err.to_string(),
            "plugin error (plugin=com.test.bad): on_load failed"
        );
    }

    #[test]
    fn storage_error_display() {
        let err = CoreError::Storage("connection failed".into());
        assert_eq!(err.to_string(), "storage error: connection failed");
    }

    #[test]
    fn message_bus_error_display() {
        let err = CoreError::MessageBus("channel closed".into());
        assert_eq!(err.to_string(), "message bus error: channel closed");
    }

    #[test]
    fn io_error_converts() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let core_err = CoreError::from(io_err);
        assert!(core_err.to_string().contains("missing"));
    }
}
