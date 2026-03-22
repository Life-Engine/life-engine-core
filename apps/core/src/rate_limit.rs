//! General per-IP rate limiting middleware.
//!
//! Uses the `governor` crate with a keyed (per-IP) GCRA rate limiter
//! backed by `DashMap` for lock-free concurrent access. Requests
//! exceeding the configured limit receive a `429 Too Many Requests`
//! response with a `Retry-After` header.
//!
//! The `/api/system/health` endpoint is exempt from rate limiting.

use axum::body::Body;
use axum::extract::{ConnectInfo, State};
use axum::http::{Request, StatusCode};
use axum::response::{IntoResponse, Response};
use governor::clock::{Clock, DefaultClock};
use governor::state::keyed::DashMapStateStore;
use governor::{Quota, RateLimiter};
use serde_json::json;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::num::NonZeroU32;
use std::sync::Arc;

/// Type alias for the governor keyed rate limiter with default clock.
type KeyedLimiter = RateLimiter<IpAddr, DashMapStateStore<IpAddr>, DefaultClock>;

/// Shared state for the general rate limiter middleware.
///
/// Wraps a `governor` keyed rate limiter that tracks request budgets
/// per client IP address using the GCRA algorithm.
#[derive(Debug, Clone)]
pub struct GeneralRateLimiter {
    /// The governor keyed rate limiter instance.
    limiter: Arc<KeyedLimiter>,
    /// Cached clock for computing retry-after durations.
    clock: DefaultClock,
}

impl GeneralRateLimiter {
    /// Create a new rate limiter with the given requests-per-minute limit.
    ///
    /// # Panics
    ///
    /// Panics if `max_requests_per_minute` is zero. The configuration
    /// layer validates this before construction.
    pub fn new(max_requests_per_minute: u32) -> Self {
        let rpm = NonZeroU32::new(max_requests_per_minute)
            .expect("max_requests_per_minute must be > 0 (validated by config)");
        let quota = Quota::per_minute(rpm);
        let limiter = Arc::new(RateLimiter::dashmap(quota));
        Self {
            limiter,
            clock: DefaultClock::default(),
        }
    }

    /// Check whether the given IP is allowed to proceed.
    ///
    /// Returns `Ok(())` if under the limit, or `Err(retry_after_secs)`
    /// with the number of seconds the client should wait before retrying.
    pub fn check(&self, ip: IpAddr) -> Result<(), u64> {
        match self.limiter.check_key(&ip) {
            Ok(_) => Ok(()),
            Err(not_until) => {
                let wait = not_until.wait_time_from(self.clock.now());
                let retry_after = wait.as_secs().max(1);
                Err(retry_after)
            }
        }
    }
}

/// Rate limiting middleware function for use with `axum::middleware::from_fn_with_state`.
///
/// Extracts the client IP from `ConnectInfo<SocketAddr>` and checks against
/// the governor rate limiter. The `/api/system/health` endpoint is exempt.
///
/// When rate limited, returns `429 Too Many Requests` with a `Retry-After`
/// header indicating how many seconds the client should wait.
pub async fn rate_limit_middleware(
    State(limiter): State<GeneralRateLimiter>,
    request: Request<Body>,
    next: axum::middleware::Next,
) -> Response {
    let path = request.uri().path();

    // Exempt health endpoint from rate limiting.
    if path == "/api/system/health" {
        return next.run(request).await;
    }

    // Prefer X-Forwarded-For when behind a reverse proxy, fall back to ConnectInfo.
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
        .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));

    match limiter.check(client_ip) {
        Ok(()) => next.run(request).await,
        Err(retry_after) => {
            tracing::warn!(ip = %client_ip, retry_after_secs = retry_after, "rate limited");
            let body = json!({
                "error": {
                    "code": "RATE_LIMITED",
                    "message": "Too many requests. Try again later."
                }
            });
            (
                StatusCode::TOO_MANY_REQUESTS,
                [("retry-after", retry_after.to_string())],
                axum::Json(body),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::get;
    use axum::Router;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_app(rpm: u32) -> Router {
        let limiter = GeneralRateLimiter::new(rpm);
        Router::new()
            .route("/api/system/health", get(|| async { "ok" }))
            .route("/api/protected", get(|| async { "protected" }))
            .layer(axum::middleware::from_fn_with_state(
                limiter,
                rate_limit_middleware,
            ))
    }

    #[tokio::test]
    async fn under_limit_passes() {
        let app = test_app(5);
        for _ in 0..5 {
            let req = Request::builder()
                .uri("/api/protected")
                .body(Body::empty())
                .unwrap();
            let resp = app.clone().oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
        }
    }

    #[tokio::test]
    async fn at_limit_returns_429() {
        // Use a small burst size. Governor uses GCRA which allows burst
        // equal to the quota amount. With per_minute(3), the initial
        // burst is 3 requests.
        let app = test_app(3);

        // Send 3 requests (within burst).
        for _ in 0..3 {
            let req = Request::builder()
                .uri("/api/protected")
                .body(Body::empty())
                .unwrap();
            let resp = app.clone().oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
        }

        // 4th request should be rate limited.
        let req = Request::builder()
            .uri("/api/protected")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "RATE_LIMITED");
        assert_eq!(
            json["error"]["message"],
            "Too many requests. Try again later."
        );
    }

    #[tokio::test]
    async fn different_ips_are_independent() {
        let limiter = GeneralRateLimiter::new(2);
        let ip1: IpAddr = "10.0.0.1".parse().unwrap();
        let ip2: IpAddr = "10.0.0.2".parse().unwrap();

        // Fill up ip1.
        assert!(limiter.check(ip1).is_ok());
        assert!(limiter.check(ip1).is_ok());
        // ip1 is now at limit.
        assert!(limiter.check(ip1).is_err());

        // ip2 should still be under limit.
        assert!(limiter.check(ip2).is_ok());
        assert!(limiter.check(ip2).is_ok());
        assert!(limiter.check(ip2).is_err());
    }

    #[tokio::test]
    async fn health_endpoint_exempt() {
        let app = test_app(1);

        // Use up the single allowed request.
        let req = Request::builder()
            .uri("/api/protected")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Health should still be accessible even though we are at limit.
        let req = Request::builder()
            .uri("/api/system/health")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // But another protected request should be blocked.
        let req = Request::builder()
            .uri("/api/protected")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn retry_after_header_present_on_429() {
        let app = test_app(1);

        // Use up the single allowed request.
        let req = Request::builder()
            .uri("/api/protected")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Second request should be rate limited with Retry-After header.
        let req = Request::builder()
            .uri("/api/protected")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);

        let retry_after = resp.headers().get("retry-after");
        assert!(
            retry_after.is_some(),
            "429 response must include Retry-After header"
        );

        // The value should be a positive integer (seconds).
        let secs: u64 = retry_after
            .unwrap()
            .to_str()
            .unwrap()
            .parse()
            .expect("Retry-After must be a valid integer");
        assert!(secs >= 1, "Retry-After must be at least 1 second");
    }

    #[tokio::test]
    async fn default_limit_is_configurable() {
        let limiter = GeneralRateLimiter::new(60);
        let ip: IpAddr = "127.0.0.1".parse().unwrap();
        // Should be able to do at least one request.
        assert!(limiter.check(ip).is_ok());
    }
}
