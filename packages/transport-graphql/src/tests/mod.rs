//! Tests for GraphQL transport.

use std::collections::HashMap;

use life_engine_types::identity::Identity;
use life_engine_types::workflow::{
    ResponseMeta, WorkflowError, WorkflowResponse, WorkflowStatus,
};

use crate::config::{generate_schema, PluginSchemaDeclaration};
use crate::types::{translate_request, translate_response, GraphqlRequest};

// ── Test 1: GraphQL request → WorkflowRequest translation (Req 8.1) ──

#[test]
fn translate_graphql_request_to_workflow_request() {
    let gql_req = GraphqlRequest {
        query: "{ tasks { id title } }".into(),
        operation_name: None,
        variables: HashMap::from([
            ("limit".into(), serde_json::json!(10)),
            ("offset".into(), serde_json::json!(0)),
            ("status".into(), serde_json::json!("active")),
        ]),
    };

    let identity = Identity {
        subject: "user-123".into(),
        issuer: "life-engine".into(),
        claims: HashMap::new(),
    };

    let wf_req = translate_request(&gql_req, identity.clone());

    // workflow is always graphql.query
    assert_eq!(wf_req.workflow, "graphql.query");
    // identity is passed through
    assert_eq!(wf_req.identity, identity);
    // params are empty (GraphQL has no path params)
    assert!(wf_req.params.is_empty());
    // variables are flattened into query
    assert_eq!(wf_req.query.get("limit").unwrap(), "10");
    assert_eq!(wf_req.query.get("offset").unwrap(), "0");
    assert_eq!(wf_req.query.get("status").unwrap(), "active");
    // body carries the raw query string
    assert_eq!(
        wf_req.body.unwrap(),
        serde_json::Value::String("{ tasks { id title } }".into())
    );
    // meta source_binding is "graphql"
    assert_eq!(wf_req.meta.source_binding, "graphql");
    // request_id is a valid UUID
    assert!(uuid::Uuid::parse_str(&wf_req.meta.request_id).is_ok());
}

// ── Test 2: Schema generation from plugin manifest (Req 9.1, 9.2, 9.3) ──

#[test]
fn generate_schema_from_plugin_declarations() {
    let declarations = vec![
        PluginSchemaDeclaration {
            collection: "tasks".into(),
            fields: HashMap::from([
                ("id".into(), "string".into()),
                ("title".into(), "string".into()),
                ("priority".into(), "integer".into()),
                ("completed".into(), "boolean".into()),
            ]),
        },
        PluginSchemaDeclaration {
            collection: "daily_notes".into(),
            fields: HashMap::from([
                ("id".into(), "string".into()),
                ("body".into(), "string".into()),
                ("word_count".into(), "number".into()),
            ]),
        },
    ];

    let types = generate_schema(&declarations);

    assert_eq!(types.len(), 2);

    // First type: "tasks" → "Tasks"
    let tasks_type = types.iter().find(|t| t.collection == "tasks").unwrap();
    assert_eq!(tasks_type.type_name, "Tasks");
    assert!(tasks_type.fields.iter().any(|(n, t)| n == "id" && t == "String"));
    assert!(tasks_type.fields.iter().any(|(n, t)| n == "priority" && t == "Int"));
    assert!(tasks_type.fields.iter().any(|(n, t)| n == "completed" && t == "Boolean"));

    // Second type: "daily_notes" → "DailyNotes" (PascalCase)
    let notes_type = types.iter().find(|t| t.collection == "daily_notes").unwrap();
    assert_eq!(notes_type.type_name, "DailyNotes");
    assert!(notes_type.fields.iter().any(|(n, t)| n == "word_count" && t == "Float"));
}

#[test]
fn generate_schema_empty_when_no_declarations() {
    let types = generate_schema(&[]);
    assert!(types.is_empty(), "no declarations should produce no types");
}

// ── Test 3: Response shape — success (Req 8.2) ──

#[test]
fn translate_success_response_to_graphql_envelope() {
    let wf_resp = WorkflowResponse {
        status: WorkflowStatus::Ok,
        data: Some(serde_json::json!([{"id": "1", "title": "Buy milk"}])),
        errors: vec![],
        meta: ResponseMeta {
            request_id: "req-001".into(),
            duration_ms: 42,
            traces: vec![],
        },
    };

    let (status_code, body) = translate_response(&wf_resp);

    assert_eq!(status_code, 200);
    // Must have { "data": ... } shape
    assert!(body.get("data").is_some());
    assert!(body.get("errors").is_none());
    let data = body.get("data").unwrap();
    assert!(data.is_array());
    assert_eq!(data.as_array().unwrap().len(), 1);
}

