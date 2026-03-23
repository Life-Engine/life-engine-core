use life_engine_plugin_sdk::prelude::*;
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
    #[error("unknown action: {0}")]
    UnknownAction(String),
}

impl EngineError for WebhookSenderError {
    fn code(&self) -> &str {
        match self {
            Self::SubscriptionNotFound(_) => "WEBHOOK_001",
            Self::DeliveryFailed(_) => "WEBHOOK_002",
            Self::RetriesExhausted(_) => "WEBHOOK_003",
            Self::UnknownAction(_) => "WEBHOOK_004",
        }
    }

    fn severity(&self) -> Severity {
        match self {
            Self::UnknownAction(_) => Severity::Fatal,
            Self::SubscriptionNotFound(_) => Severity::Fatal,
            Self::DeliveryFailed(_) => Severity::Retryable,
            Self::RetriesExhausted(_) => Severity::Fatal,
        }
    }

    fn source_module(&self) -> &str {
        "webhook-sender"
    }
}
