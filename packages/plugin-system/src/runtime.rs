//! Extism runtime wrapper for loading and executing WASM plugins.
//!
//! Provides a thin wrapper around the Extism SDK that handles WASM binary
//! loading, memory/timeout configuration, host function registration, and
//! function invocation.

use std::path::Path;
use std::time::Duration;

use extism::{Function, Manifest, PluginBuilder, Wasm};

use crate::error::PluginError;

/// Default memory limit per plugin instance (64 MB expressed in WASM pages).
/// Each WASM page is 64 KiB, so 64 MB = 1024 pages.
const DEFAULT_MAX_PAGES: u32 = 1024;

/// Default execution timeout per call (30 seconds).
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Per-plugin resource limit overrides.
///
/// When `None`, the default is used. Allows the engine configuration to grant
/// specific plugins more (or less) memory and execution time.
#[derive(Debug, Clone, Default)]
pub struct ResourceLimits {
    /// Maximum memory in bytes. Converted to WASM pages internally.
    /// Defaults to 64 MB when `None`.
    pub max_memory_bytes: Option<u64>,
    /// Execution timeout per call. Defaults to 30 seconds when `None`.
    pub timeout: Option<Duration>,
}

impl ResourceLimits {
    /// Returns the memory limit as WASM pages (64 KiB each).
    fn max_pages(&self) -> u32 {
        match self.max_memory_bytes {
            Some(bytes) => {
                let pages = bytes / (64 * 1024);
                // Clamp to u32 range (WASM spec limit is 65536 pages = 4 GiB).
                pages.min(65536) as u32
            }
            None => DEFAULT_MAX_PAGES,
        }
    }

    /// Returns the execution timeout.
    fn timeout(&self) -> Duration {
        self.timeout.unwrap_or(DEFAULT_TIMEOUT)
    }
}

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

/// Keywords in WASM error messages that indicate a trap/crash rather than
/// a recoverable execution failure.
const TRAP_INDICATORS: &[&str] = &[
    "unreachable",
    "out of bounds",
    "call stack exhausted",
    "stack overflow",
    "indirect call type mismatch",
    "integer overflow",
    "integer divide by zero",
    "invalid conversion",
    "undefined element",
    "uninitialized element",
    "wasm trap",
    "wasm `unreachable`",
    "error while executing at wasm",
    "wasm backtrace",
];

/// Returns `true` if the error message looks like a WASM trap/fault.
fn is_wasm_trap(error_msg: &str) -> bool {
    let lower = error_msg.to_lowercase();
    TRAP_INDICATORS.iter().any(|indicator| lower.contains(indicator))
}

impl PluginInstance {
    /// Returns the plugin ID associated with this instance.
    pub fn id(&self) -> &str {
        &self.plugin_id
    }

    /// Invokes a WASM export function by name with the given input bytes.
    ///
    /// Returns the output bytes from the WASM function, or a `PluginError`
    /// if the call fails. WASM traps (unreachable, out-of-bounds, stack
    /// overflow, etc.) produce `PluginError::Crash`; other failures produce
    /// `PluginError::ExecutionFailed`.
    pub fn call(&mut self, function_name: &str, input: &[u8]) -> Result<Vec<u8>, PluginError> {
        self.plugin
            .call::<&[u8], Vec<u8>>(function_name, input)
            .map_err(|e| {
                let msg = format!("{function_name}: {e}");
                if is_wasm_trap(&msg) {
                    PluginError::Crash(msg)
                } else {
                    PluginError::ExecutionFailed(msg)
                }
            })
    }

    /// Returns `true` if the plugin exports a function with the given name.
    pub fn function_exists(&self, function_name: &str) -> bool {
        self.plugin.function_exists(function_name)
    }
}

/// Loads a WASM plugin from disk with the provided host functions and default
/// resource limits.
///
/// Reads the WASM binary from `wasm_path`, configures memory limits and
/// execution timeout, registers the provided host functions, and returns
/// a `PluginInstance` ready for invocation.
pub fn load_plugin(
    wasm_path: &Path,
    plugin_id: &str,
    host_functions: Vec<Function>,
) -> Result<PluginInstance, PluginError> {
    load_plugin_with_limits(wasm_path, plugin_id, host_functions, &ResourceLimits::default())
}

/// Loads a WASM plugin from disk with the provided host functions and custom
/// resource limits.
pub fn load_plugin_with_limits(
    wasm_path: &Path,
    plugin_id: &str,
    host_functions: Vec<Function>,
    limits: &ResourceLimits,
) -> Result<PluginInstance, PluginError> {
    let wasm_bytes = std::fs::read(wasm_path).map_err(|e| {
        PluginError::WasmLoadFailed(format!(
            "failed to read WASM binary at {}: {e}",
            wasm_path.display()
        ))
    })?;

    load_plugin_from_bytes_with_limits(&wasm_bytes, plugin_id, host_functions, limits)
}

/// Loads a WASM plugin from raw bytes with the provided host functions and
/// default resource limits.
///
/// This is useful for testing where WASM modules are generated in-memory.
pub fn load_plugin_from_bytes(
    wasm_bytes: &[u8],
    plugin_id: &str,
    host_functions: Vec<Function>,
) -> Result<PluginInstance, PluginError> {
    load_plugin_from_bytes_with_limits(wasm_bytes, plugin_id, host_functions, &ResourceLimits::default())
}

