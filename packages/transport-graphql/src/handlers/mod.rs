//! GraphQL request handlers.
//!
//! Contains the Axum handler that accepts a GraphQL POST, translates it into
//! a `WorkflowRequest`, dispatches to the workflow engine, and translates
//! the `WorkflowResponse` back to the GraphQL wire format.

use axum::extract::Extension;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use life_engine_types::identity::Identity;
use life_engine_types::workflow::{ResponseMeta, WorkflowResponse, WorkflowStatus};
use serde_json::json;

use crate::types::{translate_request, translate_response, validate_mutation_collection, GraphqlRequest};

/// Axum handler for `POST /graphql`.
///
/// Extracts the authenticated identity from auth middleware and translates the
/// incoming GraphQL request into a `WorkflowRequest`. The workflow dispatcher
/// will be injected via Axum state in Phase 10; until then, a placeholder
/// response preserves the request ID for correlation.
pub async fn graphql_handler(
    Extension(identity): Extension<Identity>,
    Json(gql_req): Json<GraphqlRequest>,
) -> Response {
    // Validate mutation collection names against the CDM allowlist (CB-15).
    if let Err(invalid_collection) = validate_mutation_collection(&gql_req) {
        let body = serde_json::json!({
            "errors": [{
                "message": format!("Unknown collection: '{invalid_collection}'. Mutations may only target CDM collections."),
                "extensions": { "code": "INVALID_COLLECTION" }
            }]
        });
        return (StatusCode::BAD_REQUEST, Json(body)).into_response();
    }

    let workflow_request = translate_request(&gql_req, identity);

    // TODO: dispatch `workflow_request` to workflow engine and translate response.
    // For now, return a placeholder success preserving the request ID.
    let response = WorkflowResponse {
        status: WorkflowStatus::Ok,
        data: Some(json!({"placeholder": true})),
        errors: vec![],
        meta: ResponseMeta {
            request_id: workflow_request.meta.request_id,
            duration_ms: 0,
            traces: vec![],
        },
    };

    into_graphql_response(&response).into_response()
}

/// Translate a `WorkflowResponse` into an Axum response with the correct
/// HTTP status code and GraphQL-shaped JSON body.
pub fn into_graphql_response(resp: &WorkflowResponse) -> impl IntoResponse {
    let (status_code, body) = translate_response(resp);
    let status = StatusCode::from_u16(status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, Json(body))
}
