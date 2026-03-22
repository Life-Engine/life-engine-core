//! Data models for webhook receiver configuration and events.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Configuration for a single webhook endpoint.
///
/// Each endpoint has a unique ID used in the receive URL:
/// `POST /api/plugins/com.life-engine.webhook-receiver/receive/{id}`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEndpoint {
    /// Unique identifier for this endpoint (used in URL path).
    pub id: String,
    /// Human-readable name of the source service (e.g., "GitHub", "Stripe").
    pub source_name: String,
    /// The CDM collection to store received data in.
    pub target_collection: String,
    /// Optional HMAC-SHA256 secret for signature verification.
    pub secret: Option<String>,
    /// Optional payload field mappings (JSON path -> CDM field).
    pub payload_mappings: Option<Vec<PayloadMapping>>,
}

/// Maps a JSON path in the incoming payload to a CDM field name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayloadMapping {
    /// Dot-separated path in the source JSON (e.g., "repository.full_name").
    pub source_path: String,
    /// Target field name in the output object.
    pub target_field: String,
}

/// A processed webhook event ready for storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookReceivedEvent {
    /// The endpoint that received this webhook.
    pub endpoint_id: String,
    /// The source service name.
    pub source: String,
    /// The target CDM collection.
    pub collection: String,
    /// The processed (mapped) payload data.
    pub data: serde_json::Value,
    /// When this event was received.
    pub timestamp: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webhook_endpoint_serialization_roundtrip() {
        let endpoint = WebhookEndpoint {
            id: "test-hook".to_string(),
            source_name: "TestService".to_string(),
            target_collection: "events".to_string(),
            secret: Some("my-secret".to_string()),
            payload_mappings: Some(vec![PayloadMapping {
                source_path: "data.id".to_string(),
                target_field: "external_id".to_string(),
            }]),
        };
        let json = serde_json::to_string(&endpoint).expect("serialize");
        let restored: WebhookEndpoint = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.id, "test-hook");
        assert_eq!(restored.source_name, "TestService");
        assert!(restored.secret.is_some());
        assert_eq!(restored.payload_mappings.unwrap().len(), 1);
    }

    #[test]
    fn webhook_endpoint_without_optional_fields() {
        let endpoint = WebhookEndpoint {
            id: "minimal".to_string(),
            source_name: "Source".to_string(),
            target_collection: "items".to_string(),
            secret: None,
            payload_mappings: None,
        };
        let json = serde_json::to_string(&endpoint).expect("serialize");
        let restored: WebhookEndpoint = serde_json::from_str(&json).expect("deserialize");
        assert!(restored.secret.is_none());
        assert!(restored.payload_mappings.is_none());
    }

    #[test]
    fn received_event_serialization_roundtrip() {
        let event = WebhookReceivedEvent {
            endpoint_id: "hook-1".to_string(),
            source: "GitHub".to_string(),
            collection: "webhooks".to_string(),
            data: serde_json::json!({"key": "value"}),
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&event).expect("serialize");
        let restored: WebhookReceivedEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.endpoint_id, "hook-1");
        assert_eq!(restored.source, "GitHub");
    }

    #[test]
    fn payload_mapping_serialization_roundtrip() {
        let mapping = PayloadMapping {
            source_path: "user.email".to_string(),
            target_field: "email".to_string(),
        };
        let json = serde_json::to_string(&mapping).expect("serialize");
        let restored: PayloadMapping = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.source_path, "user.email");
        assert_eq!(restored.target_field, "email");
    }
}
