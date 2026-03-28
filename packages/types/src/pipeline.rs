//! PipelineMessage envelope types for the Life Engine data pipeline.
//!
//! All data flowing through the workflow engine is wrapped in a
//! `PipelineMessage` containing metadata (correlation, source, auth)
//! and a typed payload (either a canonical CDM type or a
//! schema-validated custom type).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use std::collections::HashMap;

use crate::workflow::WorkflowStatus;
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
    /// Soft warnings appended by pipeline steps.
    ///
    /// When an action succeeds but wants to signal non-fatal issues,
    /// it appends entries here. The pipeline executor surfaces these
    /// to the caller without failing the step.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
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

// ---------------------------------------------------------------------------
// New architecture types (pipeline-message spec)
// ---------------------------------------------------------------------------

/// Universal data envelope for the new pipeline architecture.
///
/// All data flowing through the workflow engine is wrapped in this struct.
/// The `payload` is a free-form JSON value that steps read and modify.
/// The `metadata` carries contextual information that the executor manages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PipelineEnvelope {
    /// The step's primary data — steps read, modify, or replace this.
    pub payload: serde_json::Value,
    /// Contextual information about the request, identity, and execution trace.
    pub metadata: PipelineMetadata,
}

/// Metadata propagated through every pipeline step.
///
/// Fields are divided into executor-owned (read-only to plugins) and
/// plugin-writable categories. The executor enforces this boundary via
/// snapshot-and-restore after each plugin invocation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PipelineMetadata {
    /// Unique identifier for this pipeline execution (UUID v4).
    pub request_id: String,
    /// How the pipeline was triggered: `"endpoint"`, `"event"`, or `"schedule"`.
    pub trigger_type: String,
    /// The authenticated caller, if any. `None` for unauthenticated triggers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity: Option<IdentitySummary>,
    /// Path parameters extracted by the transport handler.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub params: HashMap<String, String>,
    /// Query string parameters or flattened GraphQL arguments.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub query: HashMap<String, String>,
    /// Execution traces appended by the executor after each step.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub traces: Vec<StepTrace>,
    /// Optional status hint set by a plugin to influence the response status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_hint: Option<WorkflowStatus>,
    /// Non-fatal warnings appended by pipeline steps.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// Plugin-writable arbitrary key-value data.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Execution trace for a single pipeline step, appended by the executor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StepTrace {
    /// The plugin or step that executed.
    pub step_name: String,
    /// How long the step took in milliseconds.
    pub duration_ms: u64,
    /// Whether the step succeeded or failed.
    pub outcome: StepOutcome,
}

/// Outcome of a pipeline step execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StepOutcome {
    /// The step completed successfully.
    Success,
    /// The step failed with an error message.
    Error(String),
}

/// Minimal identity projection carried in pipeline metadata.
///
/// This is not the full `Identity` from auth middleware — just enough
/// for plugins to know who initiated the request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IdentitySummary {
    /// The authenticated user's identifier (user ID or email).
    pub subject: String,
    /// The identity provider that issued the token.
    pub issuer: String,
}

impl IdentitySummary {
    /// Create an `IdentitySummary` from a full `Identity`.
    pub fn from_identity(identity: &crate::identity::Identity) -> Self {
        Self {
            subject: identity.subject.clone(),
            issuer: identity.issuer.clone(),
        }
    }
}

impl PipelineMetadata {
    /// Create a new `PipelineMetadata` with the given request ID and trigger type.
    /// All other fields are set to their defaults.
    pub fn new(request_id: String, trigger_type: String) -> Self {
        Self {
            request_id,
            trigger_type,
            identity: None,
            params: HashMap::new(),
            query: HashMap::new(),
            traces: Vec::new(),
            status_hint: None,
            warnings: Vec::new(),
            extra: HashMap::new(),
        }
    }
}

