//! Validation tests for all 7 CDM types.
//!
//! Each module covers: required field rejection, optional field defaults,
//! enum serialization/deserialization, skip_serializing_if behaviour,
//! serde rename behaviour, and unknown field acceptance.

use serde_json::json;

const TEST_UUID: &str = "00000000-0000-0000-0000-000000000001";
const TEST_TIMESTAMP: &str = "2026-01-01T00:00:00Z";

// ---------------------------------------------------------------------------
// Task validation
// ---------------------------------------------------------------------------
mod task_validation {
    use super::*;
    use life_engine_types::{Task, TaskPriority, TaskStatus};

    fn sample_task() -> serde_json::Value {
        json!({
            "id": TEST_UUID,
            "title": "Buy groceries",
            "status": "pending",
            "priority": "high",
            "source": "test",
            "source_id": "task-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        })
    }

    // --- (a) Required field rejection ---

    #[test]
    fn task_missing_title_is_rejected() {
        let mut v = sample_task();
        v.as_object_mut().unwrap().remove("title");
        let result = serde_json::from_value::<Task>(v);
        assert!(result.is_err());
    }

    #[test]
    fn task_missing_status_is_rejected() {
        let mut v = sample_task();
        v.as_object_mut().unwrap().remove("status");
        let result = serde_json::from_value::<Task>(v);
        assert!(result.is_err());
    }

    #[test]
    fn task_missing_priority_is_rejected() {
        let mut v = sample_task();
        v.as_object_mut().unwrap().remove("priority");
        let result = serde_json::from_value::<Task>(v);
        assert!(result.is_err());
    }

    // --- (b) Optional field defaults ---

    #[test]
    fn task_without_optional_fields_deserializes_with_defaults() {
        let v = sample_task();
        let task: Task = serde_json::from_value(v).unwrap();
        assert_eq!(task.description, None);
        assert_eq!(task.due_date, None);
        assert!(task.labels.is_empty());
        assert_eq!(task.extensions, None);
    }

    // --- (c) Enum variant serialization ---

    #[test]
    fn task_status_serializes_to_lowercase() {
        assert_eq!(serde_json::to_value(TaskStatus::Pending).unwrap(), "pending");
        assert_eq!(serde_json::to_value(TaskStatus::Active).unwrap(), "active");
        assert_eq!(
            serde_json::to_value(TaskStatus::Completed).unwrap(),
            "completed"
        );
        assert_eq!(
            serde_json::to_value(TaskStatus::Cancelled).unwrap(),
            "cancelled"
        );
    }

    #[test]
    fn task_priority_serializes_to_lowercase() {
        assert_eq!(serde_json::to_value(TaskPriority::None).unwrap(), "none");
        assert_eq!(serde_json::to_value(TaskPriority::Low).unwrap(), "low");
        assert_eq!(
            serde_json::to_value(TaskPriority::Medium).unwrap(),
            "medium"
        );
        assert_eq!(serde_json::to_value(TaskPriority::High).unwrap(), "high");
        assert_eq!(
            serde_json::to_value(TaskPriority::Critical).unwrap(),
            "critical"
        );
    }

    // --- (d) Enum variant deserialization ---

    #[test]
    fn task_status_deserializes_from_lowercase() {
        assert_eq!(
            serde_json::from_value::<TaskStatus>(json!("pending")).unwrap(),
            TaskStatus::Pending
        );
        assert_eq!(
            serde_json::from_value::<TaskStatus>(json!("active")).unwrap(),
            TaskStatus::Active
        );
        assert_eq!(
            serde_json::from_value::<TaskStatus>(json!("completed")).unwrap(),
            TaskStatus::Completed
        );
        assert_eq!(
            serde_json::from_value::<TaskStatus>(json!("cancelled")).unwrap(),
            TaskStatus::Cancelled
        );
    }

    #[test]
    fn task_status_rejects_invalid_variant() {
        let result = serde_json::from_value::<TaskStatus>(json!("UNKNOWN"));
        assert!(result.is_err());
    }

    #[test]
    fn task_priority_deserializes_from_lowercase() {
        assert_eq!(
            serde_json::from_value::<TaskPriority>(json!("none")).unwrap(),
            TaskPriority::None
        );
        assert_eq!(
            serde_json::from_value::<TaskPriority>(json!("low")).unwrap(),
            TaskPriority::Low
        );
        assert_eq!(
            serde_json::from_value::<TaskPriority>(json!("medium")).unwrap(),
            TaskPriority::Medium
        );
        assert_eq!(
            serde_json::from_value::<TaskPriority>(json!("high")).unwrap(),
            TaskPriority::High
        );
        assert_eq!(
            serde_json::from_value::<TaskPriority>(json!("critical")).unwrap(),
            TaskPriority::Critical
        );
    }

    #[test]
    fn task_priority_rejects_invalid_variant() {
        let result = serde_json::from_value::<TaskPriority>(json!("URGENT"));
        assert!(result.is_err());
    }

    // --- (e) skip_serializing_if ---

    #[test]
    fn task_omits_none_description_in_json() {
        let task: Task = serde_json::from_value(sample_task()).unwrap();
        let serialized = serde_json::to_value(&task).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("description"));
    }

    #[test]
    fn task_omits_none_due_date_in_json() {
        let task: Task = serde_json::from_value(sample_task()).unwrap();
        let serialized = serde_json::to_value(&task).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("due_date"));
    }