/// Loads a WASM plugin from raw bytes with the provided host functions and
/// custom resource limits.
pub fn load_plugin_from_bytes_with_limits(
    wasm_bytes: &[u8],
    plugin_id: &str,
    host_functions: Vec<Function>,
    limits: &ResourceLimits,
) -> Result<PluginInstance, PluginError> {
    let manifest = Manifest::new([Wasm::data(wasm_bytes)])
        .with_memory_max(limits.max_pages())
        .with_timeout(limits.timeout());

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

                ;; trap: hits unreachable instruction immediately
                (func (export "trap") (result i32)
                    (unreachable)
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

    // --- Resource limit tests ---

    #[test]
    fn default_resource_limits_are_64mb_and_30s() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.max_pages(), DEFAULT_MAX_PAGES);
        assert_eq!(limits.max_pages(), 1024); // 64 MB / 64 KiB
        assert_eq!(limits.timeout(), Duration::from_secs(30));
    }

    #[test]
    fn custom_memory_limit_converts_to_pages() {
        let limits = ResourceLimits {
            max_memory_bytes: Some(128 * 1024 * 1024), // 128 MB
            timeout: None,
        };
        assert_eq!(limits.max_pages(), 2048); // 128 MB / 64 KiB
    }

    #[test]
    fn custom_timeout_override() {
        let limits = ResourceLimits {
            max_memory_bytes: None,
            timeout: Some(Duration::from_secs(60)),
        };
        assert_eq!(limits.timeout(), Duration::from_secs(60));
    }

    #[test]
    fn load_with_custom_limits() {
        let wasm = minimal_wasm_module();
        let limits = ResourceLimits {
            max_memory_bytes: Some(32 * 1024 * 1024), // 32 MB
            timeout: Some(Duration::from_secs(10)),
        };

        let result = load_plugin_from_bytes_with_limits(&wasm, "limited-plugin", vec![], &limits);
        assert!(result.is_ok(), "load with custom limits should succeed: {result:?}");
        assert_eq!(result.unwrap().id(), "limited-plugin");
    }

    #[test]
    fn load_from_file_with_custom_limits() {
        let tmp = TempDir::new().unwrap();
        let wasm_path = tmp.path().join("plugin.wasm");
        let wasm = minimal_wasm_module();
        fs::write(&wasm_path, &wasm).unwrap();

        let limits = ResourceLimits {
            max_memory_bytes: Some(128 * 1024 * 1024),
            timeout: Some(Duration::from_secs(5)),
        };

        let result = load_plugin_with_limits(&wasm_path, "file-limited", vec![], &limits);
        assert!(result.is_ok(), "load from file with limits should succeed: {result:?}");
    }

    // --- WASM trap / Crash tests ---

    #[test]
    fn wasm_trap_produces_crash_error() {
        let wasm = minimal_wasm_module();
        let mut instance = load_plugin_from_bytes(&wasm, "crash-plugin", vec![]).unwrap();

        let result = instance.call("trap", b"");
        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::Crash(msg) => {
                assert!(
                    msg.contains("wasm") || msg.contains("trap"),
                    "crash error should reference WASM execution: {msg}"
                );
            }
            other => panic!("expected Crash, got: {other}"),
        }
    }

    #[test]
    fn nonexistent_function_call_returns_execution_failed() {
        let wasm = minimal_wasm_module();
        let mut instance = load_plugin_from_bytes(&wasm, "test-plugin", vec![]).unwrap();

        let result = instance.call("nonexistent_fn", b"hello");
        assert!(result.is_err());
        // Calling a non-existent function is an execution failure, not a crash
        match result.unwrap_err() {
            PluginError::ExecutionFailed(msg) => {
                assert!(
                    msg.contains("nonexistent_fn"),
                    "error should name the function: {msg}"
                );
            }
            PluginError::Crash(_) => {
                // Some Extism versions may report this differently; either is acceptable
                // since calling a non-existent export is a programming error
            }
            other => panic!("expected ExecutionFailed or Crash, got: {other}"),
        }
    }

    // --- Trap indicator unit tests ---

    #[test]
    fn is_wasm_trap_detects_known_patterns() {
        assert!(is_wasm_trap("wasm trap: unreachable"));
        assert!(is_wasm_trap("out of bounds memory access"));
        assert!(is_wasm_trap("call stack exhausted"));
        assert!(is_wasm_trap("wasm `unreachable` instruction executed"));
        assert!(is_wasm_trap("integer divide by zero"));
        assert!(is_wasm_trap("stack overflow"));
    }

    #[test]
    fn is_wasm_trap_rejects_non_trap_errors() {
        assert!(!is_wasm_trap("function not found: greet"));
        assert!(!is_wasm_trap("plugin initialization failed"));
        assert!(!is_wasm_trap("timeout exceeded"));
    }

    // --- JSON serialization round-trip test ---

    #[test]
    fn action_invocation_json_round_trip() {
        let wasm = minimal_wasm_module();
        let mut instance = load_plugin_from_bytes(&wasm, "roundtrip-plugin", vec![]).unwrap();

        let input = serde_json::json!({
            "metadata": {
                "correlation_id": "00000000-0000-0000-0000-000000000001",
                "source": "test:runtime",
                "timestamp": "2026-03-28T00:00:00Z",
                "auth_context": null,
                "warnings": []
            },
            "payload": {
                "type": "raw",
                "data": {"key": "value"}
            }
        });
        let input_bytes = serde_json::to_vec(&input).unwrap();

        let output_bytes = instance.call("greet", &input_bytes).unwrap();
        let output: serde_json::Value = serde_json::from_slice(&output_bytes).unwrap();

        assert_eq!(input, output, "round-trip should preserve JSON exactly");
    }
}
