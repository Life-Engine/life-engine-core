//! Data models for webhook sender configuration and delivery tracking.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A webhook subscription that defines where to send events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookSubscription {
    /// Unique identifier for this subscription.
    pub id: String,
    /// The target URL to POST webhook payloads to.
    pub url: String,
    /// Event types this subscription listens for (e.g., "record.created").
    pub event_types: Vec<String>,
    /// Optional secret for HMAC-SHA256 signing of outgoing payloads.
    pub secret: Option<String>,
    /// Whether this subscription is active.
    pub active: bool,
}

/// A record of a single webhook delivery attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryRecord {
    /// Unique identifier for this delivery attempt.
    pub id: String,
    /// The subscription this delivery belongs to.
    pub subscription_id: String,
    /// The event type that triggered this delivery.
    pub event_type: String,
    /// The payload that was sent.
    pub payload: serde_json::Value,
    /// HTTP status code received (0 if request failed).
    pub status_code: u16,
    /// Whether the delivery was successful (2xx status code).
    pub success: bool,
    /// Number of attempts made (including retries).
    pub attempt: u32,
    /// Error message if the delivery failed.
    pub error: Option<String>,
    /// When this delivery was attempted.
    pub timestamp: DateTime<Utc>,
}

/// Status of a delivery attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeliveryStatus {
    /// Delivery succeeded (2xx response).
    Success,
    /// Delivery failed and will be retried.
    Failed,
    /// Delivery failed after all retry attempts.
    Exhausted,
}

impl DeliveryRecord {
    /// Create a new successful delivery record.
    pub fn success(
        id: String,
        subscription_id: String,
        event_type: String,
        payload: serde_json::Value,
        status_code: u16,
        attempt: u32,
    ) -> Self {
        Self {
            id,
            subscription_id,
            event_type,
            payload,
            status_code,
            success: true,
            attempt,
            error: None,
            timestamp: Utc::now(),
        }
    }

    /// Create a new failed delivery record.
    pub fn failure(
        id: String,
        subscription_id: String,
        event_type: String,
        payload: serde_json::Value,
        status_code: u16,
        attempt: u32,
        error: String,
    ) -> Self {
        Self {
            id,
            subscription_id,
            event_type,
            payload,
            status_code,
            success: false,
            attempt,
            error: Some(error),
            timestamp: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscription_serialization_roundtrip() {
        let sub = WebhookSubscription {
            id: "sub-1".to_string(),
            url: "https://example.com/webhook".to_string(),
            event_types: vec!["record.created".to_string(), "sync.complete".to_string()],
            secret: Some("shared-secret".to_string()),
            active: true,
        };
        let json = serde_json::to_string(&sub).expect("serialize");
        let restored: WebhookSubscription = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.id, "sub-1");
        assert_eq!(restored.event_types.len(), 2);
        assert!(restored.active);
    }

    #[test]
    fn delivery_record_success() {
        let record = DeliveryRecord::success(
            "del-1".to_string(),
            "sub-1".to_string(),
            "record.created".to_string(),
            serde_json::json!({"id": "123"}),
            200,
            1,
        );
        assert!(record.success);
        assert_eq!(record.status_code, 200);
        assert!(record.error.is_none());
        assert_eq!(record.attempt, 1);
    }

    #[test]
    fn delivery_record_failure() {
        let record = DeliveryRecord::failure(
            "del-2".to_string(),
            "sub-1".to_string(),
            "record.created".to_string(),
            serde_json::json!({"id": "456"}),
            500,
            3,
            "Internal Server Error".to_string(),
        );
        assert!(!record.success);
        assert_eq!(record.status_code, 500);
        assert_eq!(record.error.as_deref(), Some("Internal Server Error"));
        assert_eq!(record.attempt, 3);
    }

    #[test]
    fn delivery_record_serialization_roundtrip() {
        let record = DeliveryRecord::success(
            "del-1".to_string(),
            "sub-1".to_string(),
            "sync.complete".to_string(),
            serde_json::json!({}),
            201,
            1,
        );
        let json = serde_json::to_string(&record).expect("serialize");
        let restored: DeliveryRecord = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.id, "del-1");
        assert_eq!(restored.status_code, 201);
        assert!(restored.success);
    }

    #[test]
    fn delivery_status_serialization() {
        let status = DeliveryStatus::Failed;
        let json = serde_json::to_string(&status).expect("serialize");
        let restored: DeliveryStatus = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, DeliveryStatus::Failed);
    }

    #[test]
    fn subscription_without_secret() {
        let sub = WebhookSubscription {
            id: "sub-2".to_string(),
            url: "https://example.com/hook".to_string(),
            event_types: vec!["record.created".to_string()],
            secret: None,
            active: true,
        };
        let json = serde_json::to_string(&sub).expect("serialize");
        let restored: WebhookSubscription = serde_json::from_str(&json).expect("deserialize");
        assert!(restored.secret.is_none());
    }
}
