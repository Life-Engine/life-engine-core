use thiserror::Error;

/// Errors specific to the webhook sender plugin.
#[derive(Debug, Error)]
pub enum WebhookSenderError {
    #[error("subscription not found: {0}")]
    SubscriptionNotFound(String),
    #[error("delivery failed: {0}")]
    DeliveryFailed(String),
    #[error("retries exhausted for subscription: {0}")]
    RetriesExhausted(String),
}
