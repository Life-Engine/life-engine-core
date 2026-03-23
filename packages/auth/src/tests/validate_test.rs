//! Unit tests for the auth validation pipeline (`validate_request`).

use async_trait::async_trait;
use uuid::Uuid;

use crate::error::AuthError;
use crate::handlers::rate_limit::RateLimiter;
use crate::handlers::validate::validate_request;
use crate::types::AuthIdentity;
use crate::AuthProvider;

/// A configurable mock auth provider for testing `validate_request`.
struct MockAuthProvider {
    /// Result returned by `validate_token`.
    token_result: Box<dyn Fn(&str) -> Result<AuthIdentity, AuthError> + Send + Sync>,
    /// Result returned by `validate_key`.
    key_result: Box<dyn Fn(&str) -> Result<AuthIdentity, AuthError> + Send + Sync>,
}

impl MockAuthProvider {
    /// Create a mock that succeeds for both token and key validation.
    fn succeeding() -> Self {
        Self {
            token_result: Box::new(|_| {
                Ok(AuthIdentity {
                    user_id: "user-123".to_string(),
                    provider: "pocket-id".to_string(),
                    scopes: vec!["read".to_string(), "write".to_string()],
                    authenticated_at: chrono::Utc::now(),
                })
            }),
            key_result: Box::new(|_| {
                Ok(AuthIdentity {
                    user_id: "service-456".to_string(),
                    provider: "api-key".to_string(),
                    scopes: vec!["admin".to_string()],
                    authenticated_at: chrono::Utc::now(),
                })
            }),
        }
    }

    /// Create a mock where token validation returns the given error.
    fn token_fails_with(err_fn: impl Fn() -> AuthError + Send + Sync + 'static) -> Self {
        Self {
            token_result: Box::new(move |_| Err(err_fn())),
            key_result: Box::new(|_| {
                Ok(AuthIdentity {
                    user_id: "service-456".to_string(),
                    provider: "api-key".to_string(),
                    scopes: vec![],
                    authenticated_at: chrono::Utc::now(),
                })
            }),
        }
    }
}

#[async_trait]
impl AuthProvider for MockAuthProvider {
    async fn validate_token(&self, token: &str) -> Result<AuthIdentity, AuthError> {
        (self.token_result)(token)
    }

    async fn validate_key(&self, key: &str) -> Result<AuthIdentity, AuthError> {
        (self.key_result)(key)
    }

    async fn revoke_key(&self, _key_id: Uuid) -> Result<(), AuthError> {
        Ok(())
    }
}

#[tokio::test]
async fn valid_bearer_token_returns_identity() {
    let provider = MockAuthProvider::succeeding();
    let limiter = RateLimiter::new();

    let result = validate_request(
        &provider,
        Some("Bearer valid-jwt-token"),
        &limiter,
        "192.168.1.1",
    )
    .await;

    let identity = result.expect("should succeed");
    assert_eq!(identity.user_id, "user-123");
    assert_eq!(identity.provider, "pocket-id");
    assert_eq!(identity.scopes, vec!["read", "write"]);
}

#[tokio::test]
async fn valid_api_key_returns_identity() {
    let provider = MockAuthProvider::succeeding();
    let limiter = RateLimiter::new();

    let result = validate_request(
        &provider,
        Some("ApiKey my-secret-key"),
        &limiter,
        "192.168.1.1",
    )
    .await;

    let identity = result.expect("should succeed");
    assert_eq!(identity.user_id, "service-456");
    assert_eq!(identity.provider, "api-key");
    assert_eq!(identity.scopes, vec!["admin"]);
}

#[tokio::test]
async fn missing_auth_header_returns_token_missing() {
    let provider = MockAuthProvider::succeeding();
    let limiter = RateLimiter::new();

    let result = validate_request(&provider, None, &limiter, "192.168.1.1").await;

    let err = result.expect_err("should fail");
    assert!(
        matches!(err, AuthError::TokenMissing),
        "expected TokenMissing, got: {err:?}"
    );
}

#[tokio::test]
async fn expired_bearer_token_returns_token_expired() {
    let provider = MockAuthProvider::token_fails_with(|| AuthError::TokenExpired);
    let limiter = RateLimiter::new();

    let result = validate_request(
        &provider,
        Some("Bearer expired-jwt"),
        &limiter,
        "192.168.1.1",
    )
    .await;

    let err = result.expect_err("should fail");
    assert!(
        matches!(err, AuthError::TokenExpired),
        "expected TokenExpired, got: {err:?}"
    );
}

#[tokio::test]
async fn invalid_bearer_token_returns_token_invalid() {
    let provider =
        MockAuthProvider::token_fails_with(|| AuthError::TokenInvalid("bad sig".to_string()));
    let limiter = RateLimiter::new();

    let result = validate_request(
        &provider,
        Some("Bearer bad-token"),
        &limiter,
        "192.168.1.1",
    )
    .await;

    let err = result.expect_err("should fail");
    assert!(
        matches!(err, AuthError::TokenInvalid(_)),
        "expected TokenInvalid, got: {err:?}"
    );
}

#[tokio::test]
async fn unknown_auth_scheme_returns_token_invalid() {
    let provider = MockAuthProvider::succeeding();
    let limiter = RateLimiter::new();

    let result = validate_request(
        &provider,
        Some("Basic dXNlcjpwYXNz"),
        &limiter,
        "192.168.1.1",
    )
    .await;

    let err = result.expect_err("should fail");
    assert!(
        matches!(err, AuthError::TokenInvalid(_)),
        "expected TokenInvalid, got: {err:?}"
    );
}

#[tokio::test]
async fn rate_limited_ip_returns_rate_limited_error() {
    let provider = MockAuthProvider::token_fails_with(|| AuthError::TokenExpired);
    let limiter = RateLimiter::new();
    let ip = "10.0.0.1";

    // Record 5 failures to trigger the rate limiter.
    for _ in 0..5 {
        let _ = validate_request(&provider, Some("Bearer bad"), &limiter, ip).await;
    }

    // The 6th attempt should be rate-limited before even reaching the provider.
    let result = validate_request(
        &provider,
        Some("Bearer doesnt-matter"),
        &limiter,
        ip,
    )
    .await;

    let err = result.expect_err("should be rate limited");
    match err {
        AuthError::RateLimited { retry_after } => {
            assert!(retry_after > 0, "retry_after should be positive");
        }
        other => panic!("expected RateLimited, got: {other:?}"),
    }
}

#[tokio::test]
async fn failed_validation_records_failure_in_rate_limiter() {
    let provider =
        MockAuthProvider::token_fails_with(|| AuthError::TokenInvalid("bad".to_string()));
    let limiter = RateLimiter::new();
    let ip = "10.0.0.2";

    // Verify IP is not rate-limited initially.
    assert!(limiter.is_rate_limited(ip).await.is_none());

    // Fail 4 times — should not yet be rate-limited.
    for _ in 0..4 {
        let _ = validate_request(&provider, Some("Bearer bad"), &limiter, ip).await;
    }
    assert!(
        limiter.is_rate_limited(ip).await.is_none(),
        "should not be rate-limited after 4 failures"
    );

    // 5th failure should trigger the limit.
    let _ = validate_request(&provider, Some("Bearer bad"), &limiter, ip).await;
    assert!(
        limiter.is_rate_limited(ip).await.is_some(),
        "should be rate-limited after 5 failures"
    );
}