impl PipelineEnvelope {
    /// Create a new `PipelineEnvelope` with the given payload and metadata.
    pub fn new(payload: serde_json::Value, metadata: PipelineMetadata) -> Self {
        Self { payload, metadata }
    }
}

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
                warnings: vec![],
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

    // -----------------------------------------------------------------------
    // PipelineEnvelope / PipelineMetadata tests
    // -----------------------------------------------------------------------

    #[test]
    fn pipeline_envelope_round_trip_all_fields() {
        let envelope = PipelineEnvelope {
            payload: serde_json::json!({
                "title": "Test task",
                "status": "pending",
                "nested": { "key": [1, 2, 3] }
            }),
            metadata: PipelineMetadata {
                request_id: Uuid::new_v4().to_string(),
                trigger_type: "endpoint".into(),
                identity: Some(IdentitySummary {
                    subject: "user-123".into(),
                    issuer: "life-engine".into(),
                }),
                params: HashMap::from([
                    ("collection".into(), "tasks".into()),
                    ("id".into(), "task-456".into()),
                ]),
                query: HashMap::from([
                    ("limit".into(), "10".into()),
                ]),
                traces: vec![
                    StepTrace {
                        step_name: "validate".into(),
                        duration_ms: 5,
                        outcome: StepOutcome::Success,
                    },
                    StepTrace {
                        step_name: "transform".into(),
                        duration_ms: 12,
                        outcome: StepOutcome::Error("plugin crashed".into()),
                    },
                ],
                status_hint: Some(crate::workflow::WorkflowStatus::Ok),
                warnings: vec!["deprecated field used".into()],
                extra: HashMap::from([
                    ("plugin_hint".into(), serde_json::json!("value")),
                ]),
            },
        };

        let json = serde_json::to_string(&envelope).expect("serialize");
        let restored: PipelineEnvelope =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(envelope, restored);
    }

    #[test]
    fn pipeline_envelope_empty_payload() {
        let envelope = PipelineEnvelope::new(
            serde_json::Value::Null,
            PipelineMetadata::new("req-1".into(), "schedule".into()),
        );
        let json = serde_json::to_string(&envelope).expect("serialize");
        let restored: PipelineEnvelope =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(envelope, restored);
        assert!(restored.payload.is_null());
    }

    #[test]
    fn pipeline_envelope_nested_payload() {
        let large_array: Vec<serde_json::Value> =
            (0..100).map(|i| serde_json::json!({ "index": i })).collect();
        let envelope = PipelineEnvelope::new(
            serde_json::json!({ "items": large_array }),
            PipelineMetadata::new("req-big".into(), "endpoint".into()),
        );
        let json = serde_json::to_string(&envelope).expect("serialize");
        let restored: PipelineEnvelope =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(envelope, restored);
    }

    #[test]
    fn pipeline_envelope_large_payload() {
        // 1MB+ JSON payload
        let big_string = "x".repeat(1_100_000);
        let envelope = PipelineEnvelope::new(
            serde_json::json!({ "data": big_string }),
            PipelineMetadata::new("req-large".into(), "event".into()),
        );
        let json = serde_json::to_string(&envelope).expect("serialize");
        let restored: PipelineEnvelope =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(envelope, restored);
    }

    #[test]
    fn step_trace_accumulation() {
        let mut metadata = PipelineMetadata::new("req-trace".into(), "endpoint".into());
        assert!(metadata.traces.is_empty());

        metadata.traces.push(StepTrace {
            step_name: "step-1".into(),
            duration_ms: 10,
            outcome: StepOutcome::Success,
        });
        metadata.traces.push(StepTrace {
            step_name: "step-2".into(),
            duration_ms: 20,
            outcome: StepOutcome::Success,
        });
        metadata.traces.push(StepTrace {
            step_name: "step-3".into(),
            duration_ms: 5,
            outcome: StepOutcome::Error("timeout".into()),
        });

        assert_eq!(metadata.traces.len(), 3);
        assert_eq!(metadata.traces[0].step_name, "step-1");
        assert_eq!(metadata.traces[2].outcome, StepOutcome::Error("timeout".into()));
    }

    #[test]
    fn identity_summary_from_identity() {
        let identity = crate::identity::Identity {
            subject: "user-abc".into(),
            issuer: "oidc-provider".into(),
            claims: HashMap::from([
                ("role".into(), serde_json::json!("admin")),
            ]),
        };
        let summary = IdentitySummary::from_identity(&identity);
        assert_eq!(summary.subject, "user-abc");
        assert_eq!(summary.issuer, "oidc-provider");
    }

    #[test]
    fn pipeline_metadata_defaults() {
        let meta = PipelineMetadata::new("req-1".into(), "endpoint".into());
        assert_eq!(meta.request_id, "req-1");
        assert_eq!(meta.trigger_type, "endpoint");
        assert!(meta.identity.is_none());
        assert!(meta.params.is_empty());
        assert!(meta.query.is_empty());
        assert!(meta.traces.is_empty());
        assert!(meta.status_hint.is_none());
        assert!(meta.warnings.is_empty());
        assert!(meta.extra.is_empty());
    }

    #[test]
    fn step_outcome_serialisation() {
        let success = StepOutcome::Success;
        let json = serde_json::to_string(&success).expect("serialize");
        let restored: StepOutcome = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(success, restored);

        let error = StepOutcome::Error("something broke".into());
        let json = serde_json::to_string(&error).expect("serialize");
        let restored: StepOutcome = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(error, restored);
    }
}
