//! Tests for middleware stack: CORS, auth, logging, error handling, listener.

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::Router;
use life_engine_auth::{AuthError, AuthIdentity, AuthProvider, RateLimiter};
use tower::ServiceExt;
use uuid::Uuid;

use crate::middleware::auth::{AuthState, Identity, auth_middleware};
use crate::middleware::cors::cors_layer;
use crate::middleware::error_handler::panic_handler;
use crate::middleware::logging::logging_middleware;

// ── Mock auth provider ──────────────────────────────────────────────

struct MockAuthProvider;

#[async_trait]
impl AuthProvider for MockAuthProvider {
    async fn validate_token(&self, _token: &str) -> Result<AuthIdentity, AuthError> {
        Ok(AuthIdentity {
            user_id: "test-user".to_string(),
            provider: "pocket-id".to_string(),
            scopes: vec!["read".to_string()],
            authenticated_at: chrono::Utc::now(),
        })
    }

    async fn validate_key(&self, _key: &str) -> Result<AuthIdentity, AuthError> {
        Ok(AuthIdentity {
            user_id: "test-service".to_string(),
            provider: "api-key".to_string(),
            scopes: vec![],
            authenticated_at: chrono::Utc::now(),
        })
    }

    async fn revoke_key(&self, _key_id: Uuid) -> Result<(), AuthError> {
        Ok(())
    }
}

fn make_auth_state(public_routes: Vec<String>) -> AuthState {
    AuthState {
        provider: Arc::new(MockAuthProvider),
        rate_limiter: Arc::new(RateLimiter::new()),
        public_routes: Arc::new(public_routes.into_iter().collect::<HashSet<_>>()),
    }
}

// ── CORS tests ──────────────────────────────────────────────────────

#[tokio::test]
async fn cors_permissive_on_localhost() {
    let layer = cors_layer("127.0.0.1:3000", &[]);
    let app = Router::new()
        .route("/api/v1/health", get(|| async { "ok" }))
        .layer(layer);

    let req = Request::builder()
        .method("OPTIONS")
        .uri("/api/v1/health")
        .header("origin", "http://evil.com")
        .header("access-control-request-method", "GET")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();

    // Permissive CORS should return 200 for preflight with any origin.
    assert_eq!(resp.status(), StatusCode::OK);
    let acam = resp
        .headers()
        .get("access-control-allow-methods")
        .expect("should have allow-methods header");
    let methods = acam.to_str().unwrap();
    // Permissive mode returns either "*" (wildcard) or lists including "GET".
    assert!(
        methods.contains("GET") || methods == "*",
        "should allow GET (or wildcard), got: {methods}"
    );
}

#[tokio::test]
async fn cors_strict_on_wildcard_address() {
    let layer = cors_layer("0.0.0.0:3000", &[]);
    let app = Router::new()
        .route("/api/v1/health", get(|| async { "ok" }))
        .layer(layer);

    let req = Request::builder()
        .method("OPTIONS")
        .uri("/api/v1/health")
        .header("origin", "http://evil.com")
        .header("access-control-request-method", "POST")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    let acao = resp.headers().get("access-control-allow-origin");

    // Strict CORS: the evil origin should NOT be reflected.
    match acao {
        Some(val) => {
            let val = val.to_str().unwrap();
            assert_ne!(
                val, "http://evil.com",
                "strict CORS should not reflect arbitrary origins"
            );
        }
        None => {} // No ACAO header is also valid for strict.
    }
}

#[tokio::test]
async fn cors_explicit_origins_override_default() {
    let layer = cors_layer(
        "0.0.0.0:3000",
        &["https://my-app.example.com".to_string()],
    );
    let app = Router::new()
        .route("/api/v1/health", get(|| async { "ok" }))
        .layer(layer);

    let req = Request::builder()
        .method("OPTIONS")
        .uri("/api/v1/health")
        .header("origin", "https://my-app.example.com")
        .header("access-control-request-method", "GET")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    let acao = resp
        .headers()
        .get("access-control-allow-origin")
        .expect("should have ACAO header");
    assert_eq!(
        acao.to_str().unwrap(),
        "https://my-app.example.com",
        "explicit origin should be reflected"
    );
}

// ── Auth middleware tests ────────────────────────────────────────────

#[tokio::test]
async fn auth_rejects_missing_token_with_401() {
    let state = make_auth_state(vec![]);

    let app = Router::new()
        .route(
            "/api/v1/data/tasks",
            get(|| async { "protected" }),
        )
        .layer(axum::middleware::from_fn_with_state(
            state,
            auth_middleware,
        ));

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/data/tasks")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["code"], "AUTH_001");
}

#[tokio::test]
async fn auth_passes_identity_extension_on_valid_token() {
    let state = make_auth_state(vec![]);

    let app = Router::new()
        .route(
            "/api/v1/data/tasks",
            get(|ext: axum::Extension<Identity>| async move {
                ext.0.user_id
            }),
        )
        .layer(axum::middleware::from_fn_with_state(
            state,
            auth_middleware,
        ));

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/data/tasks")
        .header("authorization", "Bearer valid-token")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(std::str::from_utf8(&body).unwrap(), "test-user");
}

#[tokio::test]
async fn auth_bypasses_public_routes() {
    let state = make_auth_state(vec!["GET /api/v1/health".to_string()]);

    let app = Router::new()
        .route(
            "/api/v1/health",
            get(|| async { "healthy" }),
        )
        .layer(axum::middleware::from_fn_with_state(
            state,
            auth_middleware,
        ));

    // No auth header — should still succeed because route is public.
    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/health")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(std::str::from_utf8(&body).unwrap(), "healthy");
}

// ── Logging middleware test ──────────────────────────────────────────

#[tokio::test]
async fn logging_middleware_passes_through_and_preserves_status() {
    let app = Router::new()
        .route(
            "/api/v1/health",
            get(|| async { (StatusCode::OK, "ok") }),
        )
        .layer(axum::middleware::from_fn(logging_middleware));

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/health")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ── Error handler test ──────────────────────────────────────────────

#[tokio::test]
async fn panic_handler_returns_500_without_internal_details() {
    let resp = panic_handler(Box::new("test panic"));
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["code"], "INTERNAL_ERROR");
    assert_eq!(json["error"]["message"], "An unexpected error occurred");

    // Ensure no internal details leak.
    let body_str = serde_json::to_string(&json).unwrap();
    assert!(
        !body_str.contains("panic"),
        "response should not contain panic details"
    );
}
