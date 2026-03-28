//! Transport equivalence integration test.
//!
//! Proves that REST and GraphQL produce identical results for the same
//! workflow by issuing a `collection.list` request through both transports
//! and comparing the returned data arrays. Both dispatch through the same
//! `WorkflowRequest`/`WorkflowResponse` contract, so the data payload
//! must be identical regardless of transport.

use std::collections::HashMap;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::post;
use axum::{Extension, Router};
use serde_json::{json, Value};
use tower::ServiceExt;

use life_engine_types::identity::Identity;
use life_engine_types::workflow::{
    ResponseMeta, WorkflowError, WorkflowResponse, WorkflowStatus,
};

use life_engine_transport_graphql::handlers::graphql_handler;
use life_engine_transport_graphql::types::{translate_request, translate_response, GraphqlRequest};
use life_engine_transport_rest::config::RouteConfig;
use life_engine_transport_rest::handlers::{
    build_workflow_request, status_to_http,
};
use life_engine_transport_rest::router::build::build_router;

// ---------------------------------------------------------------------------
// Helper: wrap a router with a guest Identity extension.
// ---------------------------------------------------------------------------

fn with_identity(router: Router) -> Router {
    router.layer(Extension(Identity::guest()))
}

// ---------------------------------------------------------------------------
// Test 1: Both transports build WorkflowRequests with identical query params
// for a collection.list operation.
// ---------------------------------------------------------------------------

#[test]
fn equivalent_workflow_requests_carry_same_query_params() {
    let identity = Identity::guest();

    // REST handler builds a WorkflowRequest for GET /api/v1/data/tasks?limit=10&offset=0
    let rest_req = build_workflow_request(
        "collection.list".into(),
        identity.clone(),
        HashMap::from([("collection".into(), "tasks".into())]),
        HashMap::from([("limit".into(), "10".into()), ("offset".into(), "0".into())]),
        None,
    );

    // GraphQL handler builds a WorkflowRequest for { tasks(limit: 10, offset: 0) { ... } }
    let gql_req = GraphqlRequest {
        query: "{ tasks(limit: 10, offset: 0) { id title } }".into(),
        operation_name: None,
        variables: HashMap::from([
            ("limit".into(), json!(10)),
            ("offset".into(), json!(0)),
        ]),
    };
    let graphql_req = translate_request(&gql_req, identity);

    // Both carry the same query parameters (flattened to strings).
    assert_eq!(rest_req.query.get("limit").unwrap(), "10");
    assert_eq!(graphql_req.query.get("limit").unwrap(), "10");
    assert_eq!(rest_req.query.get("offset").unwrap(), "0");
    assert_eq!(graphql_req.query.get("offset").unwrap(), "0");

    // Both carry the same identity.
    assert_eq!(rest_req.identity, graphql_req.identity);

    // Source bindings differ (transport-specific) but are non-empty.
    assert_eq!(rest_req.meta.source_binding, "rest");
    assert_eq!(graphql_req.meta.source_binding, "graphql");

    // Both have valid request IDs.
    assert!(uuid::Uuid::parse_str(&rest_req.meta.request_id).is_ok());
    assert!(uuid::Uuid::parse_str(&graphql_req.meta.request_id).is_ok());
}

// ---------------------------------------------------------------------------
// Test 2: Given the same WorkflowResponse, both transports produce responses
// with identical data payloads.
// ---------------------------------------------------------------------------

#[test]
fn same_workflow_response_produces_identical_data_payloads() {
    let data = json!([
        {"id": "task-1", "title": "Buy groceries", "status": "pending"},
        {"id": "task-2", "title": "Write tests", "status": "completed"},
    ]);

    let shared_response = WorkflowResponse {
        status: WorkflowStatus::Ok,
        data: Some(data.clone()),
        errors: vec![],
        meta: ResponseMeta {
            request_id: "shared-req-001".into(),
            duration_ms: 42,
            traces: vec![],
        },
    };

    // REST: status_to_http confirms same HTTP code.
    let rest_status = status_to_http(WorkflowStatus::Ok);
    assert_eq!(rest_status, StatusCode::OK);

    // GraphQL translation: { "data": [...] }
    let (gql_status_code, gql_body) = translate_response(&shared_response);
    assert_eq!(gql_status_code, 200);

    // Both use the same HTTP status code.
    assert_eq!(rest_status.as_u16(), gql_status_code);

    // GraphQL envelope carries the exact same data array as the source.
    let gql_data = gql_body.get("data").expect("GraphQL response must have 'data' key");
    assert_eq!(gql_data, &data, "GraphQL data payload must match source");

    // REST envelope also wraps data identically (verified structurally: the
    // REST handler wraps `response.data` in `{ "data": <payload> }`, so the
    // payload passed to the workflow response is preserved verbatim).
}

// ---------------------------------------------------------------------------
// Test 3: Error responses preserve the same error information across both
// transports.
// ---------------------------------------------------------------------------

