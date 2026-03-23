//! WASM migration transform executor.
//!
//! Runs migration transform functions inside a pure WASM sandbox with no host
//! function access. Each transform receives a JSON record and returns a
//! transformed JSON record.

use std::path::Path;
use std::time::Duration;

use extism::{Manifest, PluginBuilder, Wasm};

use super::MigrationError;

/// Execution timeout per record transform (10 seconds).
const TRANSFORM_TIMEOUT: Duration = Duration::from_secs(10);

/// Memory limit for migration transforms (256 MB in WASM pages).
const TRANSFORM_MAX_PAGES: u32 = 4096;

/// Executes a WASM migration transform function on a single record.
///
/// Loads the WASM module from `wasm_path` into a fresh Extism instance with
/// **no host functions** — migration transforms run in a pure sandbox with no
/// storage, HTTP, or event access. The transform function is pure: JSON in,
/// JSON out, no side effects.
///
/// # Arguments
///
/// - `wasm_path` — Path to the plugin WASM binary.
/// - `function_name` — Name of the exported transform function to call.
/// - `input_record` — The record to transform, as a JSON value.
///
/// # Errors
///
/// - `MigrationError::TransformFailed` if the WASM function returns a non-zero exit code.
/// - `MigrationError::TransformCrashed` if the WASM function traps or panics,
///   or if the module cannot be loaded.
pub async fn run_transform(
    wasm_path: &Path,
    function_name: &str,
    input_record: serde_json::Value,
) -> Result<serde_json::Value, MigrationError> {
    let wasm_bytes = std::fs::read(wasm_path).map_err(|e| MigrationError::TransformCrashed {
        function: function_name.to_string(),
        cause: format!("failed to read WASM binary at {}: {e}", wasm_path.display()),
    })?;

    run_transform_from_bytes(&wasm_bytes, function_name, input_record).await
}

