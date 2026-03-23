//! Plugin execution bridge between the workflow engine and plugin WASM.
//!
//! Implements the `PluginExecutor` trait (defined in the workflow engine) for
//! the plugin system, translating workflow step calls into WASM invocations
//! via loaded `PluginHandle` instances.

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use life_engine_traits::EngineError;
use life_engine_types::PipelineMessage;
use life_engine_workflow_engine::PluginExecutor;
use tracing::{debug, error};

use crate::error::PluginError;
use crate::lifecycle::{LifecycleManager, LifecycleState};
use crate::loader::PluginHandle;

/// Bridges the workflow engine's `PluginExecutor` trait to loaded WASM plugins.
///
/// Holds the set of loaded plugin handles and the lifecycle manager. On each
/// `execute` call it:
/// 1. Looks up the plugin by ID
/// 2. Verifies the plugin is in the `Running` state
/// 3. Verifies the requested action exists in the manifest
/// 4. Serializes the input `PipelineMessage` to JSON bytes
/// 5. Calls the WASM export with the serialized input
/// 6. Deserializes the output back into a `PipelineMessage`
pub struct PluginSystemExecutor {
    /// Loaded plugin handles keyed by plugin ID.
    handles: Mutex<HashMap<String, PluginHandle>>,
    /// Lifecycle manager tracking plugin states.
    lifecycle: Mutex<LifecycleManager>,
}

impl PluginSystemExecutor {
    /// Creates a new executor from loaded plugin handles and a lifecycle manager.
    pub fn new(handles: Vec<PluginHandle>, lifecycle: LifecycleManager) -> Self {
        let map: HashMap<String, PluginHandle> = handles
            .into_iter()
            .map(|h| (h.manifest.plugin.id.clone(), h))
            .collect();

        Self {
            handles: Mutex::new(map),
            lifecycle: Mutex::new(lifecycle),
        }
    }
}

