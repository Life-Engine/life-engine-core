//! Tests for workflow contract types.
//!
//! Organised by spec requirement. Written TDD-first — these tests define
//! the contract before any implementation exists.

use std::collections::HashMap;

use chrono::Utc;
use serde_json::json;

use life_engine_types::identity::{Identity, TriggerContext};
use life_engine_types::workflow::{
    RequestMeta, ResponseMeta, WorkflowError, WorkflowRequest, WorkflowResponse, WorkflowStatus,
};

// ---------------------------------------------------------------------------
// Req 1 — WorkflowRequest
// ---------------------------------------------------------------------------
mod workflow_request {
    use super::*;

    fn sample_request() -> WorkflowRequest {
        WorkflowRequest {
            workflow: "collection.list".into(),
            identity: Identity::guest(),
            params: HashMap::from([("collection".into(), "tasks".into())]),
            query: HashMap::from([("limit".into(), "10".into())]),
            body: Some(json!({"title": "Test"})),
            meta: RequestMeta {
                request_id: "req-001".into(),
                timestamp: Utc::now(),
                source_binding: "rest".into(),
            },
        }
    }

    #[test]
    fn has_all_six_fields() {
        let req = sample_request();
        assert_eq!(req.workflow, "collection.list");
        assert_eq!(req.identity.subject, "anonymous");
        assert_eq!(req.params.get("collection").unwrap(), "tasks");
        assert_eq!(req.query.get("limit").unwrap(), "10");
        assert!(req.body.is_some());
        assert_eq!(req.meta.request_id, "req-001");
    }

    #[test]
    fn meta_has_required_fields() {
        let req = sample_request();
        assert!(!req.meta.request_id.is_empty());
        assert_eq!(req.meta.source_binding, "rest");
        // timestamp should be recent
        let elapsed = Utc::now() - req.meta.timestamp;
        assert!(elapsed.num_seconds() < 5);
    }

    #[test]
    fn empty_params_and_query_for_graphql() {
        let req = WorkflowRequest {
            workflow: "graphql.query".into(),
            identity: Identity::guest(),
            params: HashMap::new(),
            query: HashMap::new(),
            body: Some(json!("{ tasks { id title } }")),
            meta: RequestMeta {
                request_id: "req-002".into(),
                timestamp: Utc::now(),
                source_binding: "graphql".into(),
            },
        };
        assert!(req.params.is_empty());
        assert!(req.query.is_empty());
    }

    #[test]
    fn body_none_when_absent() {
        let req = WorkflowRequest {
            workflow: "system.health".into(),
            identity: Identity::guest(),
            params: HashMap::new(),
            query: HashMap::new(),
            body: None,
            meta: RequestMeta {
                request_id: "req-003".into(),
                timestamp: Utc::now(),
                source_binding: "rest".into(),
            },
        };
        assert!(req.body.is_none());
    }
}

// ---------------------------------------------------------------------------
// Req 2 — WorkflowResponse
// ---------------------------------------------------------------------------
mod workflow_response {
    use super::*;

    #[test]
    fn success_has_data_no_errors() {
        let resp = WorkflowResponse {
            status: WorkflowStatus::Ok,
            data: Some(json!({"items": []})),
            errors: vec![],
            meta: ResponseMeta {
                request_id: "req-001".into(),
                duration_ms: 42,
                traces: vec![],
            },
        };
        assert!(resp.data.is_some());
        assert!(resp.errors.is_empty());
    }

    #[test]
    fn failure_has_errors_no_data() {
        let resp = WorkflowResponse {
            status: WorkflowStatus::NotFound,
            data: None,
            errors: vec![WorkflowError {
                code: "NOT_FOUND".into(),
                message: "Resource not found".into(),
                detail: None,
            }],
            meta: ResponseMeta {
                request_id: "req-001".into(),
                duration_ms: 5,
                traces: vec![],
            },
        };
        assert!(resp.data.is_none());
        assert_eq!(resp.errors.len(), 1);
        assert_eq!(resp.errors[0].code, "NOT_FOUND");
    }

    #[test]
    fn meta_echoes_request_id() {
        let resp = WorkflowResponse {
            status: WorkflowStatus::Ok,
            data: Some(json!(null)),
            errors: vec![],
            meta: ResponseMeta {
                request_id: "req-abc".into(),
                duration_ms: 100,
                traces: vec!["step1".into()],
            },
        };
        assert_eq!(resp.meta.request_id, "req-abc");
        assert_eq!(resp.meta.duration_ms, 100);
        assert_eq!(resp.meta.traces.len(), 1);
    }
}

// ---------------------------------------------------------------------------
// Req 3, 5, 6 — WorkflowStatus
// ---------------------------------------------------------------------------
mod workflow_status {
    use super::*;

    #[test]
    fn has_exactly_six_variants() {
        // Verify all six variants can be constructed.
        let variants = [
            WorkflowStatus::Ok,
            WorkflowStatus::Created,
            WorkflowStatus::NotFound,
            WorkflowStatus::Denied,
            WorkflowStatus::Invalid,
            WorkflowStatus::Error,
        ];
        assert_eq!(variants.len(), 6);
    }