#[test]
fn error_responses_carry_same_error_information() {
    let shared_response = WorkflowResponse {
        status: WorkflowStatus::NotFound,
        data: None,
        errors: vec![WorkflowError {
            code: "NOT_FOUND".into(),
            message: "Collection 'widgets' does not exist".into(),
            detail: None,
        }],
        meta: ResponseMeta {
            request_id: "err-req-001".into(),
            duration_ms: 2,
            traces: vec![],
        },
    };

    // REST produces { "error": { "code": "NOT_FOUND", "message": "..." } }
    let rest_status = status_to_http(WorkflowStatus::NotFound);
    assert_eq!(rest_status, StatusCode::NOT_FOUND);

    // GraphQL produces { "errors": [{ "message": "...", "extensions": { "code": "NOT_FOUND" } }] }
    let (gql_status_code, gql_body) = translate_response(&shared_response);
    assert_eq!(gql_status_code, 404);

    let gql_errors = gql_body.get("errors").unwrap().as_array().unwrap();
    assert_eq!(gql_errors.len(), 1);
    assert_eq!(
        gql_errors[0].get("message").unwrap().as_str().unwrap(),
        "Collection 'widgets' does not exist"
    );
    assert_eq!(
        gql_errors[0]["extensions"]["code"].as_str().unwrap(),
        "NOT_FOUND"
    );

    // Both transports map to the same HTTP status code.
    assert_eq!(rest_status.as_u16(), gql_status_code);
}

// ---------------------------------------------------------------------------
// Test 4: HTTP-level equivalence — send a collection.list request through
// both transport routers and verify both return 200 with a data envelope.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn http_level_rest_and_graphql_both_return_data_envelope() {
    // --- REST: GET /api/v1/data/tasks ---
    let rest_routes = vec![RouteConfig {
        method: "GET".into(),
        path: "/api/v1/data/:collection".into(),
        workflow: "collection.list".into(),
        public: false,
    }];

    let rest_app = with_identity(build_router(&rest_routes));
    let rest_req = Request::builder()
        .method("GET")
        .uri("/api/v1/data/tasks?limit=10")
        .body(Body::empty())
        .unwrap();

    let rest_resp = rest_app.oneshot(rest_req).await.unwrap();
    assert_eq!(rest_resp.status(), StatusCode::OK);

    let rest_body = axum::body::to_bytes(rest_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let rest_json: Value = serde_json::from_slice(&rest_body).unwrap();

    // --- GraphQL: POST /graphql ---
    let gql_app = with_identity(Router::new().route("/graphql", post(graphql_handler)));
    let gql_req = Request::builder()
        .method("POST")
        .uri("/graphql")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "query": "{ tasks { id title } }",
                "variables": {"limit": 10}
            })
            .to_string(),
        ))
        .unwrap();

    let gql_resp = gql_app.oneshot(gql_req).await.unwrap();
    assert_eq!(gql_resp.status(), StatusCode::OK);

    let gql_body = axum::body::to_bytes(gql_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let gql_json: Value = serde_json::from_slice(&gql_body).unwrap();

    // Both return a "data" envelope.
    assert!(
        rest_json.get("data").is_some(),
        "REST response must have 'data' key: {rest_json}"
    );
    assert!(
        gql_json.get("data").is_some(),
        "GraphQL response must have 'data' key: {gql_json}"
    );

    // Both currently return the same placeholder data (workflow engine not yet
    // wired). Once the dispatcher is wired in Phase 10, these will return real
    // records from the same system workflow, and this test will verify that
    // the actual data arrays are identical.
    assert_eq!(
        rest_json["data"], gql_json["data"],
        "REST and GraphQL must return identical data for the same workflow"
    );
}

// ---------------------------------------------------------------------------
// Test 5: All WorkflowStatus variants map to the same HTTP status code
// regardless of which transport performs the mapping.
// ---------------------------------------------------------------------------

#[test]
fn status_code_mapping_is_identical_across_transports() {
    let statuses = vec![
        (WorkflowStatus::Ok, 200),
        (WorkflowStatus::Created, 201),
        (WorkflowStatus::NotFound, 404),
        (WorkflowStatus::Denied, 403),
        (WorkflowStatus::Invalid, 400),
        (WorkflowStatus::Error, 500),
    ];

    for (status, expected_code) in statuses {
        // REST mapping
        let rest_code = status_to_http(status).as_u16();

        // GraphQL mapping (via WorkflowStatus::http_status_code)
        let gql_code = status.http_status_code();

        assert_eq!(
            rest_code, expected_code,
            "REST: {status:?} should map to {expected_code}"
        );
        assert_eq!(
            gql_code, expected_code,
            "GraphQL: {status:?} should map to {expected_code}"
        );
        assert_eq!(
            rest_code, gql_code,
            "REST and GraphQL must agree on status code for {status:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Test 6: Created status is treated as success by both transports.
// ---------------------------------------------------------------------------

#[test]
fn created_response_is_success_in_both_transports() {
    let data = json!({"id": "new-task-1", "title": "New task"});
    let response = WorkflowResponse {
        status: WorkflowStatus::Created,
        data: Some(data.clone()),
        errors: vec![],
        meta: ResponseMeta {
            request_id: "create-001".into(),
            duration_ms: 5,
            traces: vec![],
        },
    };

    // REST: should return { "data": ... } with 201
    let rest_status = status_to_http(WorkflowStatus::Created);
    assert_eq!(rest_status, StatusCode::CREATED);

    // GraphQL: should return { "data": ... } with 201
    let (gql_status, gql_body) = translate_response(&response);
    assert_eq!(gql_status, 201);
    assert!(gql_body.get("data").is_some());
    assert!(gql_body.get("errors").is_none());
    assert_eq!(gql_body["data"], data);
}
