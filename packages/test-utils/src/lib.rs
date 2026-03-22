//! Shared test utilities for Life Engine.
//!
//! Provides factory functions, test fixtures, common assertions,
//! plugin test helpers, assertion macros, connector test helpers,
//! and Docker service utilities used across all Life Engine test suites.

pub mod assert_macros;
pub mod connectors;
pub mod docker;
pub mod plugin_test_helpers;

pub use life_engine_types;

use chrono::Utc;
use life_engine_types::{
    CalendarEvent, Contact, ContactName, Credential, CredentialType, Email, EmailAddress,
    FileMetadata, Note, PhoneNumber, PostalAddress, Task, TaskPriority, TaskStatus,
};
use uuid::Uuid;

/// Create a test `Task` with realistic defaults.
pub fn create_test_task() -> Task {
    let now = Utc::now();
    Task {
        id: Uuid::new_v4(),
        title: "Review pull request #42".into(),
        description: Some("Review the authentication refactor PR".into()),
        status: TaskStatus::Pending,
        priority: TaskPriority::Medium,
        due_date: Some(now + chrono::Duration::days(3)),
        labels: vec!["review".into(), "auth".into()],
        source: "test".into(),
        source_id: "test-task-001".into(),
        extensions: None,
        created_at: now,
        updated_at: now,
    }
}

/// Create a test `CalendarEvent` with realistic defaults.
pub fn create_test_event() -> CalendarEvent {
    let now = Utc::now();
    CalendarEvent {
        id: Uuid::new_v4(),
        title: "Weekly standup".into(),
        start: now + chrono::Duration::hours(1),
        end: now + chrono::Duration::hours(2),
        recurrence: Some("FREQ=WEEKLY;BYDAY=MO".into()),
        attendees: vec!["alice@example.com".into(), "bob@example.com".into()],
        location: Some("Conference Room A".into()),
        description: Some("Weekly team sync-up".into()),
        source: "test".into(),
        source_id: "test-event-001".into(),
        extensions: None,
        created_at: now,
        updated_at: now,
    }
}

/// Create a test `Contact` with realistic defaults.
pub fn create_test_contact() -> Contact {
    let now = Utc::now();
    Contact {
        id: Uuid::new_v4(),
        name: ContactName {
            given: "Alice".into(),
            family: "Johnson".into(),
            display: "Alice Johnson".into(),
        },
        emails: vec![EmailAddress {
            address: "alice@example.com".into(),
            email_type: Some("work".into()),
            primary: Some(true),
        }],
        phones: vec![PhoneNumber {
            number: "+61 400 123 456".into(),
            phone_type: Some("mobile".into()),
        }],
        addresses: vec![PostalAddress {
            street: Some("123 Main St".into()),
            city: Some("Sydney".into()),
            state: Some("NSW".into()),
            postcode: Some("2000".into()),
            country: Some("Australia".into()),
        }],
        organisation: Some("Acme Corp".into()),
        source: "test".into(),
        source_id: "test-contact-001".into(),
        extensions: None,
        created_at: now,
        updated_at: now,
    }
}

/// Create a test `Email` with realistic defaults.
pub fn create_test_email() -> Email {
    let now = Utc::now();
    Email {
        id: Uuid::new_v4(),
        from: "sender@example.com".into(),
        to: vec!["recipient@example.com".into()],
        cc: vec!["cc@example.com".into()],
        bcc: vec![],
        subject: "Project update for Q1".into(),
        body_text: "Please find the quarterly update attached.".into(),
        body_html: Some("<p>Please find the quarterly update attached.</p>".into()),
        thread_id: Some("thread-abc-123".into()),
        labels: vec!["inbox".into(), "important".into()],
        attachments: vec![],
        source: "test".into(),
        source_id: "test-email-001".into(),
        extensions: None,
        created_at: now,
        updated_at: now,
    }
}

/// Create a test `FileMetadata` with realistic defaults.
pub fn create_test_file() -> FileMetadata {
    let now = Utc::now();
    FileMetadata {
        id: Uuid::new_v4(),
        name: "quarterly-report.pdf".into(),
        mime_type: "application/pdf".into(),
        size: 245_760,
        path: "/documents/reports/quarterly-report.pdf".into(),
        checksum: Some("sha256:e3b0c44298fc1c149afbf4c8996fb924".into()),
        source: "test".into(),
        source_id: "test-file-001".into(),
        extensions: None,
        created_at: now,
        updated_at: now,
    }
}

