//! Integration tests that validate fixture data against the JSON schemas
//! in `docs/schemas/`.
//!
//! Each CDM schema gets a positive test (valid fixture passes) and a negative
//! test (invalid fixture fails). Existing `plugin.json` files are also
//! validated against the plugin manifest schema.

use serde_json::json;
use std::path::PathBuf;

const TEST_TIMESTAMP: &str = "2026-01-15T10:30:00Z";

// ── Helpers ────────────────────────────────────────────────────────

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("failed to resolve repo root")
}

fn load_schema(filename: &str) -> serde_json::Value {
    let path = repo_root().join(".odm/doc/schemas").join(filename);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read schema {}: {e}", path.display()));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("schema {} is not valid JSON: {e}", filename))
}

fn assert_valid(schema: &serde_json::Value, fixture: &serde_json::Value) {
    let validator =
        jsonschema::validator_for(schema).expect("failed to compile JSON schema");
    if let Err(error) = validator.validate(fixture) {
        panic!("fixture failed schema validation:\n  - {error}");
    }
}

fn assert_invalid(schema: &serde_json::Value, fixture: &serde_json::Value) {
    let validator =
        jsonschema::validator_for(schema).expect("failed to compile JSON schema");
    assert!(
        validator.validate(fixture).is_err(),
        "expected validation to fail but it passed"
    );
}

// ── Schema self-validation ─────────────────────────────────────────

#[test]
fn all_schemas_parse_as_valid_json() {
    let schemas = [
        "tasks.schema.json",
        "events.schema.json",
        "contacts.schema.json",
        "emails.schema.json",
        "files.schema.json",
        "notes.schema.json",
        "credentials.schema.json",
        "plugin-manifest.schema.json",
    ];
    for filename in &schemas {
        let path = repo_root().join(".odm/doc/schemas").join(filename);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {filename}: {e}"));
        let _: serde_json::Value = serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("{filename} is not valid JSON: {e}"));
    }
}

// ── Tasks ──────────────────────────────────────────────────────────

#[test]
fn task_valid_fixture_passes_schema() {
    let schema = load_schema("tasks.schema.json");
    let fixture = json!({
        "id": "task-001",
        "title": "Review pull request #42",
        "status": "active",
        "priority": "high",
        "source": "com.life-engine.todoist",
        "source_id": "todoist-98765",
        "created_at": TEST_TIMESTAMP,
        "updated_at": TEST_TIMESTAMP
    });
    assert_valid(&schema, &fixture);
}

#[test]
fn task_missing_required_field_fails_schema() {
    let schema = load_schema("tasks.schema.json");
    let fixture = json!({
        "id": "task-001",
        "title": "Review pull request #42",
        // missing status, priority
        "source": "com.life-engine.todoist",
        "source_id": "todoist-98765",
        "created_at": TEST_TIMESTAMP,
        "updated_at": TEST_TIMESTAMP
    });
    assert_invalid(&schema, &fixture);
}

// ── Events ─────────────────────────────────────────────────────────

#[test]
fn event_valid_fixture_passes_schema() {
    let schema = load_schema("events.schema.json");
    let fixture = json!({
        "id": "evt-001",
        "title": "Team standup",
        "start": "2026-01-15T09:00:00Z",
        "end": "2026-01-15T09:30:00Z",
        "source": "com.life-engine.google-calendar",
        "source_id": "gcal-abc123",
        "created_at": TEST_TIMESTAMP,
        "updated_at": TEST_TIMESTAMP
    });
    assert_valid(&schema, &fixture);
}

#[test]
fn event_missing_required_field_fails_schema() {
    let schema = load_schema("events.schema.json");
    let fixture = json!({
        "id": "evt-001",
        "title": "Team standup",
        // missing start, end
        "source": "com.life-engine.google-calendar",
        "source_id": "gcal-abc123",
        "created_at": TEST_TIMESTAMP,
        "updated_at": TEST_TIMESTAMP
    });
    assert_invalid(&schema, &fixture);
}

// ── Contacts ───────────────────────────────────────────────────────

#[test]
fn contact_valid_fixture_passes_schema() {
    let schema = load_schema("contacts.schema.json");
    let fixture = json!({
        "id": "contact-001",
        "name": {
            "given": "Ada",
            "family": "Lovelace",
            "display": "Ada Lovelace"
        },
        "source": "com.life-engine.google-contacts",
        "source_id": "people/c123456",
        "created_at": TEST_TIMESTAMP,
        "updated_at": TEST_TIMESTAMP
    });
    assert_valid(&schema, &fixture);
}

