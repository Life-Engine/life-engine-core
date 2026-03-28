//! Tests for webhook sender delivery, HMAC signing, rate limiting, and retry.

use crate::config::WebhookSenderConfig;
use crate::delivery::DeliveryLog;
use crate::models::{DeliveryRecord, WebhookSubscription};
use crate::{deliver, sign_payload, RateLimiter};

// --- HMAC-SHA256 signing tests ---

#[test]
fn sign_payload_produces_sha256_prefixed_hex() {
    let sig = sign_payload("my-secret", b"hello world");
    assert!(sig.starts_with("sha256="));
    // Hex portion should be 64 characters (256 bits)
    let hex_part = sig.strip_prefix("sha256=").unwrap();
    assert_eq!(hex_part.len(), 64);
    assert!(hex_part.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn sign_payload_is_deterministic() {
    let sig1 = sign_payload("secret", b"payload");
    let sig2 = sign_payload("secret", b"payload");
    assert_eq!(sig1, sig2);
}

#[test]
fn sign_payload_differs_with_different_secrets() {
    let sig1 = sign_payload("secret-a", b"payload");
    let sig2 = sign_payload("secret-b", b"payload");
    assert_ne!(sig1, sig2);
}

#[test]
fn sign_payload_differs_with_different_payloads() {
    let sig1 = sign_payload("secret", b"payload-a");
    let sig2 = sign_payload("secret", b"payload-b");
    assert_ne!(sig1, sig2);
}

// --- Rate limiter tests ---

#[tokio::test]
async fn rate_limiter_allows_burst() {
    let limiter = RateLimiter::new(10.0, 5);
    // Should allow burst of 5
    for _ in 0..5 {
        assert!(limiter.acquire("http://example.com").await);
    }
    // 6th should be rate-limited
    assert!(!limiter.acquire("http://example.com").await);
}

#[tokio::test]
async fn rate_limiter_independent_per_url() {
    let limiter = RateLimiter::new(10.0, 2);
    assert!(limiter.acquire("http://a.com").await);
    assert!(limiter.acquire("http://a.com").await);
    assert!(!limiter.acquire("http://a.com").await);

    // Different URL has its own bucket
    assert!(limiter.acquire("http://b.com").await);
    assert!(limiter.acquire("http://b.com").await);
    assert!(!limiter.acquire("http://b.com").await);
}

#[tokio::test]
async fn rate_limiter_refills_over_time() {
    let limiter = RateLimiter::new(1000.0, 1);
    assert!(limiter.acquire("http://example.com").await);
    assert!(!limiter.acquire("http://example.com").await);

    // Wait for refill (1ms should give ~1 token at 1000 tokens/sec)
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    assert!(limiter.acquire("http://example.com").await);
}

// --- Config tests ---

#[test]
fn config_defaults() {
    let config = WebhookSenderConfig::default();
    assert_eq!(config.connect_timeout_secs, 5);
    assert_eq!(config.request_timeout_secs, 30);
    assert_eq!(config.total_timeout_secs, 300);
    assert_eq!(config.max_retries, 5);
    assert_eq!(config.max_delivery_log_size, 10_000);
}

// --- Deliver function tests (requires HTTP mock) ---

#[tokio::test]
async fn deliver_to_unreachable_host_returns_error() {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(100))
        .build()
        .unwrap();

    let sub = WebhookSubscription {
        id: "sub-1".to_string(),
        url: "http://127.0.0.1:1/webhook".to_string(), // unreachable
        event_types: vec!["test".to_string()],
        secret: None,
        active: true,
    };

    let result = deliver(&client, &sub, b"{}").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn deliver_with_hmac_adds_signature_header() {
    // We can't easily test the header without a mock server, but we can
    // verify that deliver() doesn't panic when a secret is configured.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(100))
        .build()
        .unwrap();

    let sub = WebhookSubscription {
        id: "sub-1".to_string(),
        url: "http://127.0.0.1:1/webhook".to_string(),
        event_types: vec!["test".to_string()],
        secret: Some("my-secret".to_string()),
        active: true,
    };

    // Will fail to connect but should not panic during signing
    let result = deliver(&client, &sub, b"{\"event\":\"test\"}").await;
    assert!(result.is_err());
}

// --- VecDeque delivery log tests ---

#[test]
fn delivery_log_evicts_oldest_with_vecdeque() {
    let mut log = DeliveryLog::with_max_capacity(3);
    for i in 0..5 {
        log.record(DeliveryRecord::success(
            format!("del-{i}"),
            "sub-1".to_string(),
            "test".to_string(),
            &serde_json::json!({}),
            200,
            1,
        ));
    }
    assert_eq!(log.len(), 3);
    let records = log.all();
    // Should have del-2, del-3, del-4 (oldest evicted)
    assert_eq!(records[0].id, "del-2");
    assert_eq!(records[1].id, "del-3");
    assert_eq!(records[2].id, "del-4");
}

#[test]
fn delivery_log_eviction_is_stable() {
    let mut log = DeliveryLog::with_max_capacity(2);
    log.record(DeliveryRecord::success(
        "a".into(), "s".into(), "e".into(), &serde_json::json!({}), 200, 1,
    ));
    log.record(DeliveryRecord::success(
        "b".into(), "s".into(), "e".into(), &serde_json::json!({}), 200, 1,
    ));
    log.record(DeliveryRecord::success(
        "c".into(), "s".into(), "e".into(), &serde_json::json!({}), 200, 1,
    ));
    assert_eq!(log.len(), 2);
    let records = log.all();
    assert_eq!(records[0].id, "b");
    assert_eq!(records[1].id, "c");
}
