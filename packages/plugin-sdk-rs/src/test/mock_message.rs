//! Mock PipelineMessage builder for plugin testing.
//!
//! Provides a fluent builder API for creating `PipelineMessage` instances
//! with sensible defaults, plus convenience constructors for each CDM type.

use chrono::Utc;
use life_engine_types::{
    CalendarEvent, CdmType, Contact, ContactName, Credential, CredentialType, Email, EmailAddress,
    FileMetadata, MessageMetadata, Note, NoteFormat, PipelineMessage, SchemaValidated, Task,
    TaskPriority, TaskStatus, TypedPayload,
};
use uuid::Uuid;

/// Builder for creating [`PipelineMessage`] instances with sensible defaults.
///
/// All fields have defaults: auto-generated UUID correlation ID, current
/// timestamp, `"test"` as source, and `None` as auth context.
///
/// # Example
///
/// ```rust,ignore
/// use life_engine_plugin_sdk::test::MockMessageBuilder;
///
/// let msg = MockMessageBuilder::note("My Note", "Note body").build();
/// let msg = MockMessageBuilder::task("Buy groceries").build();
/// ```
pub struct MockMessageBuilder {
    correlation_id: Uuid,
    source: String,
    auth_context: Option<serde_json::Value>,
    payload: TypedPayload,
}

impl MockMessageBuilder {
    /// Create a builder with the given typed payload.
    pub fn new(payload: TypedPayload) -> Self {
        Self {
            correlation_id: Uuid::new_v4(),
            source: "test".to_string(),
            auth_context: None,
            payload,
        }
    }

    /// Create a builder with a CDM payload.
    pub fn with_cdm(cdm: CdmType) -> Self {
        Self::new(TypedPayload::Cdm(Box::new(cdm)))
    }

    /// Create a builder with a schema-validated custom payload.
    pub fn with_custom(
        value: serde_json::Value,
        schema: &serde_json::Value,
    ) -> Result<Self, life_engine_types::SchemaValidationError> {
        let validated = SchemaValidated::new(value, schema)?;
        Ok(Self::new(TypedPayload::Custom(validated)))
    }