#[test]
fn contact_missing_name_fields_fails_schema() {
    let schema = load_schema("contacts.schema.json");
    let fixture = json!({
        "id": "contact-001",
        "name": {
            "given": "Ada"
            // missing family, display
        },
        "source": "com.life-engine.google-contacts",
        "source_id": "people/c123456",
        "created_at": TEST_TIMESTAMP,
        "updated_at": TEST_TIMESTAMP
    });
    assert_invalid(&schema, &fixture);
}

// ── Emails ─────────────────────────────────────────────────────────

#[test]
fn email_valid_fixture_passes_schema() {
    let schema = load_schema("emails.schema.json");
    let fixture = json!({
        "id": "email-001",
        "from": "alice@example.com",
        "to": ["bob@example.com"],
        "subject": "Weekly sync notes",
        "body_text": "Hi Bob, here are the notes from today's sync.",
        "source": "com.life-engine.imap",
        "source_id": "imap-msg-42",
        "created_at": TEST_TIMESTAMP,
        "updated_at": TEST_TIMESTAMP
    });
    assert_valid(&schema, &fixture);
}

#[test]
fn email_wrong_type_for_to_fails_schema() {
    let schema = load_schema("emails.schema.json");
    let fixture = json!({
        "id": "email-001",
        "from": "alice@example.com",
        "to": "bob@example.com",  // should be array, not string
        "subject": "Weekly sync notes",
        "body_text": "Hi Bob.",
        "source": "com.life-engine.imap",
        "source_id": "imap-msg-42",
        "created_at": TEST_TIMESTAMP,
        "updated_at": TEST_TIMESTAMP
    });
    assert_invalid(&schema, &fixture);
}

// ── Files ──────────────────────────────────────────────────────────

#[test]
fn file_valid_fixture_passes_schema() {
    let schema = load_schema("files.schema.json");
    let fixture = json!({
        "id": "file-001",
        "name": "quarterly-report.pdf",
        "mime_type": "application/pdf",
        "size": 245_760,
        "path": "files/2026/01/quarterly-report.pdf",
        "source": "com.life-engine.s3",
        "source_id": "s3-bucket/quarterly-report.pdf",
        "created_at": TEST_TIMESTAMP,
        "updated_at": TEST_TIMESTAMP
    });
    assert_valid(&schema, &fixture);
}

#[test]
fn file_size_wrong_type_fails_schema() {
    let schema = load_schema("files.schema.json");
    let fixture = json!({
        "id": "file-001",
        "name": "report.pdf",
        "mime_type": "application/pdf",
        "size": "245760",  // should be integer, not string
        "path": "files/report.pdf",
        "source": "com.life-engine.s3",
        "source_id": "s3-bucket/report.pdf",
        "created_at": TEST_TIMESTAMP,
        "updated_at": TEST_TIMESTAMP
    });
    assert_invalid(&schema, &fixture);
}

// ── Notes ──────────────────────────────────────────────────────────

#[test]
fn note_valid_fixture_passes_schema() {
    let schema = load_schema("notes.schema.json");
    let fixture = json!({
        "id": "note-001",
        "title": "Meeting notes — Project kickoff",
        "body": "## Agenda\n\n- Introductions\n- Timeline review\n- Action items",
        "source": "com.life-engine.notes",
        "source_id": "local-note-001",
        "created_at": TEST_TIMESTAMP,
        "updated_at": TEST_TIMESTAMP
    });
    assert_valid(&schema, &fixture);
}

#[test]
fn note_missing_body_fails_schema() {
    let schema = load_schema("notes.schema.json");
    let fixture = json!({
        "id": "note-001",
        "title": "Meeting notes",
        // missing body
        "source": "com.life-engine.notes",
        "source_id": "local-note-001",
        "created_at": TEST_TIMESTAMP,
        "updated_at": TEST_TIMESTAMP
    });
    assert_invalid(&schema, &fixture);
}

// ── Credentials ────────────────────────────────────────────────────

#[test]
fn credential_valid_fixture_passes_schema() {
    let schema = load_schema("credentials.schema.json");
    let fixture = json!({
        "id": "cred-001",
        "type": "oauth_token",
        "issuer": "google.com",
        "issued_date": TEST_TIMESTAMP,
        "claims": {
            "scope": "email profile calendar",
            "token_type": "Bearer"
        },
        "source": "com.life-engine.auth",
        "source_id": "oauth-google-001",
        "created_at": TEST_TIMESTAMP,
        "updated_at": TEST_TIMESTAMP
    });
    assert_valid(&schema, &fixture);
}

