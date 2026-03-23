//! JSON Schema validation tests for all 7 CDM collections.
//!
//! Validates test fixtures against their JSON Schema definitions:
//! - Valid fixtures must pass with zero errors
//! - Invalid fixtures must fail with descriptive error messages
//! - Extensions field works on all collections except Credentials
//! - Required fields are enforced
//! - Enum values are restricted to defined options

use serde_json::Value;
use std::fs;
use std::path::PathBuf;

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn load_schema(collection: &str) -> Value {
    let path = project_root()
        .join(".odm/doc/schemas")
        .join(format!("{}.schema.json", collection));
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read schema {}: {}", path.display(), e));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse schema {}: {}", path.display(), e))
}

fn load_valid_fixture(collection: &str) -> Value {
    let path = project_root()
        .join("packages/test-utils/fixtures/schemas/valid")
        .join(format!("{}.json", collection));
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read valid fixture {}: {}", path.display(), e));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse valid fixture {}: {}", path.display(), e))
}

fn load_invalid_fixtures(collection: &str) -> Vec<Value> {
    let path = project_root()
        .join("packages/test-utils/fixtures/schemas/invalid")
        .join(format!("{}.json", collection));
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read invalid fixture {}: {}", path.display(), e));
    let arr: Vec<Value> = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse invalid fixture {}: {}", path.display(), e));
    arr
}

