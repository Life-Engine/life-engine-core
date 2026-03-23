//! Webhook transport error types.

use life_engine_traits::{EngineError, Severity};
use thiserror::Error;

/// Errors that can occur in the webhook transport layer.
#[derive(Debug, Error)]
pub enum WebhookError {
    /// Webhook delivery failed.
    #[error("webhook delivery failed: {0}")]
    DeliveryFailed(String),

    /// Transport failed to bind to the configured address.
    #[error("failed to bind webhook transport: {0}")]
    BindFailed(String),

    /// Configuration is invalid.
    #[error("invalid webhook transport config: {0}")]
    InvalidConfig(String),
}

impl EngineError for WebhookError {
    fn code(&self) -> &str {
        match self {
            WebhookError::DeliveryFailed(_) => "TRANSPORT_WEBHOOK_001",
            WebhookError::BindFailed(_) => "TRANSPORT_WEBHOOK_002",
            WebhookError::InvalidConfig(_) => "TRANSPORT_WEBHOOK_003",
        }
    }

    fn severity(&self) -> Severity {
        match self {
            WebhookError::DeliveryFailed(_) => Severity::Retryable,
            WebhookError::BindFailed(_) => Severity::Fatal,
            WebhookError::InvalidConfig(_) => Severity::Fatal,
        }
    }

    fn source_module(&self) -> &str {
        "transport-webhook"
    }
}