    /// Override the source field.
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }

    /// Override the correlation ID.
    pub fn with_correlation_id(mut self, id: Uuid) -> Self {
        self.correlation_id = id;
        self
    }

    /// Set the auth context.
    pub fn with_auth(mut self, auth: serde_json::Value) -> Self {
        self.auth_context = Some(auth);
        self
    }

    /// Build the final `PipelineMessage`.
    pub fn build(self) -> PipelineMessage {
        PipelineMessage {
            metadata: MessageMetadata {
                correlation_id: self.correlation_id,
                source: self.source,
                timestamp: Utc::now(),
                auth_context: self.auth_context,
                warnings: vec![],
            },
            payload: self.payload,
        }
    }

    // ── Convenience constructors for each CDM type ──

    /// Create a builder for a `CalendarEvent` with minimal required fields.
    pub fn event(title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self::with_cdm(CdmType::Event(CalendarEvent {
            id: Uuid::new_v4(),
            title: title.into(),
            start: now,
            end: None,
            description: None,
            location: None,
            all_day: None,
            recurrence: None,
            attendees: vec![],
            reminders: vec![],
            timezone: None,
            status: None,
            sequence: None,
            source: "test".to_string(),
            source_id: Uuid::new_v4().to_string(),
            extensions: None,
            created_at: now,
            updated_at: now,
        }))
    }

    /// Create a builder for a `Task` with minimal required fields.
    pub fn task(title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self::with_cdm(CdmType::Task(Task {
            id: Uuid::new_v4(),
            title: title.into(),
            description: None,
            status: TaskStatus::Pending,
            priority: TaskPriority::Medium,
            due_date: None,
            completed_at: None,
            tags: vec![],
            assignee: None,
            parent_id: None,
            source: "test".to_string(),
            source_id: Uuid::new_v4().to_string(),
            extensions: None,
            created_at: now,
            updated_at: now,
        }))
    }

    /// Create a builder for a `Contact` with minimal required fields.
    pub fn contact(given_name: impl Into<String>, family_name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self::with_cdm(CdmType::Contact(Contact {
            id: Uuid::new_v4(),
            name: ContactName {
                given: given_name.into(),
                family: family_name.into(),
                display: None,
                prefix: None,
                suffix: None,
                middle: None,
            },
            emails: vec![],
            phones: vec![],
            addresses: vec![],
            organization: None,
            title: None,
            birthday: None,
            photo_url: None,
            notes: None,
            groups: vec![],
            source: "test".to_string(),
            source_id: Uuid::new_v4().to_string(),
            extensions: None,
            created_at: now,
            updated_at: now,
        }))
    }

    /// Create a builder for a `Note` with minimal required fields.
    pub fn note(title: impl Into<String>, body: impl Into<String>) -> Self {
        let now = Utc::now();
        Self::with_cdm(CdmType::Note(Note {
            id: Uuid::new_v4(),
            title: title.into(),
            body: body.into(),
            tags: vec![],
            format: Some(NoteFormat::Plain),
            pinned: None,
            source: "test".to_string(),
            source_id: Uuid::new_v4().to_string(),
            extensions: None,
            created_at: now,
            updated_at: now,
        }))
    }

    /// Create a builder for an `Email` with minimal required fields.
    pub fn email(subject: impl Into<String>, from: impl Into<String>) -> Self {
        let now = Utc::now();
        Self::with_cdm(CdmType::Email(Email {
            id: Uuid::new_v4(),
            subject: subject.into(),
            from: EmailAddress {
                name: None,
                address: from.into(),
            },
            to: vec![],
            cc: vec![],
            bcc: vec![],
            body_text: None,
            body_html: None,
            date: now,
            message_id: None,
            in_reply_to: None,
            attachments: vec![],
            read: None,
            starred: None,
            labels: vec![],
            source: "test".to_string(),
            source_id: Uuid::new_v4().to_string(),
            extensions: None,
            created_at: now,
            updated_at: now,
        }))
    }

    /// Create a builder for a `FileMetadata` with minimal required fields.
    pub fn file(filename: impl Into<String>, mime_type: impl Into<String>) -> Self {
        let now = Utc::now();
        Self::with_cdm(CdmType::File(FileMetadata {
            id: Uuid::new_v4(),
            filename: filename.into(),
            path: "/test".to_string(),
            mime_type: mime_type.into(),
            size_bytes: 0,
            checksum: "sha256:0000".to_string(),
            storage_backend: None,
            source: "test".to_string(),
            source_id: Uuid::new_v4().to_string(),
            extensions: None,
            created_at: now,
            updated_at: now,
        }))
    }

    /// Create a builder for a `Credential` with minimal required fields.
    pub fn credential(
        name: impl Into<String>,
        service: impl Into<String>,
        credential_type: CredentialType,
    ) -> Self {
        let now = Utc::now();
        Self::with_cdm(CdmType::Credential(Credential {
            id: Uuid::new_v4(),
            name: name.into(),
            credential_type,
            service: service.into(),
            claims: serde_json::json!({}),
            encrypted: None,
            expires_at: None,
            source: "test".to_string(),
            source_id: Uuid::new_v4().to_string(),
            created_at: now,
            updated_at: now,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_builder_creates_valid_message() {
        let msg = MockMessageBuilder::note("Test Note", "Body text").build();
        assert_eq!(msg.metadata.source, "test");
        match &msg.payload {
            TypedPayload::Cdm(cdm) => match cdm.as_ref() {
                CdmType::Note(n) => {
                    assert_eq!(n.title, "Test Note");
                    assert_eq!(n.body, "Body text");
                }
                other => panic!("Expected Note, got {other:?}"),
            },
            _ => panic!("Expected Cdm payload"),
        }
    }

    #[test]
    fn task_builder_creates_valid_message() {
        let msg = MockMessageBuilder::task("Buy groceries").build();
        match &msg.payload {
            TypedPayload::Cdm(cdm) => match cdm.as_ref() {
                CdmType::Task(t) => {
                    assert_eq!(t.title, "Buy groceries");
                    assert_eq!(t.status, TaskStatus::Pending);
                }
                other => panic!("Expected Task, got {other:?}"),
            },
            _ => panic!("Expected Cdm payload"),
        }
    }

    #[test]
    fn event_builder_creates_valid_message() {
        let msg = MockMessageBuilder::event("Team Standup").build();
        match &msg.payload {
            TypedPayload::Cdm(cdm) => match cdm.as_ref() {
                CdmType::Event(e) => assert_eq!(e.title, "Team Standup"),
                other => panic!("Expected Event, got {other:?}"),
            },
            _ => panic!("Expected Cdm payload"),
        }
    }

    #[test]
    fn contact_builder_creates_valid_message() {
        let msg = MockMessageBuilder::contact("Jane", "Doe").build();
        match &msg.payload {
            TypedPayload::Cdm(cdm) => match cdm.as_ref() {
                CdmType::Contact(c) => {
                    assert_eq!(c.name.given, "Jane");
                    assert_eq!(c.name.family, "Doe");
                }
                other => panic!("Expected Contact, got {other:?}"),
            },
            _ => panic!("Expected Cdm payload"),
        }
    }

    #[test]
    fn email_builder_creates_valid_message() {
        let msg = MockMessageBuilder::email("Hello", "alice@example.com").build();
        match &msg.payload {
            TypedPayload::Cdm(cdm) => match cdm.as_ref() {
                CdmType::Email(e) => {
                    assert_eq!(e.subject, "Hello");
                    assert_eq!(e.from.address, "alice@example.com");
                }
                other => panic!("Expected Email, got {other:?}"),
            },
            _ => panic!("Expected Cdm payload"),
        }
    }

    #[test]
    fn file_builder_creates_valid_message() {
        let msg = MockMessageBuilder::file("doc.pdf", "application/pdf").build();
        match &msg.payload {
            TypedPayload::Cdm(cdm) => match cdm.as_ref() {
                CdmType::File(f) => {
                    assert_eq!(f.filename, "doc.pdf");
                    assert_eq!(f.mime_type, "application/pdf");
                }
                other => panic!("Expected File, got {other:?}"),
            },
            _ => panic!("Expected Cdm payload"),
        }
    }

    #[test]
    fn credential_builder_creates_valid_message() {
        let msg =
            MockMessageBuilder::credential("Gmail Token", "gmail", CredentialType::OauthToken)
                .build();
        match &msg.payload {
            TypedPayload::Cdm(cdm) => match cdm.as_ref() {
                CdmType::Credential(c) => {
                    assert_eq!(c.name, "Gmail Token");
                    assert_eq!(c.service, "gmail");
                    assert_eq!(c.credential_type, CredentialType::OauthToken);
                }
                other => panic!("Expected Credential, got {other:?}"),
            },
            _ => panic!("Expected Cdm payload"),
        }
    }

    #[test]
    fn builder_overrides_work() {
        let custom_id = Uuid::new_v4();
        let msg = MockMessageBuilder::note("Test", "Body")
            .with_source("custom-source")
            .with_correlation_id(custom_id)
            .with_auth(serde_json::json!({"user": "admin"}))
            .build();

        assert_eq!(msg.metadata.source, "custom-source");
        assert_eq!(msg.metadata.correlation_id, custom_id);
        assert!(msg.metadata.auth_context.is_some());
    }

    #[test]
    fn custom_payload_builder() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": { "name": { "type": "string" } },
            "required": ["name"]
        });
        let value = serde_json::json!({ "name": "test" });
        let msg = MockMessageBuilder::with_custom(value, &schema)
            .expect("validation should pass")
            .build();
        match &msg.payload {
            TypedPayload::Custom(_) => {}
            _ => panic!("Expected Custom payload"),
        }
    }
}
