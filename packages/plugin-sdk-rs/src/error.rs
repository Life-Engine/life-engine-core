//! Plugin error types for the Life Engine plugin SDK.
//!
//! Provides structured error variants that plugin actions return to
//! communicate hard failures and classify error types for the pipeline
//! executor's `on_error` strategy.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Structured error returned by plugin actions.
///
/// Each variant carries a human-readable message and an optional detail
/// string for diagnostics. The pipeline executor uses the variant to
/// decide how to handle the failure (e.g., retry, abort, skip).
///
/// # Variants
///
/// - `CapabilityDenied` — The plugin tried to use a capability it was not granted.
/// - `NotFound` — A requested resource does not exist.
/// - `ValidationError` — Input data failed validation.
/// - `StorageError` — A storage operation failed.
/// - `NetworkError` — An outbound HTTP or network request failed.
/// - `InternalError` — An unexpected internal error occurred.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "code")]
pub enum PluginError {
    /// The plugin attempted an operation requiring an ungranted capability.
    #[serde(rename = "CAPABILITY_DENIED")]
    CapabilityDenied {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    /// A requested resource was not found.
    #[serde(rename = "NOT_FOUND")]
    NotFound {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    /// Input data failed validation.
    #[serde(rename = "VALIDATION_ERROR")]
    ValidationError {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    /// A storage read or write operation failed.
    #[serde(rename = "STORAGE_ERROR")]
    StorageError {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    /// An outbound HTTP or network call failed.
    #[serde(rename = "NETWORK_ERROR")]
    NetworkError {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    /// An unexpected internal error.
    #[serde(rename = "INTERNAL_ERROR")]
    InternalError {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
}

impl PluginError {
    /// Returns the structured error code string.
    pub fn code(&self) -> &str {
        match self {
            PluginError::CapabilityDenied { .. } => "CAPABILITY_DENIED",
            PluginError::NotFound { .. } => "NOT_FOUND",
            PluginError::ValidationError { .. } => "VALIDATION_ERROR",
            PluginError::StorageError { .. } => "STORAGE_ERROR",
            PluginError::NetworkError { .. } => "NETWORK_ERROR",
            PluginError::InternalError { .. } => "INTERNAL_ERROR",
        }
    }

    /// Returns the human-readable error message.
    pub fn message(&self) -> &str {
        match self {
            PluginError::CapabilityDenied { message, .. }
            | PluginError::NotFound { message, .. }
            | PluginError::ValidationError { message, .. }
            | PluginError::StorageError { message, .. }
            | PluginError::NetworkError { message, .. }
            | PluginError::InternalError { message, .. } => message,
        }
    }

    /// Returns the optional detail string.
    pub fn detail(&self) -> Option<&str> {
        match self {
            PluginError::CapabilityDenied { detail, .. }
            | PluginError::NotFound { detail, .. }
            | PluginError::ValidationError { detail, .. }
            | PluginError::StorageError { detail, .. }
            | PluginError::NetworkError { detail, .. }
            | PluginError::InternalError { detail, .. } => detail.as_deref(),
        }
    }
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code(), self.message())
    }
}

impl std::error::Error for PluginError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_error_code_returns_correct_string() {
        let cases = vec![
            (
                PluginError::CapabilityDenied {
                    message: "denied".into(),
                    detail: None,
                },
                "CAPABILITY_DENIED",
            ),
            (
                PluginError::NotFound {
                    message: "missing".into(),
                    detail: None,
                },
                "NOT_FOUND",
            ),
            (
                PluginError::ValidationError {
                    message: "invalid".into(),
                    detail: None,
                },
                "VALIDATION_ERROR",
            ),
            (
                PluginError::StorageError {
                    message: "db fail".into(),
                    detail: None,
                },
                "STORAGE_ERROR",
            ),
            (
                PluginError::NetworkError {
                    message: "timeout".into(),
                    detail: None,
                },
                "NETWORK_ERROR",
            ),
            (
                PluginError::InternalError {
                    message: "panic".into(),
                    detail: None,
                },
                "INTERNAL_ERROR",
            ),
        ];
        for (err, expected_code) in cases {
            assert_eq!(err.code(), expected_code);
        }
    }

    #[test]
    fn plugin_error_message_and_detail() {
        let err = PluginError::StorageError {
            message: "write failed".into(),
            detail: Some("disk full".into()),
        };
        assert_eq!(err.message(), "write failed");
        assert_eq!(err.detail(), Some("disk full"));
    }

    #[test]
    fn plugin_error_detail_none() {
        let err = PluginError::NotFound {
            message: "not found".into(),
            detail: None,
        };
        assert_eq!(err.detail(), None);
    }

    #[test]
    fn plugin_error_display() {
        let err = PluginError::NetworkError {
            message: "connection refused".into(),
            detail: None,
        };
        assert_eq!(err.to_string(), "[NETWORK_ERROR] connection refused");
    }

    #[test]
    fn plugin_error_serialization_roundtrip() {
        let err = PluginError::ValidationError {
            message: "field required".into(),
            detail: Some("missing 'email'".into()),
        };
        let json = serde_json::to_string(&err).expect("serialize");
        let restored: PluginError = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.code(), "VALIDATION_ERROR");
        assert_eq!(restored.message(), "field required");
        assert_eq!(restored.detail(), Some("missing 'email'"));
    }

    #[test]
    fn plugin_error_is_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(PluginError::InternalError {
            message: "oops".into(),
            detail: None,
        });
        assert!(err.to_string().contains("INTERNAL_ERROR"));
    }
}
