//! Auth middleware for the axum HTTP router.
//!
//! Extracts `Authorization: Bearer <token>` headers, validates tokens
//! via the active `AuthProvider`, attaches `AuthIdentity` to request
//! extensions, and enforces per-IP rate limiting on failed attempts.

use crate::auth::types::AuthError;
use crate::auth::AuthProvider;

use axum::body::Body;
use axum::extract::{ConnectInfo, State};
use axum::http::{Request, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::json;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

/// Maximum number of failed auth attempts per IP within the window.
const MAX_FAILURES: usize = 5;

/// Duration of the rate-limit window in seconds.
const WINDOW_SECS: u64 = 60;

/// How often (in number of operations) to perform a full cleanup of expired entries.
const CLEANUP_INTERVAL: u64 = 100;

/// Tracks failed auth attempts per IP address.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    failures: Arc<Mutex<HashMap<IpAddr, Vec<Instant>>>>,
    /// Counter of operations since last full cleanup.
    op_count: Arc<std::sync::atomic::AtomicU64>,
}

impl RateLimiter {
    /// Create a new rate limiter.
    pub fn new() -> Self {
        Self {
            failures: Arc::new(Mutex::new(HashMap::new())),
            op_count: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    /// Record a failed auth attempt from the given IP.
    pub async fn record_failure(&self, ip: IpAddr) {
        let mut failures = self.failures.lock().await;
        failures.entry(ip).or_default().push(Instant::now());
        self.maybe_cleanup(&mut failures);
    }

    /// Check if the given IP is rate limited.
    pub async fn is_rate_limited(&self, ip: IpAddr) -> bool {
        let mut failures = self.failures.lock().await;
        self.maybe_cleanup(&mut failures);
        if let Some(attempts) = failures.get_mut(&ip) {
            let cutoff = Instant::now() - std::time::Duration::from_secs(WINDOW_SECS);
            attempts.retain(|t| *t > cutoff);
            attempts.len() >= MAX_FAILURES
        } else {
            false
        }
    }

    /// Periodically remove all expired entries across all IPs to prevent
    /// unbounded memory growth from IPs that are no longer active.
    fn maybe_cleanup(&self, failures: &mut HashMap<IpAddr, Vec<Instant>>) {
        let count = self
            .op_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if !count.is_multiple_of(CLEANUP_INTERVAL) {
            return;
        }
        let cutoff = Instant::now() - std::time::Duration::from_secs(WINDOW_SECS);
        failures.retain(|_ip, attempts| {
            attempts.retain(|t| *t > cutoff);
            !attempts.is_empty()
        });
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// Auth middleware function for use with `axum::middleware::from_fn_with_state`.
///
/// Skips auth for `/api/system/health` and `POST /api/auth/token`.
/// All other routes require a valid `Authorization: Bearer <token>` header.
pub async fn auth_middleware(
    State(state): State<AuthMiddlewareState>,
    mut request: Request<Body>,
    next: axum::middleware::Next,
) -> Response {
    let path = request.uri().path().to_string();
    let method = request.method().clone();

    // Skip auth for health endpoint, token generation, storage init, and OIDC public endpoints.
    if path == "/api/system/health" {
        return next.run(request).await;
    }
    if path == "/api/storage/init" && method == axum::http::Method::POST {
        return next.run(request).await;
    }
    if path == "/api/auth/token" && method == axum::http::Method::POST {
        return next.run(request).await;
    }
    if (path == "/api/auth/login"
        || path == "/api/auth/refresh"
        || path == "/api/auth/register"
        || path == "/api/auth/webauthn/authenticate/start"
        || path == "/api/auth/webauthn/authenticate/finish")
        && method == axum::http::Method::POST
    {
        return next.run(request).await;
    }
    if path == "/api/auth/.well-known/openid-configuration"
        && method == axum::http::Method::GET
    {
        return next.run(request).await;
    }

    // Extract client IP: prefer X-Forwarded-For when behind a reverse proxy.
    let client_ip = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .and_then(|s| s.trim().parse::<IpAddr>().ok())
        .or_else(|| {
            request
                .extensions()
                .get::<ConnectInfo<SocketAddr>>()
                .map(|ci| ci.0.ip())
        })
        .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));

    // Check rate limit.
    if state.rate_limiter.is_rate_limited(client_ip).await {
        tracing::warn!(ip = %client_ip, "auth rate limited");
        return auth_error_response(StatusCode::TOO_MANY_REQUESTS, "AUTH_RATE_LIMITED");
    }

    // Extract bearer token.
    let token = match extract_bearer_token(&request) {
        Some(t) => t,
        None => {
            tracing::debug!("missing auth token");
            return auth_error_response(StatusCode::UNAUTHORIZED, "AUTH_MISSING_TOKEN");
        }
    };

    // Validate token.
    match state.auth_provider.validate_token(&token).await {
        Ok(identity) => {
            tracing::debug!(token_id = %identity.token_id, "auth success");
            if let Some(ref bus) = state.message_bus {
                bus.publish(crate::message_bus::BusEvent::AuthSuccess {
                    identity_subject: identity.token_id.clone(),
                    method: "bearer_token".to_string(),
                });
            }
            request.extensions_mut().insert(identity);
            next.run(request).await
        }
        Err(AuthError::TokenExpired) => {
            state.rate_limiter.record_failure(client_ip).await;
            tracing::debug!("expired token presented");
            if let Some(ref bus) = state.message_bus {
                bus.publish(crate::message_bus::BusEvent::AuthFailure {
                    client_ip: client_ip.to_string(),
                    reason: "token_expired".to_string(),
                });
            }
            auth_error_response(StatusCode::UNAUTHORIZED, "AUTH_TOKEN_EXPIRED")
        }
        Err(AuthError::TokenNotFound) | Err(AuthError::InvalidCredentials) => {
            state.rate_limiter.record_failure(client_ip).await;
            tracing::debug!("invalid token presented");
            if let Some(ref bus) = state.message_bus {
                bus.publish(crate::message_bus::BusEvent::AuthFailure {
                    client_ip: client_ip.to_string(),
                    reason: "invalid_token".to_string(),
                });
            }
            auth_error_response(StatusCode::UNAUTHORIZED, "AUTH_INVALID_TOKEN")
        }
        Err(e) => {
            tracing::error!(error = %e, "auth validation error");
            if let Some(ref bus) = state.message_bus {
                bus.publish(crate::message_bus::BusEvent::AuthFailure {
                    client_ip: client_ip.to_string(),
                    reason: format!("internal_error: {e}"),
                });
            }
            auth_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "AUTH_INTERNAL_ERROR",
            )
        }
    }
}

/// State passed to the auth middleware.
#[derive(Clone)]
pub struct AuthMiddlewareState {
    /// The active auth provider.
    pub auth_provider: Arc<dyn AuthProvider>,
    /// The rate limiter for failed auth attempts.
    pub rate_limiter: RateLimiter,
    /// Optional message bus for publishing audit events.
    pub message_bus: Option<Arc<crate::message_bus::MessageBus>>,
}

/// Extract the bearer token from the Authorization header.
fn extract_bearer_token(request: &Request<Body>) -> Option<String> {
    let header = request.headers().get("authorization")?.to_str().ok()?;
    let stripped = header.strip_prefix("Bearer ")?;
    if stripped.is_empty() {
        return None;
    }
    Some(stripped.to_string())
}

/// Build a JSON error response with the given status code and error code.
fn auth_error_response(status: StatusCode, error_code: &str) -> Response {
    let body = json!({ "error": error_code });
    (status, axum::Json(body)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use crate::auth::local_token::LocalTokenProvider;
    use crate::auth::types::TokenRequest;
    use crate::auth::AuthProvider;
    use crate::test_helpers::{create_auth_state, generate_test_token};
    use axum::routing::get;
    use axum::Router;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn setup_test_app() -> (Router, Arc<LocalTokenProvider>) {
        let (auth_state, provider) = create_auth_state();

        let app = Router::new()
            .route("/api/system/health", get(|| async { "ok" }))
            .route("/api/protected", get(|| async { "protected" }))
            .route(
                "/api/auth/token",
                axum::routing::post(|| async { "token" }),
            )
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                auth_middleware,
            ));

        (app, provider)
    }

