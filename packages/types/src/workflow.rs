//! Workflow engine contract types.
//!
//! These types define the input/output boundary between transport handlers
//! and the workflow engine. Handlers translate protocol-specific requests
//! into `WorkflowRequest` and translate `WorkflowResponse` back.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::identity::Identity;

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

/// A protocol-agnostic request dispatched from a transport handler to the
/// workflow engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRequest {
    /// Dot-separated workflow name (e.g. `"collection.list"`, `"graphql.query"`).
    pub workflow: String,
    /// Verified identity from the auth middleware.
    pub identity: Identity,
    /// Path parameters extracted by the handler (e.g. `:collection`, `:id`).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub params: HashMap<String, String>,
    /// Query string parameters (REST) or flattened arguments (GraphQL).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub query: HashMap<String, String>,
    /// Parsed request body, or `None` when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
    /// Request metadata for tracing and correlation.
    pub meta: RequestMeta,
}

/// Metadata attached to every `WorkflowRequest`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMeta {
    /// Unique request identifier for correlation.
    pub request_id: String,
    /// When the request entered the system.
    pub timestamp: DateTime<Utc>,
    /// Which transport binding originated the request (e.g. `"rest"`, `"graphql"`).
    pub source_binding: String,
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// The result returned by the workflow engine to the transport handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowResponse {
    /// Result status.
    pub status: WorkflowStatus,
    /// Result payload — `Some` on success, `None` on failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    /// Error details — empty on success.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<WorkflowError>,
    /// Response metadata for tracing and timing.
    pub meta: ResponseMeta,
}

/// Metadata attached to every `WorkflowResponse`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMeta {
    /// The request ID echoed from the originating `WorkflowRequest`.
    pub request_id: String,
    /// Processing duration in milliseconds.
    pub duration_ms: u64,
    /// Trace entries from pipeline steps.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub traces: Vec<String>,
}

/// A single error entry in a `WorkflowResponse`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowError {
    /// Machine-readable error code (e.g. `"NOT_FOUND"`, `"VALIDATION_ERROR"`).
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// Optional structured detail about the error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Status enum
// ---------------------------------------------------------------------------

/// Result status for a workflow execution.
///
/// Exactly six variants, each mapping to a distinct HTTP status code.
/// New variants may only be added when they carry distinct semantics
/// across at least two handler types (Rule A).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStatus {
    /// Success with data (HTTP 200).
    Ok,
    /// New resource persisted (HTTP 201).
    Created,
    /// Resource does not exist (HTTP 404).
    NotFound,
    /// Authenticated but not authorised (HTTP 403).
    Denied,
    /// Malformed or validation failure (HTTP 400).
    Invalid,
    /// Internal failure (HTTP 500).
    Error,
}

impl WorkflowStatus {
    /// Returns `true` for success statuses (`Ok` and `Created`).
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Ok | Self::Created)
    }

    /// Maps this status to its HTTP status code.
    pub fn http_status_code(&self) -> u16 {
        match self {
            Self::Ok => 200,
            Self::Created => 201,
            Self::NotFound => 404,
            Self::Denied => 403,
            Self::Invalid => 400,
            Self::Error => 500,
        }
    }
}
