//! Connector route handlers for triggering manual sync operations.

use crate::plugin_loader::PluginStatus;
use crate::routes::health::AppState;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use life_engine_plugin_sdk::types::HttpMethod;
use serde_json::json;

/// POST /api/connectors/{id}/sync — Trigger a manual sync for a connector.
pub async fn trigger_sync(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Acquire the lock, validate the plugin, clone the Arc, then drop
    // the lock *before* performing IO via handle_route.
    let plugin = {
        let loader = state.plugin_loader.lock().await;

        // Check if the plugin exists and is loaded.
        let plugin_info = match loader.get_plugin_info(&id) {
            Some(info) => info,
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({
                        "error": {
                            "code": "CONNECTOR_NOT_FOUND",
                            "message": format!("connector '{id}' not found")
                        }
                    })),
                )
                    .into_response();
            }
        };

        // Verify the plugin is in the Loaded state.
        if plugin_info.status != PluginStatus::Loaded {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": {
                        "code": "CONNECTOR_NOT_LOADED",
                        "message": format!("connector '{id}' is not loaded")
                    }
                })),
            )
                .into_response();
        }

        // Get an Arc handle to the plugin so we can drop the lock.
        match loader.get_plugin_arc(&id) {
            Some(p) => p,
            None => {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(json!({
                        "error": {
                            "code": "CONNECTOR_NOT_LOADED",
                            "message": format!("connector '{id}' is not loaded")
                        }
                    })),
                )
                    .into_response();
            }
        }
        // `loader` (MutexGuard) is dropped here at the end of the block.
    };

    // Check the plugin declares a POST /sync route (lock already released).
    let has_sync_route = plugin.routes().iter().any(|r| {
        r.method == HttpMethod::Post && r.path == "/sync"
    });

    if !has_sync_route {
        return (
            StatusCode::METHOD_NOT_ALLOWED,
            Json(json!({
                "error": {
                    "code": "CONNECTOR_SYNC_NOT_SUPPORTED",
                    "message": format!("connector '{id}' does not support sync")
                }
            })),
        )
            .into_response();
    }

    // Invoke the plugin's handle_route for POST /sync without holding the mutex.
    match plugin
        .handle_route(&HttpMethod::Post, "/sync", json!({}))
        .await
    {
        Ok(result) => {
            // Publish a SyncComplete event on the message bus.
            state.message_bus.publish(
                crate::message_bus::BusEvent::SyncComplete {
                    plugin_id: id.clone(),
                },
            );

            (
                StatusCode::OK,
                Json(json!({
                    "data": {
                        "connector_id": id,
                        "result": result,
                    }
                })),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(connector_id = %id, error = %e, "connector sync failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "CONNECTOR_SYNC_FAILED",
                        "message": format!("sync failed for connector '{id}'")
                    }
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
    use crate::test_helpers::{
        auth_request, body_json, create_auth_state, default_app_state, generate_test_token,
    };
    use async_trait::async_trait;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::post;
    use axum::Router;
    use life_engine_plugin_sdk::types::{Capability, CoreEvent, PluginContext, PluginRoute};
    use life_engine_plugin_sdk::{CorePlugin, Result};
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tower::ServiceExt;

    /// A connector plugin that supports POST /sync.
    struct SyncableConnector;

    #[async_trait]
    impl CorePlugin for SyncableConnector {
        fn id(&self) -> &str {
            "com.test.connector-sync"
        }
        fn display_name(&self) -> &str {
            "Syncable Connector"
        }
        fn version(&self) -> &str {
            "1.0.0"
        }
        fn capabilities(&self) -> Vec<Capability> {
            vec![Capability::StorageRead, Capability::StorageWrite]
        }
        async fn on_load(&mut self, _ctx: &PluginContext) -> Result<()> {
            Ok(())
        }
        async fn on_unload(&mut self) -> Result<()> {
            Ok(())
        }
        fn routes(&self) -> Vec<PluginRoute> {
            vec![PluginRoute {
                method: HttpMethod::Post,
                path: "/sync".into(),
            }]
        }
        async fn handle_event(&self, _event: &CoreEvent) -> Result<()> {
            Ok(())
        }
        async fn handle_route(
            &self,
            method: &HttpMethod,
            path: &str,
            _body: serde_json::Value,
        ) -> Result<serde_json::Value> {
            match (method, path) {
                (HttpMethod::Post, "/sync") => Ok(json!({
                    "status": "completed",
                    "new_records": 5,
                    "updated_records": 2,
                    "deleted_records": 1
                })),
                _ => Err(anyhow::anyhow!("unsupported route")),
            }
        }
    }

    /// A connector plugin that does NOT support /sync.
    struct NoSyncConnector;

    #[async_trait]
    impl CorePlugin for NoSyncConnector {
        fn id(&self) -> &str {
            "com.test.connector-nosync"
        }
        fn display_name(&self) -> &str {
            "No Sync Connector"
        }
        fn version(&self) -> &str {
            "1.0.0"
        }
        fn capabilities(&self) -> Vec<Capability> {
            vec![]
        }
        async fn on_load(&mut self, _ctx: &PluginContext) -> Result<()> {
            Ok(())
        }
        async fn on_unload(&mut self) -> Result<()> {
            Ok(())
        }
        fn routes(&self) -> Vec<PluginRoute> {
            vec![PluginRoute {
                method: HttpMethod::Get,
                path: "/status".into(),
            }]
        }
        async fn handle_event(&self, _event: &CoreEvent) -> Result<()> {
            Ok(())
        }
    }

    /// A connector plugin whose handle_route always fails.
    struct FailingSyncConnector;

    #[async_trait]
    impl CorePlugin for FailingSyncConnector {
        fn id(&self) -> &str {
            "com.test.connector-failing"
        }
        fn display_name(&self) -> &str {
            "Failing Connector"
        }
        fn version(&self) -> &str {
            "1.0.0"
        }
        fn capabilities(&self) -> Vec<Capability> {
            vec![]
        }
        async fn on_load(&mut self, _ctx: &PluginContext) -> Result<()> {
            Ok(())
        }
        async fn on_unload(&mut self) -> Result<()> {
            Ok(())
        }
        fn routes(&self) -> Vec<PluginRoute> {
            vec![PluginRoute {
                method: HttpMethod::Post,
                path: "/sync".into(),
            }]
        }
        async fn handle_event(&self, _event: &CoreEvent) -> Result<()> {
            Ok(())
        }
        async fn handle_route(
            &self,
            _method: &HttpMethod,
            _path: &str,
            _body: serde_json::Value,
        ) -> Result<serde_json::Value> {
            Err(anyhow::anyhow!("connection refused"))
        }
    }

    async fn setup_test_app_with_plugins(
        plugins: Vec<Box<dyn CorePlugin>>,
    ) -> (Router, String) {
        let mut loader = crate::plugin_loader::PluginLoader::new();
        for plugin in plugins {
            loader.register(plugin).unwrap();
        }
        loader.load_all().await;

        let (auth_state, provider) = create_auth_state();
        let mut state = default_app_state();
        state.plugin_loader = Arc::new(Mutex::new(loader));

        let app = Router::new()
            .route("/api/connectors/{id}/sync", post(trigger_sync))
            .with_state(state)
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                auth_middleware,
            ));

        let token = generate_test_token(&provider).await;
        (app, token)
    }

    #[tokio::test]
    async fn trigger_sync_returns_200_on_success() {
        let (app, token) =
            setup_test_app_with_plugins(vec![Box::new(SyncableConnector)]).await;

        let req = auth_request(
            "POST",
            "/api/connectors/com.test.connector-sync/sync",
            &token,
            None,
        );
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["data"]["connector_id"], "com.test.connector-sync");
        assert_eq!(json["data"]["result"]["status"], "completed");
        assert_eq!(json["data"]["result"]["new_records"], 5);
        assert_eq!(json["data"]["result"]["updated_records"], 2);
        assert_eq!(json["data"]["result"]["deleted_records"], 1);
    }

    #[tokio::test]
    async fn trigger_sync_returns_404_for_unknown_connector() {
        let (app, token) = setup_test_app_with_plugins(vec![]).await;

        let req = auth_request(
            "POST",
            "/api/connectors/com.test.nonexistent/sync",
            &token,
            None,
        );
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let json = body_json(resp).await;
        assert_eq!(json["error"]["code"], "CONNECTOR_NOT_FOUND");
    }

    #[tokio::test]
    async fn trigger_sync_returns_401_without_auth() {
        let (app, _token) =
            setup_test_app_with_plugins(vec![Box::new(SyncableConnector)]).await;

        let req = Request::builder()
            .method("POST")
            .uri("/api/connectors/com.test.connector-sync/sync")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn trigger_sync_returns_405_when_no_sync_route() {
        let (app, token) =
            setup_test_app_with_plugins(vec![Box::new(NoSyncConnector)]).await;

        let req = auth_request(
            "POST",
            "/api/connectors/com.test.connector-nosync/sync",
            &token,
            None,
        );
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);

        let json = body_json(resp).await;
        assert_eq!(json["error"]["code"], "CONNECTOR_SYNC_NOT_SUPPORTED");
    }

    #[tokio::test]
    async fn trigger_sync_returns_500_when_sync_fails() {
        let (app, token) =
            setup_test_app_with_plugins(vec![Box::new(FailingSyncConnector)]).await;

        let req = auth_request(
            "POST",
            "/api/connectors/com.test.connector-failing/sync",
            &token,
            None,
        );
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let json = body_json(resp).await;
        assert_eq!(json["error"]["code"], "CONNECTOR_SYNC_FAILED");
        let msg = json["error"]["message"].as_str().unwrap();
        assert!(msg.contains("sync failed"));
        // Internal error details must not leak to the client.
        assert!(!msg.contains("connection refused"));
    }

    #[tokio::test]
    async fn trigger_sync_json_structure_has_data_wrapper() {
        let (app, token) =
            setup_test_app_with_plugins(vec![Box::new(SyncableConnector)]).await;

        let req = auth_request(
            "POST",
            "/api/connectors/com.test.connector-sync/sync",
            &token,
            None,
        );
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_json(resp).await;
        // Top-level should have "data" key with connector_id and result.
        assert!(json.get("data").is_some(), "response must have 'data' key");
        assert!(
            json["data"].get("connector_id").is_some(),
            "data must have 'connector_id'"
        );
        assert!(
            json["data"].get("result").is_some(),
            "data must have 'result'"
        );
    }

    #[tokio::test]
    async fn trigger_sync_error_json_structure() {
        let (app, token) = setup_test_app_with_plugins(vec![]).await;

        let req = auth_request(
            "POST",
            "/api/connectors/com.test.missing/sync",
            &token,
            None,
        );
        let resp = app.oneshot(req).await.unwrap();

        let json = body_json(resp).await;
        // Error responses should have "error" key with "code" and "message".
        assert!(json.get("error").is_some(), "response must have 'error' key");
        assert!(
            json["error"].get("code").is_some(),
            "error must have 'code'"
        );
        assert!(
            json["error"].get("message").is_some(),
            "error must have 'message'"
        );
    }
}
