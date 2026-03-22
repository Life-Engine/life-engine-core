//! Conflict resolution API routes.
//!
//! Provides endpoints for listing, inspecting, resolving, and dismissing
//! sync conflicts detected by the conflict resolution engine.

use crate::conflict::ConflictResolution;
use crate::routes::health::AppState;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

/// Query parameters for the list conflicts endpoint.
#[derive(Debug, Deserialize)]
pub struct ListConflictsParams {
    /// Maximum number of conflicts to return (default 50).
    pub limit: Option<usize>,
    /// Number of conflicts to skip (default 0).
    pub offset: Option<usize>,
}

/// Request body for resolving a conflict.
#[derive(Debug, Deserialize)]
pub struct ResolveBody {
    /// The resolution type: "keep_local", "keep_remote", or "merge".
    pub resolution: String,
    /// Merged data (required when resolution is "merge").
    pub merged_data: Option<Value>,
}

/// GET /api/conflicts — List unresolved conflicts with pagination.
pub async fn list_conflicts(
    State(state): State<AppState>,
    Query(params): Query<ListConflictsParams>,
) -> impl IntoResponse {
    let Some(store) = &state.conflict_store else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": { "code": "SYSTEM_INTERNAL_ERROR", "message": "conflict store not available" }
            })),
        )
            .into_response();
    };

    let limit = params.limit.unwrap_or(50).min(1000);
    let offset = params.offset.unwrap_or(0);

    let (conflicts, total) = store.list_unresolved(limit, offset);

    (
        StatusCode::OK,
        Json(json!({
            "data": conflicts,
            "total": total,
            "limit": limit,
            "offset": offset,
        })),
    )
        .into_response()
}

/// GET /api/conflicts/{id} — Get a specific conflict with both versions.
pub async fn get_conflict(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(store) = &state.conflict_store else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": { "code": "SYSTEM_INTERNAL_ERROR", "message": "conflict store not available" }
            })),
        )
            .into_response();
    };

    match store.get(&id) {
        Some(conflict) => {
            (StatusCode::OK, Json(json!({ "data": conflict }))).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": { "code": "CONFLICT_NOT_FOUND", "message": format!("conflict '{id}' not found") }
            })),
        )
            .into_response(),
    }
}

/// POST /api/conflicts/{id}/resolve — Resolve a conflict.
pub async fn resolve_conflict(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ResolveBody>,
) -> impl IntoResponse {
    let Some(store) = &state.conflict_store else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": { "code": "SYSTEM_INTERNAL_ERROR", "message": "conflict store not available" }
            })),
        )
            .into_response();
    };

    let resolution = match body.resolution.as_str() {
        "keep_local" => ConflictResolution::KeepLocal,
        "keep_remote" => ConflictResolution::KeepRemote,
        "merge" => {
            let Some(data) = body.merged_data else {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": { "code": "DATA_VALIDATION_FAILED", "message": "merged_data is required when resolution is 'merge'" }
                    })),
                )
                    .into_response();
            };
            ConflictResolution::Merged { data }
        }
        other => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": {
                        "code": "DATA_VALIDATION_FAILED",
                        "message": format!("invalid resolution '{other}': must be 'keep_local', 'keep_remote', or 'merge'")
                    }
                })),
            )
                .into_response();
        }
    };

    if store.resolve(&id, resolution) {
        let conflict = store.get(&id).expect("just resolved");
        (StatusCode::OK, Json(json!({ "data": conflict }))).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": { "code": "CONFLICT_NOT_FOUND", "message": format!("conflict '{id}' not found") }
            })),
        )
            .into_response()
    }
}

