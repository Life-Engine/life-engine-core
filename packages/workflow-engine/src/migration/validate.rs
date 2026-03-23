//! WASM export validation for migration transform functions.
//!
//! Verifies that every transform function name declared in a plugin's migration
//! entries actually exists as an exported function in the plugin's WASM binary.
//! This validation runs at plugin load time — a plugin with missing migration
//! exports is rejected before it can run.

use std::path::Path;

use wasmparser::{ExternalKind, Parser, Payload};

use crate::migration::MigrationEntry;
use crate::migration::MigrationError;

/// Validates that all transform export names from migration entries exist as
/// exported functions in the given WASM binary.
///
/// Parses the WASM module's export section and checks that each
/// `MigrationEntry.transform` name is present as a function export. Returns an
/// error listing all missing exports if any are absent.
pub fn validate_wasm_exports(
    wasm_path: &Path,
    entries: &[MigrationEntry],
) -> Result<(), MigrationError> {
    if entries.is_empty() {
        return Ok(());
    }

    let wasm_bytes = std::fs::read(wasm_path).map_err(|e| {
        MigrationError::ManifestValidation(format!(
            "failed to read WASM binary at {}: {e}",
            wasm_path.display()
        ))
    })?;

    validate_wasm_exports_from_bytes(&wasm_bytes, entries)
}

/// Validates transform exports against raw WASM bytes.
///
/// Useful for testing where WASM modules are generated in-memory.
pub fn validate_wasm_exports_from_bytes(
    wasm_bytes: &[u8],
    entries: &[MigrationEntry],
) -> Result<(), MigrationError> {
    if entries.is_empty() {
        return Ok(());
    }

    let exported_functions = parse_function_exports(wasm_bytes)?;

    let missing: Vec<&str> = entries
        .iter()
        .map(|e| e.transform.as_str())
        .filter(|name| !exported_functions.contains(&name.to_string()))
        .collect();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(MigrationError::ManifestValidation(format!(
            "WASM module is missing migration transform exports: {}",
            missing.join(", ")
        )))
    }
}

/// Parses a WASM binary and returns the names of all exported functions.
fn parse_function_exports(wasm_bytes: &[u8]) -> Result<Vec<String>, MigrationError> {
    let mut exports = Vec::new();

    for payload in Parser::new(0).parse_all(wasm_bytes) {
        let payload = payload.map_err(|e| {
            MigrationError::ManifestValidation(format!("invalid WASM binary: {e}"))
        })?;

        if let Payload::ExportSection(reader) = payload {
            for export in reader {
                let export = export.map_err(|e| {
                    MigrationError::ManifestValidation(format!(
                        "failed to parse WASM export: {e}"
                    ))
                })?;
                if export.kind == ExternalKind::Func {
                    exports.push(export.name.to_string());
                }
            }
        }
    }

    Ok(exports)
}

#[cfg(test)]
mod tests {
    use super::*;
    use semver::Version;
    use std::fs;
    use tempfile::TempDir;

    /// Creates a minimal WASM module exporting the given function names.
    fn wasm_with_exports(names: &[&str]) -> Vec<u8> {
        let funcs: String = names
            .iter()
            .map(|name| format!("(func (export \"{name}\") (result i32) (i32.const 0))"))
            .collect::<Vec<_>>()
            .join("\n                ");

        let wat = format!(
            r#"(module
                (memory (export "memory") 1)
                {funcs}
            )"#
        );

        wat::parse_str(&wat).expect("failed to compile WAT")
    }

    fn entry(transform: &str) -> MigrationEntry {
        MigrationEntry {
            from: "1.0.x".to_string(),
            to: Version::new(2, 0, 0),
            transform: transform.to_string(),
            description: "test migration".to_string(),
            collection: "items".to_string(),
        }
    }

    #[test]
    fn all_exports_present() {
        let wasm = wasm_with_exports(&["migrate_v1_to_v2", "migrate_v2_to_v3"]);
        let entries = vec![entry("migrate_v1_to_v2"), entry("migrate_v2_to_v3")];

        let result = validate_wasm_exports_from_bytes(&wasm, &entries);
        assert!(result.is_ok(), "expected Ok, got: {result:?}");
    }

    #[test]
    fn missing_single_export() {
        let wasm = wasm_with_exports(&["migrate_v1_to_v2"]);
        let entries = vec![entry("migrate_v1_to_v2"), entry("migrate_v2_to_v3")];

        let err = validate_wasm_exports_from_bytes(&wasm, &entries).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("migrate_v2_to_v3"), "error should list missing export: {msg}");
        assert!(!msg.contains("migrate_v1_to_v2"), "error should not list present export: {msg}");
    }

    #[test]
    fn all_exports_missing() {
        let wasm = wasm_with_exports(&[]);
        let entries = vec![entry("migrate_v1_to_v2"), entry("migrate_v2_to_v3")];

        let err = validate_wasm_exports_from_bytes(&wasm, &entries).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("migrate_v1_to_v2"), "should list first missing: {msg}");
        assert!(msg.contains("migrate_v2_to_v3"), "should list second missing: {msg}");
    }

    #[test]
    fn empty_entries_always_passes() {
        let wasm = wasm_with_exports(&[]);
        let result = validate_wasm_exports_from_bytes(&wasm, &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn invalid_wasm_binary() {
        let bad_bytes = b"not a wasm module";
        let entries = vec![entry("migrate_v1_to_v2")];

        let err = validate_wasm_exports_from_bytes(bad_bytes, &entries).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("invalid WASM binary"), "error: {msg}");
    }

    #[test]
    fn non_function_exports_ignored() {
        // Module exports memory but not the required function
        let wasm = wat::parse_str(
            r#"(module
                (memory (export "memory") 1)
                (global (export "some_global") i32 (i32.const 42))
            )"#,
        )
        .unwrap();

        let entries = vec![entry("some_global")];
        let err = validate_wasm_exports_from_bytes(&wasm, &entries).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("some_global"),
            "global export should not satisfy function requirement: {msg}"
        );
    }

    #[test]
    fn validate_from_file() {
        let tmp = TempDir::new().unwrap();
        let wasm_path = tmp.path().join("plugin.wasm");
        let wasm = wasm_with_exports(&["migrate_v1_to_v2"]);
        fs::write(&wasm_path, &wasm).unwrap();

        let entries = vec![entry("migrate_v1_to_v2")];
        let result = validate_wasm_exports(&wasm_path, &entries);
        assert!(result.is_ok(), "file-based validation should pass: {result:?}");
    }

    #[test]
    fn validate_from_nonexistent_file() {
        let entries = vec![entry("migrate_v1_to_v2")];
        let err = validate_wasm_exports(Path::new("/nonexistent/plugin.wasm"), &entries).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("failed to read WASM binary"), "error: {msg}");
    }
}
