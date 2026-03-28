//! Shared type definitions for Life Engine canonical data models.
//!
//! This crate defines the Canonical Data Model (CDM) types used across
//! Core, App, and both plugin SDKs. These types are the single source
//! of truth for all data structures in the Life Engine ecosystem.

pub mod contacts;
pub mod credentials;
pub mod emails;
pub mod events;
pub mod extensions;
pub mod file_helpers;
pub mod files;
pub mod identity;
pub mod migrations;
pub mod notes;
pub mod pipeline;
pub mod storage;
pub mod tasks;
pub mod trigger;
pub mod workflow;

// Re-export all canonical types at crate root for convenience.
pub use contacts::{
    Contact, ContactAddress, ContactEmail, ContactInfoType, ContactName, ContactPhone, PhoneType,
};
pub use credentials::{Credential, CredentialType};
pub use emails::{Email, EmailAddress, EmailAttachment};
pub use events::{
    Attendee, AttendeeStatus, CalendarEvent, EventStatus, Recurrence, RecurrenceFrequency,
    Reminder, ReminderMethod,
};
pub use extensions::{get_ext, set_ext, validate_extension_namespace, ExtensionError};
pub use files::FileMetadata;
pub use notes::{Note, NoteFormat};
pub use pipeline::{
    CdmType, IdentitySummary, MessageMetadata, PipelineEnvelope, PipelineMessage,
    PipelineMetadata, SchemaValidated, SchemaValidationError, StepOutcome, StepTrace,
    TypedPayload,
};
pub use storage::{
    FilterOp, QueryFilter, SortDirection, SortField, StorageMutation, StorageQuery,
};
pub use tasks::{Task, TaskPriority, TaskStatus};
pub use identity::Identity;
pub use trigger::TriggerContext;
pub use workflow::{RequestMeta, ResponseMeta, WorkflowError, WorkflowRequest, WorkflowResponse, WorkflowStatus};

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn test_task_round_trip() {
        let now = Utc::now();
        let original = Task {
            id: Uuid::new_v4(),
            title: "Test task".into(),
            description: Some("A task for testing".into()),
            status: TaskStatus::Pending,
            priority: TaskPriority::High,
            due_date: Some(now),
            completed_at: None,
            tags: vec!["test".into()],
            assignee: None,
            parent_id: None,
            source: "test".into(),
            source_id: "task-001".into(),
            extensions: None,
            created_at: now,
            updated_at: now,
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: Task = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, restored);
    }

    #[test]
    fn test_event_round_trip() {
        let now = Utc::now();
        let original = CalendarEvent {
            id: Uuid::new_v4(),
            title: "Test event".into(),
            start: now,
            end: Some(now + chrono::Duration::hours(1)),
            description: None,
            location: Some("Room A".into()),
            all_day: None,
            recurrence: None,
            attendees: vec![Attendee::from_email("user@example.com")],
            reminders: vec![],
            timezone: None,
            status: None,
            sequence: None,
            source: "test".into(),
            source_id: "event-001".into(),
            extensions: None,
            created_at: now,
            updated_at: now,
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: CalendarEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, restored);
    }

    #[test]
    fn test_contact_round_trip() {
        let now = Utc::now();
        let original = Contact {
            id: Uuid::new_v4(),
            name: ContactName {
                given: "Jane".into(),
                family: "Doe".into(),
                prefix: None,
                suffix: None,
                middle: None,
            },
            emails: vec![ContactEmail {
                address: "jane@example.com".into(),
                email_type: Some(ContactInfoType::Work),
                primary: Some(true),
            }],
            phones: vec![],
            addresses: vec![],
            organization: None,
            title: None,
            birthday: None,
            photo_url: None,
            notes: None,
            groups: vec![],
            source: "test".into(),
            source_id: "contact-001".into(),
            extensions: None,
            created_at: now,
            updated_at: now,
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: Contact = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, restored);
    }

    #[test]
    fn test_email_round_trip() {
        let now = Utc::now();
        let original = Email {
            id: Uuid::new_v4(),
            subject: "Test email".into(),
            from: EmailAddress {
                name: Some("Sender".into()),
                address: "sender@example.com".into(),
            },
            to: vec![EmailAddress {
                name: None,
                address: "recipient@example.com".into(),
            }],
            cc: vec![],
            bcc: vec![],
            body_text: Some("Body content".into()),
            body_html: None,
            date: now,
            message_id: None,
            in_reply_to: None,
            attachments: vec![],
            read: Some(false),
            starred: None,
            labels: vec![],
            source: "test".into(),
            source_id: "email-001".into(),
            extensions: None,
            created_at: now,
            updated_at: now,
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: Email = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, restored);
    }

    #[test]
    fn test_file_round_trip() {
        let now = Utc::now();
        let original = FileMetadata {
            id: Uuid::new_v4(),
            filename: "test.txt".into(),
            path: "/test/test.txt".into(),
            mime_type: "text/plain".into(),
            size_bytes: 1024,
            checksum: "a".repeat(64),
            storage_backend: None,
            source: "test".into(),
            source_id: "file-001".into(),
            extensions: None,
            created_at: now,
            updated_at: now,
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: FileMetadata = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, restored);
    }

    #[test]
    fn test_note_round_trip() {
        let now = Utc::now();
        let original = Note {
            id: Uuid::new_v4(),
            title: "Test note".into(),
            body: "Note body content".into(),
            tags: vec!["test".into()],
            format: Some(NoteFormat::Markdown),
            pinned: Some(true),
            source: "test".into(),
            source_id: "note-001".into(),
            extensions: None,
            created_at: now,
            updated_at: now,
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: Note = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, restored);
    }

    #[test]
    fn test_credential_round_trip() {
        let now = Utc::now();
        let original = Credential {
            id: Uuid::new_v4(),
            name: "Test API Key".into(),
            credential_type: CredentialType::ApiKey,
            service: "api.example.com".into(),
            claims: serde_json::json!({"scope": "read"}),
            encrypted: Some(false),
            expires_at: Some(now),
            source: "test".into(),
            source_id: "cred-001".into(),
            created_at: now,
            updated_at: now,
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: Credential = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, restored);
    }
}
