//! Extism runtime wrapper for loading and executing WASM plugins.
//!
//! Provides a thin wrapper around the Extism SDK that handles WASM binary
//! loading, memory/timeout configuration, host function registration, and
//! function invocation.

use std::path::Path;
use std::time::Duration;

use extism::{Function, Manifest, PluginBuilder, Wasm};

use crate::error::PluginError;

/// Default memory limit per plugin instance (256 MB expressed in WASM pages).
/// Each WASM page is 64 KiB, so 256 MB = 4096 pages.
const DEFAULT_MAX_PAGES: u32 = 4096;

/// Default execution timeout per call (30 seconds).
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// A loaded WASM plugin instance backed by the Extism runtime.
pub struct PluginInstance {
    /// The underlying Extism plugin.
    plugin: extism::Plugin,
    /// The plugin ID from the manifest (set after loading).
    plugin_id: String,
}

impl std::fmt::Debug for PluginInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginInstance")
            .field("plugin_id", &self.plugin_id)
            .finish_non_exhaustive()
    }
}

impl PluginInstance {
    /// Returns the plugin ID associated with this instance.
    pub fn id(&self) -> &str {
        &self.plugin_id
    }

    /// Invokes a WASM export function by name with the given input bytes.
    ///
    /// Returns the output bytes from the WASM function, or a `PluginError`
    /// if the call fails (timeout, trap, invalid function, etc.).
    pub fn call(&mut self, function_name: &str, input: &[u8]) -> Result<Vec<u8>, PluginError> {
        self.plugin
            .call::<&[u8], Vec<u8>>(function_name, input)
            .map_err(|e| PluginError::ExecutionFailed(format!("{function_name}: {e}")))
    }

    /// Returns `true` if the plugin exports a function with the given name.
    pub fn function_exists(&self, function_name: &str) -> bool {
        self.plugin.function_exists(function_name)
    }
}

/// Loads a WASM plugin from disk with the provided host functions.
///
/// Reads the WASM binary from `wasm_path`, configures memory limits and
/// execution timeout, registers the provided host functions, and returns
/// a `PluginInstance` ready for invocation.
pub fn load_plugin(
    wasm_path: &Path,
    plugin_id: &str,
    host_functions: Vec<Function>,
) -> Result<PluginInstance, PluginError> {
    let wasm_bytes = std::fs::read(wasm_path).map_err(|e| {
        PluginError::WasmLoadFailed(format!(
            "failed to read WASM binary at {}: {e}",
            wasm_path.display()
        ))
    })?;

    load_plugin_from_bytes(&wasm_bytes, plugin_id, host_functions)
}

/// Loads a WASM plugin from raw bytes with the provided host functions.
///
/// This is useful for testing where WASM modules are generated in-memory.
pub fn load_plugin_from_bytes(
    wasm_bytes: &[u8],
    plugin_id: &str,
    host_functions: Vec<Function>,
) -> Result<PluginInstance, PluginError> {
    let manifest = Manifest::new([Wasm::data(wasm_bytes)])
        .with_memory_max(DEFAULT_MAX_PAGES)
        .with_timeout(DEFAULT_TIMEOUT);

    let plugin = PluginBuilder::new(manifest)
        .with_wasi(true)
        .with_functions(host_functions)
        .build()
        .map_err(|e| PluginError::WasmLoadFailed(format!("failed to initialize plugin: {e}")))?;

    Ok(PluginInstance {
        plugin,
        plugin_id: plugin_id.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Minimal valid WASM module (WAT format compiled to bytes).
    /// This module exports a single function "greet" that returns the input unchanged.
    fn minimal_wasm_module() -> Vec<u8> {
        // A minimal WASM module that exports a function called "greet"
        // which just returns 0 (success). The Extism PDK expects specific
        // exports, so we use the extism kernel approach: provide a module
        // that has the right shape.
        //
        // This is the simplest valid Extism plugin: it has a "greet" export
        // that reads input and writes it back as output.
        //
        // We use a hand-crafted WAT (WebAssembly Text) compiled to binary.
        wat::parse_str(
            r#"
            (module
                ;; Import Extism functions
                (import "extism:host/env" "input_length" (func $input_length (result i64)))
                (import "extism:host/env" "input_load_u8" (func $input_load_u8 (param i64) (result i32)))
                (import "extism:host/env" "output_set" (func $output_set (param i64 i64)))
                (import "extism:host/env" "length" (func $length (param i64) (result i64)))
                (import "extism:host/env" "alloc" (func $alloc (param i64) (result i64)))
                (import "extism:host/env" "store_u8" (func $store_u8 (param i64 i32)))

                (memory (export "memory") 1)

                ;; greet: copies input to output
                (func (export "greet") (result i32)
                    (local $len i64)
                    (local $offset i64)
                    (local $i i64)
                    (local $byte i32)

                    ;; Get input length
                    (local.set $len (call $input_length))

                    ;; Allocate output buffer
                    (local.set $offset (call $alloc (local.get $len)))

                    ;; Copy input to output byte by byte
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

                    ;; Set output
                    (call $output_set (local.get $offset) (local.get $len))

                    ;; Return 0 (success)
                    (i32.const 0)
                )
            )
            "#,
        )
        .expect("failed to compile WAT to WASM")
    }

    #[test]
    fn loads_valid_wasm_module() {
        let wasm = minimal_wasm_module();
        let result = load_plugin_from_bytes(&wasm, "test-plugin", vec![]);
        assert!(result.is_ok(), "failed to load valid WASM: {result:?}");

        let instance = result.unwrap();
        assert_eq!(instance.id(), "test-plugin");
    }

    #[test]
    fn call_returns_output() {
        let wasm = minimal_wasm_module();
        let mut instance = load_plugin_from_bytes(&wasm, "test-plugin", vec![]).unwrap();

        let output = instance.call("greet", b"hello").unwrap();
        assert_eq!(output, b"hello");
    }

    #[test]
    fn corrupt_binary_returns_error() {
        let corrupt = b"this is not a valid wasm binary";
        let result = load_plugin_from_bytes(corrupt, "bad-plugin", vec![]);

        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::WasmLoadFailed(msg) => {
                assert!(
                    msg.contains("failed to initialize plugin"),
                    "unexpected error message: {msg}"
                );
            }
            other => panic!("expected WasmLoadFailed, got: {other}"),
        }
    }

    #[test]
    fn load_from_file() {
        let tmp = TempDir::new().unwrap();
        let wasm_path = tmp.path().join("plugin.wasm");
        let wasm = minimal_wasm_module();
        fs::write(&wasm_path, &wasm).unwrap();

        let result = load_plugin(&wasm_path, "file-plugin", vec![]);
        assert!(result.is_ok(), "failed to load from file: {result:?}");
        assert_eq!(result.unwrap().id(), "file-plugin");
    }

    #[test]
    fn load_nonexistent_file_returns_error() {
        let result = load_plugin(Path::new("/nonexistent/plugin.wasm"), "missing", vec![]);

        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::WasmLoadFailed(msg) => {
                assert!(msg.contains("failed to read WASM binary"));
            }
            other => panic!("expected WasmLoadFailed, got: {other}"),
        }
    }

    #[test]
    fn function_exists_check() {
        let wasm = minimal_wasm_module();
        let instance = load_plugin_from_bytes(&wasm, "test-plugin", vec![]).unwrap();

        assert!(instance.function_exists("greet"));
        assert!(!instance.function_exists("nonexistent"));
    }
}
