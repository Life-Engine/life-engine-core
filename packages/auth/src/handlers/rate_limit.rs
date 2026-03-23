//! Per-IP sliding window rate limiter for failed authentication attempts.

use std::collections::HashMap;
use std::time::Instant;

use tokio::sync::RwLock;

/// Per-IP sliding window rate limiter.
///
/// Tracks failed authentication attempts per IP address. After 5 failures
/// within a 60-second sliding window, further attempts are blocked until
/// the oldest failure ages out.
pub struct RateLimiter {
    /// Map of IP address to timestamps of recent failed attempts.
    failures: RwLock<HashMap<String, Vec<Instant>>>,
    /// Maximum failures allowed within the window.
    max_failures: u32,
    /// Sliding window duration in seconds.
    window_secs: u64,
}

impl RateLimiter {
    /// Create a new rate limiter with default settings (5 failures per 60 seconds).
    pub fn new() -> Self {
        Self {
            failures: RwLock::new(HashMap::new()),
            max_failures: 5,
            window_secs: 60,
        }
    }

    /// Check if the given IP is currently rate-limited.
    ///
    /// Returns `Some(retry_after_secs)` if rate-limited, `None` otherwise.
    /// Prunes expired entries from the sliding window as a side effect.
    pub async fn is_rate_limited(&self, ip: &str) -> Option<u64> {
        let mut failures = self.failures.write().await;
        let now = Instant::now();
        let window = std::time::Duration::from_secs(self.window_secs);

        let entries = match failures.get_mut(ip) {
            Some(entries) => entries,
            None => return None,
        };

        // Prune entries outside the sliding window.
        entries.retain(|t| now.duration_since(*t) < window);

        if entries.is_empty() {
            failures.remove(ip);
            return None;
        }

        if entries.len() >= self.max_failures as usize {
            // The oldest entry determines when the window slides enough to allow new attempts.
            let oldest = entries[0];
            let elapsed = now.duration_since(oldest).as_secs();
            let retry_after = self.window_secs.saturating_sub(elapsed);
            Some(if retry_after == 0 { 1 } else { retry_after })
        } else {
            None
        }
    }

    /// Record a failed authentication attempt for the given IP.
    pub async fn record_failure(&self, ip: &str) {
        let mut failures = self.failures.write().await;
        failures
            .entry(ip.to_string())
            .or_default()
            .push(Instant::now());
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}