    #[test]
    fn task_omits_none_extensions_in_json() {
        let task: Task = serde_json::from_value(sample_task()).unwrap();
        let serialized = serde_json::to_value(&task).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("extensions"));
    }

    #[test]
    fn task_includes_description_when_present() {
        let mut v = sample_task();
        v["description"] = json!("Get milk and eggs");
        let task: Task = serde_json::from_value(v).unwrap();
        let serialized = serde_json::to_value(&task).unwrap();
        assert_eq!(serialized["description"], "Get milk and eggs");
    }

    // --- (g) Unknown field acceptance ---

    #[test]
    fn task_accepts_unknown_fields() {
        let mut v = sample_task();
        v["unknown_field"] = json!("extra data");
        v["another_unknown"] = json!(42);
        let result = serde_json::from_value::<Task>(v);
        assert!(result.is_ok());
    }
}

// ---------------------------------------------------------------------------
// Event validation
// ---------------------------------------------------------------------------
mod event_validation {
    use super::*;
    use life_engine_types::CalendarEvent;

    fn sample_event() -> serde_json::Value {
        json!({
            "id": TEST_UUID,
            "title": "Team standup",
            "start": TEST_TIMESTAMP,
            "end": "2026-01-01T01:00:00Z",
            "source": "test",
            "source_id": "event-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        })
    }

    // --- (a) Required field rejection ---

    #[test]
    fn event_missing_title_is_rejected() {
        let mut v = sample_event();
        v.as_object_mut().unwrap().remove("title");
        let result = serde_json::from_value::<CalendarEvent>(v);
        assert!(result.is_err());
    }

    #[test]
    fn event_missing_start_is_rejected() {
        let mut v = sample_event();
        v.as_object_mut().unwrap().remove("start");
        let result = serde_json::from_value::<CalendarEvent>(v);
        assert!(result.is_err());
    }

    #[test]
    fn event_missing_end_is_rejected() {
        let mut v = sample_event();
        v.as_object_mut().unwrap().remove("end");
        let result = serde_json::from_value::<CalendarEvent>(v);
        assert!(result.is_err());
    }

    // --- (b) Optional field defaults ---

    #[test]
    fn event_without_optional_fields_deserializes_with_defaults() {
        let event: CalendarEvent = serde_json::from_value(sample_event()).unwrap();
        assert_eq!(event.recurrence, None);
        assert!(event.attendees.is_empty());
        assert_eq!(event.location, None);
        assert_eq!(event.description, None);
        assert_eq!(event.extensions, None);
    }

    // --- (e) skip_serializing_if ---

    #[test]
    fn event_omits_none_recurrence_in_json() {
        let event: CalendarEvent = serde_json::from_value(sample_event()).unwrap();
        let serialized = serde_json::to_value(&event).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("recurrence"));
    }

    #[test]
    fn event_omits_empty_attendees_in_json() {
        let event: CalendarEvent = serde_json::from_value(sample_event()).unwrap();
        let serialized = serde_json::to_value(&event).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("attendees"));
    }

