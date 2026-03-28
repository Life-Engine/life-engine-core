//! Module-internal types for the GraphQL transport crate.
//!
//! Contains the `GraphqlRequest` wire type and helper functions for
//! translating between GraphQL and the protocol-agnostic workflow contract.

use std::collections::HashMap;

use chrono::Utc;
use life_engine_types::identity::Identity;
use life_engine_types::workflow::{RequestMeta, WorkflowRequest, WorkflowResponse, WorkflowStatus};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Wire-format body of an incoming GraphQL POST request.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GraphqlRequest {
    /// The GraphQL query or mutation string.
    pub query: String,
    /// Optional operation name when the document contains multiple operations.
    #[serde(default, rename = "operationName")]
    pub operation_name: Option<String>,
    /// Flattened GraphQL variables (limit, offset, filters, etc.).
    #[serde(default)]
    pub variables: HashMap<String, serde_json::Value>,
}

/// Translate a `GraphqlRequest` into a `WorkflowRequest` (Requirement 8.1).
///
/// - `workflow` is always `"graphql.query"`.
/// - `params` is empty (GraphQL has no path parameters).
/// - `query` is flattened from `variables` — only string-representable scalars
///   are included so that the workflow engine sees the same shape as REST query
///   parameters.
/// - `body` carries the raw query/mutation string.
/// - `meta` records a fresh request ID, timestamp, and `"graphql"` source.
pub fn translate_request(req: &GraphqlRequest, identity: Identity) -> WorkflowRequest {
    let query: HashMap<String, String> = req
        .variables
        .iter()
        .map(|(k, v)| {
            let s = match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            (k.clone(), s)
        })
        .collect();

    // Determine whether this is a mutation or a query based on the GraphQL document.
    let is_mutation = req.query.trim_start().starts_with("mutation");
    let workflow = if is_mutation {
        "graphql.mutation"
    } else {
        "graphql.query"
    };

    WorkflowRequest {
        workflow: workflow.into(),
        identity,
        params: HashMap::new(),
        query,
        body: Some(serde_json::Value::String(req.query.clone())),
        meta: RequestMeta {
            request_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            source_binding: "graphql".into(),
        },
    }
}

/// The set of valid CDM collection names that mutations may target.
///
/// Mutations referencing a collection outside this set are rejected before
/// reaching the workflow engine.
pub const CDM_COLLECTIONS: &[&str] = &[
    "events",
    "tasks",
    "contacts",
    "notes",
    "emails",
    "files",
    "credentials",
];

/// Check whether a collection name is a valid CDM collection.
pub fn is_valid_cdm_collection(name: &str) -> bool {
    CDM_COLLECTIONS.contains(&name)
}

/// Extract the `collection` variable from a GraphQL mutation request and
/// validate it against the CDM allowlist.
///
/// Returns `Ok(())` if the request is not a mutation, has no `collection`
/// variable, or the collection is in the allowlist. Returns `Err` with
/// the invalid collection name otherwise.
pub fn validate_mutation_collection(req: &GraphqlRequest) -> Result<(), String> {
    let is_mutation = req.query.trim_start().starts_with("mutation");
    if !is_mutation {
        return Ok(());
    }

    if let Some(collection_val) = req.variables.get("collection") {
        let collection = match collection_val {
            serde_json::Value::String(s) => s.as_str(),
            _ => return Err(collection_val.to_string()),
        };
        if !is_valid_cdm_collection(collection) {
            return Err(collection.to_string());
        }
    }

    Ok(())
}

/// GraphQL-shaped success envelope (Requirement 8.2).
#[derive(Debug, Serialize)]
pub struct GraphqlSuccessResponse {
    pub data: serde_json::Value,
}

/// A single entry in the GraphQL errors array (Requirement 8.3).
#[derive(Debug, Serialize)]
pub struct GraphqlErrorEntry {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<GraphqlErrorExtensions>,
}

/// Machine-readable extensions attached to a GraphQL error.
#[derive(Debug, Serialize)]
pub struct GraphqlErrorExtensions {
    pub code: String,
}

/// GraphQL-shaped error envelope.
#[derive(Debug, Serialize)]
pub struct GraphqlErrorResponse {
    pub errors: Vec<GraphqlErrorEntry>,
}

/// Translate a `WorkflowResponse` into a JSON value suitable for the
/// GraphQL wire format (Requirements 8.2, 8.3).
pub fn translate_response(resp: &WorkflowResponse) -> (u16, serde_json::Value) {
    let status_code = resp.status.http_status_code();

    if resp.status.is_success() {
        let body = GraphqlSuccessResponse {
            data: resp.data.clone().unwrap_or(serde_json::Value::Null),
        };
        (status_code, serde_json::to_value(body).unwrap())
    } else {
        let errors: Vec<GraphqlErrorEntry> = if resp.errors.is_empty() {
            vec![GraphqlErrorEntry {
                message: default_error_message(resp.status),
                extensions: Some(GraphqlErrorExtensions {
                    code: format!("{:?}", resp.status),
                }),
            }]
        } else {
            resp.errors
                .iter()
                .map(|e| GraphqlErrorEntry {
                    message: e.message.clone(),
                    extensions: Some(GraphqlErrorExtensions {
                        code: e.code.clone(),
                    }),
                })
                .collect()
        };
        let body = GraphqlErrorResponse { errors };
        (status_code, serde_json::to_value(body).unwrap())
    }
}

fn default_error_message(status: WorkflowStatus) -> String {
    match status {
        WorkflowStatus::NotFound => "Not found".into(),
        WorkflowStatus::Denied => "Access denied".into(),
        WorkflowStatus::Invalid => "Invalid request".into(),
        WorkflowStatus::Error => "Internal error".into(),
        _ => "Unknown error".into(),
    }
}
