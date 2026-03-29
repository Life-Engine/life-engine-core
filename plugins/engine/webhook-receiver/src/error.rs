use life_engine_plugin_sdk::prelude::*;
use thiserror::Error;

/// Errors specific to the webhook receiver plugin.
#[derive(Debug, Error)]
pub enum WebhookReceiverError {
    #[error("unknown webhook endpoint: {0}")]
    UnknownEndpoint(String),
    #[error("unknown action: {0}")]
    UnknownAction(String),
}

impl EngineError for WebhookReceiverError {
    fn code(&self) -> &str {
        match self {
            Self::UnknownEndpoint(_) => "WEBHOOK_RECV_001",
            Self::UnknownAction(_) => "WEBHOOK_RECV_002",
        }
    }

    fn severity(&self) -> Severity {
        match self {
            Self::UnknownEndpoint(_) => Severity::Fatal,
            Self::UnknownAction(_) => Severity::Fatal,
        }
    }

    fn source_module(&self) -> &str {
        "webhook-receiver"
    }
}
