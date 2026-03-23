//! Tests for the per-IP sliding window rate limiter.

use crate::handlers::rate_limit::RateLimiter;

#[tokio::test]
async fn first_four_failures_do_not_trigger_rate_limit() {
    let limiter = RateLimiter::new();
    let ip = "192.168.1.1";

    for _ in 0..4 {
        limiter.record_failure(ip).await;
    }

    assert!(
        limiter.is_rate_limited(ip).await.is_none(),
        "4 failures should not trigger rate limit"
    );
}

#[tokio::test]
async fn fifth_failure_triggers_rate_limit() {
    let limiter = RateLimiter::new();
    let ip = "10.0.0.1";

    for _ in 0..5 {
        limiter.record_failure(ip).await;
    }

    let result = limiter.is_rate_limited(ip).await;
    assert!(result.is_some(), "5 failures should trigger rate limit");

    let retry_after = result.unwrap();
    assert!(
        retry_after > 0 && retry_after <= 60,
        "retry_after should be between 1 and 60, got {retry_after}"
    );
}

#[tokio::test]
async fn different_ips_are_tracked_independently() {
    let limiter = RateLimiter::new();
    let ip_a = "192.168.1.1";
    let ip_b = "192.168.1.2";

    // Rate-limit IP A.
    for _ in 0..5 {
        limiter.record_failure(ip_a).await;
    }

    // IP B has no failures.
    assert!(
        limiter.is_rate_limited(ip_a).await.is_some(),
        "IP A should be rate-limited"
    );
    assert!(
        limiter.is_rate_limited(ip_b).await.is_none(),
        "IP B should not be rate-limited"
    );
}

#[tokio::test]
async fn window_expiry_clears_rate_limit() {
    // Use a 1-second window for fast testing.
    let limiter = RateLimiter::with_config(5, 1);
    let ip = "10.0.0.5";

    for _ in 0..5 {
        limiter.record_failure(ip).await;
    }

    assert!(
        limiter.is_rate_limited(ip).await.is_some(),
        "should be rate-limited immediately after 5 failures"
    );

    // Wait for the window to expire.
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

    assert!(
        limiter.is_rate_limited(ip).await.is_none(),
        "should no longer be rate-limited after window expires"
    );
}

#[tokio::test]
async fn retry_after_decreases_as_window_slides() {
    // 2-second window, 2 max failures for easier reasoning.
    let limiter = RateLimiter::with_config(2, 2);
    let ip = "10.0.0.10";

    limiter.record_failure(ip).await;
    limiter.record_failure(ip).await;

    let first = limiter.is_rate_limited(ip).await;
    assert!(first.is_some(), "should be rate-limited");
    let first_retry = first.unwrap();

    // Wait a bit, retry_after should be less.
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

    let second = limiter.is_rate_limited(ip).await;
    // May still be rate-limited or may have cleared; if still limited, retry_after should be smaller.
    if let Some(second_retry) = second {
        assert!(
            second_retry <= first_retry,
            "retry_after should decrease over time: first={first_retry}, second={second_retry}"
        );
    }
    // If None, the window has fully expired which is also valid.
}

#[tokio::test]
async fn cleanup_removes_stale_entries() {
    let limiter = RateLimiter::with_config(5, 1);
    let ip = "10.0.0.20";

    limiter.record_failure(ip).await;
    assert_eq!(limiter.tracked_ips().await, 1);

    // Wait for window to expire.
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

    limiter.cleanup().await;
    assert_eq!(
        limiter.tracked_ips().await,
        0,
        "stale IP should be removed after cleanup"
    );
}

#[tokio::test]
async fn cleanup_preserves_active_entries() {
    let limiter = RateLimiter::with_config(5, 60);
    let ip_active = "10.0.0.30";
    let ip_stale = "10.0.0.31";

    limiter.record_failure(ip_active).await;
    limiter.record_failure(ip_stale).await;

    assert_eq!(limiter.tracked_ips().await, 2);

    // Both are still within the window, so cleanup should preserve both.
    limiter.cleanup().await;
    assert_eq!(
        limiter.tracked_ips().await,
        2,
        "active entries should survive cleanup"
    );
}

#[tokio::test]
async fn no_failures_means_not_rate_limited() {
    let limiter = RateLimiter::new();
    assert!(
        limiter.is_rate_limited("1.2.3.4").await.is_none(),
        "IP with no failures should not be rate-limited"
    );
}

#[tokio::test]
async fn default_impl_matches_new() {
    let limiter = RateLimiter::default();
    let ip = "10.0.0.99";

    // Verify default works the same as new().
    for _ in 0..4 {
        limiter.record_failure(ip).await;
    }
    assert!(limiter.is_rate_limited(ip).await.is_none());

    limiter.record_failure(ip).await;
    assert!(limiter.is_rate_limited(ip).await.is_some());
}
