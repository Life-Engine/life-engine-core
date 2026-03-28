//! REST request handlers.
//!
//! Translates HTTP requests into `WorkflowRequest` and `WorkflowResponse`
//! back into HTTP responses. This module is the REST-specific boundary
//! between Axum and the protocol-agnostic workflow engine.

use std::collections::HashMap;

use axum::extract::{Extension, Path, Query};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;

use life_engine_types::identity::Identity;
use life_engine_types::workflow::{
    RequestMeta, ResponseMeta, WorkflowError, WorkflowRequest, WorkflowResponse, WorkflowStatus,
};

/// Route configuration carried as an Axum extension on each matched route.
///
/// The router attaches this when the route is built so that the handler
/// knows which workflow to dispatch to.
#[derive(Debug, Clone)]
pub struct RouteConfig {
    /// The workflow to invoke (e.g. `"collection.list"`).
    pub workflow: String,
}

/// Builds a `WorkflowRequest` from the HTTP request components.
///
/// Requirement 7.1: populate workflow, identity, params, query, body, meta.
pub fn build_workflow_request(
    workflow: String,
    identity: Identity,
    params: HashMap<String, String>,
    query: HashMap<String, String>,
    body: Option<Value>,
) -> WorkflowRequest {
    WorkflowRequest {
        workflow,
        identity,
        params,
        query,
        body,
        meta: RequestMeta {
            request_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            source_binding: "rest".to_string(),
        },
    }
}

