//! Credential CRUD API routes.
//!
//! Exposes the encrypted credential store via REST endpoints at
//! `/api/credentials`. Credential values are NEVER included in
//! list responses — only metadata (plugin_id, key, timestamps).

use crate::routes::health::AppState;

use axum::extract::{Path, Query, State};
use life_engine_plugin_sdk::credential_store::CredentialStore;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Query parameters for listing credentials.
#[derive(Debug, Deserialize)]
pub struct ListCredentialsQuery {
    /// The plugin ID to list credentials for (required).
    pub plugin_id: String,
}

/// Request body for storing a credential.
#[derive(Debug, Deserialize)]
pub struct StoreCredentialRequest {
    /// The plugin ID that owns this credential.
    pub plugin_id: String,
    /// The credential key (e.g., "imap", "smtp", "caldav").
    pub key: String,
    /// The credential value (password, token, etc.) — never logged.
    pub value: String,
}

/// Credential metadata returned in responses (no secrets).
#[derive(Debug, Serialize)]
pub struct CredentialInfo {
    /// The owning plugin ID.
    pub plugin_id: String,
    /// The credential key.
    pub key: String,
}

/// POST /api/credentials — Store a credential.
pub async fn store_credential(
    State(state): State<AppState>,
    Json(body): Json<StoreCredentialRequest>,
) -> impl IntoResponse {
    let store = match &state.credential_store {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": {
                        "code": "SYSTEM_INTERNAL_ERROR",
                        "message": "credential store not available"
                    }
                })),
            )
                .into_response();
        }
    };

    if body.plugin_id.is_empty() || body.key.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "code": "DATA_VALIDATION_FAILED",
                    "message": "plugin_id and key must not be empty"
                }
            })),
        )
            .into_response();
    }

    match store.store(&body.plugin_id, &body.key, &body.value).await {
        Ok(()) => {
            state.message_bus.publish(crate::message_bus::BusEvent::CredentialEvent {
                action: "modify".to_string(),
                plugin_id: body.plugin_id.clone(),
                key: body.key.clone(),
            });
            (
                StatusCode::CREATED,
                Json(json!({
                    "plugin_id": body.plugin_id,
                    "key": body.key,
                    "message": "credential stored"
                })),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to store credential");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "SYSTEM_INTERNAL_ERROR",
                        "message": "failed to store credential"
                    }
                })),
            )
                .into_response()
        }
    }
}

/// GET /api/credentials?plugin_id={id} — List credential keys for a plugin.
pub async fn list_credentials(
    State(state): State<AppState>,
    Query(query): Query<ListCredentialsQuery>,
) -> impl IntoResponse {
    let store = match &state.credential_store {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": {
                        "code": "SYSTEM_INTERNAL_ERROR",
                        "message": "credential store not available"
                    }
                })),
            )
                .into_response();
        }
    };

    match store.list_keys(&query.plugin_id).await {
        Ok(keys) => {
            let credentials: Vec<_> = keys
                .into_iter()
                .map(|key| CredentialInfo {
                    plugin_id: query.plugin_id.clone(),
                    key,
                })
                .collect();
            let total = credentials.len();
            (
                StatusCode::OK,
                Json(json!({
                    "credentials": credentials,
                    "total": total
                })),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to list credentials");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "SYSTEM_INTERNAL_ERROR",
                        "message": "failed to list credentials"
                    }
                })),
            )
                .into_response()
        }
    }
}