    #[tokio::test]
    async fn health_endpoint_skips_auth() {
        let (app, _provider) = setup_test_app().await;
        let request = Request::builder()
            .uri("/api/system/health")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn token_endpoint_post_skips_auth() {
        let (app, _provider) = setup_test_app().await;
        let request = Request::builder()
            .method("POST")
            .uri("/api/auth/token")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn missing_token_returns_401() {
        let (app, _provider) = setup_test_app().await;
        let request = Request::builder()
            .uri("/api/protected")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "AUTH_MISSING_TOKEN");
    }

    #[tokio::test]
    async fn invalid_token_returns_401() {
        let (app, _provider) = setup_test_app().await;
        let request = Request::builder()
            .uri("/api/protected")
            .header("Authorization", "Bearer invalid-token-value")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "AUTH_INVALID_TOKEN");
    }

    #[tokio::test]
    async fn valid_token_passes_through() {
        let (app, provider) = setup_test_app().await;
        let token = generate_test_token(&provider).await;
        let request = Request::builder()
            .uri("/api/protected")
            .header("Authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn expired_token_returns_401_expired() {
        let (app, provider) = setup_test_app().await;
        let req = TokenRequest {
            passphrase: "test-pass".into(),
            expires_in_days: Some(1),
        };
        let resp = provider.generate_token(&req).await.unwrap();

        // Manually expire the token.
        {
            let mut state = provider.state.write().await;
            let stored = state.tokens.get_mut(&resp.token_id).unwrap();
            stored.expires_at = Utc::now() - chrono::Duration::hours(1);
        }

        let request = Request::builder()
            .uri("/api/protected")
            .header("Authorization", format!("Bearer {}", resp.token))
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "AUTH_TOKEN_EXPIRED");
    }

