//! PipelineMessage envelope types for the Life Engine data pipeline.
//!
//! All data flowing through the workflow engine is wrapped in a
//! `PipelineMessage` containing metadata (correlation, source, auth)
//! and a typed payload (either a canonical CDM type or a
//! schema-validated custom type).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    CalendarEvent, Contact, Credential, Email, FileMetadata, Note, Task,
};

/// Envelope that wraps all data flowing through the pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineMessage {
    /// Routing and tracing metadata.
    pub metadata: MessageMetadata,
    /// The typed data payload.
    pub payload: TypedPayload,
}

/// Metadata propagated through every pipeline step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMetadata {
    /// Unique identifier for the originating request, propagated
    /// through all pipeline steps for distributed tracing.
    pub correlation_id: Uuid,
    /// Trigger type and value, e.g. `"endpoint:POST /email/sync"`.
    pub source: String,
    /// When this message entered the pipeline.
    pub timestamp: DateTime<Utc>,
    /// Authenticated identity from the auth module, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_context: Option<serde_json::Value>,
}

/// The data carried by a `PipelineMessage`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum TypedPayload {
    /// A canonical data model value.
    Cdm(Box<CdmType>),
    /// A plugin-defined type that has been validated against a JSON Schema.
    Custom(SchemaValidated<serde_json::Value>),
}

/// Discriminated union of all canonical collection types plus batch variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "collection", content = "value")]
pub enum CdmType {
    Event(CalendarEvent),
    Task(Task),
    Contact(Contact),
    Note(Note),
    Email(Email),
    File(FileMetadata),
    Credential(Credential),
    EventBatch(Vec<CalendarEvent>),
    TaskBatch(Vec<Task>),
    ContactBatch(Vec<Contact>),
    NoteBatch(Vec<Note>),
    EmailBatch(Vec<Email>),
    FileBatch(Vec<FileMetadata>),
    CredentialBatch(Vec<Credential>),
}

/// Newtype wrapper guaranteeing the inner value has been validated
/// against a JSON Schema before entering the pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SchemaValidated<T>(T);

impl<T> SchemaValidated<T> {
    /// Validate `value` against `schema` and wrap it on success.
    ///
    /// Returns an error if validation fails.
    pub fn new(value: T, schema: &serde_json::Value) -> Result<Self, SchemaValidationError>
    where
        T: Serialize,
    {
        let instance = serde_json::to_value(&value)
            .map_err(|e| SchemaValidationError(format!("serialization failed: {e}")))?;
        let validator = jsonschema::validator_for(schema)
            .map_err(|e| SchemaValidationError(format!("invalid schema: {e}")))?;
        if let Err(error) = validator.validate(&instance) {
            return Err(SchemaValidationError(error.to_string()));
        }
        Ok(Self(value))
    }

    /// Access the inner value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> std::ops::Deref for SchemaValidated<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

/// Error returned when schema validation fails.
#[derive(Debug, Clone)]
pub struct SchemaValidationError(pub String);

impl std::fmt::Display for SchemaValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "schema validation failed: {}", self.0)
    }
}

impl std::error::Error for SchemaValidationError {}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn pipeline_message_round_trip() {
        let msg = PipelineMessage {
            metadata: MessageMetadata {
                correlation_id: Uuid::new_v4(),
                source: "endpoint:POST /tasks".into(),
                timestamp: Utc::now(),
                auth_context: None,
            },
            payload: TypedPayload::Cdm(Box::new(CdmType::Task(Task {
                id: Uuid::new_v4(),
                title: "Test".into(),
                description: None,
                status: crate::TaskStatus::Pending,
                priority: crate::TaskPriority::Medium,
                due_date: None,
                completed_at: None,
                tags: vec![],
                assignee: None,
                parent_id: None,
                source: "test".into(),
                source_id: "t-1".into(),
                extensions: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }))),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let restored: PipelineMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.metadata.correlation_id, msg.metadata.correlation_id);
    }

    #[test]
    fn schema_validated_accepts_valid_value() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": { "name": { "type": "string" } },
            "required": ["name"]
        });
        let value = serde_json::json!({ "name": "test" });
        let validated = SchemaValidated::new(value, &schema).expect("should validate");
        assert_eq!(validated["name"], "test");
    }

    #[test]
    fn schema_validated_rejects_invalid_value() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": { "name": { "type": "string" } },
            "required": ["name"]
        });
        let value = serde_json::json!({ "age": 30 });
        let result = SchemaValidated::new(value, &schema);
        assert!(result.is_err());
    }

    #[test]
    fn schema_validated_deref() {
        let schema = serde_json::json!({ "type": "number" });
        let validated = SchemaValidated::new(serde_json::json!(42), &schema).unwrap();
        assert_eq!(*validated, serde_json::json!(42));
    }

    #[test]
    fn cdm_type_batch_round_trip() {
        let batch = CdmType::TaskBatch(vec![]);
        let json = serde_json::to_string(&batch).expect("serialize");
        let restored: CdmType = serde_json::from_str(&json).expect("deserialize");
        matches!(restored, CdmType::TaskBatch(v) if v.is_empty());
    }
}
