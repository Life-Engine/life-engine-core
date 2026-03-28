//! Data models for webhook sender configuration and delivery tracking.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use url::Url;

/// A webhook subscription that defines where to send events.
#[derive(Clone, Serialize, Deserialize)]
pub struct WebhookSubscription {
    /// Unique identifier for this subscription.
    pub id: String,
    /// The target URL to POST webhook payloads to.
    pub url: String,
    /// Event types this subscription listens for (e.g., "record.created").
    pub event_types: Vec<String>,
    /// Optional secret for HMAC-SHA256 signing of outgoing payloads.
    #[serde(skip_serializing)]
    pub secret: Option<String>,
    /// Whether this subscription is active.
    pub active: bool,
}

impl WebhookSubscription {
    /// Validate the subscription configuration, returning an error if the URL
    /// is malformed or uses an unsupported scheme.
    pub fn validate(&self) -> anyhow::Result<()> {
        let parsed = Url::parse(&self.url)
            .map_err(|e| anyhow::anyhow!("invalid webhook URL '{}': {}", self.url, e))?;
        match parsed.scheme() {
            "http" | "https" => {}
            scheme => anyhow::bail!(
                "unsupported URL scheme '{}' in webhook URL '{}': only http and https are allowed",
                scheme,
                self.url
            ),
        }
        Ok(())
    }
}

impl std::fmt::Debug for WebhookSubscription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebhookSubscription")
            .field("id", &self.id)
            .field("url", &self.url)
            .field("event_types", &self.event_types)
            .field(
                "secret",
                &self.secret.as_ref().map(|_| "[REDACTED]"),
            )
            .field("active", &self.active)
            .finish()
    }
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
    /// SHA-256 hash of the payload that was sent (hex-encoded).
    /// Avoids storing full payload copies across retries.
    pub payload_hash: String,
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
    /// Compute a hex-encoded SHA-256 hash of a JSON payload.
    pub fn hash_payload(payload: &serde_json::Value) -> String {
        use sha2::{Digest, Sha256};
        let bytes = serde_json::to_vec(payload).unwrap_or_default();
        let digest = Sha256::digest(&bytes);
        hex::encode(digest)
    }

    /// Create a new successful delivery record.
    pub fn success(
        id: String,
        subscription_id: String,
        event_type: String,
        payload: &serde_json::Value,
        status_code: u16,
        attempt: u32,
    ) -> Self {
        Self {
            id,
            subscription_id,
            event_type,
            payload_hash: Self::hash_payload(payload),
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
        payload: &serde_json::Value,
        status_code: u16,
        attempt: u32,
        error: String,
    ) -> Self {
        Self {
            id,
            subscription_id,
            event_type,
            payload_hash: Self::hash_payload(payload),
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
    fn subscription_serialization_skips_secret() {
        let sub = WebhookSubscription {
            id: "sub-1".to_string(),
            url: "https://example.com/webhook".to_string(),
            event_types: vec!["record.created".to_string(), "sync.complete".to_string()],
            secret: Some("shared-secret".to_string()),
            active: true,
        };
        let json = serde_json::to_string(&sub).expect("serialize");
        assert!(!json.contains("shared-secret"), "secret must not appear in serialized output");
        // Deserialize with secret provided externally.
        let json_with_secret = r#"{"id":"sub-1","url":"https://example.com/webhook","event_types":["record.created","sync.complete"],"secret":"shared-secret","active":true}"#;
        let restored: WebhookSubscription = serde_json::from_str(json_with_secret).expect("deserialize");
        assert_eq!(restored.id, "sub-1");
        assert_eq!(restored.event_types.len(), 2);
        assert_eq!(restored.secret.as_deref(), Some("shared-secret"));
        assert!(restored.active);
    }

    #[test]
    fn subscription_debug_redacts_secret() {
        let sub = WebhookSubscription {
            id: "sub-1".to_string(),
            url: "https://example.com/webhook".to_string(),
            event_types: vec!["record.created".to_string()],
            secret: Some("super-secret-value".to_string()),
            active: true,
        };
        let debug_output = format!("{:?}", sub);
        assert!(!debug_output.contains("super-secret-value"));
        assert!(debug_output.contains("[REDACTED]"));
    }

    #[test]
    fn delivery_record_success() {
        let payload = serde_json::json!({"id": "123"});
        let record = DeliveryRecord::success(
            "del-1".to_string(),
            "sub-1".to_string(),
            "record.created".to_string(),
            &payload,
            200,
            1,
        );
        assert!(record.success);
        assert_eq!(record.status_code, 200);
        assert!(record.error.is_none());
        assert_eq!(record.attempt, 1);
        assert_eq!(record.payload_hash, DeliveryRecord::hash_payload(&payload));
    }

    #[test]
    fn delivery_record_failure() {
        let record = DeliveryRecord::failure(
            "del-2".to_string(),
            "sub-1".to_string(),
            "record.created".to_string(),
            &serde_json::json!({"id": "456"}),
            500,
            3,
            "Internal Server Error".to_string(),
        );
        assert!(!record.success);
        assert_eq!(record.status_code, 500);
        assert_eq!(record.error.as_deref(), Some("Internal Server Error"));
        assert_eq!(record.attempt, 3);
        assert!(!record.payload_hash.is_empty());
    }

    #[test]
    fn delivery_record_serialization_roundtrip() {
        let record = DeliveryRecord::success(
            "del-1".to_string(),
            "sub-1".to_string(),
            "sync.complete".to_string(),
            &serde_json::json!({}),
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

    #[test]
    fn validate_accepts_https_url() {
        let sub = WebhookSubscription {
            id: "s".into(),
            url: "https://example.com/webhook".into(),
            event_types: vec![],
            secret: None,
            active: true,
        };
        assert!(sub.validate().is_ok());
    }

    #[test]
    fn validate_accepts_http_url() {
        let sub = WebhookSubscription {
            id: "s".into(),
            url: "http://localhost:8080/hook".into(),
            event_types: vec![],
            secret: None,
            active: true,
        };
        assert!(sub.validate().is_ok());
    }

    #[test]
    fn validate_rejects_malformed_url() {
        let sub = WebhookSubscription {
            id: "s".into(),
            url: "not a url".into(),
            event_types: vec![],
            secret: None,
            active: true,
        };
        assert!(sub.validate().is_err());
    }

    #[test]
    fn validate_rejects_unsupported_scheme() {
        let sub = WebhookSubscription {
            id: "s".into(),
            url: "ftp://example.com/file".into(),
            event_types: vec![],
            secret: None,
            active: true,
        };
        let err = sub.validate().unwrap_err();
        assert!(err.to_string().contains("unsupported URL scheme"));
    }

    #[test]
    fn payload_hash_is_deterministic() {
        let payload = serde_json::json!({"key": "value"});
        let h1 = DeliveryRecord::hash_payload(&payload);
        let h2 = DeliveryRecord::hash_payload(&payload);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // SHA-256 hex
    }
}
