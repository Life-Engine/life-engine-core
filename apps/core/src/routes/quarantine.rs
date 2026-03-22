//! Admin endpoints for managing quarantined records.
//!
//! Provides list, reprocess, and delete operations for records that
//! failed schema validation and were placed in the `_quarantine` collection.

use crate::routes::health::AppState;
use crate::schema_registry::{QuarantineError, QUARANTINE_COLLECTION};
use crate::storage::{Pagination, StorageAdapter};

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use serde_json::json;

/// Default plugin ID for system-level quarantine access.
const CORE_PLUGIN_ID: &str = "core";

/// Query parameters for the quarantine list endpoint.
#[derive(Debug, Deserialize)]
pub struct QuarantineListParams {
    /// Number of records to skip.
    pub offset: Option<u32>,
    /// Maximum number of records to return.
    pub limit: Option<u32>,
}

/// GET /api/system/quarantine — List quarantined records with pagination.
pub async fn list_quarantine(
    State(state): State<AppState>,
    Query(params): Query<QuarantineListParams>,
) -> impl IntoResponse {
    let validated_storage = match &state.validated_storage {
        Some(vs) => vs,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": { "code": "SYSTEM_INTERNAL_ERROR", "message": "storage not available" }
                })),
            )
                .into_response();
        }
    };

    let pagination = Pagination {
        offset: params.offset.unwrap_or(0),
        limit: params.limit.unwrap_or(50),
    }
    .clamped();

    match validated_storage
        .inner()
        .list(CORE_PLUGIN_ID, QUARANTINE_COLLECTION, None, pagination)
        .await
    {
        Ok(qr) => (
            StatusCode::OK,
            Json(json!({
                "data": qr.records,
                "total": qr.total,
                "limit": qr.limit,
                "offset": qr.offset,
            })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "list quarantine failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": { "code": "SYSTEM_INTERNAL_ERROR", "message": "failed to list quarantine records" }
                })),
            )
                .into_response()
        }
    }
}

/// POST /api/system/quarantine/{id}/reprocess — Re-validate and restore a quarantined record.
pub async fn reprocess_quarantine(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let validated_storage = match &state.validated_storage {
        Some(vs) => vs,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": { "code": "SYSTEM_INTERNAL_ERROR", "message": "storage not available" }
                })),
            )
                .into_response();
        }
    };

    match validated_storage.reprocess_quarantined(&id).await {
        Ok(record) => (
            StatusCode::OK,
            Json(json!({ "data": record })),
        )
            .into_response(),
        Err(QuarantineError::NotFound(_)) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": { "code": "DATA_NOT_FOUND", "message": format!("quarantine record '{id}' not found") }
            })),
        )
            .into_response(),
        Err(QuarantineError::ValidationFailed { errors, .. }) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "error": {
                    "code": "DATA_VALIDATION_FAILED",
                    "message": "record still fails validation",
                    "details": errors,
                }
            })),
        )
            .into_response(),
        Err(QuarantineError::Internal(msg)) => {
            tracing::error!(error = %msg, "reprocess quarantine failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": { "code": "SYSTEM_INTERNAL_ERROR", "message": "failed to reprocess quarantine record" }
                })),
            )
                .into_response()
        }
    }
}

