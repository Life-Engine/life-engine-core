//! Tests for webhook transport.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tower::ServiceExt;

use crate::handlers::WebhookState;
use crate::types::{HEADER_IDEMPOTENCY_KEY, HEADER_SIGNATURE, HEADER_TIMESTAMP};
use crate::WebhookTransport;

type HmacSha256 = Hmac<Sha256>;

fn sign_payload(secret: &str, body: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(body.as_bytes());
    format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
}

fn current_timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string()
}

fn test_state(secret: Option<&str>) -> Arc<WebhookState> {
    Arc::new(WebhookState {
        secret: secret.map(|s| s.to_string()),
        seen_keys: Mutex::new(HashSet::new()),
        max_seen_keys: 100,
    })
}

fn test_router(secret: Option<&str>) -> axum::Router {
    let config_toml = toml::Value::try_from(toml::toml! {
        host = "127.0.0.1"
        port = 3001
        base_path = "/webhooks"
    })
    .unwrap();
    let transport = WebhookTransport::from_config(&config_toml).unwrap();
    transport.build_router(test_state(secret))
}

// --- Content-Type validation ---

#[tokio::test]
async fn rejects_missing_content_type() {
    let app = test_router(None);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhooks/inbound")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn rejects_wrong_content_type() {
    let app = test_router(None);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhooks/inbound")
                .header("content-type", "text/plain")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn accepts_valid_json_content_type() {
    let app = test_router(None);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhooks/inbound")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"event":"test"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

// --- HMAC signature verification ---

#[tokio::test]
async fn rejects_missing_signature_when_secret_configured() {
    let app = test_router(Some("my-secret"));

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhooks/inbound")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"event":"test"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn rejects_invalid_signature() {
    let app = test_router(Some("my-secret"));

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhooks/inbound")
                .header("content-type", "application/json")
                .header(HEADER_SIGNATURE, "sha256=invalid")
                .body(Body::from(r#"{"event":"test"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn accepts_valid_signature() {
    let secret = "my-secret";
    let body = r#"{"event":"test"}"#;
    let sig = sign_payload(secret, body);
    let app = test_router(Some(secret));

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhooks/inbound")
                .header("content-type", "application/json")
                .header(HEADER_SIGNATURE, sig)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn accepts_signature_without_prefix() {
    let secret = "my-secret";
    let body = r#"{"event":"test"}"#;
    let sig = sign_payload(secret, body);
    // Strip the sha256= prefix
    let sig_bare = sig.strip_prefix("sha256=").unwrap();
    let app = test_router(Some(secret));

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhooks/inbound")
                .header("content-type", "application/json")
                .header(HEADER_SIGNATURE, sig_bare)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

// --- Timestamp replay protection ---

#[tokio::test]
async fn rejects_expired_timestamp() {
    let app = test_router(None);
    let old_ts = "1000000"; // Very old timestamp

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhooks/inbound")
                .header("content-type", "application/json")
                .header(HEADER_TIMESTAMP, old_ts)
                .body(Body::from(r#"{"event":"test"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn accepts_recent_timestamp() {
    let app = test_router(None);
    let ts = current_timestamp();

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhooks/inbound")
                .header("content-type", "application/json")
                .header(HEADER_TIMESTAMP, ts)
                .body(Body::from(r#"{"event":"test"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn accepts_missing_timestamp() {
    let app = test_router(None);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhooks/inbound")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"event":"test"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

// --- Idempotency key deduplication ---

#[tokio::test]
async fn duplicate_idempotency_key_returns_ok() {
    let state = test_state(None);
    let config_toml = toml::Value::try_from(toml::toml! {
        host = "127.0.0.1"
        port = 3001
        base_path = "/webhooks"
    })
    .unwrap();
    let transport = WebhookTransport::from_config(&config_toml).unwrap();
    let app = transport.build_router(state.clone());

    // First request with key
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhooks/inbound")
                .header("content-type", "application/json")
                .header(HEADER_IDEMPOTENCY_KEY, "unique-key-123")
                .body(Body::from(r#"{"event":"test"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Second request with same key
    let app2 = transport.build_router(state);
    let resp2 = app2
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhooks/inbound")
                .header("content-type", "application/json")
                .header(HEADER_IDEMPOTENCY_KEY, "unique-key-123")
                .body(Body::from(r#"{"event":"test"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp2.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(resp2.into_body(), 1024 * 1024).await.unwrap();
    let text = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert!(text.contains("Duplicate"));
}

// --- Full integration: signature + timestamp + key ---

#[tokio::test]
async fn full_validated_webhook() {
    let secret = "test-secret-key";
    let body = r#"{"event":"payment.completed","amount":42}"#;
    let sig = sign_payload(secret, body);
    let ts = current_timestamp();
    let app = test_router(Some(secret));

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhooks/inbound")
                .header("content-type", "application/json")
                .header(HEADER_SIGNATURE, sig)
                .header(HEADER_TIMESTAMP, ts)
                .header(HEADER_IDEMPOTENCY_KEY, "payment-42")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let text = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert!(text.contains("accepted"));
}
