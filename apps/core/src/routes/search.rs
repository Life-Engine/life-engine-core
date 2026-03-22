//! Search API endpoint for full-text search across records.

use crate::routes::health::AppState;
use crate::search::SearchEngine;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use serde_json::json;

/// Query parameters for the search endpoint.
#[derive(Debug, Deserialize)]
pub struct SearchParams {
    /// The search query string (required).
    pub q: Option<String>,
    /// Optional collection filter.
    pub collection: Option<String>,
    /// Maximum number of results (default 20, max 100).
    pub limit: Option<usize>,
    /// Number of results to skip.
    pub offset: Option<usize>,
}

/// GET /api/search — Full-text search across records.
pub async fn search(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let engine: &SearchEngine = match &state.search_engine {
        Some(e) => e,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": { "code": "SEARCH_UNAVAILABLE", "message": "search engine not initialised" }
                })),
            )
                .into_response();
        }
    };

    let query = match &params.q {
        Some(q) if !q.trim().is_empty() => q.as_str(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": { "code": "SEARCH_QUERY_EMPTY", "message": "query parameter 'q' is required and must not be empty" }
                })),
            )
                .into_response();
        }
    };

    let limit = params.limit.unwrap_or(20).min(100);
    let offset = params.offset.unwrap_or(0);
    let collection_filter = params.collection.as_deref();

    match engine.search(query, collection_filter, limit, offset) {
        Ok(results) => (StatusCode::OK, Json(json!(results))).into_response(),
        Err(e) => {
            tracing::error!(error = %e, query = %query, "search failed");
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": { "code": "SEARCH_QUERY_INVALID", "message": format!("invalid search query: {e}") }
                })),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::SearchEngine;
    use crate::storage::Record;
    use crate::test_helpers::{body_json, default_app_state};
    use axum::body::Body;
    use axum::http::Request;
    use axum::Router;
    use serde_json::Value;
    use std::sync::Arc;
    use tower::ServiceExt;

    async fn setup_search_app() -> (Router, Arc<SearchEngine>) {
        let engine = Arc::new(SearchEngine::new().unwrap());

        let mut state = default_app_state();
        state.search_engine = Some(Arc::clone(&engine));

        let app = Router::new()
            .route("/api/search", axum::routing::get(search))
            .with_state(state);

        (app, engine)
    }

    fn make_record(id: &str, collection: &str, data: Value) -> Record {
        let now = chrono::Utc::now();
        Record {
            id: id.to_string(),
            plugin_id: "core".to_string(),
            collection: collection.to_string(),
            data,
            version: 1,
            user_id: None,
            household_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn search_returns_results() {
        let (app, engine) = setup_search_app().await;
        let record = make_record("r1", "tasks", json!({"title": "Deploy service"}));
        engine.index_record(&record).await.unwrap();

        let req = Request::builder()
            .uri("/api/search?q=deploy")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["total"], 1);
        assert_eq!(json["hits"][0]["id"], "r1");
    }

    #[tokio::test]
    async fn search_missing_query_returns_400() {
        let (app, _) = setup_search_app().await;

        let req = Request::builder()
            .uri("/api/search")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        let json = body_json(resp).await;
        assert_eq!(json["error"]["code"], "SEARCH_QUERY_EMPTY");
    }

    #[tokio::test]
    async fn search_empty_query_returns_400() {
        let (app, _) = setup_search_app().await;

        let req = Request::builder()
            .uri("/api/search?q=")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn search_with_collection_filter() {
        let (app, engine) = setup_search_app().await;
        engine.index_record(&make_record("r1", "tasks", json!({"title": "Important task"}))).await.unwrap();
        engine.index_record(&make_record("r2", "notes", json!({"title": "Important note", "body": ""}))).await.unwrap();

        let req = Request::builder()
            .uri("/api/search?q=important&collection=tasks")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["total"], 1);
        assert_eq!(json["hits"][0]["collection"], "tasks");
    }

    #[tokio::test]
    async fn search_with_pagination() {
        let (app, engine) = setup_search_app().await;
        for i in 0..5 {
            engine.index_record(&make_record(
                &format!("r{i}"),
                "tasks",
                json!({"title": format!("Gamma item {i}")}),
            )).await.unwrap();
        }

        let req = Request::builder()
            .uri("/api/search?q=gamma&limit=2&offset=0")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        let json = body_json(resp).await;
        assert_eq!(json["hits"].as_array().unwrap().len(), 2);
        assert_eq!(json["total"], 5);
    }

    #[tokio::test]
    async fn search_unavailable_without_engine() {
        let state = default_app_state();

        let app = Router::new()
            .route("/api/search", axum::routing::get(search))
            .with_state(state);

        let req = Request::builder()
            .uri("/api/search?q=test")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