/// Executes a WASM migration transform from raw bytes.
///
/// This is useful for testing where WASM modules are generated in-memory.
pub async fn run_transform_from_bytes(
    wasm_bytes: &[u8],
    function_name: &str,
    input_record: serde_json::Value,
) -> Result<serde_json::Value, MigrationError> {
    // Serialize the input record to JSON bytes.
    let input_bytes = serde_json::to_vec(&input_record).map_err(|e| {
        MigrationError::TransformFailed {
            function: function_name.to_string(),
            cause: format!("failed to serialize input record: {e}"),
        }
    })?;

    // Load the WASM module into a fresh Extism instance with NO host functions.
    let manifest = Manifest::new([Wasm::data(wasm_bytes)])
        .with_memory_max(TRANSFORM_MAX_PAGES)
        .with_timeout(TRANSFORM_TIMEOUT);

    let mut plugin = PluginBuilder::new(manifest)
        .with_wasi(true)
        .build()
        .map_err(|e| MigrationError::TransformCrashed {
            function: function_name.to_string(),
            cause: format!("failed to initialize WASM sandbox: {e}"),
        })?;

    // Call the named export function with the serialized input.
    let output_bytes = plugin
        .call::<&[u8], Vec<u8>>(function_name, &input_bytes)
        .map_err(|e| {
            let msg = e.to_string();
            // Distinguish between a controlled error return and a trap/panic.
            // WASM traps manifest as backtrace messages or explicit trap/panic keywords.
            if msg.contains("unreachable")
                || msg.contains("trap")
                || msg.contains("panic")
                || msg.contains("wasm backtrace")
            {
                MigrationError::TransformCrashed {
                    function: function_name.to_string(),
                    cause: msg,
                }
            } else {
                MigrationError::TransformFailed {
                    function: function_name.to_string(),
                    cause: msg,
                }
            }
        })?;

    // Deserialize the output bytes as JSON.
    serde_json::from_slice(&output_bytes).map_err(|e| MigrationError::TransformFailed {
        function: function_name.to_string(),
        cause: format!("transform output is not valid JSON: {e}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Creates a minimal Extism plugin that echoes input to output (identity transform).
    fn identity_transform_wasm() -> Vec<u8> {
        wat::parse_str(
            r#"
            (module
                (import "extism:host/env" "input_length" (func $input_length (result i64)))
                (import "extism:host/env" "input_load_u8" (func $input_load_u8 (param i64) (result i32)))
                (import "extism:host/env" "output_set" (func $output_set (param i64 i64)))
                (import "extism:host/env" "alloc" (func $alloc (param i64) (result i64)))
                (import "extism:host/env" "store_u8" (func $store_u8 (param i64 i32)))

                (memory (export "memory") 1)

                (func (export "migrate_v1_to_v2") (result i32)
                    (local $len i64)
                    (local $offset i64)
                    (local $i i64)
                    (local $byte i32)

                    (local.set $len (call $input_length))
                    (local.set $offset (call $alloc (local.get $len)))

                    (local.set $i (i64.const 0))
                    (block $break
                        (loop $loop
                            (br_if $break (i64.ge_u (local.get $i) (local.get $len)))
                            (local.set $byte (call $input_load_u8 (local.get $i)))
                            (call $store_u8
                                (i64.add (local.get $offset) (local.get $i))
                                (local.get $byte)
                            )
                            (local.set $i (i64.add (local.get $i) (i64.const 1)))
                            (br $loop)
                        )
                    )

                    (call $output_set (local.get $offset) (local.get $len))
                    (i32.const 0)
                )
            )
            "#,
        )
        .expect("failed to compile WAT to WASM")
    }

    /// Creates a WASM module whose transform function traps (unreachable).
    fn trapping_transform_wasm() -> Vec<u8> {
        wat::parse_str(
            r#"
            (module
                (memory (export "memory") 1)
                (func (export "bad_migrate") (result i32)
                    unreachable
                )
            )
            "#,
        )
        .expect("failed to compile WAT to WASM")
    }

    #[tokio::test]
    async fn identity_transform_preserves_record() {
        let wasm = identity_transform_wasm();
        let input = serde_json::json!({"name": "Alice", "age": 30});

        let output = run_transform_from_bytes(&wasm, "migrate_v1_to_v2", input.clone())
            .await
            .expect("transform should succeed");

        assert_eq!(output, input);
    }

    #[tokio::test]
    async fn transform_from_file() {
        let tmp = TempDir::new().unwrap();
        let wasm_path = tmp.path().join("plugin.wasm");
        fs::write(&wasm_path, identity_transform_wasm()).unwrap();

        let input = serde_json::json!({"key": "value"});
        let output = run_transform(&wasm_path, "migrate_v1_to_v2", input.clone())
            .await
            .expect("transform should succeed");

        assert_eq!(output, input);
    }

    #[tokio::test]
    async fn nonexistent_function_returns_error() {
        let wasm = identity_transform_wasm();
        let input = serde_json::json!({"data": 1});

        let err = run_transform_from_bytes(&wasm, "no_such_function", input)
            .await
            .expect_err("should fail for missing function");

        match err {
            MigrationError::TransformFailed { function, .. } => {
                assert_eq!(function, "no_such_function");
            }
            other => panic!("expected TransformFailed, got: {other}"),
        }
    }

    #[tokio::test]
    async fn trapping_function_returns_crashed() {
        let wasm = trapping_transform_wasm();
        let input = serde_json::json!({"data": 1});

        let err = run_transform_from_bytes(&wasm, "bad_migrate", input)
            .await
            .expect_err("should fail for trapping function");

        match err {
            MigrationError::TransformCrashed { function, cause } => {
                assert_eq!(function, "bad_migrate");
                assert!(
                    cause.contains("wasm backtrace") || cause.contains("unreachable"),
                    "expected wasm backtrace or unreachable in cause, got: {cause}"
                );
            }
            other => panic!("expected TransformCrashed, got: {other}"),
        }
    }

    #[tokio::test]
    async fn nonexistent_wasm_file_returns_crashed() {
        let input = serde_json::json!({"data": 1});

        let err = run_transform(Path::new("/nonexistent/plugin.wasm"), "migrate", input)
            .await
            .expect_err("should fail for missing file");

        match err {
            MigrationError::TransformCrashed { cause, .. } => {
                assert!(cause.contains("failed to read WASM binary"));
            }
            other => panic!("expected TransformCrashed, got: {other}"),
        }
    }

    #[tokio::test]
    async fn corrupt_wasm_returns_crashed() {
        let input = serde_json::json!({"data": 1});

        let err = run_transform_from_bytes(b"not wasm", "migrate", input)
            .await
            .expect_err("should fail for corrupt WASM");

        match err {
            MigrationError::TransformCrashed { cause, .. } => {
                assert!(cause.contains("failed to initialize WASM sandbox"));
            }
            other => panic!("expected TransformCrashed, got: {other}"),
        }
    }
}
