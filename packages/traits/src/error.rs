//! Engine error trait and severity types.
//!
//! Defines the `EngineError` trait that all module error types must implement,
//! and the `Severity` enum that classifies error handling behavior.

use std::fmt;

/// Classifies how the system should respond to an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    /// Abort the pipeline and run the error handler.
    Fatal,
    /// Retry up to the configured limit, then fail.
    Retryable,
    /// Log and continue.
    Warning,
}

impl Severity {
    /// Returns `true` if this severity is `Fatal`.
    pub fn is_fatal(&self) -> bool {
        matches!(self, Severity::Fatal)
    }

    /// Returns `true` if this severity is `Retryable`.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Severity::Retryable)
    }

    /// Returns `true` if this severity is `Warning`.
    pub fn is_warning(&self) -> bool {
        matches!(self, Severity::Warning)
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Fatal => write!(f, "Fatal"),
            Severity::Retryable => write!(f, "Retryable"),
            Severity::Warning => write!(f, "Warning"),
        }
    }
}

/// Trait for engine-level errors.
///
/// All module error types must implement this trait to provide structured
/// error codes, severity classification, and source module identification.
pub trait EngineError: std::error::Error + Send + Sync + 'static {
    /// Structured error code (e.g., "STORAGE_001", "AUTH_002", "WORKFLOW_003").
    fn code(&self) -> &str;

    /// How the system should respond to this error.
    fn severity(&self) -> Severity;

    /// The module that produced this error (e.g., "storage-sqlite", "auth", "workflow-engine").
    fn source_module(&self) -> &str;
}
