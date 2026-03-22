//! Shared test helper functions for the Core crate.
//!
//! Eliminates duplication of auth setup, request building, and response
//! parsing across route and middleware test modules.

use crate::auth::local_token::LocalTokenProvider;
use crate::auth::middleware::{AuthMiddlewareState, RateLimiter};
use crate::auth::types::TokenRequest;
use crate::auth::AuthProvider;
use crate::message_bus::MessageBus;
use crate::plugin_loader::PluginLoader;
use crate::routes::health::AppState;
use axum::body::Body;
use axum::http::Request;
use http_body_util::BodyExt;
use serde_json::Value;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

/// Create an `AuthMiddlewareState` and its backing `LocalTokenProvider`.
///
/// Returns both so callers can generate tokens from the provider.
pub fn create_auth_state() -> (AuthMiddlewareState, Arc<LocalTokenProvider>) {
    let provider = Arc::new(LocalTokenProvider::new());
    let rate_limiter = RateLimiter::new();
    let auth_state = AuthMiddlewareState {
        auth_provider: provider.clone(),
        rate_limiter,
    };
    (auth_state, provider)
}

/// Generate a bearer token string from the given provider.
pub async fn generate_test_token(provider: &Arc<LocalTokenProvider>) -> String {
    let req = TokenRequest {
        passphrase: "test".into(),
        expires_in_days: Some(30),
    };
    provider.generate_token(&req).await.unwrap().token
}

/// Build an HTTP request with `Authorization: Bearer` and `Content-Type: application/json` headers.
///
/// Pass `None` for `body` to send an empty body.
pub fn auth_request(method: &str, uri: &str, token: &str, body: Option<String>) -> Request<Body> {
    let builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json");

    match body {
        Some(b) => builder.body(Body::from(b)).unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    }
}

/// Consume an axum response and parse its body as JSON.
pub async fn body_json(response: axum::http::Response<Body>) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

/// Create a default `AppState` with all optional subsystems set to `None`.
pub fn default_app_state() -> AppState {
    AppState {
        start_time: Instant::now(),
        plugin_loader: Arc::new(Mutex::new(PluginLoader::new())),
        storage: None,
        message_bus: Arc::new(MessageBus::new()),
        conflict_store: None,
        validated_storage: None,
        search_engine: None,
        credential_store: None,
        household_store: None,
        federation_store: None,
        identity_store: None,
    }
}