    #[tokio::test]
    async fn rate_limiter_records_and_checks() {
        let limiter = RateLimiter::new();
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        // Not rate limited initially.
        assert!(!limiter.is_rate_limited(ip).await);

        // Record 4 failures — still under limit.
        for _ in 0..4 {
            limiter.record_failure(ip).await;
        }
        assert!(!limiter.is_rate_limited(ip).await);

        // 5th failure — now rate limited.
        limiter.record_failure(ip).await;
        assert!(limiter.is_rate_limited(ip).await);
    }

    #[tokio::test]
    async fn rate_limiter_different_ips_independent() {
        let limiter = RateLimiter::new();
        let ip1: IpAddr = "10.0.0.1".parse().unwrap();
        let ip2: IpAddr = "10.0.0.2".parse().unwrap();

        for _ in 0..5 {
            limiter.record_failure(ip1).await;
        }

        assert!(limiter.is_rate_limited(ip1).await);
        assert!(!limiter.is_rate_limited(ip2).await);
    }

    #[tokio::test]
    async fn rate_limiter_default_impl() {
        let limiter = RateLimiter::default();
        let ip: IpAddr = "127.0.0.1".parse().unwrap();
        assert!(!limiter.is_rate_limited(ip).await);
    }

    #[test]
    fn extract_bearer_token_valid() {
        let request = Request::builder()
            .header("Authorization", "Bearer my-token-123")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            extract_bearer_token(&request),
            Some("my-token-123".to_string())
        );
    }

    #[test]
    fn extract_bearer_token_missing_header() {
        let request = Request::builder().body(Body::empty()).unwrap();
        assert!(extract_bearer_token(&request).is_none());
    }

    #[test]
    fn extract_bearer_token_wrong_scheme() {
        let request = Request::builder()
            .header("Authorization", "Basic abc123")
            .body(Body::empty())
            .unwrap();
        assert!(extract_bearer_token(&request).is_none());
    }

    #[test]
    fn extract_bearer_token_empty_value() {
        let request = Request::builder()
            .header("Authorization", "Bearer ")
            .body(Body::empty())
            .unwrap();
        assert!(extract_bearer_token(&request).is_none());
    }

    #[test]
    fn auth_error_response_has_correct_status() {
        let response = auth_error_response(StatusCode::UNAUTHORIZED, "TEST_ERROR");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
