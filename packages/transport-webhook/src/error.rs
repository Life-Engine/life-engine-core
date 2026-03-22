//! Webhook transport error types.

use thiserror::Error;

/// Errors that can occur in the webhook transport layer.
#[derive(Debug, Error)]
pub enum WebhookError {
    /// Webhook delivery failed.
    #[error("webhook delivery failed: {0}")]
    DeliveryFailed(String),
}