#[async_trait]
impl PluginExecutor for PluginSystemExecutor {
    async fn execute(
        &self,
        plugin_id: &str,
        action: &str,
        input: PipelineMessage,
    ) -> Result<PipelineMessage, Box<dyn EngineError>> {
        // 1. Check lifecycle state
        {
            let lifecycle = self.lifecycle.lock().unwrap();
            match lifecycle.state(plugin_id) {
                Some(LifecycleState::Running) => {} // OK
                Some(state) => {
                    return Err(Box::new(PluginError::ExecutionFailed(format!(
                        "plugin '{plugin_id}' is not running (current state: {state})"
                    ))));
                }
                None => {
                    return Err(Box::new(PluginError::ExecutionFailed(format!(
                        "plugin '{plugin_id}' not found"
                    ))));
                }
            }
        }

        // 2. Verify action exists in manifest and call WASM
        let mut handles = self.handles.lock().unwrap();
        let handle = handles.get_mut(plugin_id).ok_or_else(|| {
            Box::new(PluginError::ExecutionFailed(format!(
                "plugin '{plugin_id}' not found"
            ))) as Box<dyn EngineError>
        })?;

        if !handle.manifest.actions.contains_key(action) {
            return Err(Box::new(PluginError::ExecutionFailed(format!(
                "unknown action '{action}' for plugin '{plugin_id}'"
            ))));
        }

        // 3. Serialize input to JSON bytes
        let input_bytes = serde_json::to_vec(&input).map_err(|e| {
            Box::new(PluginError::ExecutionFailed(format!(
                "failed to serialize input for plugin '{plugin_id}': {e}"
            ))) as Box<dyn EngineError>
        })?;

        debug!(plugin_id, action, input_size = input_bytes.len(), "executing plugin action");

        // 4. Call WASM export
        let output_bytes = handle
            .instance
            .call(action, &input_bytes)
            .map_err(|e| {
                error!(plugin_id, action, error = %e, "WASM execution failed");
                Box::new(e) as Box<dyn EngineError>
            })?;

        // 5. Deserialize output
        let output: PipelineMessage =
            serde_json::from_slice(&output_bytes).map_err(|e| {
                Box::new(PluginError::ExecutionFailed(format!(
                    "invalid output from plugin '{plugin_id}' action '{action}': {e}"
                ))) as Box<dyn EngineError>
            })?;

        debug!(plugin_id, action, output_size = output_bytes.len(), "plugin action completed");

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::{HashMap, HashSet};

    use life_engine_traits::Severity;
    use life_engine_types::{CdmType, MessageMetadata, PipelineMessage, TypedPayload};
    use uuid::Uuid;

    use crate::capability::ApprovedCapabilities;
    use crate::manifest::{ActionDef, CapabilitySet, PluginManifest, PluginMeta};
    use crate::runtime::load_plugin_from_bytes;

    /// Minimal WASM module that echoes input back as output.
    fn echo_wasm_module() -> Vec<u8> {
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
        .expect("failed to compile WAT to WASM")
    }

    fn test_manifest(id: &str, actions: Vec<&str>) -> PluginManifest {
        let mut action_map = HashMap::new();
        for action in actions {
            action_map.insert(
                action.to_string(),
                ActionDef {
                    description: format!("{action} action"),
                    input_schema: None,
                    output_schema: None,
                },
            );
        }
        PluginManifest {
            plugin: PluginMeta {
                id: id.to_string(),
                name: format!("Test Plugin {id}"),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
            },
            actions: action_map,
            capabilities: CapabilitySet::default(),
            config: None,
        }
    }

    fn test_handle(id: &str, actions: Vec<&str>) -> PluginHandle {
        let wasm = echo_wasm_module();
        let instance = load_plugin_from_bytes(&wasm, id, vec![]).unwrap();
        PluginHandle {
            instance,
            manifest: test_manifest(id, actions),
            capabilities: ApprovedCapabilities::new(HashSet::new()),
        }
    }

    fn test_pipeline_message() -> PipelineMessage {
        PipelineMessage {
            metadata: MessageMetadata {
                correlation_id: Uuid::new_v4(),
                source: "test:execute".to_string(),
                timestamp: chrono::Utc::now(),
                auth_context: None,
            },
            payload: TypedPayload::Cdm(Box::new(CdmType::TaskBatch(vec![]))),
        }
    }

    fn running_lifecycle(plugin_ids: &[&str]) -> LifecycleManager {
        let mut mgr = LifecycleManager::new();
        for id in plugin_ids {
            mgr.register(id);
        }
        mgr.start_all();
        mgr
    }

    #[tokio::test]
    async fn execute_valid_action_returns_pipeline_message() {
        let handle = test_handle("my-plugin", vec!["greet"]);
        let lifecycle = running_lifecycle(&["my-plugin"]);
        let executor = PluginSystemExecutor::new(vec![handle], lifecycle);

        let input = test_pipeline_message();
        let result = executor.execute("my-plugin", "greet", input.clone()).await;

        assert!(result.is_ok(), "unexpected error: {result:?}");
        let output = result.unwrap();
        // The echo WASM module returns the input unchanged, so the
        // deserialized output should match the input's source field.
        assert_eq!(output.metadata.source, input.metadata.source);
    }

    #[tokio::test]
    async fn unknown_action_returns_error() {
        let handle = test_handle("my-plugin", vec!["greet"]);
        let lifecycle = running_lifecycle(&["my-plugin"]);
        let executor = PluginSystemExecutor::new(vec![handle], lifecycle);

        let input = test_pipeline_message();
        let result = executor
            .execute("my-plugin", "nonexistent", input)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("unknown action 'nonexistent'"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn plugin_not_running_returns_error() {
        let handle = test_handle("my-plugin", vec!["greet"]);
        // Register but do NOT start — stays in Discovered state
        let mut lifecycle = LifecycleManager::new();
        lifecycle.register("my-plugin");

        let executor = PluginSystemExecutor::new(vec![handle], lifecycle);
        let input = test_pipeline_message();
        let result = executor.execute("my-plugin", "greet", input).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("not running"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn plugin_not_found_returns_error() {
        let lifecycle = LifecycleManager::new();
        let executor = PluginSystemExecutor::new(vec![], lifecycle);

        let input = test_pipeline_message();
        let result = executor.execute("missing-plugin", "greet", input).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("not found"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn wasm_execution_error_propagates_with_context() {
        // Call an action that exists in manifest but not as a WASM export
        let mut actions = HashMap::new();
        actions.insert(
            "missing_export".to_string(),
            ActionDef {
                description: "An action with no WASM export".to_string(),
                input_schema: None,
                output_schema: None,
            },
        );
        let wasm = echo_wasm_module();
        let instance = load_plugin_from_bytes(&wasm, "my-plugin", vec![]).unwrap();
        let handle = PluginHandle {
            instance,
            manifest: PluginManifest {
                plugin: PluginMeta {
                    id: "my-plugin".to_string(),
                    name: "Test Plugin".to_string(),
                    version: "1.0.0".to_string(),
                    description: None,
                    author: None,
                },
                actions,
                capabilities: CapabilitySet::default(),
                config: None,
            },
            capabilities: ApprovedCapabilities::new(HashSet::new()),
        };

        let lifecycle = running_lifecycle(&["my-plugin"]);
        let executor = PluginSystemExecutor::new(vec![handle], lifecycle);

        let input = test_pipeline_message();
        let result = executor.execute("my-plugin", "missing_export", input).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        // Error should be from WASM execution failure
        assert_eq!(err.code(), "PLUGIN_007");
        assert_eq!(err.severity(), Severity::Retryable);
    }

    #[tokio::test]
    async fn execution_error_has_retryable_severity() {
        let lifecycle = LifecycleManager::new();
        let executor = PluginSystemExecutor::new(vec![], lifecycle);

        let input = test_pipeline_message();
        let result = executor.execute("missing", "action", input).await;

        let err = result.unwrap_err();
        // ExecutionFailed errors are Retryable
        assert_eq!(err.severity(), Severity::Retryable);
    }
}