    #[test]
    fn event_omits_none_location_in_json() {
        let event: CalendarEvent = serde_json::from_value(sample_event()).unwrap();
        let serialized = serde_json::to_value(&event).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("location"));
    }

    #[test]
    fn event_omits_none_description_in_json() {
        let event: CalendarEvent = serde_json::from_value(sample_event()).unwrap();
        let serialized = serde_json::to_value(&event).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("description"));
    }

    #[test]
    fn event_omits_none_extensions_in_json() {
        let event: CalendarEvent = serde_json::from_value(sample_event()).unwrap();
        let serialized = serde_json::to_value(&event).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("extensions"));
    }

    #[test]
    fn event_includes_attendees_when_non_empty() {
        let mut v = sample_event();
        v["attendees"] = json!(["alice@example.com"]);
        let event: CalendarEvent = serde_json::from_value(v).unwrap();
        let serialized = serde_json::to_value(&event).unwrap();
        assert_eq!(serialized["attendees"], json!(["alice@example.com"]));
    }

    // --- (g) Unknown field acceptance ---

    #[test]
    fn event_accepts_unknown_fields() {
        let mut v = sample_event();
        v["calendar_color"] = json!("#ff0000");
        let result = serde_json::from_value::<CalendarEvent>(v);
        assert!(result.is_ok());
    }
}

// ---------------------------------------------------------------------------
// Contact validation
// ---------------------------------------------------------------------------
mod contact_validation {
    use super::*;
    use life_engine_types::{Contact, EmailAddress, PhoneNumber};

    fn sample_contact() -> serde_json::Value {
        json!({
            "id": TEST_UUID,
            "name": {
                "given": "Jane",
                "family": "Doe",
                "display": "Jane Doe"
            },
            "source": "test",
            "source_id": "contact-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        })
    }

    // --- (a) Required field rejection ---

    #[test]
    fn contact_missing_name_is_rejected() {
        let mut v = sample_contact();
        v.as_object_mut().unwrap().remove("name");
        let result = serde_json::from_value::<Contact>(v);
        assert!(result.is_err());
    }

    #[test]
    fn contact_missing_source_is_rejected() {
        let mut v = sample_contact();
        v.as_object_mut().unwrap().remove("source");
        let result = serde_json::from_value::<Contact>(v);
        assert!(result.is_err());
    }

    #[test]
    fn contact_missing_name_given_is_rejected() {
        let mut v = sample_contact();
        v["name"].as_object_mut().unwrap().remove("given");
        let result = serde_json::from_value::<Contact>(v);
        assert!(result.is_err());
    }

    // --- (b) Optional field defaults ---

    #[test]
    fn contact_without_optional_fields_deserializes_with_defaults() {
        let contact: Contact = serde_json::from_value(sample_contact()).unwrap();
        assert!(contact.emails.is_empty());
        assert!(contact.phones.is_empty());
        assert!(contact.addresses.is_empty());
        assert_eq!(contact.organisation, None);
        assert_eq!(contact.extensions, None);
    }

    // --- (e) skip_serializing_if ---

    #[test]
    fn contact_omits_empty_emails_in_json() {
        let contact: Contact = serde_json::from_value(sample_contact()).unwrap();
        let serialized = serde_json::to_value(&contact).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("emails"));
    }

    #[test]
    fn contact_omits_empty_phones_in_json() {
        let contact: Contact = serde_json::from_value(sample_contact()).unwrap();
        let serialized = serde_json::to_value(&contact).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("phones"));
    }

