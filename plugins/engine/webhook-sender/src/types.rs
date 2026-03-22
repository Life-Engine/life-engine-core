use serde::{Deserialize, Serialize};

/// Summary of webhook sender status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookSenderStatus {
    /// Number of active subscriptions.
    pub active_subscriptions: usize,
    /// Total deliveries attempted.
    pub total_deliveries: usize,
    /// Total successful deliveries.
    pub successful_deliveries: usize,
    /// Total failed deliveries.
    pub failed_deliveries: usize,
}
