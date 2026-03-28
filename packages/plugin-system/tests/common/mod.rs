//! Shared test fixtures and mocks for plugin-system integration tests.
//!
//! Centralizes WASM module construction and mock backends that were
//! previously duplicated across multiple test files.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use life_engine_traits::{EngineError, StorageBackend};
use life_engine_types::{PipelineMessage, StorageMutation, StorageQuery};
use life_engine_workflow_engine::WorkflowEventEmitter;

// ---------------------------------------------------------------------------
// WASM fixtures
// ---------------------------------------------------------------------------

/// A minimal WASM module that echoes input back as output via the
/// Extism ABI. Exports a `greet` function.
pub fn echo_wasm_module() -> Vec<u8> {
    wat::parse_str(
        r#"
        (module
            (import "extism:host/env" "input_length" (func $input_length (result i64)))
            (import "extism:host/env" "input_load_u8" (func $input_load_u8 (param i64) (result i32)))
            (import "extism:host/env" "output_set" (func $output_set (param i64 i64)))
            (import "extism:host/env" "length" (func $length (param i64) (result i64)))
            (import "extism:host/env" "alloc" (func $alloc (param i64) (result i64)))
            (import "extism:host/env" "store_u8" (func $store_u8 (param i64 i32)))

            (memory (export "memory") 1)

            (func (export "greet") (result i32)
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
    .expect("failed to compile echo WAT to WASM")
}

/// A WASM module whose `execute` export hits `unreachable`, causing a trap.
/// Used to test crash isolation.
pub fn panicking_wasm_module() -> Vec<u8> {
    wat::parse_str(
        r#"
        (module
            (import "extism:host/env" "input_length" (func $input_length (result i64)))
            (import "extism:host/env" "input_load_u8" (func $input_load_u8 (param i64) (result i32)))
            (import "extism:host/env" "output_set" (func $output_set (param i64 i64)))
            (import "extism:host/env" "length" (func $length (param i64) (result i64)))
            (import "extism:host/env" "alloc" (func $alloc (param i64) (result i64)))
            (import "extism:host/env" "store_u8" (func $store_u8 (param i64 i32)))

            (memory (export "memory") 1)

            ;; Deliberately traps (simulates a panic inside the plugin).
            (func (export "execute") (result i32)
                unreachable
            )
        )
        "#,
    )
    .expect("failed to compile panicking WAT to WASM")
}

// ---------------------------------------------------------------------------
// Mock backends
// ---------------------------------------------------------------------------

/// Minimal mock storage backend that returns empty results.
pub struct MockStorage;

#[async_trait]
impl StorageBackend for MockStorage {
    async fn execute(
        &self,
        _query: StorageQuery,
    ) -> Result<Vec<PipelineMessage>, Box<dyn EngineError>> {
        Ok(vec![])
    }

    async fn mutate(&self, _op: StorageMutation) -> Result<(), Box<dyn EngineError>> {
        Ok(())
    }

    async fn init(
        _config: toml::Value,
        _key: [u8; 32],
    ) -> Result<Self, Box<dyn EngineError>> {
        Ok(MockStorage)
    }
}

/// Mock event bus that records all emitted events.
pub struct MockEventBus {
    pub emit_calls: Mutex<Vec<(String, serde_json::Value)>>,
}

impl MockEventBus {
    pub fn new() -> Self {
        Self {
            emit_calls: Mutex::new(vec![]),
        }
    }
}

#[async_trait]
impl WorkflowEventEmitter for MockEventBus {
    async fn emit(&self, event_name: &str, payload: serde_json::Value) {
        self.emit_calls
            .lock()
            .unwrap()
            .push((event_name.to_string(), payload));
    }
}

// ---------------------------------------------------------------------------
// Convenience constructors
// ---------------------------------------------------------------------------

pub fn mock_storage() -> Arc<dyn StorageBackend> {
    Arc::new(MockStorage)
}

pub fn mock_event_bus() -> Arc<dyn WorkflowEventEmitter> {
    Arc::new(MockEventBus::new())
}