fn validate(schema: &Value, instance: &Value) -> Result<(), Vec<String>> {
    let validator = jsonschema::validator_for(schema)
        .expect("Failed to compile JSON Schema");
    let errors: Vec<String> = validator
        .iter_errors(instance)
        .map(|e| format!("{} at {}", e, e.instance_path))
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

const COLLECTIONS: &[&str] = &[
    "events",
    "tasks",
    "contacts",
    "notes",
    "emails",
    "files",
    "credentials",
];

// ---------------------------------------------------------------------------
// Valid fixture tests — one per collection
// ---------------------------------------------------------------------------

mod valid_fixtures {
    use super::*;

    #[test]
    fn events_valid_fixture_passes_schema() {
        let schema = load_schema("events");
        let fixture = load_valid_fixture("events");
        assert!(validate(&schema, &fixture).is_ok(), "Valid events fixture should pass schema validation");
    }

    #[test]
    fn tasks_valid_fixture_passes_schema() {
        let schema = load_schema("tasks");
        let fixture = load_valid_fixture("tasks");
        assert!(validate(&schema, &fixture).is_ok(), "Valid tasks fixture should pass schema validation");
    }

    #[test]
    fn contacts_valid_fixture_passes_schema() {
        let schema = load_schema("contacts");
        let fixture = load_valid_fixture("contacts");
        assert!(validate(&schema, &fixture).is_ok(), "Valid contacts fixture should pass schema validation");
    }

    #[test]
    fn notes_valid_fixture_passes_schema() {
        let schema = load_schema("notes");
        let fixture = load_valid_fixture("notes");
        assert!(validate(&schema, &fixture).is_ok(), "Valid notes fixture should pass schema validation");
    }

    #[test]
    fn emails_valid_fixture_passes_schema() {
        let schema = load_schema("emails");
        let fixture = load_valid_fixture("emails");
        assert!(validate(&schema, &fixture).is_ok(), "Valid emails fixture should pass schema validation");
    }

    #[test]
    fn files_valid_fixture_passes_schema() {
        let schema = load_schema("files");
        let fixture = load_valid_fixture("files");
        assert!(validate(&schema, &fixture).is_ok(), "Valid files fixture should pass schema validation");
    }

    #[test]
    fn credentials_valid_fixture_passes_schema() {
        let schema = load_schema("credentials");
        let fixture = load_valid_fixture("credentials");
        assert!(validate(&schema, &fixture).is_ok(), "Valid credentials fixture should pass schema validation");
    }
}

// ---------------------------------------------------------------------------
// Invalid fixture tests — each invalid record must fail validation
// ---------------------------------------------------------------------------

mod invalid_fixtures {
    use super::*;

    fn assert_all_invalid(collection: &str) {
        let schema = load_schema(collection);
        let fixtures = load_invalid_fixtures(collection);
        assert!(!fixtures.is_empty(), "Expected at least one invalid fixture for {}", collection);

        for (i, fixture) in fixtures.iter().enumerate() {
            let comment = fixture.get("_comment")
                .and_then(|c| c.as_str())
                .unwrap_or("(no comment)");
            let result = validate(&schema, fixture);
            assert!(
                result.is_err(),
                "Invalid {} fixture #{} should fail validation: {}",
                collection, i + 1, comment
            );
            let errors = result.unwrap_err();
            assert!(
                !errors.is_empty(),
                "Invalid {} fixture #{} should have descriptive error messages: {}",
                collection, i + 1, comment
            );
        }
    }

    #[test]
    fn events_invalid_fixtures_fail_schema() {
        assert_all_invalid("events");
    }

    #[test]
    fn tasks_invalid_fixtures_fail_schema() {
        assert_all_invalid("tasks");
    }

    #[test]
    fn contacts_invalid_fixtures_fail_schema() {
        assert_all_invalid("contacts");
    }

    #[test]
    fn notes_invalid_fixtures_fail_schema() {
        assert_all_invalid("notes");
    }

    #[test]
    fn emails_invalid_fixtures_fail_schema() {
        assert_all_invalid("emails");
    }

    #[test]
    fn files_invalid_fixtures_fail_schema() {
        assert_all_invalid("files");
    }

    #[test]
    fn credentials_invalid_fixtures_fail_schema() {
        assert_all_invalid("credentials");
    }
}

// ---------------------------------------------------------------------------
// Extensions field tests
// ---------------------------------------------------------------------------

mod extensions {
    use super::*;
    use serde_json::json;

    const TEST_UUID: &str = "00000000-0000-0000-0000-000000000001";
    const TEST_TIMESTAMP: &str = "2026-01-01T00:00:00Z";

    fn minimal_record(collection: &str) -> Value {
        let common = json!({
            "id": TEST_UUID,
            "source": "test",
            "source_id": "ext-test-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let mut record = common.as_object().unwrap().clone();

        match collection {
            "events" => {
                record.insert("title".into(), json!("Test Event"));
                record.insert("start".into(), json!(TEST_TIMESTAMP));
            }
            "tasks" => {
                record.insert("title".into(), json!("Test Task"));
            }
            "contacts" => {
                record.insert("name".into(), json!({"given": "Test", "family": "User"}));
            }
            "notes" => {
                record.insert("title".into(), json!("Test Note"));
                record.insert("body".into(), json!("Test body"));
            }
            "emails" => {
                record.insert("subject".into(), json!("Test Email"));
                record.insert("from".into(), json!({"address": "test@example.com"}));
                record.insert("to".into(), json!([{"address": "to@example.com"}]));
                record.insert("date".into(), json!(TEST_TIMESTAMP));
            }
            "files" => {
                record.insert("filename".into(), json!("test.txt"));
                record.insert("path".into(), json!("/data/test.txt"));
                record.insert("mime_type".into(), json!("text/plain"));
                record.insert("size_bytes".into(), json!(1024));
                record.insert("checksum".into(), json!("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"));
            }
            "credentials" => {
                record.insert("name".into(), json!("Test Credential"));
                record.insert("credential_type".into(), json!("api_key"));
                record.insert("service".into(), json!("test-service"));
                record.insert("claims".into(), json!({"key": "value"}));
            }
            _ => panic!("Unknown collection: {}", collection),
        }

        Value::Object(record.into())
    }

    #[test]
    fn extensions_accepted_on_all_except_credentials() {
        let extension_data = json!({
            "com.example.plugin": {
                "custom_field": "custom_value",
                "nested": {"deep": true}
            }
        });

        for collection in &["events", "tasks", "contacts", "notes", "emails", "files"] {
            let schema = load_schema(collection);
            let mut record = minimal_record(collection);
            record.as_object_mut().unwrap().insert("extensions".into(), extension_data.clone());

            let result = validate(&schema, &record);
            assert!(
                result.is_ok(),
                "Extensions should be accepted on {} collection: {:?}",
                collection,
                result.err()
            );
        }
    }

    #[test]
    fn credentials_rejects_extensions_field() {
        let schema = load_schema("credentials");
        let mut record = minimal_record("credentials");
        record.as_object_mut().unwrap().insert(
            "extensions".into(),
            json!({"com.example.plugin": {"key": "value"}}),
        );

        let result = validate(&schema, &record);
        assert!(
            result.is_err(),
            "Credentials should reject extensions field"
        );
    }
}

// ---------------------------------------------------------------------------
// Required field enforcement tests
// ---------------------------------------------------------------------------

mod required_fields {
    use super::*;
    use serde_json::json;

    const COMMON_REQUIRED: &[&str] = &["id", "source", "source_id", "created_at", "updated_at"];

    fn collection_specific_required(collection: &str) -> &'static [&'static str] {
        match collection {
            "events" => &["title", "start"],
            "tasks" => &["title"],
            "contacts" => &["name"],
            "notes" => &["title", "body"],
            "emails" => &["subject", "from", "to", "date"],
            "files" => &["filename", "path", "mime_type", "size_bytes", "checksum"],
            "credentials" => &["name", "credential_type", "service", "claims"],
            _ => &[],
        }
    }

    #[test]
    fn missing_required_fields_rejected() {
        for collection in super::COLLECTIONS {
            let schema = load_schema(collection);

            let all_required: Vec<&str> = COMMON_REQUIRED.iter()
                .chain(collection_specific_required(collection).iter())
                .copied()
                .collect();

            for field in &all_required {
                let empty = json!({});
                let result = validate(&schema, &empty);
                assert!(
                    result.is_err(),
                    "Empty object should fail {} schema (missing {})",
                    collection, field
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Enum constraint tests
// ---------------------------------------------------------------------------

mod enum_constraints {
    use super::*;

    #[test]
    fn task_status_rejects_invalid_enum() {
        let schema = load_schema("tasks");
        let fixture = load_valid_fixture("tasks");
        let mut record = fixture.clone();
        record.as_object_mut().unwrap().insert("status".into(), serde_json::json!("done"));

        let result = validate(&schema, &record);
        assert!(result.is_err(), "Task status 'done' should be rejected");
    }

    #[test]
    fn task_priority_rejects_invalid_enum() {
        let schema = load_schema("tasks");
        let fixture = load_valid_fixture("tasks");
        let mut record = fixture.clone();
        record.as_object_mut().unwrap().insert("priority".into(), serde_json::json!("critical"));

        let result = validate(&schema, &record);
        assert!(result.is_err(), "Task priority 'critical' should be rejected");
    }

    #[test]
    fn event_status_rejects_invalid_enum() {
        let schema = load_schema("events");
        let fixture = load_valid_fixture("events");
        let mut record = fixture.clone();
        record.as_object_mut().unwrap().insert("status".into(), serde_json::json!("maybe"));

        let result = validate(&schema, &record);
        assert!(result.is_err(), "Event status 'maybe' should be rejected");
    }

    #[test]
    fn note_format_rejects_invalid_enum() {
        let schema = load_schema("notes");
        let fixture = load_valid_fixture("notes");
        let mut record = fixture.clone();
        record.as_object_mut().unwrap().insert("format".into(), serde_json::json!("rtf"));

        let result = validate(&schema, &record);
        assert!(result.is_err(), "Note format 'rtf' should be rejected");
    }

    #[test]
    fn credential_type_rejects_invalid_enum() {
        let schema = load_schema("credentials");
        let fixture = load_valid_fixture("credentials");
        let mut record = fixture.clone();
        record.as_object_mut().unwrap().insert("credential_type".into(), serde_json::json!("password"));

        let result = validate(&schema, &record);
        assert!(result.is_err(), "Credential type 'password' should be rejected");
    }
}