    #[test]
    fn contact_omits_empty_addresses_in_json() {
        let contact: Contact = serde_json::from_value(sample_contact()).unwrap();
        let serialized = serde_json::to_value(&contact).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("addresses"));
    }

    #[test]
    fn contact_omits_none_organisation_in_json() {
        let contact: Contact = serde_json::from_value(sample_contact()).unwrap();
        let serialized = serde_json::to_value(&contact).unwrap();
        assert!(!serialized
            .as_object()
            .unwrap()
            .contains_key("organisation"));
    }

    #[test]
    fn contact_omits_none_extensions_in_json() {
        let contact: Contact = serde_json::from_value(sample_contact()).unwrap();
        let serialized = serde_json::to_value(&contact).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("extensions"));
    }

    #[test]
    fn contact_includes_emails_when_non_empty() {
        let mut v = sample_contact();
        v["emails"] = json!([{"address": "jane@example.com"}]);
        let contact: Contact = serde_json::from_value(v).unwrap();
        let serialized = serde_json::to_value(&contact).unwrap();
        assert!(serialized.as_object().unwrap().contains_key("emails"));
    }

    // --- (f) serde rename ---

    #[test]
    fn email_address_type_uses_json_key_type() {
        let ea = EmailAddress {
            address: "jane@example.com".into(),
            email_type: Some("work".into()),
            primary: None,
        };
        let serialized = serde_json::to_value(&ea).unwrap();
        assert_eq!(serialized["type"], "work");
        assert!(!serialized.as_object().unwrap().contains_key("email_type"));
    }

    #[test]
    fn email_address_deserializes_from_type_key() {
        let v = json!({"address": "a@b.com", "type": "home"});
        let ea: EmailAddress = serde_json::from_value(v).unwrap();
        assert_eq!(ea.email_type, Some("home".into()));
    }

    #[test]
    fn phone_number_type_uses_json_key_type() {
        let pn = PhoneNumber {
            number: "+1234567890".into(),
            phone_type: Some("mobile".into()),
        };
        let serialized = serde_json::to_value(&pn).unwrap();
        assert_eq!(serialized["type"], "mobile");
        assert!(!serialized.as_object().unwrap().contains_key("phone_type"));
    }

    #[test]
    fn phone_number_deserializes_from_type_key() {
        let v = json!({"number": "+1234567890", "type": "work"});
        let pn: PhoneNumber = serde_json::from_value(v).unwrap();
        assert_eq!(pn.phone_type, Some("work".into()));
    }

    // --- (g) Unknown field acceptance ---

    #[test]
    fn contact_accepts_unknown_fields() {
        let mut v = sample_contact();
        v["nickname"] = json!("JD");
        let result = serde_json::from_value::<Contact>(v);
        assert!(result.is_ok());
    }
}

// ---------------------------------------------------------------------------
// Email validation
// ---------------------------------------------------------------------------
mod email_validation {
    use super::*;
    use life_engine_types::Email;

    fn sample_email() -> serde_json::Value {
        json!({
            "id": TEST_UUID,
            "from": "sender@example.com",
            "to": ["recipient@example.com"],
            "subject": "Hello",
            "body_text": "Message body",
            "source": "test",
            "source_id": "email-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        })
    }

    // --- (a) Required field rejection ---

    #[test]
    fn email_missing_from_is_rejected() {
        let mut v = sample_email();
        v.as_object_mut().unwrap().remove("from");
        let result = serde_json::from_value::<Email>(v);
        assert!(result.is_err());
    }

    #[test]
    fn email_missing_subject_is_rejected() {
        let mut v = sample_email();
        v.as_object_mut().unwrap().remove("subject");
        let result = serde_json::from_value::<Email>(v);
        assert!(result.is_err());
    }

    #[test]
    fn email_missing_body_text_is_rejected() {
        let mut v = sample_email();
        v.as_object_mut().unwrap().remove("body_text");
        let result = serde_json::from_value::<Email>(v);
        assert!(result.is_err());
    }

    // --- (b) Optional field defaults ---

    #[test]
    fn email_without_optional_fields_deserializes_with_defaults() {
        let email: Email = serde_json::from_value(sample_email()).unwrap();
        assert!(email.cc.is_empty());
        assert!(email.bcc.is_empty());
        assert_eq!(email.body_html, None);
        assert_eq!(email.thread_id, None);
        assert!(email.labels.is_empty());
        assert!(email.attachments.is_empty());
        assert_eq!(email.extensions, None);
    }

    // --- (e) skip_serializing_if ---

    #[test]
    fn email_omits_empty_cc_in_json() {
        let email: Email = serde_json::from_value(sample_email()).unwrap();
        let serialized = serde_json::to_value(&email).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("cc"));
    }

    #[test]
    fn email_omits_empty_bcc_in_json() {
        let email: Email = serde_json::from_value(sample_email()).unwrap();
        let serialized = serde_json::to_value(&email).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("bcc"));
    }