/// GET /api/credentials/{plugin_id}/{key} — Retrieve a credential value.
pub async fn get_credential(
    State(state): State<AppState>,
    Path((plugin_id, key)): Path<(String, String)>,
) -> impl IntoResponse {
    let store = match &state.credential_store {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": {
                        "code": "SYSTEM_INTERNAL_ERROR",
                        "message": "credential store not available"
                    }
                })),
            )
                .into_response();
        }
    };

    match store.retrieve(&plugin_id, &key).await {
        Ok(Some(value)) => {
            tracing::info!(plugin_id = %plugin_id, key = %key, "credential retrieved");
            state.message_bus.publish(crate::message_bus::BusEvent::CredentialEvent {
                action: "access".to_string(),
                plugin_id: plugin_id.clone(),
                key: key.clone(),
            });
            (
                StatusCode::OK,
                [("cache-control", "no-store")],
                Json(json!({
                    "plugin_id": plugin_id,
                    "key": key,
                    "value": value
                })),
            )
                .into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": {
                    "code": "CREDENTIAL_NOT_FOUND",
                    "message": format!("credential '{key}' not found for plugin '{plugin_id}'")
                }
            })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to retrieve credential");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "SYSTEM_INTERNAL_ERROR",
                        "message": "failed to retrieve credential"
                    }
                })),
            )
                .into_response()
        }
    }
}

/// DELETE /api/credentials/{plugin_id}/{key} — Delete a credential.
pub async fn delete_credential(
    State(state): State<AppState>,
    Path((plugin_id, key)): Path<(String, String)>,
) -> impl IntoResponse {
    let store = match &state.credential_store {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": {
                        "code": "SYSTEM_INTERNAL_ERROR",
                        "message": "credential store not available"
                    }
                })),
            )
                .into_response();
        }
    };

    match store.delete(&plugin_id, &key).await {
        Ok(true) => {
            state.message_bus.publish(crate::message_bus::BusEvent::CredentialEvent {
                action: "delete".to_string(),
                plugin_id: plugin_id.clone(),
                key: key.clone(),
            });
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": {
                    "code": "CREDENTIAL_NOT_FOUND",
                    "message": format!("credential '{key}' not found for plugin '{plugin_id}'")
                }
            })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to delete credential");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "SYSTEM_INTERNAL_ERROR",
                        "message": "failed to delete credential"
                    }
                })),
            )
                .into_response()
        }
    }
}

