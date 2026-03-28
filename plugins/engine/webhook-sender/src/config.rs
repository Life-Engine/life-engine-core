use serde::Deserialize;

/// Configuration for the webhook sender plugin.
#[derive(Debug, Clone, Deserialize)]
pub struct WebhookSenderConfig {
    /// Maximum number of retry attempts per delivery.
    pub max_retries: u32,
    /// Maximum number of delivery records to retain.
    pub max_delivery_log_size: usize,
    /// Connect timeout in seconds for outbound webhook requests.
    #[serde(default = "default_connect_timeout_secs")]
    pub connect_timeout_secs: u64,
    /// Per-request timeout in seconds for outbound webhook requests.
    #[serde(default = "default_request_timeout_secs")]
    pub request_timeout_secs: u64,
    /// Total timeout in seconds across all retry attempts for a single delivery.
    #[serde(default = "default_total_timeout_secs")]
    pub total_timeout_secs: u64,
}

fn default_connect_timeout_secs() -> u64 {
    5
}

fn default_request_timeout_secs() -> u64 {
    30
}

fn default_total_timeout_secs() -> u64 {
    300
}

impl Default for WebhookSenderConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            max_delivery_log_size: 10_000,
            connect_timeout_secs: default_connect_timeout_secs(),
            request_timeout_secs: default_request_timeout_secs(),
            total_timeout_secs: default_total_timeout_secs(),
        }
    }
}