    #[test]
    fn email_omits_none_body_html_in_json() {
        let email: Email = serde_json::from_value(sample_email()).unwrap();
        let serialized = serde_json::to_value(&email).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("body_html"));
    }

    #[test]
    fn email_omits_none_thread_id_in_json() {
        let email: Email = serde_json::from_value(sample_email()).unwrap();
        let serialized = serde_json::to_value(&email).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("thread_id"));
    }

    #[test]
    fn email_omits_empty_labels_in_json() {
        let email: Email = serde_json::from_value(sample_email()).unwrap();
        let serialized = serde_json::to_value(&email).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("labels"));
    }

    #[test]
    fn email_omits_empty_attachments_in_json() {
        let email: Email = serde_json::from_value(sample_email()).unwrap();
        let serialized = serde_json::to_value(&email).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("attachments"));
    }

    #[test]
    fn email_omits_none_extensions_in_json() {
        let email: Email = serde_json::from_value(sample_email()).unwrap();
        let serialized = serde_json::to_value(&email).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("extensions"));
    }

    #[test]
    fn email_includes_cc_when_non_empty() {
        let mut v = sample_email();
        v["cc"] = json!(["cc@example.com"]);
        let email: Email = serde_json::from_value(v).unwrap();
        let serialized = serde_json::to_value(&email).unwrap();
        assert_eq!(serialized["cc"], json!(["cc@example.com"]));
    }

    // --- (g) Unknown field acceptance ---

    #[test]
    fn email_accepts_unknown_fields() {
        let mut v = sample_email();
        v["importance"] = json!("high");
        let result = serde_json::from_value::<Email>(v);
        assert!(result.is_ok());
    }
}

// ---------------------------------------------------------------------------
// File validation
// ---------------------------------------------------------------------------
mod file_validation {
    use super::*;
    use life_engine_types::FileMetadata;

    fn sample_file() -> serde_json::Value {
        json!({
            "id": TEST_UUID,
            "name": "report.pdf",
            "mime_type": "application/pdf",
            "size": 2048,
            "path": "/documents/report.pdf",
            "source": "test",
            "source_id": "file-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        })
    }

    // --- (a) Required field rejection ---

    #[test]
    fn file_missing_name_is_rejected() {
        let mut v = sample_file();
        v.as_object_mut().unwrap().remove("name");
        let result = serde_json::from_value::<FileMetadata>(v);
        assert!(result.is_err());
    }

    #[test]
    fn file_missing_mime_type_is_rejected() {
        let mut v = sample_file();
        v.as_object_mut().unwrap().remove("mime_type");
        let result = serde_json::from_value::<FileMetadata>(v);
        assert!(result.is_err());
    }

    #[test]
    fn file_missing_size_is_rejected() {
        let mut v = sample_file();
        v.as_object_mut().unwrap().remove("size");
        let result = serde_json::from_value::<FileMetadata>(v);
        assert!(result.is_err());
    }

    // --- (b) Optional field defaults ---

    #[test]
    fn file_without_optional_fields_deserializes_with_defaults() {
        let file: FileMetadata = serde_json::from_value(sample_file()).unwrap();
        assert_eq!(file.checksum, None);
        assert_eq!(file.extensions, None);
    }

    // --- (e) skip_serializing_if ---

    #[test]
    fn file_omits_none_checksum_in_json() {
        let file: FileMetadata = serde_json::from_value(sample_file()).unwrap();
        let serialized = serde_json::to_value(&file).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("checksum"));
    }

    #[test]
    fn file_omits_none_extensions_in_json() {
        let file: FileMetadata = serde_json::from_value(sample_file()).unwrap();
        let serialized = serde_json::to_value(&file).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("extensions"));
    }