/// DELETE /api/credentials/{plugin_id} — Delete all credentials for a plugin.
pub async fn delete_plugin_credentials(
    State(state): State<AppState>,
    Path(plugin_id): Path<String>,
) -> impl IntoResponse {
    let store = match &state.credential_store {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": {
                        "code": "SYSTEM_INTERNAL_ERROR",
                        "message": "credential store not available"
                    }
                })),
            )
                .into_response();
        }
    };

    match store.delete_all_for_plugin(&plugin_id).await {
        Ok(count) => {
            state.message_bus.publish(crate::message_bus::BusEvent::CredentialEvent {
                action: "delete".to_string(),
                plugin_id: plugin_id.clone(),
                key: "*".to_string(),
            });
            (
                StatusCode::OK,
                Json(json!({
                    "deleted": count,
                    "plugin_id": plugin_id
                })),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to delete plugin credentials");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "SYSTEM_INTERNAL_ERROR",
                        "message": "failed to delete plugin credentials"
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
    use crate::credential_store::SqliteCredentialStore;
    use crate::test_helpers::{auth_request, body_json, create_auth_state, generate_test_token};
    use axum::routing::{delete, get, post};
    use axum::Router;
    use life_engine_plugin_sdk::credential_store::CredentialStore;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tower::ServiceExt;

    #[tokio::test]
    async fn store_and_list_credentials() {
        let (auth_state, provider) = create_auth_state();
        let token = generate_test_token(&provider).await;

        let conn =
            rusqlite::Connection::open_in_memory().expect("in-memory SQLite should open");
        let conn = Arc::new(Mutex::new(conn));
        let store = Arc::new(
            SqliteCredentialStore::new(conn, "test-secret")
                .expect("store should create"),
        );
        store.init().await.expect("init should succeed");

        let mut state = crate::test_helpers::default_app_state();
        state.credential_store = Some(Arc::clone(&store));

        let app = Router::new()
            .route("/api/credentials", post(store_credential).get(list_credentials))
            .with_state(state)
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                crate::auth::middleware::auth_middleware,
            ));

        // Store a credential.
        let req = auth_request(
            "POST",
            "/api/credentials",
            &token,
            Some(r#"{"plugin_id":"com.test","key":"imap","value":"secret123"}"#.into()),
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let json = body_json(resp).await;
        assert_eq!(json["plugin_id"], "com.test");
        assert_eq!(json["key"], "imap");

        // List credentials.
        let req = auth_request(
            "GET",
            "/api/credentials?plugin_id=com.test",
            &token,
            None,
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["total"], 1);
        assert_eq!(json["credentials"][0]["key"], "imap");
        // Value should NOT appear in list response.
        assert!(json["credentials"][0].get("value").is_none());
    }

    #[tokio::test]
    async fn get_and_delete_credential() {
        let (auth_state, provider) = create_auth_state();
        let token = generate_test_token(&provider).await;

        let conn =
            rusqlite::Connection::open_in_memory().expect("in-memory SQLite should open");
        let conn = Arc::new(Mutex::new(conn));
        let cred_store = Arc::new(
            SqliteCredentialStore::new(conn, "test-secret")
                .expect("store should create"),
        );
        cred_store.init().await.expect("init should succeed");

        // Pre-populate a credential.
        cred_store
            .store("com.test", "smtp", "password456")
            .await
            .unwrap();

        let mut state = crate::test_helpers::default_app_state();
        state.credential_store = Some(Arc::clone(&cred_store));

        let app = Router::new()
            .route(
                "/api/credentials/{plugin_id}/{key}",
                get(get_credential).delete(delete_credential),
            )
            .with_state(state)
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                crate::auth::middleware::auth_middleware,
            ));

        // Get the credential.
        let req = auth_request("GET", "/api/credentials/com.test/smtp", &token, None);
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["value"], "password456");

        // Delete it.
        let req = auth_request("DELETE", "/api/credentials/com.test/smtp", &token, None);
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);

        // Verify it's gone.
        let req = auth_request("GET", "/api/credentials/com.test/smtp", &token, None);
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn store_rejects_empty_fields() {
        let (auth_state, provider) = create_auth_state();
        let token = generate_test_token(&provider).await;

        let conn =
            rusqlite::Connection::open_in_memory().expect("in-memory SQLite should open");
        let conn = Arc::new(Mutex::new(conn));
        let cred_store = Arc::new(
            SqliteCredentialStore::new(conn, "test-secret")
                .expect("store should create"),
        );
        cred_store.init().await.expect("init should succeed");

        let mut state = crate::test_helpers::default_app_state();
        state.credential_store = Some(Arc::clone(&cred_store));

        let app = Router::new()
            .route("/api/credentials", post(store_credential))
            .with_state(state)
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                crate::auth::middleware::auth_middleware,
            ));

        let req = auth_request(
            "POST",
            "/api/credentials",
            &token,
            Some(r#"{"plugin_id":"","key":"imap","value":"secret"}"#.into()),
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        let json = body_json(resp).await;
        assert_eq!(json["error"]["code"], "DATA_VALIDATION_FAILED");
    }

    #[tokio::test]
    async fn credential_not_found_returns_404() {
        let (auth_state, provider) = create_auth_state();
        let token = generate_test_token(&provider).await;

        let conn =
            rusqlite::Connection::open_in_memory().expect("in-memory SQLite should open");
        let conn = Arc::new(Mutex::new(conn));
        let cred_store = Arc::new(
            SqliteCredentialStore::new(conn, "test-secret")
                .expect("store should create"),
        );
        cred_store.init().await.expect("init should succeed");

        let mut state = crate::test_helpers::default_app_state();
        state.credential_store = Some(Arc::clone(&cred_store));

        let app = Router::new()
            .route(
                "/api/credentials/{plugin_id}/{key}",
                get(get_credential),
            )
            .with_state(state)
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                crate::auth::middleware::auth_middleware,
            ));

        let req = auth_request("GET", "/api/credentials/com.test/nonexistent", &token, None);
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let json = body_json(resp).await;
        assert_eq!(json["error"]["code"], "CREDENTIAL_NOT_FOUND");
    }
}