    #[test]
    fn is_success_true_for_ok_and_created() {
        assert!(WorkflowStatus::Ok.is_success());
        assert!(WorkflowStatus::Created.is_success());
    }

    #[test]
    fn is_success_false_for_error_variants() {
        assert!(!WorkflowStatus::NotFound.is_success());
        assert!(!WorkflowStatus::Denied.is_success());
        assert!(!WorkflowStatus::Invalid.is_success());
        assert!(!WorkflowStatus::Error.is_success());
    }

    #[test]
    fn http_status_code_ok() {
        assert_eq!(WorkflowStatus::Ok.http_status_code(), 200);
    }

    #[test]
    fn http_status_code_created() {
        assert_eq!(WorkflowStatus::Created.http_status_code(), 201);
    }

    #[test]
    fn http_status_code_not_found() {
        assert_eq!(WorkflowStatus::NotFound.http_status_code(), 404);
    }

    #[test]
    fn http_status_code_denied() {
        assert_eq!(WorkflowStatus::Denied.http_status_code(), 403);
    }

    #[test]
    fn http_status_code_invalid() {
        assert_eq!(WorkflowStatus::Invalid.http_status_code(), 400);
    }

    #[test]
    fn http_status_code_error() {
        assert_eq!(WorkflowStatus::Error.http_status_code(), 500);
    }
}

// ---------------------------------------------------------------------------
// Req 2.3 — WorkflowError
// ---------------------------------------------------------------------------
mod workflow_error {
    use super::*;

    #[test]
    fn error_with_detail() {
        let err = WorkflowError {
            code: "VALIDATION_ERROR".into(),
            message: "Field 'title' is required".into(),
            detail: Some(json!({"field": "title"})),
        };
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert_eq!(err.message, "Field 'title' is required");
        assert!(err.detail.is_some());
    }

    #[test]
    fn error_without_detail() {
        let err = WorkflowError {
            code: "INTERNAL".into(),
            message: "Unexpected failure".into(),
            detail: None,
        };
        assert!(err.detail.is_none());
    }
}

// ---------------------------------------------------------------------------
// Req 7 — Identity
// ---------------------------------------------------------------------------
mod identity {
    use super::*;

    #[test]
    fn verified_identity() {
        let id = Identity {
            subject: "user-123".into(),
            issuer: "life-engine".into(),
            claims: HashMap::from([("role".into(), json!("admin"))]),
        };
        assert_eq!(id.subject, "user-123");
        assert_eq!(id.issuer, "life-engine");
        assert_eq!(id.claims.get("role").unwrap(), &json!("admin"));
    }

    #[test]
    fn guest_identity() {
        let id = Identity::guest();
        assert_eq!(id.subject, "anonymous");
        assert_eq!(id.issuer, "system");
        assert!(id.claims.is_empty());
    }
}

// ---------------------------------------------------------------------------
// TriggerContext
// ---------------------------------------------------------------------------
mod trigger_context {
    use super::*;

    #[test]
    fn endpoint_variant() {
        let ctx = TriggerContext::Endpoint {
            method: "POST".into(),
            path: "/api/v1/tasks".into(),
        };
        if let TriggerContext::Endpoint { method, path } = &ctx {
            assert_eq!(method, "POST");
            assert_eq!(path, "/api/v1/tasks");
        } else {
            panic!("Expected Endpoint variant");
        }
    }

    #[test]
    fn event_variant() {
        let ctx = TriggerContext::Event {
            event_type: "record.created".into(),
            source: "email-plugin".into(),
        };
        if let TriggerContext::Event {
            event_type,
            source,
        } = &ctx
        {
            assert_eq!(event_type, "record.created");
            assert_eq!(source, "email-plugin");
        } else {
            panic!("Expected Event variant");
        }
    }

    #[test]
    fn schedule_variant() {
        let ctx = TriggerContext::Schedule {
            cron_expr: "0 */5 * * *".into(),
        };
        if let TriggerContext::Schedule { cron_expr } = &ctx {
            assert_eq!(cron_expr, "0 */5 * * *");
        } else {
            panic!("Expected Schedule variant");
        }
    }
}

// ---------------------------------------------------------------------------
// Serialisation round-trips
// ---------------------------------------------------------------------------
mod serialisation {
    use super::*;

    #[test]
    fn workflow_request_round_trip() {
        let req = WorkflowRequest {
            workflow: "collection.get".into(),
            identity: Identity {
                subject: "user-1".into(),
                issuer: "le-core".into(),
                claims: HashMap::new(),
            },
            params: HashMap::from([("id".into(), "abc".into())]),
            query: HashMap::new(),
            body: None,
            meta: RequestMeta {
                request_id: "r-1".into(),
                timestamp: Utc::now(),
                source_binding: "rest".into(),
            },
        };
        let json = serde_json::to_string(&req).expect("serialize request");
        let restored: WorkflowRequest = serde_json::from_str(&json).expect("deserialize request");
        assert_eq!(restored.workflow, req.workflow);
        assert_eq!(restored.meta.request_id, req.meta.request_id);
        assert_eq!(restored.identity.subject, req.identity.subject);
    }