    #[test]
    fn file_includes_checksum_when_present() {
        let mut v = sample_file();
        v["checksum"] = json!("sha256:abc123");
        let file: FileMetadata = serde_json::from_value(v).unwrap();
        let serialized = serde_json::to_value(&file).unwrap();
        assert_eq!(serialized["checksum"], "sha256:abc123");
    }

    // --- (g) Unknown field acceptance ---

    #[test]
    fn file_accepts_unknown_fields() {
        let mut v = sample_file();
        v["thumbnail_url"] = json!("https://example.com/thumb.png");
        let result = serde_json::from_value::<FileMetadata>(v);
        assert!(result.is_ok());
    }
}

// ---------------------------------------------------------------------------
// Note validation
// ---------------------------------------------------------------------------
mod note_validation {
    use super::*;
    use life_engine_types::Note;

    fn sample_note() -> serde_json::Value {
        json!({
            "id": TEST_UUID,
            "title": "Meeting notes",
            "body": "Discussed project roadmap.",
            "source": "test",
            "source_id": "note-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        })
    }

    // --- (a) Required field rejection ---

    #[test]
    fn note_missing_title_is_rejected() {
        let mut v = sample_note();
        v.as_object_mut().unwrap().remove("title");
        let result = serde_json::from_value::<Note>(v);
        assert!(result.is_err());
    }

    #[test]
    fn note_missing_body_is_rejected() {
        let mut v = sample_note();
        v.as_object_mut().unwrap().remove("body");
        let result = serde_json::from_value::<Note>(v);
        assert!(result.is_err());
    }

    #[test]
    fn note_missing_source_id_is_rejected() {
        let mut v = sample_note();
        v.as_object_mut().unwrap().remove("source_id");
        let result = serde_json::from_value::<Note>(v);
        assert!(result.is_err());
    }

    // --- (b) Optional field defaults ---

    #[test]
    fn note_without_optional_fields_deserializes_with_defaults() {
        let note: Note = serde_json::from_value(sample_note()).unwrap();
        assert!(note.tags.is_empty());
        assert_eq!(note.extensions, None);
    }

    // --- (e) skip_serializing_if ---

    #[test]
    fn note_omits_empty_tags_in_json() {
        let note: Note = serde_json::from_value(sample_note()).unwrap();
        let serialized = serde_json::to_value(&note).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("tags"));
    }

    #[test]
    fn note_omits_none_extensions_in_json() {
        let note: Note = serde_json::from_value(sample_note()).unwrap();
        let serialized = serde_json::to_value(&note).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("extensions"));
    }

    #[test]
    fn note_includes_tags_when_non_empty() {
        let mut v = sample_note();
        v["tags"] = json!(["meeting", "roadmap"]);
        let note: Note = serde_json::from_value(v).unwrap();
        let serialized = serde_json::to_value(&note).unwrap();
        assert_eq!(serialized["tags"], json!(["meeting", "roadmap"]));
    }

    // --- (g) Unknown field acceptance ---

    #[test]
    fn note_accepts_unknown_fields() {
        let mut v = sample_note();
        v["pinned"] = json!(true);
        let result = serde_json::from_value::<Note>(v);
        assert!(result.is_ok());
    }
}

// ---------------------------------------------------------------------------
// Credential validation
// ---------------------------------------------------------------------------
mod credential_validation {
    use super::*;
    use life_engine_types::{Credential, CredentialType};