/// DELETE /api/system/quarantine/{id} — Permanently delete a quarantined record.
pub async fn delete_quarantine(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let validated_storage = match &state.validated_storage {
        Some(vs) => vs,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": { "code": "SYSTEM_INTERNAL_ERROR", "message": "storage not available" }
                })),
            )
                .into_response();
        }
    };

    match validated_storage
        .inner()
        .delete(CORE_PLUGIN_ID, QUARANTINE_COLLECTION, &id)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": { "code": "DATA_NOT_FOUND", "message": format!("quarantine record '{id}' not found") }
            })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "delete quarantine failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": { "code": "SYSTEM_INTERNAL_ERROR", "message": "failed to delete quarantine record" }
                })),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::middleware::auth_middleware;
    use crate::schema_registry::{QuarantineEntry, SchemaRegistry, ValidatedStorage};
    use crate::sqlite_storage::SqliteStorage;
    use crate::test_helpers::{
        auth_request, body_json, create_auth_state, default_app_state, generate_test_token,
    };
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::{delete, get, post};
    use axum::Router;
    use serde_json::json;
    use std::sync::Arc;
    use tower::ServiceExt;

    fn task_schema() -> serde_json::Value {
        json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "title": "Task",
            "type": "object",
            "required": ["id", "title", "status"],
            "properties": {
                "id": { "type": "string" },
                "title": { "type": "string" },
                "status": {
                    "type": "string",
                    "enum": ["pending", "in_progress", "completed", "cancelled"]
                }
            },
            "additionalProperties": false
        })
    }

    async fn setup_test_app() -> (Router, String, Arc<ValidatedStorage>) {
        let storage = Arc::new(SqliteStorage::open_in_memory().unwrap());
        let registry = SchemaRegistry::new();
        registry.register("tasks", &task_schema()).unwrap();

        let validated_storage = Arc::new(ValidatedStorage::new(
            Arc::clone(&storage),
            Arc::new(registry),
        ));

        let (auth_state, provider) = create_auth_state();

        let mut state = default_app_state();
        state.storage = Some(Arc::clone(&storage));
        state.validated_storage = Some(Arc::clone(&validated_storage));

        let app = Router::new()
            .route("/api/system/quarantine", get(list_quarantine))
            .route(
                "/api/system/quarantine/{id}/reprocess",
                post(reprocess_quarantine),
            )
            .route(
                "/api/system/quarantine/{id}",
                delete(delete_quarantine),
            )
            .with_state(state)
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                auth_middleware,
            ));

        let token = generate_test_token(&provider).await;

        (app, token, validated_storage)
    }

    #[tokio::test]
    async fn list_empty_quarantine() {
        let (app, token, _) = setup_test_app().await;

        let req = auth_request("GET", "/api/system/quarantine", &token, None);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["total"], 0);
        assert!(json["data"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn list_quarantine_with_records() {
        let (app, token, vs) = setup_test_app().await;

        // Quarantine some invalid data.
        let _ = vs
            .validated_create("plug1", "tasks", json!({"id": "t1"}))
            .await;
        let _ = vs
            .validated_create("plug1", "tasks", json!({"id": "t2"}))
            .await;

        let req = auth_request("GET", "/api/system/quarantine", &token, None);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["total"], 2);
    }

    #[tokio::test]
    async fn delete_quarantine_record() {
        let (app, token, vs) = setup_test_app().await;

        // Create a quarantine record.
        let err = vs
            .validated_create("plug1", "tasks", json!({"id": "t1"}))
            .await
            .unwrap_err();
        let qid = match err {
            QuarantineError::ValidationFailed { quarantine_id, .. } => quarantine_id,
            _ => panic!("expected ValidationFailed"),
        };

        let req = auth_request("DELETE", &format!("/api/system/quarantine/{qid}"), &token, None);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn delete_nonexistent_quarantine_returns_404() {
        let (app, token, _) = setup_test_app().await;

        let req = auth_request("DELETE", "/api/system/quarantine/nope", &token, None);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn reprocess_quarantine_succeeds() {
        let (app, token, vs) = setup_test_app().await;

        // Manually insert a quarantine record with data that IS valid.
        let entry = QuarantineEntry {
            original_data: json!({ "id": "t1", "title": "Fixed", "status": "pending" }),
            original_collection: "tasks".to_string(),
            source_plugin_id: "plug1".to_string(),
            validation_errors: vec!["was broken".into()],
            schema_version: "Task".to_string(),
            quarantined_at: chrono::Utc::now().to_rfc3339(),
        };
        let qr = vs
            .inner()
            .create(
                "core",
                QUARANTINE_COLLECTION,
                serde_json::to_value(&entry).unwrap(),
            )
            .await
            .unwrap();

        let req = auth_request(
            "POST",
            &format!("/api/system/quarantine/{}/reprocess", qr.id),
            &token,
            None,
        );
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["data"]["collection"], "tasks");
    }

    #[tokio::test]
    async fn reprocess_still_invalid_returns_422() {
        let (app, token, vs) = setup_test_app().await;

        // Quarantine record with data that is still invalid.
        let entry = QuarantineEntry {
            original_data: json!({ "id": "t1" }),
            original_collection: "tasks".to_string(),
            source_plugin_id: "plug1".to_string(),
            validation_errors: vec!["missing title".into()],
            schema_version: "Task".to_string(),
            quarantined_at: chrono::Utc::now().to_rfc3339(),
        };
        let qr = vs
            .inner()
            .create(
                "core",
                QUARANTINE_COLLECTION,
                serde_json::to_value(&entry).unwrap(),
            )
            .await
            .unwrap();

        let req = auth_request(
            "POST",
            &format!("/api/system/quarantine/{}/reprocess", qr.id),
            &token,
            None,
        );
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let json = body_json(resp).await;
        assert_eq!(json["error"]["code"], "DATA_VALIDATION_FAILED");
        assert!(!json["error"]["details"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn reprocess_nonexistent_returns_404() {
        let (app, token, _) = setup_test_app().await;

        let req = auth_request(
            "POST",
            "/api/system/quarantine/nope/reprocess",
            &token,
            None,
        );
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn quarantine_requires_auth() {
        let (app, _, _) = setup_test_app().await;

        let req = Request::builder()
            .uri("/api/system/quarantine")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