// ── Test 4: Response shape — error (Req 8.3) ──

#[test]
fn translate_error_response_to_graphql_envelope() {
    let wf_resp = WorkflowResponse {
        status: WorkflowStatus::NotFound,
        data: None,
        errors: vec![WorkflowError {
            code: "NOT_FOUND".into(),
            message: "Collection 'widgets' does not exist".into(),
            detail: None,
        }],
        meta: ResponseMeta {
            request_id: "req-002".into(),
            duration_ms: 5,
            traces: vec![],
        },
    };

    let (status_code, body) = translate_response(&wf_resp);

    assert_eq!(status_code, 404);
    // Must have { "errors": [...] } shape
    assert!(body.get("errors").is_some());
    assert!(body.get("data").is_none());
    let errors = body.get("errors").unwrap().as_array().unwrap();
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].get("message").unwrap().as_str().unwrap(),
        "Collection 'widgets' does not exist"
    );
    assert_eq!(
        errors[0]
            .get("extensions")
            .unwrap()
            .get("code")
            .unwrap()
            .as_str()
            .unwrap(),
        "NOT_FOUND"
    );
}

// ── Test 5: Transport equivalence — identical WorkflowRequest shape (Req 10.1) ──

#[test]
fn transport_equivalence_same_workflow_request_shape() {
    // Simulate the REST handler building a WorkflowRequest for collection.list
    // and the GraphQL handler building one for graphql.query. The key invariant
    // is that both use the same WorkflowRequest struct with the same fields.
    let identity = Identity::guest();

    // REST-style request (what the REST handler would produce)
    let rest_request = life_engine_types::workflow::WorkflowRequest {
        workflow: "collection.list".into(),
        identity: identity.clone(),
        params: HashMap::from([("collection".into(), "tasks".into())]),
        query: HashMap::from([("limit".into(), "10".into())]),
        body: None,
        meta: life_engine_types::workflow::RequestMeta {
            request_id: "rest-001".into(),
            timestamp: chrono::Utc::now(),
            source_binding: "rest".into(),
        },
    };

    // GraphQL-style request
    let gql_req = GraphqlRequest {
        query: "{ tasks(limit: 10) { id title } }".into(),
        operation_name: None,
        variables: HashMap::from([("limit".into(), serde_json::json!(10))]),
    };
    let graphql_request = translate_request(&gql_req, identity);

    // Both produce WorkflowRequest — the struct is identical.
    // The workflow engine processes them the same way.
    assert_eq!(rest_request.query.get("limit").unwrap(), "10");
    assert_eq!(graphql_request.query.get("limit").unwrap(), "10");
    assert_eq!(graphql_request.meta.source_binding, "graphql");
    assert_eq!(rest_request.meta.source_binding, "rest");

    // Given the same WorkflowResponse, both transports produce equivalent data.
    let shared_response = WorkflowResponse {
        status: WorkflowStatus::Ok,
        data: Some(serde_json::json!([{"id": "1", "title": "Task"}])),
        errors: vec![],
        meta: ResponseMeta {
            request_id: "shared-001".into(),
            duration_ms: 10,
            traces: vec![],
        },
    };

    let (gql_status, gql_body) = translate_response(&shared_response);
    assert_eq!(gql_status, 200);
    // The data payload is the same regardless of transport
    assert_eq!(
        gql_body.get("data").unwrap(),
        &serde_json::json!([{"id": "1", "title": "Task"}])
    );
}

// ── Test 6: Error response with no explicit errors falls back to default ──

#[test]
fn translate_error_response_with_no_explicit_errors() {
    let wf_resp = WorkflowResponse {
        status: WorkflowStatus::Denied,
        data: None,
        errors: vec![], // no explicit error entries
        meta: ResponseMeta {
            request_id: "req-003".into(),
            duration_ms: 1,
            traces: vec![],
        },
    };

    let (status_code, body) = translate_response(&wf_resp);

    assert_eq!(status_code, 403);
    let errors = body.get("errors").unwrap().as_array().unwrap();
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].get("message").unwrap().as_str().unwrap(),
        "Access denied"
    );
}