    fn sample_credential() -> serde_json::Value {
        json!({
            "id": TEST_UUID,
            "type": "api_key",
            "issuer": "https://auth.example.com",
            "issued_date": "2026-01-01",
            "claims": {"scope": "read"},
            "source": "test",
            "source_id": "cred-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        })
    }

    // --- (a) Required field rejection ---

    #[test]
    fn credential_missing_type_is_rejected() {
        let mut v = sample_credential();
        v.as_object_mut().unwrap().remove("type");
        let result = serde_json::from_value::<Credential>(v);
        assert!(result.is_err());
    }

    #[test]
    fn credential_missing_issuer_is_rejected() {
        let mut v = sample_credential();
        v.as_object_mut().unwrap().remove("issuer");
        let result = serde_json::from_value::<Credential>(v);
        assert!(result.is_err());
    }

    #[test]
    fn credential_missing_claims_is_rejected() {
        let mut v = sample_credential();
        v.as_object_mut().unwrap().remove("claims");
        let result = serde_json::from_value::<Credential>(v);
        assert!(result.is_err());
    }

    // --- (b) Optional field defaults ---

    #[test]
    fn credential_without_optional_fields_deserializes_with_defaults() {
        let cred: Credential = serde_json::from_value(sample_credential()).unwrap();
        assert_eq!(cred.expiry_date, None);
    }

    // --- (c) Enum variant serialization ---

    #[test]
    fn credential_type_serializes_to_snake_case() {
        assert_eq!(
            serde_json::to_value(CredentialType::OauthToken).unwrap(),
            "oauth_token"
        );
        assert_eq!(
            serde_json::to_value(CredentialType::ApiKey).unwrap(),
            "api_key"
        );
        assert_eq!(
            serde_json::to_value(CredentialType::IdentityDocument).unwrap(),
            "identity_document"
        );
        assert_eq!(
            serde_json::to_value(CredentialType::Passkey).unwrap(),
            "passkey"
        );
    }

    // --- (d) Enum variant deserialization ---

    #[test]
    fn credential_type_deserializes_from_snake_case() {
        assert_eq!(
            serde_json::from_value::<CredentialType>(json!("oauth_token")).unwrap(),
            CredentialType::OauthToken
        );
        assert_eq!(
            serde_json::from_value::<CredentialType>(json!("api_key")).unwrap(),
            CredentialType::ApiKey
        );
        assert_eq!(
            serde_json::from_value::<CredentialType>(json!("identity_document")).unwrap(),
            CredentialType::IdentityDocument
        );
        assert_eq!(
            serde_json::from_value::<CredentialType>(json!("passkey")).unwrap(),
            CredentialType::Passkey
        );
    }

    #[test]
    fn credential_type_rejects_invalid_variant() {
        let result = serde_json::from_value::<CredentialType>(json!("password"));
        assert!(result.is_err());
    }

    // --- (e) skip_serializing_if ---

    #[test]
    fn credential_omits_none_expiry_date_in_json() {
        let cred: Credential = serde_json::from_value(sample_credential()).unwrap();
        let serialized = serde_json::to_value(&cred).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("expiry_date"));
    }

    #[test]
    fn credential_includes_expiry_date_when_present() {
        let mut v = sample_credential();
        v["expiry_date"] = json!("2027-01-01");
        let cred: Credential = serde_json::from_value(v).unwrap();
        let serialized = serde_json::to_value(&cred).unwrap();
        assert_eq!(serialized["expiry_date"], "2027-01-01");
    }

    // --- (f) serde rename ---

    #[test]
    fn credential_type_field_uses_json_key_type() {
        let cred: Credential = serde_json::from_value(sample_credential()).unwrap();
        let serialized = serde_json::to_value(&cred).unwrap();
        assert!(serialized.as_object().unwrap().contains_key("type"));
        assert!(!serialized
            .as_object()
            .unwrap()
            .contains_key("credential_type"));
        assert_eq!(serialized["type"], "api_key");
    }

    #[test]
    fn credential_deserializes_type_from_json_key() {
        let v = sample_credential();
        let cred: Credential = serde_json::from_value(v).unwrap();
        assert_eq!(cred.credential_type, CredentialType::ApiKey);
    }

    #[test]
    fn credential_rejects_credential_type_as_json_key() {
        let mut v = sample_credential();
        v.as_object_mut().unwrap().remove("type");
        v["credential_type"] = json!("api_key");
        // Without "type" key, deserialization must fail because the Rust field
        // is renamed to "type" in JSON.
        let result = serde_json::from_value::<Credential>(v);
        assert!(result.is_err());
    }

    // --- (g) Unknown field acceptance ---

    #[test]
    fn credential_accepts_unknown_fields() {
        let mut v = sample_credential();
        v["revoked"] = json!(false);
        let result = serde_json::from_value::<Credential>(v);
        assert!(result.is_ok());
    }
}
