//! GraphQL request handlers.
//!
//! Contains the Axum handler that accepts a GraphQL POST, translates it into
//! a `WorkflowRequest`, dispatches to the workflow engine, and translates
//! the `WorkflowResponse` back to the GraphQL wire format.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use life_engine_types::identity::Identity;
use life_engine_types::workflow::WorkflowResponse;

use crate::types::{translate_request, translate_response, validate_mutation_collection, GraphqlRequest};

/// Axum handler for `POST /graphql`.
///
/// In production the workflow dispatcher is injected via Axum state; this
/// function demonstrates the translation pipeline and returns the translated
/// `WorkflowRequest` when no dispatcher is present (useful for integration
/// testing).
///
/// When a real dispatcher is wired in, the handler calls
/// `dispatcher.dispatch(workflow_request)` and translates the response.
pub async fn graphql_handler(
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

    // For now, build the WorkflowRequest to prove the translation pipeline.
    // A real implementation injects the Identity from auth middleware and the
    // dispatcher from Axum state.
    let _workflow_request = translate_request(&gql_req, Identity::guest());

    // Placeholder: return the translated request as JSON so tests can verify
    // the translation without a full workflow engine.
    (StatusCode::OK, Json(serde_json::to_value(&_workflow_request).unwrap())).into_response()
}

/// Translate a `WorkflowResponse` into an Axum response with the correct
/// HTTP status code and GraphQL-shaped JSON body.
pub fn into_graphql_response(resp: &WorkflowResponse) -> impl IntoResponse {
    let (status_code, body) = translate_response(resp);
    let status = StatusCode::from_u16(status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, Json(body))
}
