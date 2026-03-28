//! WASM guest-side bindings for Life Engine plugins.
//!
//! This module provides types and helpers for WASM plugins to communicate
//! with the Core host via the host function interface. Plugin authors use
//! these types to make storage, event, config, logging, and HTTP requests
//! that are routed through the capability-enforced host bridge.
//!
//! # Architecture
//!
//! WASM plugins communicate with Core through a JSON-based protocol:
//!
//! 1. Plugin serializes a `HostRequest` to JSON
//! 2. Calls the `host_call` import function with the JSON bytes
//! 3. Receives a `HostResponse` JSON back
//! 4. Deserializes and handles the result
//!
//! All requests are subject to the plugin's declared capabilities.
//! Undeclared capabilities return an error response.

use serde::{Deserialize, Serialize};

use crate::types::HttpMethod;

/// Request types for calling host functions from WASM guest code.
///
/// Each variant maps to a host function in the `WasmHostBridge`.
/// The `type` field in the serialized JSON determines the dispatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum HostRequest {
    // --- Document storage ---

    /// Read a document by ID. Requires `storage:doc:read` capability.
    StoreRead {
        collection: String,
        id: String,
    },
    /// Create a document. Requires `storage:doc:write` capability.
    StoreWrite {
        collection: String,
        data: serde_json::Value,
    },
    /// Query documents. Requires `storage:doc:read` capability.
    StoreQuery {
        collection: String,
        filters: serde_json::Value,
        limit: Option<u32>,
        offset: Option<u32>,
    },
    /// Count documents matching a query. Requires `storage:doc:read` capability.
    StoreCount {
        collection: String,
        filters: serde_json::Value,
    },
    /// Partially update a document (merge patch). Requires `storage:doc:write` capability.
    StorePartialUpdate {
        collection: String,
        id: String,
        patch: serde_json::Value,
    },
    /// Create multiple documents in one call. Requires `storage:doc:write` capability.
    StoreBatchCreate {
        collection: String,
        documents: Vec<serde_json::Value>,
    },
    /// Update multiple documents in one call. Requires `storage:doc:write` capability.
    StoreBatchUpdate {
        collection: String,
        updates: Vec<serde_json::Value>,
    },
    /// Delete a document. Requires `storage:doc:delete` capability.
    StoreDelete {
        collection: String,
        id: String,
    },

    // --- Blob storage ---

    /// Store a blob. Requires `storage:blob:write` capability.
    BlobStore {
        key: String,
        data_base64: String,
        content_type: Option<String>,
    },
    /// Retrieve a blob. Requires `storage:blob:read` capability.
    BlobRetrieve {
        key: String,
    },
    /// Delete a blob. Requires `storage:blob:delete` capability.
    BlobDelete {
        key: String,
    },

    // --- Config ---

    /// Read a config value. Requires `config:read` capability.
    ConfigGet {
        key: String,
    },

    // --- Events ---

    /// Subscribe to events. Requires `events:subscribe` capability.
    EventSubscribe {
        event_type: String,
    },
    /// Emit an event. Requires `events:emit` capability.
    EventEmit {
        event_type: String,
        payload: serde_json::Value,
    },

    // --- Logging ---

    /// Log at info level. No capability required.
    LogInfo {
        message: String,
    },
    /// Log at warn level. No capability required.
    LogWarn {
        message: String,
    },
    /// Log at error level. No capability required.
    LogError {
        message: String,
    },

    // --- HTTP ---

    /// Make an HTTP request. Requires `http:outbound` capability.
    HttpRequest {
        url: String,
        method: HttpMethod,
        headers: Option<serde_json::Value>,
        body: Option<String>,
    },
}

/// Response from a host function call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostResponse {
    /// Whether the call succeeded.
    pub success: bool,
    /// The result data (if success).
    pub data: Option<serde_json::Value>,
    /// Error message (if failure).
    pub error: Option<String>,
}

impl HostResponse {
    /// Returns `Ok(data)` if the response was successful, `Err` otherwise.
    pub fn into_result(self) -> anyhow::Result<serde_json::Value> {
        if self.success {
            Ok(self.data.unwrap_or(serde_json::Value::Null))
        } else {
            Err(anyhow::anyhow!(
                self.error.unwrap_or_else(|| "unknown host error".into())
            ))
        }
    }
}

// --- Host call bridge ---

/// Declare the host-provided FFI function for WASM plugins.
///
/// In a WASM build, this is the import provided by the Extism host.
/// In native builds, this is a stub that always returns an error.
#[cfg(target_arch = "wasm32")]
extern "C" {
    fn __host_call(input_ptr: *const u8, input_len: usize, output_ptr: *mut u8, output_len: *mut usize) -> i32;
}

