use serde::Deserialize;

/// Configuration for the webhook sender plugin.
#[derive(Debug, Clone, Deserialize)]
pub struct WebhookSenderConfig {
    /// Maximum number of retry attempts per delivery.
    pub max_retries: u32,
    /// Maximum number of delivery records to retain.
    pub max_delivery_log_size: usize,
}

impl Default for WebhookSenderConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            max_delivery_log_size: 10_000,
        }
    }
}
