//! Logging host function for WASM plugins.
//!
//! Allows plugins to emit structured log entries tagged with their plugin ID.
//! This host function does NOT require any capability — all plugins can log.
//! Rate limiting is applied: max 100 log entries per second per plugin to
//! prevent log flooding from buggy plugins.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use tracing::{debug, error, info, trace, warn};

use crate::error::PluginError;

/// Log level values that plugins can emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginLogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl PluginLogLevel {
    /// Parse a log level string from the plugin.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Result<Self, PluginError> {
        match s {
            "trace" => Ok(Self::Trace),
            "debug" => Ok(Self::Debug),
            "info" => Ok(Self::Info),
            "warn" => Ok(Self::Warn),
            "error" => Ok(Self::Error),
            _ => Err(PluginError::ExecutionFailed(format!(
                "invalid log level '{s}': expected one of trace, debug, info, warn, error"
            ))),
        }
    }
}

/// A log entry from a plugin.
#[derive(Debug, Clone)]
pub struct PluginLogEntry {
    /// The log level.
    pub level: PluginLogLevel,
    /// The log message.
    pub message: String,
}

/// Tracks rate limiting state for a single plugin.
struct RateLimitState {
    /// Count of log entries in the current window.
    count: u32,
    /// Start of the current 1-second window.
    window_start: Instant,
    /// Whether we've already warned about rate limiting in this window.
    warned: bool,
}

/// Rate limiter that enforces max 100 log entries per second per plugin.
pub struct LogRateLimiter {
    states: Mutex<HashMap<String, RateLimitState>>,
    max_per_second: u32,
}

impl LogRateLimiter {
    /// Create a new rate limiter with the default limit of 100 entries/sec.
    pub fn new() -> Self {
        Self {
            states: Mutex::new(HashMap::new()),
            max_per_second: 100,
        }
    }

    /// Create a rate limiter with a custom limit (useful for testing).
    #[cfg(test)]
    pub fn with_limit(max_per_second: u32) -> Self {
        Self {
            states: Mutex::new(HashMap::new()),
            max_per_second,
        }
    }

    /// Check if the plugin is allowed to log. Returns `true` if allowed,
    /// `false` if rate limited. Emits a single warning when the limit is hit.
    pub fn check(&self, plugin_id: &str) -> bool {
        let mut states = self.states.lock().expect("rate limiter lock poisoned");
        let now = Instant::now();

        let state = states
            .entry(plugin_id.to_string())
            .or_insert_with(|| RateLimitState {
                count: 0,
                window_start: now,
                warned: false,
            });

        // Reset window if more than 1 second has elapsed
        if now.duration_since(state.window_start).as_secs() >= 1 {
            state.count = 0;
            state.window_start = now;
            state.warned = false;
        }

        state.count += 1;

        if state.count > self.max_per_second {
            if !state.warned {
                state.warned = true;
                warn!(
                    plugin_id = %plugin_id,
                    limit = self.max_per_second,
                    "plugin log rate limit exceeded, dropping excess entries"
                );
            }
            return false;
        }

        true
    }
}

impl Default for LogRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// Context passed to the logging host function.
pub struct LoggingHostContext {
    /// The plugin ID making the log call.
    pub plugin_id: String,
    /// Shared rate limiter instance.
    pub rate_limiter: std::sync::Arc<LogRateLimiter>,
}

