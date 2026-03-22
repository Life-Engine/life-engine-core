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

/// Request types for calling host functions from WASM guest code.
///
/// Each variant maps to a host function in the `WasmHostBridge`.
/// The `type` field in the serialized JSON determines the dispatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum HostRequest {
    /// Read a record by ID. Requires `StorageRead` capability.
    StoreRead {
        collection: String,
        id: String,
    },
    /// Create a record. Requires `StorageWrite` capability.
    StoreWrite {
        collection: String,
        data: serde_json::Value,
    },
    /// Query records. Requires `StorageRead` capability.
    StoreQuery {
        collection: String,
        filters: serde_json::Value,
        limit: Option<u32>,
        offset: Option<u32>,
    },
    /// Delete a record. Requires `StorageWrite` capability.
    StoreDelete {
        collection: String,
        id: String,
    },
    /// Read a config value. Requires `ConfigRead` capability.
    ConfigGet {
        key: String,
    },
    /// Subscribe to events. Requires `EventsSubscribe` capability.
    EventSubscribe {
        event_type: String,
    },
    /// Emit an event. Requires `EventsEmit` capability.
    EventEmit {
        event_type: String,
        payload: serde_json::Value,
    },
    /// Log at info level. Requires `Logging` capability.
    LogInfo {
        message: String,
    },
    /// Log at warn level. Requires `Logging` capability.
    LogWarn {
        message: String,
    },
    /// Log at error level. Requires `Logging` capability.
    LogError {
        message: String,
    },
    /// Make an HTTP request. Requires `HttpOutbound` capability.
    HttpRequest {
        url: String,
        method: String,
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
            HostRequest::StoreDelete {
                collection: "c".into(),
                id: "i".into(),
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
                method: "GET".into(),
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
    fn limits_constants_correct() {
        assert_eq!(limits::DEFAULT_MEMORY_BYTES, 64 * 1024 * 1024);
        assert_eq!(limits::DEFAULT_TIMEOUT_SECS, 30);
        assert_eq!(limits::DEFAULT_RATE_LIMIT, 1000);
    }
}
