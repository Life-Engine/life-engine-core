//! CRUD data routes for `/api/data/{collection}`.
//!
//! Provides list, get, create, update, and delete operations against
//! the Core storage layer.

use crate::message_bus::BusEvent;
use crate::routes::health::AppState;
use crate::storage::{Pagination, QueryFilters, SortDirection, SortOptions, StorageAdapter, StorageError};

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

/// Default plugin ID for API-level data access.
const CORE_PLUGIN_ID: &str = "core";

/// Query parameters for the list endpoint.
#[derive(Debug, Deserialize)]
pub struct ListParams {
    /// Number of records to skip.
    pub offset: Option<u32>,
    /// Maximum number of records to return.
    pub limit: Option<u32>,
    /// Field to sort by.
    pub sort_by: Option<String>,
    /// Sort direction: "asc" or "desc".
    pub sort_dir: Option<String>,
    /// JSON-encoded query filters.
    pub filter: Option<String>,
}

/// GET /api/data/{collection} — List records with optional filters, sort, and pagination.
pub async fn list_records(
    State(state): State<AppState>,
    Path(collection): Path<String>,
    Query(params): Query<ListParams>,
) -> impl IntoResponse {
    let storage = match &state.storage {
        Some(s) => s,
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

    let sort = params.sort_by.map(|sort_by| {
        let sort_dir = match params.sort_dir.as_deref() {
            Some("desc") => SortDirection::Desc,
            _ => SortDirection::Asc,
        };
        SortOptions { sort_by, sort_dir }
    });

    let filters = if let Some(filter_str) = &params.filter {
        match serde_json::from_str::<QueryFilters>(filter_str) {
            Ok(f) => f,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": { "code": "DATA_VALIDATION_FAILED", "message": format!("invalid filter: {e}") }
                    })),
                )
                    .into_response();
            }
        }
    } else {
        QueryFilters::default()
    };

    let has_filters = !filters.equality.is_empty()
        || !filters.comparison.is_empty()
        || !filters.text_search.is_empty()
        || !filters.and.is_empty()
        || !filters.or.is_empty();

    let result = if has_filters {
        storage
            .query(CORE_PLUGIN_ID, &collection, filters, sort, pagination)
            .await
    } else {
        storage
            .list(CORE_PLUGIN_ID, &collection, sort, pagination)
            .await
    };

    match result {
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
            tracing::error!(error = %e, "list records failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": { "code": "SYSTEM_INTERNAL_ERROR", "message": "failed to list records" }
                })),
            )
                .into_response()
        }
    }
}

/// GET /api/data/{collection}/{id} — Get a single record by ID.
pub async fn get_record(
    State(state): State<AppState>,
    Path((collection, id)): Path<(String, String)>,
) -> impl IntoResponse {
    let storage = match &state.storage {
        Some(s) => s,
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

    match storage.get(CORE_PLUGIN_ID, &collection, &id).await {
        Ok(Some(record)) => (StatusCode::OK, Json(json!({ "data": record }))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": { "code": "DATA_NOT_FOUND", "message": format!("record '{id}' not found in '{collection}'") }
            })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "get record failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": { "code": "SYSTEM_INTERNAL_ERROR", "message": "failed to get record" }
                })),
            )
                .into_response()
        }
    }
}

/// POST /api/data/{collection} — Create a new record.
///
/// When the request includes an authenticated user identity with a
/// `user_id`, the record is automatically tagged with `user_id` and
/// `household_id` for multi-user data isolation.
pub async fn create_record(
    State(state): State<AppState>,
    Path(collection): Path<String>,
    identity: Option<axum::Extension<crate::auth::types::AuthIdentity>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let storage = match &state.storage {
        Some(s) => s,
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

    // Strip any client-supplied _user_id / _household_id to prevent
    // identity spoofing, then inject from the authenticated identity.
    let mut record_data = body;
    if let Some(obj) = record_data.as_object_mut() {
        obj.remove("_user_id");
        obj.remove("_household_id");
    }
    if let Some(axum::Extension(ref id)) = identity
        && let Some(ref uid) = id.user_id
        && let Some(obj) = record_data.as_object_mut()
    {
        obj.insert("_user_id".to_string(), json!(uid));
        if let Some(ref hid) = id.household_id {
            obj.insert("_household_id".to_string(), json!(hid));
        }
    }

    match storage.create(CORE_PLUGIN_ID, &collection, record_data).await {
        Ok(record) => {
            // Publish bus events for SSE subscribers and search processor.
            state.message_bus.publish(BusEvent::NewRecords {
                collection: collection.clone(),
                count: 1,
            });
            state.message_bus.publish(BusEvent::RecordChanged {
                record: record.clone(),
            });

            (StatusCode::CREATED, Json(json!({ "data": record }))).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "create record failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": { "code": "SYSTEM_INTERNAL_ERROR", "message": "failed to create record" }
                })),
            )
                .into_response()
        }
    }
}

