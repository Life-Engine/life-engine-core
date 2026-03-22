//! System information and plugin listing routes.

use crate::plugin_loader::PluginStatus;
use crate::routes::health::AppState;

use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

/// GET /api/system/info — Version, uptime, and system information.
pub async fn system_info(State(state): State<AppState>) -> Json<Value> {
    let uptime = state.start_time.elapsed().as_secs();
    let loader = state.plugin_loader.lock().await;

    Json(json!({
        "data": {
            "version": env!("CARGO_PKG_VERSION"),
            "plugins_loaded": loader.loaded_count(),
            "storage": "sqlite",
            "uptime_seconds": uptime,
        }
    }))
}

/// GET /api/system/plugins — List all loaded plugins.
pub async fn system_plugins(State(state): State<AppState>) -> Json<Value> {
    let loader = state.plugin_loader.lock().await;

    let plugins: Vec<Value> = loader
        .plugin_info()
        .into_iter()
        .map(|info| {
            json!({
                "id": info.id,
                "name": info.display_name,
                "version": info.version,
                "status": match info.status {
                    PluginStatus::Registered => "registered",
                    PluginStatus::Loaded => "loaded",
                    PluginStatus::Failed(_) => "failed",
                    PluginStatus::Unloaded => "unloaded",
                },
            })
        })
        .collect();

    Json(json!({ "data": plugins }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::middleware::auth_middleware;
    use crate::routes::health::health_check;
    use crate::test_helpers::{
        auth_request, body_json, create_auth_state, default_app_state, generate_test_token,
    };
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    async fn setup_test_app() -> (Router, String) {
        let (auth_state, provider) = create_auth_state();
        let state = default_app_state();

        let app = Router::new()
            .route("/api/system/health", get(health_check))
            .route("/api/system/info", get(system_info))
            .route("/api/system/plugins", get(system_plugins))
            .with_state(state)
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                auth_middleware,
            ));

        let token = generate_test_token(&provider).await;

        (app, token)
    }

    #[tokio::test]
    async fn system_info_returns_valid_data() {
        let (app, token) = setup_test_app().await;

        let req = auth_request("GET", "/api/system/info", &token, None);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["data"]["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(json["data"]["storage"], "sqlite");
        assert_eq!(json["data"]["plugins_loaded"], 0);
        assert!(json["data"]["uptime_seconds"].is_number());
    }

    #[tokio::test]
    async fn system_plugins_returns_empty_list() {
        let (app, token) = setup_test_app().await;

        let req = auth_request("GET", "/api/system/plugins", &token, None);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        let json = body_json(resp).await;
        assert!(json["data"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn system_info_requires_auth() {
        let (app, _token) = setup_test_app().await;

        let req = Request::builder()
            .uri("/api/system/info")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn system_plugins_requires_auth() {
        let (app, _token) = setup_test_app().await;

        let req = Request::builder()
            .uri("/api/system/plugins")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn health_still_skips_auth() {
        let (app, _token) = setup_test_app().await;

        let req = Request::builder()
            .uri("/api/system/health")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
    }
}