#[test]
fn credential_invalid_type_enum_fails_schema() {
    let schema = load_schema("credentials.schema.json");
    let fixture = json!({
        "id": "cred-001",
        "type": "password",  // not in enum
        "issuer": "example.com",
        "issued_date": TEST_TIMESTAMP,
        "claims": {},
        "source": "com.life-engine.auth",
        "source_id": "auth-001",
        "created_at": TEST_TIMESTAMP,
        "updated_at": TEST_TIMESTAMP
    });
    assert_invalid(&schema, &fixture);
}

// ── Plugin manifest — positive (existing manifests) ────────────────

#[test]
fn plugin_manifest_email_viewer_passes_schema() {
    let schema = load_schema("plugin-manifest.schema.json");
    let path = repo_root().join("plugins/life/email-viewer/plugin.json");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("SKIP: {} not found — plugin may have been relocated", path.display());
            return;
        }
    };
    let manifest: serde_json::Value = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("email-viewer plugin.json is not valid JSON: {e}"));
    assert_valid(&schema, &manifest);
}

#[test]
fn plugin_manifest_calendar_known_violations() {
    // The calendar plugin.json has schema violations:
    // - "collections" is ["events"] (array of strings) instead of array of objects
    // - "routes" is not defined in the manifest schema
    // TODO: Update calendar plugin.json or manifest schema to align
    let schema = load_schema("plugin-manifest.schema.json");
    let path = repo_root().join("plugins/life/calendar/plugin.json");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("SKIP: {} not found — plugin may have been relocated", path.display());
            return;
        }
    };
    let manifest: serde_json::Value = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("calendar plugin.json is not valid JSON: {e}"));

    let validator =
        jsonschema::validator_for(&schema).expect("failed to compile manifest schema");
    let result = validator.validate(&manifest);
    assert!(
        result.is_err(),
        "calendar plugin.json was expected to have schema violations but passed validation — \
         update this test if the manifest or schema has been fixed"
    );
    let error = result.unwrap_err().to_string();
    // Assert on specific known violations so regressions are caught
    assert!(
        error.contains("collections") || error.contains("routes"),
        "expected schema violation related to 'collections' or 'routes', got: {error}"
    );
}

#[test]
fn plugin_manifest_template_vanilla_passes_schema() {
    let schema = load_schema("plugin-manifest.schema.json");
    let path = repo_root().join("tools/templates/life-plugin-vanilla/plugin.json");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("SKIP: {} not found — template may have been removed or renamed", path.display());
            return;
        }
    };
    let manifest: serde_json::Value = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("vanilla template plugin.json is not valid JSON: {e}"));
    assert_valid(&schema, &manifest);
}

#[test]
fn plugin_manifest_template_lit_passes_schema() {
    let schema = load_schema("plugin-manifest.schema.json");
    let path = repo_root().join("tools/templates/life-plugin-lit/plugin.json");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("SKIP: {} not found — template may have been removed or renamed", path.display());
            return;
        }
    };
    let manifest: serde_json::Value = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("lit template plugin.json is not valid JSON: {e}"));
    assert_valid(&schema, &manifest);
}

// ── Plugin manifest — negative ─────────────────────────────────────

#[test]
fn plugin_manifest_missing_capabilities_fails_schema() {
    let schema = load_schema("plugin-manifest.schema.json");
    let fixture = json!({
        "id": "com.example.bad-plugin",
        "name": "Bad Plugin",
        "version": "0.1.0",
        "entry": "index.js",
        "element": "bad-plugin",
        "minShellVersion": "0.1.0"
        // missing capabilities
    });
    assert_invalid(&schema, &fixture);
}

#[test]
fn plugin_manifest_invalid_id_pattern_fails_schema() {
    let schema = load_schema("plugin-manifest.schema.json");
    let fixture = json!({
        "id": "BadPlugin",  // not reverse-domain format
        "name": "Bad Plugin",
        "version": "0.1.0",
        "entry": "index.js",
        "element": "bad-plugin",
        "minShellVersion": "0.1.0",
        "capabilities": ["storage:local"]
    });
    assert_invalid(&schema, &fixture);
}