/// Call a host function from WASM guest code.
///
/// Serializes the request to JSON, calls the host function, and deserializes the response.
/// Returns a `PluginError` if the host returns an error or if serialization fails.
pub fn host_call(request: &HostRequest) -> Result<HostResponse, crate::error::PluginError> {
    let request_json = serde_json::to_vec(request)?;

    #[cfg(target_arch = "wasm32")]
    {
        let mut output_buf = vec![0u8; 65536];
        let mut output_len: usize = 0;
        let rc = unsafe {
            __host_call(
                request_json.as_ptr(),
                request_json.len(),
                output_buf.as_mut_ptr(),
                &mut output_len,
            )
        };
        if rc != 0 {
            return Err(crate::error::PluginError::InternalError {
                message: format!("host_call failed with return code {rc}"),
                detail: None,
            });
        }
        let response: HostResponse = serde_json::from_slice(&output_buf[..output_len])?;
        Ok(response)
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = request_json;
        Err(crate::error::PluginError::InternalError {
            message: "host_call is only available in WASM builds".into(),
            detail: None,
        })
    }
}

// --- Convenience wrappers for common host operations ---

/// Read a document by ID from a collection.
pub fn doc_read(collection: &str, id: &str) -> Result<HostResponse, crate::error::PluginError> {
    host_call(&HostRequest::StoreRead {
        collection: collection.into(),
        id: id.into(),
    })
}

/// Write a document to a collection.
pub fn doc_write(collection: &str, data: serde_json::Value) -> Result<HostResponse, crate::error::PluginError> {
    host_call(&HostRequest::StoreWrite {
        collection: collection.into(),
        data,
    })
}

/// Delete a document by ID from a collection.
pub fn doc_delete(collection: &str, id: &str) -> Result<HostResponse, crate::error::PluginError> {
    host_call(&HostRequest::StoreDelete {
        collection: collection.into(),
        id: id.into(),
    })
}

/// Emit an event to the Core event bus.
pub fn emit_event(event_type: &str, payload: serde_json::Value) -> Result<HostResponse, crate::error::PluginError> {
    host_call(&HostRequest::EventEmit {
        event_type: event_type.into(),
        payload,
    })
}

/// Read a configuration value by key.
pub fn config_get(key: &str) -> Result<HostResponse, crate::error::PluginError> {
    host_call(&HostRequest::ConfigGet { key: key.into() })
}

/// Make an outbound HTTP request.
pub fn http_request(
    method: HttpMethod,
    url: &str,
    headers: Option<serde_json::Value>,
    body: Option<String>,
) -> Result<HostResponse, crate::error::PluginError> {
    host_call(&HostRequest::HttpRequest {
        url: url.into(),
        method,
        headers,
        body,
    })
}

/// Resource limits applied to WASM plugins.
///
/// These limits are enforced by the host runtime. Plugin authors can
/// reference these constants to understand the constraints their code
/// runs under.
pub mod limits {
    /// Default memory limit per plugin: 64 MB.
    pub const DEFAULT_MEMORY_BYTES: u64 = 64 * 1024 * 1024;

    /// Default execution timeout per request: 30 seconds.
    pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

    /// Default rate limit for host function calls: 1000 per second.
    pub const DEFAULT_RATE_LIMIT: u32 = 1000;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn host_request_store_read_serializes() {
        let req = HostRequest::StoreRead {
            collection: "tasks".into(),
            id: "abc".into(),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["type"], "StoreRead");
        assert_eq!(json["collection"], "tasks");
        assert_eq!(json["id"], "abc");
    }

    #[test]
    fn host_request_store_write_serializes() {
        let req = HostRequest::StoreWrite {
            collection: "tasks".into(),
            data: json!({"title": "Test"}),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["type"], "StoreWrite");
        assert_eq!(json["data"]["title"], "Test");
    }