/// DELETE /api/conflicts/{id} — Dismiss/remove a conflict.
pub async fn delete_conflict(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(store) = &state.conflict_store else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": { "code": "SYSTEM_INTERNAL_ERROR", "message": "conflict store not available" }
            })),
        )
            .into_response();
    };

    if store.remove(&id) {
        StatusCode::NO_CONTENT.into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": { "code": "CONFLICT_NOT_FOUND", "message": format!("conflict '{id}' not found") }
            })),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::middleware::auth_middleware;
    use crate::conflict::{Conflict, ConflictStore, ResolutionStrategy};
    use crate::sqlite_storage::SqliteStorage;
    use crate::storage::Record;
    use crate::test_helpers::{
        auth_request, body_json, create_auth_state, default_app_state, generate_test_token,
    };
    use axum::Router;
    use chrono::Utc;
    use serde_json::json;
    use std::sync::Arc;
    use tower::ServiceExt;

    fn make_test_conflict(id: &str) -> Conflict {
        let now = Utc::now();
        Conflict {
            id: id.into(),
            collection: "tasks".into(),
            record_id: format!("r-{id}"),
            local_version: Record {
                id: format!("r-{id}"),
                plugin_id: "core".into(),
                collection: "tasks".into(),
                data: json!({"title": "Local"}),
                version: 2,
                user_id: None,
                household_id: None,
                created_at: now,
                updated_at: now,
            },
            remote_version: Record {
                id: format!("r-{id}"),
                plugin_id: "core".into(),
                collection: "tasks".into(),
                data: json!({"title": "Remote"}),
                version: 3,
                user_id: None,
                household_id: None,
                created_at: now,
                updated_at: now,
            },
            strategy: ResolutionStrategy::LastWriteWins,
            resolved: false,
            resolution: None,
            detected_at: now,
        }
    }

    async fn setup_test_app() -> (Router, String, Arc<ConflictStore>) {
        let storage = Arc::new(SqliteStorage::open_in_memory().unwrap());
        let (auth_state, provider) = create_auth_state();
        let conflict_store = Arc::new(ConflictStore::new());

        let mut state = default_app_state();
        state.storage = Some(Arc::clone(&storage));
        state.conflict_store = Some(Arc::clone(&conflict_store));

        let app = Router::new()
            .route("/api/conflicts", axum::routing::get(list_conflicts))
            .route(
                "/api/conflicts/{id}",
                axum::routing::get(get_conflict).delete(delete_conflict),
            )
            .route(
                "/api/conflicts/{id}/resolve",
                axum::routing::post(resolve_conflict),
            )
            .with_state(state)
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                auth_middleware,
            ));

        let token = generate_test_token(&provider).await;

        (app, token, conflict_store)
    }

    #[tokio::test]
    async fn list_conflicts_empty() {
        let (app, token, _store) = setup_test_app().await;

        let req = auth_request("GET", "/api/conflicts", &token, None);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["total"], 0);
        assert_eq!(json["data"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn list_conflicts_with_data() {
        let (app, token, store) = setup_test_app().await;

        store.add(make_test_conflict("c1"));
        store.add(make_test_conflict("c2"));

        let req = auth_request("GET", "/api/conflicts?limit=1&offset=0", &token, None);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["total"], 2);
        assert_eq!(json["data"].as_array().unwrap().len(), 1);
        assert_eq!(json["limit"], 1);
    }

    #[tokio::test]
    async fn get_conflict_found() {
        let (app, token, store) = setup_test_app().await;

        store.add(make_test_conflict("c1"));

        let req = auth_request("GET", "/api/conflicts/c1", &token, None);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["data"]["id"], "c1");
        assert_eq!(json["data"]["record_id"], "r-c1");
    }

    #[tokio::test]
    async fn get_conflict_not_found() {
        let (app, token, _store) = setup_test_app().await;

        let req = auth_request("GET", "/api/conflicts/nonexistent", &token, None);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let json = body_json(resp).await;
        assert_eq!(json["error"]["code"], "CONFLICT_NOT_FOUND");
    }

    #[tokio::test]
    async fn resolve_conflict_keep_local() {
        let (app, token, store) = setup_test_app().await;

        store.add(make_test_conflict("c1"));

        let req = auth_request(
            "POST",
            "/api/conflicts/c1/resolve",
            &token,
            Some(json!({"resolution": "keep_local"}).to_string()),
        );
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["data"]["resolved"], true);
    }

    #[tokio::test]
    async fn resolve_conflict_merge_requires_data() {
        let (app, token, store) = setup_test_app().await;

        store.add(make_test_conflict("c1"));

        let req = auth_request(
            "POST",
            "/api/conflicts/c1/resolve",
            &token,
            Some(json!({"resolution": "merge"}).to_string()),
        );
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        let json = body_json(resp).await;
        assert_eq!(json["error"]["code"], "DATA_VALIDATION_FAILED");
    }

    #[tokio::test]
    async fn resolve_conflict_invalid_resolution() {
        let (app, token, store) = setup_test_app().await;

        store.add(make_test_conflict("c1"));

        let req = auth_request(
            "POST",
            "/api/conflicts/c1/resolve",
            &token,
            Some(json!({"resolution": "invalid"}).to_string()),
        );
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn delete_conflict_found() {
        let (app, token, store) = setup_test_app().await;

        store.add(make_test_conflict("c1"));

        let req = auth_request("DELETE", "/api/conflicts/c1", &token, None);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);

        // Verify it's gone.
        assert!(store.get("c1").is_none());
    }

    #[tokio::test]
    async fn delete_conflict_not_found() {
        let (app, token, _store) = setup_test_app().await;

        let req = auth_request("DELETE", "/api/conflicts/nonexistent", &token, None);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
