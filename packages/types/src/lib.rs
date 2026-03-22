//! Shared type definitions for Life Engine canonical data models.
//!
//! This crate defines the Canonical Data Model (CDM) types used across
//! Core, App, and both plugin SDKs. These types are the single source
//! of truth for all data structures in the Life Engine ecosystem.

pub mod contacts;
pub mod credentials;
pub mod emails;
pub mod events;
pub mod file_helpers;
pub mod files;
pub mod notes;
pub mod tasks;

// Re-export all canonical types at crate root for convenience.
pub use contacts::{Contact, ContactName, EmailAddress, PhoneNumber, PostalAddress};
pub use credentials::{Credential, CredentialType};
pub use emails::{Email, EmailAttachment};
pub use events::CalendarEvent;
pub use files::FileMetadata;
pub use notes::Note;
pub use tasks::{Task, TaskPriority, TaskStatus};

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
            labels: vec!["test".into()],
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
            end: now + chrono::Duration::hours(1),
            recurrence: None,
            attendees: vec!["user@example.com".into()],
            location: Some("Room A".into()),
            description: None,
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
                display: "Jane Doe".into(),
            },
            emails: vec![EmailAddress {
                address: "jane@example.com".into(),
                email_type: Some("work".into()),
                primary: Some(true),
            }],
            phones: vec![],
            addresses: vec![],
            organisation: None,
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
            from: "sender@example.com".into(),
            to: vec!["recipient@example.com".into()],
            cc: vec![],
            bcc: vec![],
            subject: "Test email".into(),
            body_text: "Body content".into(),
            body_html: None,
            thread_id: None,
            labels: vec![],
            attachments: vec![],
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
            name: "test.txt".into(),
            mime_type: "text/plain".into(),
            size: 1024,
            path: "/test/test.txt".into(),
            checksum: Some("sha256:abc123".into()),
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
            credential_type: CredentialType::ApiKey,
            issuer: "https://auth.example.com".into(),
            issued_date: "2026-01-01".into(),
            expiry_date: Some("2027-01-01".into()),
            claims: serde_json::json!({"scope": "read"}),
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
