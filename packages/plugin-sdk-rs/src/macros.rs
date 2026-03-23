//! Registration macro for WASM plugin entry-point generation.
//!
//! The [`register_plugin!`] macro generates the Extism-compatible WASM export
//! boilerplate so plugin authors never write `unsafe` or `extern "C"` code.
//!
//! # Usage
//!
//! ```rust,ignore
//! use life_engine_plugin_sdk::prelude::*;
//!
//! #[derive(Default)]
//! struct MyPlugin;
//!
//! impl Plugin for MyPlugin {
//!     fn id(&self) -> &str { "my-plugin" }
//!     fn display_name(&self) -> &str { "My Plugin" }
//!     fn version(&self) -> &str { "0.1.0" }
//!     fn actions(&self) -> Vec<Action> { vec![] }
//!     fn execute(&self, action: &str, input: PipelineMessage)
//!         -> Result<PipelineMessage, Box<dyn EngineError>> { todo!() }
//! }
//!
//! register_plugin!(MyPlugin);
//! ```

use life_engine_types::PipelineMessage;
use serde::{Deserialize, Serialize};

/// Input envelope sent by the Core host when invoking a plugin action.
///
/// The host serializes this to JSON bytes and passes it as the Extism
/// input to the plugin's `execute` export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInvocation {
    /// The action name to execute (must match one of the plugin's declared actions).
    pub action: String,
    /// The pipeline message carrying the data payload.
    pub message: PipelineMessage,
}

/// Output envelope returned by a plugin's `execute` export.
///
/// Serialized to JSON bytes and written to Extism output memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum PluginOutput {
    /// Successful execution with a result message.
    #[serde(rename = "ok")]
    Ok {
        /// The output pipeline message.
        message: PipelineMessage,
    },
    /// Failed execution with error details.
    #[serde(rename = "error")]
    Error {
        /// Human-readable error description.
        message: String,
        /// Error severity level (Fatal, Retryable, Warning).
        severity: String,
        /// Structured error code (e.g., "PLUGIN_001").
        code: String,
        /// Module that produced the error.
        source_module: String,
    },
}