    #[test]
    fn workflow_response_round_trip() {
        let resp = WorkflowResponse {
            status: WorkflowStatus::Created,
            data: Some(json!({"id": "new-1"})),
            errors: vec![],
            meta: ResponseMeta {
                request_id: "r-1".into(),
                duration_ms: 15,
                traces: vec!["validate".into(), "persist".into()],
            },
        };
        let json = serde_json::to_string(&resp).expect("serialize response");
        let restored: WorkflowResponse =
            serde_json::from_str(&json).expect("deserialize response");
        assert_eq!(
            restored.status.http_status_code(),
            resp.status.http_status_code()
        );
        assert_eq!(restored.data, resp.data);
        assert_eq!(restored.meta.traces.len(), 2);
    }

    #[test]
    fn workflow_status_round_trip() {
        let variants = [
            WorkflowStatus::Ok,
            WorkflowStatus::Created,
            WorkflowStatus::NotFound,
            WorkflowStatus::Denied,
            WorkflowStatus::Invalid,
            WorkflowStatus::Error,
        ];
        for status in &variants {
            let json = serde_json::to_string(status).expect("serialize status");
            let restored: WorkflowStatus =
                serde_json::from_str(&json).expect("deserialize status");
            assert_eq!(restored.http_status_code(), status.http_status_code());
        }
    }

    #[test]
    fn identity_round_trip() {
        let id = Identity {
            subject: "u-42".into(),
            issuer: "auth0".into(),
            claims: HashMap::from([("scope".into(), json!("read write"))]),
        };
        let json = serde_json::to_string(&id).expect("serialize identity");
        let restored: Identity = serde_json::from_str(&json).expect("deserialize identity");
        assert_eq!(restored.subject, id.subject);
        assert_eq!(restored.claims.get("scope"), id.claims.get("scope"));
    }

    #[test]
    fn trigger_context_round_trip() {
        let contexts = [
            TriggerContext::Endpoint {
                method: "GET".into(),
                path: "/health".into(),
            },
            TriggerContext::Event {
                event_type: "sync.complete".into(),
                source: "calendar".into(),
            },
            TriggerContext::Schedule {
                cron_expr: "0 0 * * *".into(),
            },
        ];
        for ctx in &contexts {
            let json = serde_json::to_string(ctx).expect("serialize trigger");
            let restored: TriggerContext =
                serde_json::from_str(&json).expect("deserialize trigger");
            let original_json = serde_json::to_value(ctx).unwrap();
            let restored_json = serde_json::to_value(&restored).unwrap();
            assert_eq!(original_json, restored_json);
        }
    }

    #[test]
    fn workflow_error_round_trip() {
        let err = WorkflowError {
            code: "BAD_INPUT".into(),
            message: "Missing field".into(),
            detail: Some(json!({"field": "name"})),
        };
        let json = serde_json::to_string(&err).expect("serialize error");
        let restored: WorkflowError = serde_json::from_str(&json).expect("deserialize error");
        assert_eq!(restored.code, err.code);
        assert_eq!(restored.detail, err.detail);
    }

    #[test]
    fn workflow_status_serialises_as_snake_case() {
        assert_eq!(
            serde_json::to_string(&WorkflowStatus::Ok).unwrap(),
            "\"ok\""
        );
        assert_eq!(
            serde_json::to_string(&WorkflowStatus::Created).unwrap(),
            "\"created\""
        );
        assert_eq!(
            serde_json::to_string(&WorkflowStatus::NotFound).unwrap(),
            "\"not_found\""
        );
        assert_eq!(
            serde_json::to_string(&WorkflowStatus::Denied).unwrap(),
            "\"denied\""
        );
        assert_eq!(
            serde_json::to_string(&WorkflowStatus::Invalid).unwrap(),
            "\"invalid\""
        );
        assert_eq!(
            serde_json::to_string(&WorkflowStatus::Error).unwrap(),
            "\"error\""
        );
    }

    #[test]
    fn request_skips_empty_params_and_query() {
        let req = WorkflowRequest {
            workflow: "system.health".into(),
            identity: Identity::guest(),
            params: HashMap::new(),
            query: HashMap::new(),
            body: None,
            meta: RequestMeta {
                request_id: "r-skip".into(),
                timestamp: Utc::now(),
                source_binding: "rest".into(),
            },
        };
        let json_str = serde_json::to_string(&req).unwrap();
        // Empty HashMaps should be skipped
        assert!(!json_str.contains("\"params\""));
        assert!(!json_str.contains("\"query\""));
        // None body should be skipped
        assert!(!json_str.contains("\"body\""));
    }

    #[test]
    fn response_skips_empty_errors_and_traces() {
        let resp = WorkflowResponse {
            status: WorkflowStatus::Ok,
            data: Some(json!("ok")),
            errors: vec![],
            meta: ResponseMeta {
                request_id: "r-skip".into(),
                duration_ms: 1,
                traces: vec![],
            },
        };
        let json_str = serde_json::to_string(&resp).unwrap();
        assert!(!json_str.contains("\"errors\""));
        assert!(!json_str.contains("\"traces\""));
    }
}