/// Request body for updating a record.
#[derive(Debug, Deserialize)]
pub struct UpdateBody {
    /// The updated data payload.
    pub data: Value,
    /// The expected version for optimistic concurrency.
    pub version: i64,
}

/// PUT /api/data/{collection}/{id} — Update an existing record.
pub async fn update_record(
    State(state): State<AppState>,
    Path((collection, id)): Path<(String, String)>,
    Json(body): Json<UpdateBody>,
) -> impl IntoResponse {
    let storage = match &state.storage {
        Some(s) => s,
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

    // Strip client-supplied _user_id / _household_id to prevent identity spoofing.
    let mut update_data = body.data;
    if let Some(obj) = update_data.as_object_mut() {
        obj.remove("_user_id");
        obj.remove("_household_id");
    }

    match storage
        .update(CORE_PLUGIN_ID, &collection, &id, update_data, body.version)
        .await
    {
        Ok(record) => {
            // Publish bus event for SSE subscribers.
            state.message_bus.publish(BusEvent::NewRecords {
                collection: collection.clone(),
                count: 1,
            });

            // Notify search processor via bus event.
            state.message_bus.publish(BusEvent::RecordChanged {
                record: record.clone(),
            });

            (StatusCode::OK, Json(json!({ "data": record }))).into_response()
        }
        Err(StorageError::VersionMismatch) => {
            (
                StatusCode::CONFLICT,
                Json(json!({
                    "error": { "code": "DATA_VERSION_CONFLICT", "message": "version conflict: record was modified by another request" }
                })),
            )
                .into_response()
        }
        Err(StorageError::NotFound) => {
            (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": { "code": "DATA_NOT_FOUND", "message": format!("record '{id}' not found in '{collection}'") }
                })),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "update record failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": { "code": "SYSTEM_INTERNAL_ERROR", "message": "failed to update record" }
                })),
            )
                .into_response()
        }
    }
}