/// Generates the Extism WASM entry-point boilerplate for a plugin struct.
///
/// The macro produces an `extern "C" fn execute()` that:
///
/// 1. Reads input bytes from Extism host memory
/// 2. Deserializes a [`PluginInvocation`] (action name + [`PipelineMessage`])
/// 3. Instantiates the plugin struct via `Default::default()`
/// 4. Calls [`Plugin::execute`] with the action and message
/// 5. Serializes the result as a [`PluginOutput`] back to Extism output memory
/// 6. Returns 0 on success, 1 on error
///
/// The generated code is gated behind `#[cfg(target_arch = "wasm32")]` so it
/// only compiles when building for WASM targets. Plugin authors never need
/// to write `unsafe` code — the macro handles all FFI details.
///
/// # Requirements
///
/// The plugin type must implement both [`Plugin`](life_engine_traits::Plugin)
/// and [`Default`].
#[macro_export]
macro_rules! register_plugin {
    ($plugin_type:ty) => {
        #[cfg(target_arch = "wasm32")]
        mod __life_engine_wasm_entry {
            use super::*;

            // Extism host-provided functions for guest memory management.
            // These are linked at WASM instantiation time by the Extism runtime.
            extern "C" {
                fn extism_input_length() -> u64;
                fn extism_input_offset() -> u64;
                fn extism_alloc(n: u64) -> u64;
                fn extism_output_set(offs: u64, n: u64);
                fn extism_error_set(offs: u64);
                fn extism_load(dest: u64, src: u64, n: u64);
                fn extism_store(dest: u64, src: u64, n: u64);
            }

            /// Read the full input buffer from Extism host memory.
            fn read_input() -> Vec<u8> {
                unsafe {
                    let len = extism_input_length() as usize;
                    let offs = extism_input_offset();
                    let mut buf = vec![0u8; len];
                    if len > 0 {
                        extism_load(buf.as_mut_ptr() as u64, offs, len as u64);
                    }
                    buf
                }
            }

            /// Write bytes to Extism output memory.
            fn write_output(bytes: &[u8]) {
                unsafe {
                    let len = bytes.len() as u64;
                    let offs = extism_alloc(len);
                    if len > 0 {
                        extism_store(offs, bytes.as_ptr() as u64, len);
                    }
                    extism_output_set(offs, len);
                }
            }

            /// Write an error string to Extism error memory.
            fn write_error(msg: &str) {
                unsafe {
                    let bytes = msg.as_bytes();
                    let len = bytes.len() as u64;
                    let offs = extism_alloc(len);
                    if len > 0 {
                        extism_store(offs, bytes.as_ptr() as u64, len);
                    }
                    extism_error_set(offs);
                }
            }

            /// Extism WASM entry point. Called by the host via `plugin.call("execute", input)`.
            #[no_mangle]
            pub extern "C" fn execute() -> i32 {
                let input_bytes = read_input();

                // Deserialize the invocation envelope.
                let invocation: $crate::macros::PluginInvocation =
                    match $crate::serde_json::from_slice(&input_bytes) {
                        Ok(inv) => inv,
                        Err(e) => {
                            let output = $crate::macros::PluginOutput::Error {
                                message: format!("Failed to deserialize plugin input: {}", e),
                                severity: "Fatal".to_string(),
                                code: "PLUGIN_DESER_001".to_string(),
                                source_module: "plugin-sdk".to_string(),
                            };
                            if let Ok(bytes) = $crate::serde_json::to_vec(&output) {
                                write_output(&bytes);
                            } else {
                                write_error(&format!("deserialization failed: {}", e));
                            }
                            return 1;
                        }
                    };

                // Instantiate the plugin and execute the action.
                let plugin = <$plugin_type>::default();

                match $crate::Plugin::execute(&plugin, &invocation.action, invocation.message) {
                    Ok(output_msg) => {
                        let output = $crate::macros::PluginOutput::Ok {
                            message: output_msg,
                        };
                        match $crate::serde_json::to_vec(&output) {
                            Ok(bytes) => {
                                write_output(&bytes);
                                0
                            }
                            Err(e) => {
                                write_error(&format!("Failed to serialize plugin output: {}", e));
                                1
                            }
                        }
                    }
                    Err(e) => {
                        let output = $crate::macros::PluginOutput::Error {
                            message: e.to_string(),
                            severity: e.severity().to_string(),
                            code: e.code().to_string(),
                            source_module: e.source_module().to_string(),
                        };
                        match $crate::serde_json::to_vec(&output) {
                            Ok(bytes) => {
                                write_output(&bytes);
                                1
                            }
                            Err(ser_err) => {
                                write_error(&format!(
                                    "Plugin error ({}): {} [serialization also failed: {}]",
                                    e.code(),
                                    e,
                                    ser_err
                                ));
                                1
                            }
                        }
                    }
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plugin_invocation_round_trip() {
        use chrono::Utc;
        use life_engine_types::{
            CdmType, MessageMetadata, Task, TaskPriority, TaskStatus, TypedPayload,
        };
        use uuid::Uuid;

        let invocation = PluginInvocation {
            action: "process_task".to_string(),
            message: PipelineMessage {
                metadata: MessageMetadata {
                    correlation_id: Uuid::new_v4(),
                    source: "test".to_string(),
                    timestamp: Utc::now(),
                    auth_context: None,
                },
                payload: TypedPayload::Cdm(Box::new(CdmType::Task(Task {
                    id: Uuid::new_v4(),
                    title: "Test task".to_string(),
                    description: None,
                    status: TaskStatus::Pending,
                    priority: TaskPriority::Medium,
                    due_date: None,
                    completed_at: None,
                    tags: vec![],
                    assignee: None,
                    parent_id: None,
                    source: "test".to_string(),
                    source_id: "t-1".to_string(),
                    extensions: None,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                }))),
            },
        };

        let json = serde_json::to_string(&invocation).expect("serialize");
        let restored: PluginInvocation = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.action, "process_task");
        assert_eq!(
            restored.message.metadata.correlation_id,
            invocation.message.metadata.correlation_id
        );
    }

    #[test]
    fn plugin_output_ok_serializes_with_status_tag() {
        use chrono::Utc;
        use life_engine_types::{CdmType, MessageMetadata, Note, NoteFormat, TypedPayload};
        use uuid::Uuid;

        let output = PluginOutput::Ok {
            message: PipelineMessage {
                metadata: MessageMetadata {
                    correlation_id: Uuid::new_v4(),
                    source: "test".to_string(),
                    timestamp: Utc::now(),
                    auth_context: None,
                },
                payload: TypedPayload::Cdm(Box::new(CdmType::Note(Note {
                    id: Uuid::new_v4(),
                    title: "Hello".to_string(),
                    body: "World".to_string(),
                    format: Some(NoteFormat::Plain),
                    pinned: None,
                    tags: vec![],
                    source: "test".to_string(),
                    source_id: "n-1".to_string(),
                    extensions: None,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                }))),
            },
        };

        let value = serde_json::to_value(&output).expect("serialize");
        assert_eq!(value["status"], "ok");
        assert!(value["message"].is_object());
    }

    #[test]
    fn plugin_output_error_serializes_with_status_tag() {
        let output = PluginOutput::Error {
            message: "something went wrong".to_string(),
            severity: "Fatal".to_string(),
            code: "PLUGIN_001".to_string(),
            source_module: "test-plugin".to_string(),
        };

        let value = serde_json::to_value(&output).expect("serialize");
        assert_eq!(value["status"], "error");
        assert_eq!(value["message"], "something went wrong");
        assert_eq!(value["severity"], "Fatal");
        assert_eq!(value["code"], "PLUGIN_001");
        assert_eq!(value["source_module"], "test-plugin");
    }

    #[test]
    fn plugin_output_round_trip() {
        let output = PluginOutput::Error {
            message: "test error".to_string(),
            severity: "Retryable".to_string(),
            code: "TEST_001".to_string(),
            source_module: "test".to_string(),
        };

        let json = serde_json::to_string(&output).expect("serialize");
        let restored: PluginOutput = serde_json::from_str(&json).expect("deserialize");

        match restored {
            PluginOutput::Error {
                message,
                severity,
                code,
                source_module,
            } => {
                assert_eq!(message, "test error");
                assert_eq!(severity, "Retryable");
                assert_eq!(code, "TEST_001");
                assert_eq!(source_module, "test");
            }
            _ => panic!("expected Error variant"),
        }
    }
}
