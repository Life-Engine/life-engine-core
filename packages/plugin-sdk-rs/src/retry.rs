//! Exponential backoff retry logic for plugin operations.
//!
//! Provides a reusable retry state tracker used by connectors (sync retry)
//! and webhook sender (delivery retry). Backoff starts at a configurable
//! minimum, doubles on each failure, and caps at a configurable maximum.

use std::time::Duration;

/// Default minimum backoff duration (1 minute).
const DEFAULT_BACKOFF_MIN_SECS: u64 = 60;

/// Default maximum backoff duration (1 hour).
const DEFAULT_BACKOFF_MAX_SECS: u64 = 3600;

/// Default maximum number of retry attempts.
const DEFAULT_MAX_RETRIES: u32 = 5;

/// Tracks retry state with exponential backoff.
///
/// # Example
///
/// ```rust
/// use life_engine_plugin_sdk::retry::RetryState;
/// use std::time::Duration;
///
/// let mut state = RetryState::new();
///
/// // Record a failure and get the backoff duration
/// let backoff = state.record_failure();
/// assert_eq!(backoff, Duration::from_secs(60));
///
/// // Record success to reset
/// state.record_success();
/// assert_eq!(state.failure_count, 0);
/// ```
#[derive(Debug, Clone)]
pub struct RetryState {
    /// Number of consecutive failures.
    pub failure_count: u32,
    /// Maximum retry attempts allowed.
    pub max_retries: u32,
    /// Minimum backoff duration in seconds.
    pub backoff_min_secs: u64,
    /// Maximum backoff duration in seconds.
    pub backoff_max_secs: u64,
}

impl RetryState {
    /// Create a new retry state with default settings
    /// (5 max retries, 60s min backoff, 3600s max backoff).
    pub fn new() -> Self {
        Self {
            failure_count: 0,
            max_retries: DEFAULT_MAX_RETRIES,
            backoff_min_secs: DEFAULT_BACKOFF_MIN_SECS,
            backoff_max_secs: DEFAULT_BACKOFF_MAX_SECS,
        }
    }

    /// Create a retry state with custom settings.
    pub fn with_config(max_retries: u32, backoff_min_secs: u64, backoff_max_secs: u64) -> Self {
        Self {
            failure_count: 0,
            max_retries,
            backoff_min_secs,
            backoff_max_secs,
        }
    }

    /// Record a failure and return the computed backoff duration.
    pub fn record_failure(&mut self) -> Duration {
        self.failure_count = self.failure_count.saturating_add(1);
        self.compute_backoff()
    }

    /// Record a success: resets failure count.
    pub fn record_success(&mut self) {
        self.failure_count = 0;
    }

    /// Whether more retries are allowed.
    pub fn can_retry(&self) -> bool {
        self.failure_count < self.max_retries
    }

    /// Whether all retries have been exhausted.
    pub fn exhausted(&self) -> bool {
        self.failure_count >= self.max_retries
    }

    /// Compute the backoff duration based on the current failure count.
    ///
    /// Formula: `min(backoff_min * 2^(failure_count - 1), backoff_max)`
    pub fn compute_backoff(&self) -> Duration {
        if self.failure_count == 0 {
            return Duration::ZERO;
        }
        let exponent = self.failure_count.saturating_sub(1).min(31);
        let backoff_secs = self.backoff_min_secs.saturating_mul(1u64 << exponent);
        let capped = backoff_secs.min(self.backoff_max_secs);
        Duration::from_secs(capped)
    }
}

impl Default for RetryState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state() {
        let state = RetryState::new();
        assert_eq!(state.failure_count, 0);
        assert!(state.can_retry());
        assert!(!state.exhausted());
        assert_eq!(state.compute_backoff(), Duration::ZERO);
    }

    #[test]
    fn backoff_doubles_on_each_failure() {
        let mut state = RetryState::new();

        let d1 = state.record_failure();
        assert_eq!(d1, Duration::from_secs(60));

        let d2 = state.record_failure();
        assert_eq!(d2, Duration::from_secs(120));

        let d3 = state.record_failure();
        assert_eq!(d3, Duration::from_secs(240));
    }

    #[test]
    fn backoff_caps_at_max() {
        let mut state = RetryState::new();

        for _ in 0..20 {
            state.record_failure();
        }

        assert_eq!(state.compute_backoff(), Duration::from_secs(3600));
    }

    #[test]
    fn resets_on_success() {
        let mut state = RetryState::new();
        state.record_failure();
        state.record_failure();
        assert_eq!(state.failure_count, 2);

        state.record_success();
        assert_eq!(state.failure_count, 0);
        assert!(state.can_retry());
    }

    #[test]
    fn exhausted_after_max_retries() {
        let mut state = RetryState::new();
        for _ in 0..5 {
            state.record_failure();
        }
        assert!(state.exhausted());
        assert!(!state.can_retry());
    }

    #[test]
    fn custom_config() {
        let mut state = RetryState::with_config(3, 10, 100);
        assert_eq!(state.max_retries, 3);

        let d1 = state.record_failure();
        assert_eq!(d1, Duration::from_secs(10));

        let d2 = state.record_failure();
        assert_eq!(d2, Duration::from_secs(20));

        let d3 = state.record_failure();
        assert_eq!(d3, Duration::from_secs(40));

        assert!(state.exhausted());
    }

    #[test]
    fn default_impl() {
        let state = RetryState::default();
        assert_eq!(state.failure_count, 0);
        assert_eq!(state.max_retries, 5);
    }
}