/// Create a test `Note` with realistic defaults.
pub fn create_test_note() -> Note {
    let now = Utc::now();
    Note {
        id: Uuid::new_v4(),
        title: "Meeting notes — Architecture review".into(),
        body: "Discussed plugin sandboxing approach. Decided on Extism for WASM runtime.".into(),
        tags: vec!["meeting".into(), "architecture".into()],
        source: "test".into(),
        source_id: "test-note-001".into(),
        extensions: None,
        created_at: now,
        updated_at: now,
    }
}

/// Create a test `Credential` with realistic defaults.
pub fn create_test_credential() -> Credential {
    let now = Utc::now();
    Credential {
        id: Uuid::new_v4(),
        credential_type: CredentialType::OauthToken,
        issuer: "https://auth.example.com".into(),
        issued_date: chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
        expiry_date: Some(chrono::NaiveDate::from_ymd_opt(2027, 1, 15).unwrap()),
        claims: serde_json::json!({
            "scope": "read write",
            "sub": "user-12345"
        }),
        source: "test".into(),
        source_id: "test-cred-001".into(),
        extensions: None,
        created_at: now,
        updated_at: now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_test_task() {
        let task = create_test_task();
        assert!(!task.id.is_nil());
        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.priority, TaskPriority::Medium);
        assert!(task.due_date.is_some());
        let json = serde_json::to_string(&task);
        assert!(json.is_ok(), "Task should serialize to JSON");
    }

    #[test]
    fn test_create_test_event() {
        let event = create_test_event();
        assert!(!event.id.is_nil());
        assert!(event.start < event.end);
        assert_eq!(event.attendees.len(), 2);
        let json = serde_json::to_string(&event);
        assert!(json.is_ok(), "CalendarEvent should serialize to JSON");
    }

    #[test]
    fn test_create_test_contact() {
        let contact = create_test_contact();
        assert!(!contact.id.is_nil());
        assert_eq!(contact.name.given, "Alice");
        assert_eq!(contact.emails.len(), 1);
        assert_eq!(contact.phones.len(), 1);
        assert_eq!(contact.addresses.len(), 1);
        let json = serde_json::to_string(&contact);
        assert!(json.is_ok(), "Contact should serialize to JSON");
    }

    #[test]
    fn test_create_test_email() {
        let email = create_test_email();
        assert!(!email.id.is_nil());
        assert_eq!(email.to.len(), 1);
        assert_eq!(email.cc.len(), 1);
        assert!(email.bcc.is_empty());
        let json = serde_json::to_string(&email);
        assert!(json.is_ok(), "Email should serialize to JSON");
    }

    #[test]
    fn test_create_test_file() {
        let file = create_test_file();
        assert!(!file.id.is_nil());
        assert_eq!(file.mime_type, "application/pdf");
        assert!(file.size > 0);
        assert!(file.checksum.is_some());
        let json = serde_json::to_string(&file);
        assert!(json.is_ok(), "FileMetadata should serialize to JSON");
    }

    #[test]
    fn test_create_test_note() {
        let note = create_test_note();
        assert!(!note.id.is_nil());
        assert_eq!(note.tags.len(), 2);
        assert!(!note.body.is_empty());
        let json = serde_json::to_string(&note);
        assert!(json.is_ok(), "Note should serialize to JSON");
    }

    #[test]
    fn test_create_test_credential() {
        let cred = create_test_credential();
        assert!(!cred.id.is_nil());
        assert_eq!(cred.credential_type, CredentialType::OauthToken);
        assert!(cred.expiry_date.is_some());
        let json = serde_json::to_string(&cred);
        assert!(json.is_ok(), "Credential should serialize to JSON");
    }

    #[test]
    fn test_task_round_trip() {
        let original = create_test_task();
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: Task = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, restored);
    }

    #[test]
    fn test_event_round_trip() {
        let original = create_test_event();
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: CalendarEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, restored);
    }

    #[test]
    fn test_contact_round_trip() {
        let original = create_test_contact();
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: Contact = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, restored);
    }

    #[test]
    fn test_email_round_trip() {
        let original = create_test_email();
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: Email = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, restored);
    }

    #[test]
    fn test_file_round_trip() {
        let original = create_test_file();
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: FileMetadata = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, restored);
    }

    #[test]
    fn test_note_round_trip() {
        let original = create_test_note();
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: Note = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, restored);
    }

    #[test]
    fn test_credential_round_trip() {
        let original = create_test_credential();
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: Credential = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, restored);
    }
}