/// DELETE /api/data/{collection}/{id} — Delete a record by ID.
pub async fn delete_record(
    State(state): State<AppState>,
    Path((collection, id)): Path<(String, String)>,
) -> impl IntoResponse {
    let storage = match &state.storage {
        Some(s) => s,
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

    match storage.delete(CORE_PLUGIN_ID, &collection, &id).await {
        Ok(true) => {
            // Notify search processor via bus event.
            state.message_bus.publish(BusEvent::RecordDeleted {
                record_id: id.clone(),
                collection: collection.clone(),
            });

            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": { "code": "DATA_NOT_FOUND", "message": format!("record '{id}' not found in '{collection}'") }
            })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "delete record failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": { "code": "SYSTEM_INTERNAL_ERROR", "message": "failed to delete record" }
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
    use crate::sqlite_storage::SqliteStorage;
    use crate::test_helpers::{
        auth_request, body_json, create_auth_state, default_app_state, generate_test_token,
    };
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use axum::Router;
    use std::sync::Arc;
    use tower::ServiceExt;

    async fn setup_test_app() -> (Router, String) {
        let storage = Arc::new(SqliteStorage::open_in_memory().unwrap());
        let (auth_state, provider) = create_auth_state();

        let mut state = default_app_state();
        state.storage = Some(Arc::clone(&storage));

        let app = Router::new()
            .route(
                "/api/data/{collection}",
                get(list_records).post(create_record),
            )
            .route(
                "/api/data/{collection}/{id}",
                get(get_record).put(update_record).delete(delete_record),
            )
            .with_state(state)
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                auth_middleware,
            ));

        let token = generate_test_token(&provider).await;

        (app, token)
    }

    #[tokio::test]
    async fn create_and_get_record() {
        let (app, token) = setup_test_app().await;

        // Create a record.
        let req = auth_request(
            "POST",
            "/api/data/tasks",
            &token,
            Some(json!({"title": "Test task"}).to_string()),
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let json = body_json(resp).await;
        let record = &json["data"];
        assert_eq!(record["collection"], "tasks");
        assert_eq!(record["data"]["title"], "Test task");
        assert_eq!(record["version"], 1);
        let id = record["id"].as_str().unwrap();

        // Get the record.
        let req = auth_request("GET", &format!("/api/data/tasks/{id}"), &token, None);
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["data"]["id"], id);
        assert_eq!(json["data"]["data"]["title"], "Test task");
    }

    #[tokio::test]
    async fn get_nonexistent_returns_404() {
        let (app, token) = setup_test_app().await;

        let req = auth_request("GET", "/api/data/tasks/nonexistent", &token, None);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let json = body_json(resp).await;
        assert_eq!(json["error"]["code"], "DATA_NOT_FOUND");
    }

    #[tokio::test]
    async fn list_records_with_pagination() {
        let (app, token) = setup_test_app().await;

        // Create 3 records.
        for i in 0..3 {
            let req = auth_request(
                "POST",
                "/api/data/tasks",
                &token,
                Some(json!({"index": i}).to_string()),
            );
            app.clone().oneshot(req).await.unwrap();
        }

        // List with limit=2.
        let req = auth_request(
            "GET",
            "/api/data/tasks?limit=2&offset=0",
            &token,
            None,
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["total"], 3);
        assert_eq!(json["limit"], 2);
        assert_eq!(json["offset"], 0);
        assert_eq!(json["data"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn update_record_with_version() {
        let (app, token) = setup_test_app().await;

        // Create.
        let req = auth_request(
            "POST",
            "/api/data/tasks",
            &token,
            Some(json!({"title": "Original"}).to_string()),
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        let json = body_json(resp).await;
        let id = json["data"]["id"].as_str().unwrap().to_string();

        // Update.
        let req = auth_request(
            "PUT",
            &format!("/api/data/tasks/{id}"),
            &token,
            Some(json!({"data": {"title": "Updated"}, "version": 1}).to_string()),
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["data"]["data"]["title"], "Updated");
        assert_eq!(json["data"]["version"], 2);
    }

    #[tokio::test]
    async fn update_with_wrong_version_returns_409() {
        let (app, token) = setup_test_app().await;

        // Create.
        let req = auth_request(
            "POST",
            "/api/data/tasks",
            &token,
            Some(json!({"title": "Original"}).to_string()),
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        let json = body_json(resp).await;
        let id = json["data"]["id"].as_str().unwrap().to_string();

        // Update with wrong version.
        let req = auth_request(
            "PUT",
            &format!("/api/data/tasks/{id}"),
            &token,
            Some(json!({"data": {"title": "Conflict"}, "version": 99}).to_string()),
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CONFLICT);

        let json = body_json(resp).await;
        assert_eq!(json["error"]["code"], "DATA_VERSION_CONFLICT");
    }

    #[tokio::test]
    async fn delete_record_returns_204() {
        let (app, token) = setup_test_app().await;

        // Create.
        let req = auth_request(
            "POST",
            "/api/data/tasks",
            &token,
            Some(json!({"title": "To delete"}).to_string()),
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        let json = body_json(resp).await;
        let id = json["data"]["id"].as_str().unwrap().to_string();

        // Delete.
        let req = auth_request("DELETE", &format!("/api/data/tasks/{id}"), &token, None);
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);

        // Verify not found.
        let req = auth_request("GET", &format!("/api/data/tasks/{id}"), &token, None);
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn delete_nonexistent_returns_404() {
        let (app, token) = setup_test_app().await;

        let req = auth_request("DELETE", "/api/data/tasks/nonexistent", &token, None);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let json = body_json(resp).await;
        assert_eq!(json["error"]["code"], "DATA_NOT_FOUND");
    }

    #[tokio::test]
    async fn data_routes_require_auth() {
        let (app, _token) = setup_test_app().await;

        let req = Request::builder()
            .uri("/api/data/tasks")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn list_empty_collection() {
        let (app, token) = setup_test_app().await;

        let req = auth_request("GET", "/api/data/empty", &token, None);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["total"], 0);
        assert_eq!(json["data"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn list_with_sort() {
        let (app, token) = setup_test_app().await;

        for title in ["Banana", "Apple", "Cherry"] {
            let req = auth_request(
                "POST",
                "/api/data/tasks",
                &token,
                Some(json!({"title": title}).to_string()),
            );
            app.clone().oneshot(req).await.unwrap();
        }

        let req = auth_request(
            "GET",
            "/api/data/tasks?sort_by=created_at&sort_dir=desc",
            &token,
            None,
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["total"], 3);
    }

    #[tokio::test]
    async fn list_with_filter_query_param() {
        let (app, token) = setup_test_app().await;

        // Create records.
        let req = auth_request(
            "POST",
            "/api/data/tasks",
            &token,
            Some(json!({"title": "Searchable item"}).to_string()),
        );
        app.clone().oneshot(req).await.unwrap();

        let req = auth_request(
            "POST",
            "/api/data/tasks",
            &token,
            Some(json!({"title": "Other"}).to_string()),
        );
        app.clone().oneshot(req).await.unwrap();

        // Filter with text search.
        let filter = serde_json::to_string(&json!({
            "text_search": [{"field": "title", "contains": "Searchable"}]
        }))
        .unwrap();
        let encoded = urlencoding::encode(&filter);
        let req = auth_request(
            "GET",
            &format!("/api/data/tasks?filter={encoded}"),
            &token,
            None,
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["total"], 1);
    }

    #[tokio::test]
    async fn invalid_filter_returns_400() {
        let (app, token) = setup_test_app().await;

        let req = auth_request(
            "GET",
            "/api/data/tasks?filter=not-valid-json",
            &token,
            None,
        );
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        let json = body_json(resp).await;
        assert_eq!(json["error"]["code"], "DATA_VALIDATION_FAILED");
    }

    async fn setup_test_app_with_search() -> (Router, String, Arc<crate::search::SearchEngine>) {
        let storage = Arc::new(SqliteStorage::open_in_memory().unwrap());
        let (auth_state, provider) = create_auth_state();
        let search_engine = Arc::new(crate::search::SearchEngine::new().unwrap());
        let bus = Arc::new(crate::message_bus::MessageBus::new());

        // Spawn the search processor so bus events trigger indexing.
        crate::search_processor::spawn(&bus, Arc::clone(&search_engine));

        let mut state = default_app_state();
        state.storage = Some(Arc::clone(&storage));
        state.search_engine = Some(Arc::clone(&search_engine));
        state.message_bus = Arc::clone(&bus);

        let app = Router::new()
            .route(
                "/api/data/{collection}",
                get(list_records).post(create_record),
            )
            .route(
                "/api/data/{collection}/{id}",
                get(get_record).put(update_record).delete(delete_record),
            )
            .with_state(state)
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                auth_middleware,
            ));

        let token = generate_test_token(&provider).await;

        (app, token, search_engine)
    }

    #[tokio::test]
    async fn created_record_is_indexed_in_search() {
        let (app, token, engine) = setup_test_app_with_search().await;

        let req = auth_request(
            "POST",
            "/api/data/tasks",
            &token,
            Some(json!({"title": "Searchable unique item"}).to_string()),
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        // Allow the spawned indexing task to complete.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let results = engine.search("searchable", None, 10, 0).unwrap();
        assert_eq!(results.total, 1);
        assert_eq!(results.hits[0].collection, "tasks");
    }

    #[tokio::test]
    async fn updated_record_is_reindexed_in_search() {
        let (app, token, engine) = setup_test_app_with_search().await;

        // Create a record.
        let req = auth_request(
            "POST",
            "/api/data/tasks",
            &token,
            Some(json!({"title": "OriginalUniqueWord"}).to_string()),
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        let json = body_json(resp).await;
        let id = json["data"]["id"].as_str().unwrap().to_string();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Update the record.
        let req = auth_request(
            "PUT",
            &format!("/api/data/tasks/{id}"),
            &token,
            Some(json!({"data": {"title": "UpdatedUniqueWord"}, "version": 1}).to_string()),
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Old term should be gone.
        let results = engine.search("OriginalUniqueWord", None, 10, 0).unwrap();
        assert_eq!(results.total, 0);

        // New term should be findable.
        let results = engine.search("UpdatedUniqueWord", None, 10, 0).unwrap();
        assert_eq!(results.total, 1);
    }

    #[tokio::test]
    async fn deleted_record_is_removed_from_search() {
        let (app, token, engine) = setup_test_app_with_search().await;

        // Create.
        let req = auth_request(
            "POST",
            "/api/data/tasks",
            &token,
            Some(json!({"title": "DeletableUniqueItem"}).to_string()),
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        let json = body_json(resp).await;
        let id = json["data"]["id"].as_str().unwrap().to_string();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let results = engine.search("DeletableUniqueItem", None, 10, 0).unwrap();
        assert_eq!(results.total, 1);

        // Delete.
        let req = auth_request("DELETE", &format!("/api/data/tasks/{id}"), &token, None);
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let results = engine.search("DeletableUniqueItem", None, 10, 0).unwrap();
        assert_eq!(results.total, 0);
    }

    #[tokio::test]
    async fn create_publishes_bus_event() {
        let storage = Arc::new(SqliteStorage::open_in_memory().unwrap());
        let (auth_state, provider) = create_auth_state();
        let bus = Arc::new(crate::message_bus::MessageBus::new());
        let mut rx = bus.subscribe();

        let mut state = default_app_state();
        state.storage = Some(Arc::clone(&storage));
        state.message_bus = Arc::clone(&bus);

        let app = Router::new()
            .route(
                "/api/data/{collection}",
                get(list_records).post(create_record),
            )
            .with_state(state)
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                auth_middleware,
            ));

        let token = generate_test_token(&provider).await;

        let req = auth_request(
            "POST",
            "/api/data/tasks",
            &token,
            Some(json!({"title": "Bus event test"}).to_string()),
        );
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let event = rx.recv().await.expect("should receive bus event");
        match event {
            crate::message_bus::BusEvent::NewRecords { collection, count } => {
                assert_eq!(collection, "tasks");
                assert_eq!(count, 1);
            }
            _ => panic!("unexpected event variant"),
        }
    }

    #[tokio::test]
    async fn update_publishes_bus_event() {
        let storage = Arc::new(SqliteStorage::open_in_memory().unwrap());
        let (auth_state, provider) = create_auth_state();
        let bus = Arc::new(crate::message_bus::MessageBus::new());
        let mut rx = bus.subscribe();

        let mut state = default_app_state();
        state.storage = Some(Arc::clone(&storage));
        state.message_bus = Arc::clone(&bus);

        let app = Router::new()
            .route(
                "/api/data/{collection}",
                get(list_records).post(create_record),
            )
            .route(
                "/api/data/{collection}/{id}",
                get(get_record).put(update_record).delete(delete_record),
            )
            .with_state(state)
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                auth_middleware,
            ));

        let token = generate_test_token(&provider).await;

        // Create a record first.
        let req = auth_request(
            "POST",
            "/api/data/tasks",
            &token,
            Some(json!({"title": "Before update"}).to_string()),
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let json = body_json(resp).await;
        let id = json["data"]["id"].as_str().unwrap().to_string();

        // Drain the create events (NewRecords + RecordChanged).
        let _create_event1 = rx.recv().await.expect("should receive create bus event 1");
        let _create_event2 = rx.recv().await.expect("should receive create bus event 2");

        // Update the record.
        let req = auth_request(
            "PUT",
            &format!("/api/data/tasks/{id}"),
            &token,
            Some(json!({"data": {"title": "After update"}, "version": 1}).to_string()),
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let event = rx.recv().await.expect("should receive update bus event");
        match event {
            crate::message_bus::BusEvent::NewRecords { collection, count } => {
                assert_eq!(collection, "tasks");
                assert_eq!(count, 1);
            }
            _ => panic!("unexpected event variant"),
        }
    }

    #[tokio::test]
    async fn search_unavailable_does_not_cause_api_errors() {
        // When search_engine is None, all CRUD operations should still succeed.
        let (app, token) = setup_test_app().await;

        // Create succeeds without search engine.
        let req = auth_request(
            "POST",
            "/api/data/tasks",
            &token,
            Some(json!({"title": "No search engine"}).to_string()),
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let json = body_json(resp).await;
        let id = json["data"]["id"].as_str().unwrap().to_string();

        // Update succeeds without search engine.
        let req = auth_request(
            "PUT",
            &format!("/api/data/tasks/{id}"),
            &token,
            Some(json!({"data": {"title": "Updated no search"}, "version": 1}).to_string()),
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Delete succeeds without search engine.
        let req = auth_request("DELETE", &format!("/api/data/tasks/{id}"), &token, None);
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    }
}
