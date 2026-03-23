//! Migration manifest parsing and validation.

use thiserror::Error;

use life_engine_traits::{EngineError, Severity};

pub mod manifest;
pub mod runner;
pub mod validate;

pub use manifest::{parse_migration_entries, parse_migration_entries_from_str, MigrationEntry};
pub use runner::{run_transform, run_transform_from_bytes};
pub use validate::{validate_wasm_exports, validate_wasm_exports_from_bytes};

/// Errors that can occur during migration manifest parsing and execution.
#[derive(Debug, Error)]
pub enum MigrationError {
    /// Failed to read or parse the manifest TOML.
    #[error("migration manifest parse error: {0}")]
    ManifestParse(String),

    /// Manifest content failed validation rules.
    #[error("migration manifest validation error: {0}")]
    ManifestValidation(String),

    /// A WASM transform function failed during execution.
    #[error("migration transform failed for '{function}': {cause}")]
    TransformFailed { function: String, cause: String },

    /// A WASM transform function crashed (panic/trap).
    #[error("migration transform crashed for '{function}': {cause}")]
    TransformCrashed { function: String, cause: String },
}

impl EngineError for MigrationError {
    fn code(&self) -> &str {
        match self {
            MigrationError::ManifestParse(_) => "MIGRATION_001",
            MigrationError::ManifestValidation(_) => "MIGRATION_002",
            MigrationError::TransformFailed { .. } => "MIGRATION_003",
            MigrationError::TransformCrashed { .. } => "MIGRATION_004",
        }
    }

    fn severity(&self) -> Severity {
        match self {
            MigrationError::ManifestParse(_) | MigrationError::ManifestValidation(_) => {
                Severity::Fatal
            }
            MigrationError::TransformFailed { .. } => Severity::Retryable,
            MigrationError::TransformCrashed { .. } => Severity::Fatal,
        }
    }

    fn source_module(&self) -> &str {
        "migration"
    }
}
