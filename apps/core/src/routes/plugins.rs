//! Dynamic plugin route handlers.
//!
//! Plugin routes are mounted at `/api/plugins/{plugin_id}/{*path}`.
//! In Phase 1, all matched plugin routes return `501 Not Implemented`
//! with a stub JSON body.

use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// Phase 1 stub handler for plugin routes.
///
/// Returns 501 with a JSON error body indicating that the plugin route
/// handler is not yet implemented.
pub async fn plugin_route_stub(
    Path((_plugin_id, _path)): Path<(String, String)>,
) -> Response {
    let body = json!({
        "error": {
            "code": "PLUGIN_ROUTE_STUB",
            "message": "Plugin route handler not yet implemented"
        }
    });
    (StatusCode::NOT_IMPLEMENTED, axum::Json(body)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::get;
    use axum::Router;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_app() -> Router {
        Router::new().route(
            "/api/plugins/{plugin_id}/{*path}",
            get(plugin_route_stub)
                .post(plugin_route_stub)
                .put(plugin_route_stub)
                .delete(plugin_route_stub)
                .patch(plugin_route_stub),
        )
    }

    #[tokio::test]
    async fn plugin_route_returns_501() {
        let app = test_app();
        let req = axum::http::Request::builder()
            .uri("/api/plugins/com.test.plugin/items")
            .body(axum::body::Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "PLUGIN_ROUTE_STUB");
        assert_eq!(
            json["error"]["message"],
            "Plugin route handler not yet implemented"
        );
    }

    #[tokio::test]
    async fn non_existent_plugin_route_returns_404() {
        // A route that does NOT match /api/plugins/{plugin_id}/{*path}
        // should fall through to 404.
        let app = test_app();
        let req = axum::http::Request::builder()
            .uri("/api/plugins/")
            .body(axum::body::Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn completely_unrelated_route_returns_404() {
        let app = test_app();
        let req = axum::http::Request::builder()
            .uri("/api/other/endpoint")
            .body(axum::body::Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
