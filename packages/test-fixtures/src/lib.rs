//! Canonical test fixture data for all Life Engine CDM collections.
//!
//! Provides static, deterministic JSON fixtures and typed loader functions
//! for use across all test suites. Every fixture conforms to its JSON Schema
//! in `docs/schemas/`.

pub use life_engine_types;

use life_engine_types::{
    CalendarEvent, Contact, Credential, Email, FileMetadata, Note, Task,
};

/// Raw JSON for a canonical Task fixture.
pub const TASK_JSON: &str = include_str!("../fixtures/task.json");

/// Raw JSON for a canonical CalendarEvent fixture.
pub const EVENT_JSON: &str = include_str!("../fixtures/event.json");

/// Raw JSON for a canonical Contact fixture.
pub const CONTACT_JSON: &str = include_str!("../fixtures/contact.json");

/// Raw JSON for a canonical Email fixture.
pub const EMAIL_JSON: &str = include_str!("../fixtures/email.json");

/// Raw JSON for a canonical FileMetadata fixture.
pub const FILE_JSON: &str = include_str!("../fixtures/file.json");

/// Raw JSON for a canonical Note fixture.
pub const NOTE_JSON: &str = include_str!("../fixtures/note.json");

/// Raw JSON for a canonical Credential fixture.
pub const CREDENTIAL_JSON: &str = include_str!("../fixtures/credential.json");

/// Load the canonical Task fixture.
pub fn task() -> Task {
    serde_json::from_str(TASK_JSON).expect("task fixture should deserialize")
}

/// Load the canonical CalendarEvent fixture.
pub fn event() -> CalendarEvent {
    serde_json::from_str(EVENT_JSON).expect("event fixture should deserialize")
}

/// Load the canonical Contact fixture.
pub fn contact() -> Contact {
    serde_json::from_str(CONTACT_JSON).expect("contact fixture should deserialize")
}

/// Load the canonical Email fixture.
pub fn email() -> Email {
    serde_json::from_str(EMAIL_JSON).expect("email fixture should deserialize")
}

/// Load the canonical FileMetadata fixture.
pub fn file() -> FileMetadata {
    serde_json::from_str(FILE_JSON).expect("file fixture should deserialize")
}

/// Load the canonical Note fixture.
pub fn note() -> Note {
    serde_json::from_str(NOTE_JSON).expect("note fixture should deserialize")
}

/// Load the canonical Credential fixture.
pub fn credential() -> Credential {
    serde_json::from_str(CREDENTIAL_JSON).expect("credential fixture should deserialize")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Resolve a schema file path relative to the workspace root using
    /// `CARGO_MANIFEST_DIR` instead of brittle `../../../` traversal.
    fn load_schema(filename: &str) -> String {
        let schema_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.odm/doc/schemas")
            .canonicalize()
            .expect("failed to resolve schema directory");
        let path = schema_dir.join(filename);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read schema {}: {e}", path.display()))
    }

    fn validate_fixture_against_schema(fixture_json: &str, schema_json: &str, name: &str) {
        let instance: serde_json::Value = serde_json::from_str(fixture_json)
            .unwrap_or_else(|e| panic!("{name} fixture is not valid JSON: {e}"));
        let schema: serde_json::Value = serde_json::from_str(schema_json)
            .unwrap_or_else(|e| panic!("{name} schema is not valid JSON: {e}"));
        let validator = jsonschema::draft7::new(&schema)
            .unwrap_or_else(|e| panic!("{name} schema is not a valid JSON Schema: {e}"));
        let errors: Vec<String> = validator
            .iter_errors(&instance)
            .map(|e| format!("  - {e}"))
            .collect();
        assert!(
            errors.is_empty(),
            "{name} fixture failed schema validation:\n{}",
            errors.join("\n")
        );
    }

    // -- Schema validation tests --

    #[test]
    fn task_fixture_validates_against_schema() {
        validate_fixture_against_schema(
            TASK_JSON,
            &load_schema("tasks.schema.json"),
            "task",
        );
    }

    #[test]
    fn event_fixture_validates_against_schema() {
        validate_fixture_against_schema(
            EVENT_JSON,
            &load_schema("events.schema.json"),
            "event",
        );
    }

    #[test]
    fn contact_fixture_validates_against_schema() {
        validate_fixture_against_schema(
            CONTACT_JSON,
            &load_schema("contacts.schema.json"),
            "contact",
        );
    }

    #[test]
    fn email_fixture_validates_against_schema() {
        validate_fixture_against_schema(
            EMAIL_JSON,
            &load_schema("emails.schema.json"),
            "email",
        );
    }

    #[test]
    fn file_fixture_validates_against_schema() {
        validate_fixture_against_schema(
            FILE_JSON,
            &load_schema("files.schema.json"),
            "file",
        );
    }

    #[test]
    fn note_fixture_validates_against_schema() {
        validate_fixture_against_schema(
            NOTE_JSON,
            &load_schema("notes.schema.json"),
            "note",
        );
    }

    #[test]
    fn credential_fixture_validates_against_schema() {
        validate_fixture_against_schema(
            CREDENTIAL_JSON,
            &load_schema("credentials.schema.json"),
            "credential",
        );
    }

    // -- Deserialization round-trip tests --

    #[test]
    fn task_fixture_deserializes() {
        let t = task();
        assert_eq!(t.title, "Implement user authentication flow");
        let json = serde_json::to_string(&t).expect("serialize");
        let _: Task = serde_json::from_str(&json).expect("round-trip deserialize");
    }

    #[test]
    fn event_fixture_deserializes() {
        let e = event();
        assert_eq!(e.title, "Architecture review session");
        let json = serde_json::to_string(&e).expect("serialize");
        let _: CalendarEvent = serde_json::from_str(&json).expect("round-trip deserialize");
    }

    #[test]
    fn contact_fixture_deserializes() {
        let c = contact();
        assert_eq!(c.name.given, "Eleanor");
        assert_eq!(c.emails.len(), 2);
        let json = serde_json::to_string(&c).expect("serialize");
        let _: Contact = serde_json::from_str(&json).expect("round-trip deserialize");
    }

    #[test]
    fn email_fixture_deserializes() {
        let e = email();
        assert_eq!(e.subject, "Re: Plugin SDK API review — feedback requested");
        assert_eq!(e.attachments.len(), 1);
        let json = serde_json::to_string(&e).expect("serialize");
        let _: Email = serde_json::from_str(&json).expect("round-trip deserialize");
    }

    #[test]
    fn file_fixture_deserializes() {
        let f = file();
        assert_eq!(f.filename, "architecture-diagram-v2.png");
        let json = serde_json::to_string(&f).expect("serialize");
        let _: FileMetadata = serde_json::from_str(&json).expect("round-trip deserialize");
    }

    #[test]
    fn note_fixture_deserializes() {
        let n = note();
        assert_eq!(n.title, "Sprint retrospective — January 2026");
        let json = serde_json::to_string(&n).expect("serialize");
        let _: Note = serde_json::from_str(&json).expect("round-trip deserialize");
    }

    #[test]
    fn credential_fixture_deserializes() {
        let c = credential();
        assert_eq!(c.name, "Google Calendar OAuth Token");
        assert_eq!(c.service, "accounts.google.com");
        let json = serde_json::to_string(&c).expect("serialize");
        let _: Credential = serde_json::from_str(&json).expect("round-trip deserialize");
    }
}