/// Processes a log entry from a plugin.
///
/// Parses the input bytes as a JSON object with `level` and `message` fields,
/// tags the entry with the plugin ID, and forwards to the tracing subscriber.
/// No capability is required — all plugins can log.
pub fn host_log(ctx: &LoggingHostContext, input: &[u8]) -> Result<Vec<u8>, PluginError> {
    // Deserialize the log entry
    let value: serde_json::Value = serde_json::from_slice(input).map_err(|e| {
        PluginError::ExecutionFailed(format!(
            "failed to deserialize log entry from plugin '{}': {e}",
            ctx.plugin_id
        ))
    })?;

    let level_str = value["level"]
        .as_str()
        .ok_or_else(|| {
            PluginError::ExecutionFailed(format!(
                "log entry from plugin '{}' missing 'level' string field",
                ctx.plugin_id
            ))
        })?;

    let message = value["message"]
        .as_str()
        .ok_or_else(|| {
            PluginError::ExecutionFailed(format!(
                "log entry from plugin '{}' missing 'message' string field",
                ctx.plugin_id
            ))
        })?;

    let level = PluginLogLevel::from_str(level_str)?;

    // Check rate limit
    if !ctx.rate_limiter.check(&ctx.plugin_id) {
        // Silently drop — the rate limiter already emitted a warning
        return Ok(b"{}".to_vec());
    }

    // Emit the log entry tagged with the plugin ID
    match level {
        PluginLogLevel::Trace => {
            trace!(plugin_id = %ctx.plugin_id, "{message}");
        }
        PluginLogLevel::Debug => {
            debug!(plugin_id = %ctx.plugin_id, "{message}");
        }
        PluginLogLevel::Info => {
            info!(plugin_id = %ctx.plugin_id, "{message}");
        }
        PluginLogLevel::Warn => {
            warn!(plugin_id = %ctx.plugin_id, "{message}");
        }
        PluginLogLevel::Error => {
            error!(plugin_id = %ctx.plugin_id, "{message}");
        }
    }

    Ok(b"{}".to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn make_context(plugin_id: &str) -> LoggingHostContext {
        LoggingHostContext {
            plugin_id: plugin_id.to_string(),
            rate_limiter: Arc::new(LogRateLimiter::new()),
        }
    }

    fn make_context_with_limiter(
        plugin_id: &str,
        limiter: Arc<LogRateLimiter>,
    ) -> LoggingHostContext {
        LoggingHostContext {
            plugin_id: plugin_id.to_string(),
            rate_limiter: limiter,
        }
    }

    fn make_log_entry(level: &str, message: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "level": level,
            "message": message
        }))
        .unwrap()
    }

    // --- PluginLogLevel tests ---

    #[test]
    fn parse_valid_log_levels() {
        assert_eq!(PluginLogLevel::from_str("trace").unwrap(), PluginLogLevel::Trace);
        assert_eq!(PluginLogLevel::from_str("debug").unwrap(), PluginLogLevel::Debug);
        assert_eq!(PluginLogLevel::from_str("info").unwrap(), PluginLogLevel::Info);
        assert_eq!(PluginLogLevel::from_str("warn").unwrap(), PluginLogLevel::Warn);
        assert_eq!(PluginLogLevel::from_str("error").unwrap(), PluginLogLevel::Error);
    }

    #[test]
    fn parse_invalid_log_level_returns_error() {
        let result = PluginLogLevel::from_str("critical");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid log level"));
    }

    // --- host_log tests ---

    #[test]
    fn log_info_succeeds_without_capability() {
        let ctx = make_context("test-plugin");
        let input = make_log_entry("info", "Fetched 42 new emails");

        let result = host_log(&ctx, &input);
        assert!(result.is_ok());
    }

    #[test]
    fn log_all_levels_succeed() {
        let ctx = make_context("test-plugin");

        for level in &["trace", "debug", "info", "warn", "error"] {
            let input = make_log_entry(level, &format!("test message at {level}"));
            let result = host_log(&ctx, &input);
            assert!(result.is_ok(), "log at level {level} should succeed");
        }
    }

    #[test]
    fn invalid_json_returns_error() {
        let ctx = make_context("test-plugin");

        let result = host_log(&ctx, b"not valid json");
        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ExecutionFailed(_)));
        assert!(err.to_string().contains("deserialize"));
    }

    #[test]
    fn missing_level_field_returns_error() {
        let ctx = make_context("test-plugin");
        let input = serde_json::to_vec(&serde_json::json!({
            "message": "no level"
        }))
        .unwrap();

        let result = host_log(&ctx, &input);
        let err = result.unwrap_err();
        assert!(err.to_string().contains("level"));
    }

    #[test]
    fn missing_message_field_returns_error() {
        let ctx = make_context("test-plugin");
        let input = serde_json::to_vec(&serde_json::json!({
            "level": "info"
        }))
        .unwrap();

        let result = host_log(&ctx, &input);
        let err = result.unwrap_err();
        assert!(err.to_string().contains("message"));
    }

    #[test]
    fn invalid_level_string_returns_error() {
        let ctx = make_context("test-plugin");
        let input = make_log_entry("critical", "bad level");

        let result = host_log(&ctx, &input);
        let err = result.unwrap_err();
        assert!(err.to_string().contains("invalid log level"));
    }

    // --- Rate limiting tests ---

    #[test]
    fn rate_limiter_allows_entries_under_limit() {
        let limiter = LogRateLimiter::with_limit(5);

        for _ in 0..5 {
            assert!(limiter.check("test-plugin"));
        }
    }

    #[test]
    fn rate_limiter_drops_entries_over_limit() {
        let limiter = LogRateLimiter::with_limit(3);

        assert!(limiter.check("test-plugin"));
        assert!(limiter.check("test-plugin"));
        assert!(limiter.check("test-plugin"));
        // 4th entry should be dropped
        assert!(!limiter.check("test-plugin"));
        assert!(!limiter.check("test-plugin"));
    }

    #[test]
    fn rate_limiter_tracks_plugins_independently() {
        let limiter = LogRateLimiter::with_limit(2);

        assert!(limiter.check("plugin-a"));
        assert!(limiter.check("plugin-a"));
        assert!(!limiter.check("plugin-a")); // plugin-a over limit

        // plugin-b should still be allowed
        assert!(limiter.check("plugin-b"));
        assert!(limiter.check("plugin-b"));
        assert!(!limiter.check("plugin-b")); // now plugin-b over limit too
    }

    #[test]
    fn host_log_drops_excess_entries_silently() {
        let limiter = Arc::new(LogRateLimiter::with_limit(2));
        let ctx = make_context_with_limiter("test-plugin", limiter);

        let input = make_log_entry("info", "message");

        // First two succeed normally
        assert!(host_log(&ctx, &input).is_ok());
        assert!(host_log(&ctx, &input).is_ok());

        // Third is dropped but still returns Ok (silent drop)
        let result = host_log(&ctx, &input);
        assert!(result.is_ok());
    }
}