/// Converts a `WorkflowStatus` to the corresponding `axum::http::StatusCode`.
///
/// Requirement 7.4:
/// - Ok -> 200, Created -> 201, NotFound -> 404,
/// - Denied -> 403, Invalid -> 400, Error -> 500.
pub fn status_to_http(status: WorkflowStatus) -> StatusCode {
    match status {
        WorkflowStatus::Ok => StatusCode::OK,
        WorkflowStatus::Created => StatusCode::CREATED,
        WorkflowStatus::NotFound => StatusCode::NOT_FOUND,
        WorkflowStatus::Denied => StatusCode::FORBIDDEN,
        WorkflowStatus::Invalid => StatusCode::BAD_REQUEST,
        WorkflowStatus::Error => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Converts a `WorkflowResponse` into an Axum HTTP response.
///
/// Requirement 7.2: success responses produce `{ "data": ... }`.
/// Requirement 7.3: error responses produce `{ "error": { "code": "...", "message": "..." } }`.
pub fn workflow_response_to_http(response: WorkflowResponse) -> impl IntoResponse {
    let http_status = status_to_http(response.status);

    let body = if response.status.is_success() {
        json!({ "data": response.data })
    } else {
        let first_error = response.errors.first().cloned().unwrap_or(WorkflowError {
            code: "UNKNOWN".to_string(),
            message: "An unknown error occurred".to_string(),
            detail: None,
        });
        json!({
            "error": {
                "code": first_error.code,
                "message": first_error.message,
            }
        })
    };

    (http_status, Json(body))
}

/// Handler for requests that carry a JSON body (POST, PUT, PATCH).
///
/// Extracts route config, identity, path params, query params, and body,
/// then builds a `WorkflowRequest`. In a full integration the request
/// would be dispatched to the workflow engine; here we return the request
/// for testability until the engine dispatcher is wired.
pub async fn handle_with_body(
    Extension(route_config): Extension<RouteConfig>,
    Extension(identity): Extension<Identity>,
    Path(params): Path<HashMap<String, String>>,
    Query(query): Query<HashMap<String, String>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let _request = build_workflow_request(
        route_config.workflow,
        identity,
        params,
        query,
        Some(body),
    );

    // TODO: dispatch to workflow engine and translate response.
    // For now, return a placeholder success.
    let response = WorkflowResponse {
        status: WorkflowStatus::Ok,
        data: Some(json!({"placeholder": true})),
        errors: vec![],
        meta: ResponseMeta {
            request_id: _request.meta.request_id,
            duration_ms: 0,
            traces: vec![],
        },
    };

    workflow_response_to_http(response)
}

/// Handler for requests without a body (GET, DELETE).
///
/// Same translation logic as `handle_with_body` but without consuming
/// a JSON body from the request.
pub async fn handle_without_body(
    Extension(route_config): Extension<RouteConfig>,
    Extension(identity): Extension<Identity>,
    Path(params): Path<HashMap<String, String>>,
    Query(query): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let _request = build_workflow_request(
        route_config.workflow,
        identity,
        params,
        query,
        None,
    );

    let response = WorkflowResponse {
        status: WorkflowStatus::Ok,
        data: Some(json!({"placeholder": true})),
        errors: vec![],
        meta: ResponseMeta {
            request_id: _request.meta.request_id,
            duration_ms: 0,
            traces: vec![],
        },
    };

    workflow_response_to_http(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ---------------------------------------------------------------
    // Test 1: WorkflowRequest is built with all required fields (Req 7.1)
    // ---------------------------------------------------------------
    #[test]
    fn build_request_populates_all_fields() {
        let identity = Identity::guest();
        let mut params = HashMap::new();
        params.insert("collection".to_string(), "tasks".to_string());
        let mut query = HashMap::new();
        query.insert("limit".to_string(), "10".to_string());
        let body = Some(json!({"title": "Buy milk"}));

        let req = build_workflow_request(
            "collection.create".to_string(),
            identity.clone(),
            params.clone(),
            query.clone(),
            body.clone(),
        );

        assert_eq!(req.workflow, "collection.create");
        assert_eq!(req.identity, identity);
        assert_eq!(req.params, params);
        assert_eq!(req.query, query);
        assert_eq!(req.body, body);
        assert_eq!(req.meta.source_binding, "rest");
        assert!(!req.meta.request_id.is_empty());
    }

    // ---------------------------------------------------------------
    // Test 2: Request without body has body = None (Req 7.1)
    // ---------------------------------------------------------------
    #[test]
    fn build_request_without_body() {
        let req = build_workflow_request(
            "collection.list".to_string(),
            Identity::guest(),
            HashMap::new(),
            HashMap::new(),
            None,
        );

        assert!(req.body.is_none());
        assert_eq!(req.workflow, "collection.list");
    }

    // ---------------------------------------------------------------
    // Test 3: Status code mapping (Req 7.4)
    // ---------------------------------------------------------------
    #[test]
    fn status_maps_to_correct_http_codes() {
        assert_eq!(status_to_http(WorkflowStatus::Ok), StatusCode::OK);
        assert_eq!(status_to_http(WorkflowStatus::Created), StatusCode::CREATED);
        assert_eq!(
            status_to_http(WorkflowStatus::NotFound),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            status_to_http(WorkflowStatus::Denied),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            status_to_http(WorkflowStatus::Invalid),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            status_to_http(WorkflowStatus::Error),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    // ---------------------------------------------------------------
    // Test 4: Success response produces { "data": ... } envelope (Req 7.2)
    // ---------------------------------------------------------------
    #[tokio::test]
    async fn success_response_wraps_data() {
        let response = WorkflowResponse {
            status: WorkflowStatus::Ok,
            data: Some(json!({"id": "abc", "title": "Task"})),
            errors: vec![],
            meta: ResponseMeta {
                request_id: "req-1".to_string(),
                duration_ms: 5,
                traces: vec![],
            },
        };

        let (parts, body) = workflow_response_to_http(response).into_response().into_parts();

        assert_eq!(parts.status, StatusCode::OK);

        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let json_body: Value = serde_json::from_slice(&bytes).unwrap();
        assert!(json_body.get("data").is_some());
        assert_eq!(json_body["data"]["id"], "abc");
    }

    // ---------------------------------------------------------------
    // Test 5: Error response produces { "error": { "code", "message" } } (Req 7.3)
    // ---------------------------------------------------------------
    #[tokio::test]
    async fn error_response_wraps_error_envelope() {
        let response = WorkflowResponse {
            status: WorkflowStatus::NotFound,
            data: None,
            errors: vec![WorkflowError {
                code: "NOT_FOUND".to_string(),
                message: "Collection 'widgets' does not exist".to_string(),
                detail: None,
            }],
            meta: ResponseMeta {
                request_id: "req-2".to_string(),
                duration_ms: 1,
                traces: vec![],
            },
        };

        let (parts, body) = workflow_response_to_http(response).into_response().into_parts();

        assert_eq!(parts.status, StatusCode::NOT_FOUND);

        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let json_body: Value = serde_json::from_slice(&bytes).unwrap();
        assert!(json_body.get("error").is_some());
        assert_eq!(json_body["error"]["code"], "NOT_FOUND");
        assert_eq!(
            json_body["error"]["message"],
            "Collection 'widgets' does not exist"
        );
    }

    // ---------------------------------------------------------------
    // Test 6: Error response with no errors provides fallback (Req 7.3)
    // ---------------------------------------------------------------
    #[tokio::test]
    async fn error_response_with_empty_errors_uses_fallback() {
        let response = WorkflowResponse {
            status: WorkflowStatus::Error,
            data: None,
            errors: vec![],
            meta: ResponseMeta {
                request_id: "req-3".to_string(),
                duration_ms: 0,
                traces: vec![],
            },
        };

        let (parts, body) = workflow_response_to_http(response).into_response().into_parts();

        assert_eq!(parts.status, StatusCode::INTERNAL_SERVER_ERROR);

        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let json_body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json_body["error"]["code"], "UNKNOWN");
        assert_eq!(json_body["error"]["message"], "An unknown error occurred");
    }

    // ---------------------------------------------------------------
    // Test 7: Created status returns 201 with data envelope (Req 7.2, 7.4)
    // ---------------------------------------------------------------
    #[tokio::test]
    async fn created_response_returns_201_with_data() {
        let response = WorkflowResponse {
            status: WorkflowStatus::Created,
            data: Some(json!({"id": "new-123"})),
            errors: vec![],
            meta: ResponseMeta {
                request_id: "req-4".to_string(),
                duration_ms: 2,
                traces: vec![],
            },
        };

        let (parts, body) = workflow_response_to_http(response).into_response().into_parts();

        assert_eq!(parts.status, StatusCode::CREATED);

        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let json_body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json_body["data"]["id"], "new-123");
    }

    // ---------------------------------------------------------------
    // Test 8: Meta has unique request IDs per call
    // ---------------------------------------------------------------
    #[test]
    fn each_request_gets_unique_id() {
        let r1 = build_workflow_request(
            "collection.list".to_string(),
            Identity::guest(),
            HashMap::new(),
            HashMap::new(),
            None,
        );
        let r2 = build_workflow_request(
            "collection.list".to_string(),
            Identity::guest(),
            HashMap::new(),
            HashMap::new(),
            None,
        );

        assert_ne!(r1.meta.request_id, r2.meta.request_id);
    }

    // ---------------------------------------------------------------
    // Test 9: Path parameters are correctly propagated (Req 5.3)
    // ---------------------------------------------------------------
    #[test]
    fn path_params_propagated_to_request() {
        let mut params = HashMap::new();
        params.insert("collection".to_string(), "contacts".to_string());
        params.insert("id".to_string(), "42".to_string());

        let req = build_workflow_request(
            "collection.get".to_string(),
            Identity::guest(),
            params,
            HashMap::new(),
            None,
        );

        assert_eq!(req.params.get("collection").unwrap(), "contacts");
        assert_eq!(req.params.get("id").unwrap(), "42");
    }
}