    #[test]
    fn host_request_roundtrip() {
        let req = HostRequest::EventEmit {
            event_type: "task.created".into(),
            payload: json!({"id": "123"}),
        };
        let serialized = serde_json::to_string(&req).unwrap();
        let deserialized: HostRequest = serde_json::from_str(&serialized).unwrap();
        match deserialized {
            HostRequest::EventEmit {
                event_type,
                payload,
            } => {
                assert_eq!(event_type, "task.created");
                assert_eq!(payload["id"], "123");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn host_response_success_into_result() {
        let resp = HostResponse {
            success: true,
            data: Some(json!({"value": 42})),
            error: None,
        };
        let result = resp.into_result();
        assert!(result.is_ok());
        assert_eq!(result.unwrap()["value"], 42);
    }

    #[test]
    fn host_response_error_into_result() {
        let resp = HostResponse {
            success: false,
            data: None,
            error: Some("permission denied".into()),
        };
        let result = resp.into_result();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("permission denied"));
    }

    #[test]
    fn all_request_variants_serialize() {
        let requests = vec![
            HostRequest::StoreRead {
                collection: "c".into(),
                id: "i".into(),
            },
            HostRequest::StoreWrite {
                collection: "c".into(),
                data: json!({}),
            },
            HostRequest::StoreQuery {
                collection: "c".into(),
                filters: json!({}),
                limit: Some(10),
                offset: None,
            },
            HostRequest::StoreCount {
                collection: "c".into(),
                filters: json!({}),
            },
            HostRequest::StorePartialUpdate {
                collection: "c".into(),
                id: "i".into(),
                patch: json!({"field": "value"}),
            },
            HostRequest::StoreBatchCreate {
                collection: "c".into(),
                documents: vec![json!({}), json!({})],
            },
            HostRequest::StoreBatchUpdate {
                collection: "c".into(),
                updates: vec![json!({"id": "1", "data": {}})],
            },
            HostRequest::StoreDelete {
                collection: "c".into(),
                id: "i".into(),
            },
            HostRequest::BlobStore {
                key: "photos/img.png".into(),
                data_base64: "AQID".into(),
                content_type: Some("image/png".into()),
            },
            HostRequest::BlobRetrieve {
                key: "photos/img.png".into(),
            },
            HostRequest::BlobDelete {
                key: "photos/img.png".into(),
            },
            HostRequest::ConfigGet { key: "k".into() },
            HostRequest::EventSubscribe {
                event_type: "e".into(),
            },
            HostRequest::EventEmit {
                event_type: "e".into(),
                payload: json!({}),
            },
            HostRequest::LogInfo {
                message: "m".into(),
            },
            HostRequest::LogWarn {
                message: "m".into(),
            },
            HostRequest::LogError {
                message: "m".into(),
            },
            HostRequest::HttpRequest {
                url: "u".into(),
                method: HttpMethod::Get,
                headers: None,
                body: None,
            },
        ];

        for req in requests {
            let json = serde_json::to_string(&req).unwrap();
            let _: HostRequest = serde_json::from_str(&json).unwrap();
        }
    }

    #[test]
    fn http_method_serializes_uppercase() {
        assert_eq!(
            serde_json::to_value(HttpMethod::Get).unwrap(),
            json!("GET")
        );
        assert_eq!(
            serde_json::to_value(HttpMethod::Post).unwrap(),
            json!("POST")
        );
        assert_eq!(
            serde_json::to_value(HttpMethod::Delete).unwrap(),
            json!("DELETE")
        );
    }

    #[test]
    fn http_method_deserializes_uppercase() {
        let method: HttpMethod = serde_json::from_value(json!("GET")).unwrap();
        assert_eq!(method, HttpMethod::Get);
    }

    #[test]
    fn http_method_rejects_invalid() {
        let result = serde_json::from_value::<HttpMethod>(json!("INVALID"));
        assert!(result.is_err());
    }

    #[test]
    fn http_method_display() {
        assert_eq!(HttpMethod::Get.to_string(), "GET");
        assert_eq!(HttpMethod::Post.to_string(), "POST");
    }

    #[test]
    fn limits_constants_correct() {
        assert_eq!(limits::DEFAULT_MEMORY_BYTES, 64 * 1024 * 1024);
        assert_eq!(limits::DEFAULT_TIMEOUT_SECS, 30);
        assert_eq!(limits::DEFAULT_RATE_LIMIT, 1000);
    }

    // --- Convenience wrapper tests (non-WASM returns PluginError) ---

    #[test]
    fn doc_read_returns_error_on_native() {
        let result = doc_read("tasks", "id-1");
        assert!(result.is_err());
    }

    #[test]
    fn doc_write_returns_error_on_native() {
        let result = doc_write("tasks", json!({"title": "test"}));
        assert!(result.is_err());
    }

    #[test]
    fn doc_delete_returns_error_on_native() {
        let result = doc_delete("tasks", "id-1");
        assert!(result.is_err());
    }

    #[test]
    fn emit_event_returns_error_on_native() {
        let result = emit_event("task.created", json!({"id": "123"}));
        assert!(result.is_err());
    }

    #[test]
    fn config_get_returns_error_on_native() {
        let result = config_get("some.key");
        assert!(result.is_err());
    }

    #[test]
    fn http_request_returns_error_on_native() {
        let result = http_request(HttpMethod::Get, "https://example.com", None, None);
        assert!(result.is_err());
    }

    #[test]
    fn host_response_serialization_roundtrip() {
        let resp = HostResponse {
            success: true,
            data: Some(json!({"key": "value"})),
            error: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let restored: HostResponse = serde_json::from_str(&json).unwrap();
        assert!(restored.success);
        assert_eq!(restored.data.unwrap()["key"], "value");
        assert!(restored.error.is_none());
    }
}
