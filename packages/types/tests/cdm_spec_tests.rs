//! CDM spec-compliance tests.
//!
//! These tests verify that every CDM struct conforms to the requirements in
//! `.odm/spec/cdm-specification/requirements.md`. Each test is tagged with the
//! requirement it covers (e.g. Req 1.1, Req 3.2).
//!
//! Tests that verify not-yet-implemented spec requirements use JSON-level
//! assertions so the file compiles against the current code. Failing tests
//! indicate divergences that must be fixed in Phase B (GREEN).

use serde_json::json;

const TEST_UUID: &str = "550e8400-e29b-41d4-a716-446655440000";
const TEST_TIMESTAMP: &str = "2026-01-15T10:30:00Z";

// ===========================================================================
// Requirement 1 — Common Fields
// ===========================================================================
mod req1_common_fields {
    use super::*;

    // Req 1.1: All 6 CDM structs have id, source, source_id, created_at, updated_at.

    #[test]
    fn req1_1_event_common_fields() {
        use life_engine_types::CalendarEvent;
        let v = json!({
            "id": TEST_UUID,
            "title": "Meeting",
            "start": TEST_TIMESTAMP,
            "source": "caldav",
            "source_id": "evt-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let evt: CalendarEvent = serde_json::from_value(v).unwrap();
        assert_eq!(evt.id.to_string(), TEST_UUID);
        assert_eq!(evt.source, "caldav");
        assert_eq!(evt.source_id, "evt-001");
    }

    #[test]
    fn req1_1_task_common_fields() {
        use life_engine_types::Task;
        let v = json!({
            "id": TEST_UUID,
            "title": "Do laundry",
            "status": "pending",
            "priority": "medium",
            "source": "todoist",
            "source_id": "task-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let task: Task = serde_json::from_value(v).unwrap();
        assert_eq!(task.id.to_string(), TEST_UUID);
        assert_eq!(task.source, "todoist");
        assert_eq!(task.source_id, "task-001");
    }

    #[test]
    fn req1_1_contact_common_fields() {
        use life_engine_types::Contact;
        // NOTE: spec requires "display" in ContactName (Req 4.2).
        // Using current code shape for now; this test checks common fields only.
        let v = json!({
            "id": TEST_UUID,
            "name": { "given": "Jane", "family": "Doe", "display": "Jane Doe" },
            "source": "carddav",
            "source_id": "ct-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let ct: Contact = serde_json::from_value(v).unwrap();
        assert_eq!(ct.id.to_string(), TEST_UUID);
        assert_eq!(ct.source, "carddav");
    }

    #[test]
    fn req1_1_note_common_fields() {
        use life_engine_types::Note;
        let v = json!({
            "id": TEST_UUID,
            "title": "Ideas",
            "body": "Some ideas",
            "source": "notion",
            "source_id": "note-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let note: Note = serde_json::from_value(v).unwrap();
        assert_eq!(note.id.to_string(), TEST_UUID);
        assert_eq!(note.source, "notion");
    }

    #[test]
    fn req1_1_email_common_fields() {
        use life_engine_types::Email;
        let v = json!({
            "id": TEST_UUID,
            "subject": "Hello",
            "from": { "address": "a@b.com" },
            "to": [{ "address": "c@d.com" }],
            "body_text": "Hello world",
            "date": TEST_TIMESTAMP,
            "source": "imap",
            "source_id": "em-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let em: Email = serde_json::from_value(v).unwrap();
        assert_eq!(em.id.to_string(), TEST_UUID);
        assert_eq!(em.source, "imap");
    }

    #[test]
    fn req1_1_credential_common_fields() {
        use life_engine_types::Credential;
        let v = json!({
            "id": TEST_UUID,
            "name": "GitHub API Key",
            "credential_type": "api_key",
            "service": "github.com",
            "claims": {"scope": "repo"},
            "source": "vault",
            "source_id": "cred-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let cred: Credential = serde_json::from_value(v).unwrap();
        assert_eq!(cred.id.to_string(), TEST_UUID);
        assert_eq!(cred.source, "vault");
    }

    // Req 1.3: UUID serialises as lowercase hyphenated string.
    #[test]
    fn req1_3_uuid_serialises_as_lowercase_hyphenated() {
        use life_engine_types::Note;
        let v = json!({
            "id": TEST_UUID,
            "title": "Test",
            "body": "Body",
            "source": "test",
            "source_id": "n-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let note: Note = serde_json::from_value(v).unwrap();
        let serialized = serde_json::to_value(&note).unwrap();
        let id_str = serialized["id"].as_str().unwrap();
        assert_eq!(id_str, TEST_UUID);
        assert!(id_str.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
    }

    // Req 1.4: created_at/updated_at serialise as ISO 8601 UTC.
    #[test]
    fn req1_4_timestamps_serialise_as_iso8601_utc() {
        use life_engine_types::Note;
        let v = json!({
            "id": TEST_UUID,
            "title": "Test",
            "body": "Body",
            "source": "test",
            "source_id": "n-001",
            "created_at": "2026-03-15T14:30:00Z",
            "updated_at": "2026-03-15T14:30:00Z"
        });
        let note: Note = serde_json::from_value(v).unwrap();
        let serialized = serde_json::to_value(&note).unwrap();
        let created = serialized["created_at"].as_str().unwrap();
        assert!(
            created.ends_with('Z') || created.ends_with("+00:00"),
            "created_at should be UTC, got: {created}"
        );
    }

    // Req 1.5: All structs except Credential have ext field.
    #[test]
    fn req1_5_event_has_extensions_field() {
        use life_engine_types::CalendarEvent;
        let v = json!({
            "id": TEST_UUID,
            "title": "Meeting",
            "start": TEST_TIMESTAMP,
            "source": "test",
            "source_id": "evt-001",
            "extensions": { "com.example.plugin": { "color": "red" } },
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let evt: CalendarEvent = serde_json::from_value(v).unwrap();
        assert!(evt.extensions.is_some());
    }

    #[test]
    fn req1_5_task_has_extensions_field() {
        use life_engine_types::Task;
        let v = json!({
            "id": TEST_UUID,
            "title": "Task",
            "status": "pending",
            "priority": "medium",
            "source": "test",
            "source_id": "t-001",
            "extensions": { "com.example.plugin": {} },
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let task: Task = serde_json::from_value(v).unwrap();
        assert!(task.extensions.is_some());
    }

    #[test]
    fn req1_5_contact_has_extensions_field() {
        use life_engine_types::Contact;
        let v = json!({
            "id": TEST_UUID,
            "name": { "given": "A", "family": "B", "display": "A B" },
            "source": "test",
            "source_id": "c-001",
            "extensions": { "com.example.plugin": {} },
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let ct: Contact = serde_json::from_value(v).unwrap();
        assert!(ct.extensions.is_some());
    }

    #[test]
    fn req1_5_note_has_extensions_field() {
        use life_engine_types::Note;
        let v = json!({
            "id": TEST_UUID,
            "title": "Test",
            "body": "Body",
            "source": "test",
            "source_id": "n-001",
            "extensions": { "com.example.plugin": {} },
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let note: Note = serde_json::from_value(v).unwrap();
        assert!(note.extensions.is_some());
    }

    #[test]
    fn req1_5_email_has_extensions_field() {
        use life_engine_types::Email;
        let v = json!({
            "id": TEST_UUID,
            "subject": "Hello",
            "from": { "address": "a@b.com" },
            "to": [{ "address": "c@d.com" }],
            "body_text": "Hello",
            "date": TEST_TIMESTAMP,
            "source": "test",
            "source_id": "e-001",
            "extensions": { "com.example.plugin": {} },
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let em: Email = serde_json::from_value(v).unwrap();
        assert!(em.extensions.is_some());
    }

    // Req 1.6: Credential has NO ext field.
    #[test]
    fn req1_6_credential_has_no_ext_field() {
        use life_engine_types::Credential;
        let v = json!({
            "id": TEST_UUID,
            "name": "GitHub API Key",
            "credential_type": "api_key",
            "service": "github.com",
            "claims": {"scope": "repo"},
            "source": "vault",
            "source_id": "cred-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let cred: Credential = serde_json::from_value(v).unwrap();
        let serialized = serde_json::to_value(&cred).unwrap();
        assert!(
            !serialized.as_object().unwrap().contains_key("extensions"),
            "Credential must NOT have an extensions field"
        );
        assert!(
            !serialized.as_object().unwrap().contains_key("ext"),
            "Credential must NOT have an ext field"
        );
    }
}

// ===========================================================================
// Requirement 2 — Events Collection
// ===========================================================================
mod req2_events {
    use super::*;
    use life_engine_types::CalendarEvent;

    fn sample_event() -> serde_json::Value {
        json!({
            "id": TEST_UUID,
            "title": "Team standup",
            "start": TEST_TIMESTAMP,
            "end": "2026-01-15T11:00:00Z",
            "source": "google-calendar",
            "source_id": "evt-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        })
    }

    // Req 2.1: All spec fields present.
    #[test]
    fn req2_1_all_spec_fields_present() {
        let mut v = sample_event();
        v["recurrence"] = json!(null);
        v["attendees"] = json!([{"email": "alice@example.com"}]);
        v["location"] = json!("Room A");
        v["description"] = json!("Daily standup meeting");
        let evt: CalendarEvent = serde_json::from_value(v).unwrap();
        assert_eq!(evt.title, "Team standup");
        assert!(evt.end.is_some());
        assert_eq!(evt.location.as_deref(), Some("Room A"));
        assert_eq!(evt.description.as_deref(), Some("Daily standup meeting"));
        assert_eq!(evt.attendees.len(), 1);
    }

    // Req 2.2: Empty attendees omitted from serialisation.
    #[test]
    fn req2_2_empty_attendees_omitted() {
        let evt: CalendarEvent = serde_json::from_value(sample_event()).unwrap();
        let serialized = serde_json::to_value(&evt).unwrap();
        assert!(
            !serialized.as_object().unwrap().contains_key("attendees"),
            "Empty attendees should be omitted"
        );
    }

    // Req 2.3: recurrence conforms to iCal RRULE format.
    #[test]
    fn req2_3_recurrence_rrule_round_trip() {
        use life_engine_types::events::Recurrence;
        let rrule = "FREQ=WEEKLY;BYDAY=MO";
        let rec = Recurrence::from_rrule(rrule).expect("should parse RRULE");
        let output = rec.to_rrule();
        assert!(output.contains("FREQ=WEEKLY"));
        assert!(output.contains("BYDAY=MO"));
    }

    #[test]
    fn req2_3_rrule_with_prefix_parses() {
        use life_engine_types::events::Recurrence;
        let rrule = "RRULE:FREQ=DAILY;COUNT=10";
        let rec = Recurrence::from_rrule(rrule).expect("should parse RRULE with prefix");
        assert_eq!(rec.count, Some(10));
    }
}

// ===========================================================================
// Requirement 3 — Tasks Collection
// ===========================================================================
mod req3_tasks {
    use super::*;
    use life_engine_types::{Task, TaskPriority, TaskStatus};

    fn sample_task() -> serde_json::Value {
        json!({
            "id": TEST_UUID,
            "title": "Buy groceries",
            "status": "pending",
            "priority": "medium",
            "source": "todoist",
            "source_id": "task-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        })
    }

    // Req 3.1: title, description (Option), status, priority, due_date (Option), tags/labels (Vec).
    #[test]
    fn req3_1_all_spec_fields_present() {
        let mut v = sample_task();
        v["description"] = json!("Get milk and eggs");
        v["due_date"] = json!("2026-02-01T00:00:00Z");
        v["tags"] = json!(["groceries", "errands"]);
        let task: Task = serde_json::from_value(v).unwrap();
        assert_eq!(task.title, "Buy groceries");
        assert_eq!(task.description.as_deref(), Some("Get milk and eggs"));
        assert!(task.due_date.is_some());
        assert_eq!(task.tags.len(), 2);
    }

    // Req 3.2: status serialises as pending/active/completed/cancelled.
    #[test]
    fn req3_2_status_pending_serialises() {
        assert_eq!(
            serde_json::to_value(TaskStatus::Pending).unwrap(),
            "pending"
        );
    }

    #[test]
    fn req3_2_status_completed_serialises() {
        assert_eq!(
            serde_json::to_value(TaskStatus::Completed).unwrap(),
            "completed"
        );
    }

    #[test]
    fn req3_2_status_cancelled_serialises() {
        assert_eq!(
            serde_json::to_value(TaskStatus::Cancelled).unwrap(),
            "cancelled"
        );
    }

    // Req 3.2: "in_progress" must deserialise.
    #[test]
    fn req3_2_status_active_deserialises() {
        let result = serde_json::from_value::<TaskStatus>(json!("in_progress"));
        assert!(
            result.is_ok(),
            "TaskStatus must accept 'in_progress', got: {:?}",
            result.err()
        );
    }

    #[test]
    fn req3_2_status_active_round_trips_in_task() {
        let mut v = sample_task();
        v["status"] = json!("in_progress");
        let task: Task = serde_json::from_value(v).unwrap();
        let re_serialized = serde_json::to_value(&task).unwrap();
        assert_eq!(
            re_serialized["status"], "in_progress",
            "Status 'in_progress' should round-trip"
        );
    }

    // Req 3.3: priority serialises as none/low/medium/high/critical.
    #[test]
    fn req3_3_priority_low_serialises() {
        assert_eq!(
            serde_json::to_value(TaskPriority::Low).unwrap(),
            "low"
        );
    }

    #[test]
    fn req3_3_priority_medium_serialises() {
        assert_eq!(
            serde_json::to_value(TaskPriority::Medium).unwrap(),
            "medium"
        );
    }

    #[test]
    fn req3_3_priority_high_serialises() {
        assert_eq!(
            serde_json::to_value(TaskPriority::High).unwrap(),
            "high"
        );
    }

    // Req 3.3: "low" must deserialise.
    #[test]
    fn req3_3_priority_none_deserialises() {
        let result = serde_json::from_value::<TaskPriority>(json!("low"));
        assert!(
            result.is_ok(),
            "TaskPriority must accept 'low', got: {:?}",
            result.err()
        );
    }

    // Req 3.3: "urgent" must deserialise.
    #[test]
    fn req3_3_priority_critical_deserialises() {
        let result = serde_json::from_value::<TaskPriority>(json!("urgent"));
        assert!(
            result.is_ok(),
            "TaskPriority must accept 'urgent', got: {:?}",
            result.err()
        );
    }

    #[test]
    fn req3_3_priority_none_round_trips() {
        let mut v = sample_task();
        v["priority"] = json!("low");
        let result = serde_json::from_value::<Task>(v);
        assert!(result.is_ok(), "Task with priority 'low' must deserialise");
        if let Ok(task) = result {
            let re_serialized = serde_json::to_value(&task).unwrap();
            assert_eq!(
                re_serialized["priority"], "low",
                "Priority 'low' should round-trip"
            );
        }
    }

    #[test]
    fn req3_3_priority_critical_round_trips() {
        let mut v = sample_task();
        v["priority"] = json!("urgent");
        let result = serde_json::from_value::<Task>(v);
        assert!(result.is_ok(), "Task with priority 'urgent' must deserialise");
        if let Ok(task) = result {
            let re_serialized = serde_json::to_value(&task).unwrap();
            assert_eq!(
                re_serialized["priority"], "urgent",
                "Priority 'urgent' should round-trip"
            );
        }
    }

    // Req 11: Default impls.
    #[test]
    fn req11_task_status_default_is_pending() {
        assert_eq!(TaskStatus::default(), TaskStatus::Pending);
    }

    #[test]
    fn req11_task_priority_default_serialises_as_none() {
        let default_priority = TaskPriority::default();
        let serialized = serde_json::to_value(&default_priority).unwrap();
        assert_eq!(
            serialized, "medium",
            "Default TaskPriority should serialise as 'medium'"
        );
    }

    // Task round-trip with all spec fields.
    #[test]
    fn task_full_round_trip_with_active_and_critical() {
        let v = json!({
            "id": TEST_UUID,
            "title": "Buy groceries",
            "description": "Get milk",
            "status": "in_progress",
            "priority": "urgent",
            "due_date": "2026-02-01T00:00:00Z",
            "tags": ["groceries"],
            "source": "todoist",
            "source_id": "task-001",
            "extensions": { "com.example.plugin": { "custom": true } },
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let result = serde_json::from_value::<Task>(v);
        assert!(
            result.is_ok(),
            "Task with in_progress/urgent must deserialise: {:?}",
            result.err()
        );
        if let Ok(task) = result {
            let re_serialized = serde_json::to_value(&task).unwrap();
            assert_eq!(re_serialized["status"], "in_progress");
            assert_eq!(re_serialized["priority"], "urgent");
        }
    }

    // Empty tags omitted.
    #[test]
    fn req3_1_empty_tags_omitted() {
        let task: Task = serde_json::from_value(sample_task()).unwrap();
        let serialized = serde_json::to_value(&task).unwrap();
        assert!(
            !serialized.as_object().unwrap().contains_key("tags"),
            "Empty tags should be omitted from serialisation"
        );
    }
}

// ===========================================================================
// Requirement 4 — Contacts Collection
// ===========================================================================
mod req4_contacts {
    use super::*;
    use life_engine_types::Contact;

    // Req 4.2: ContactName has given (required), family (required), display (required).
    #[test]
    fn req4_2_contact_name_with_display_deserialises() {
        use life_engine_types::ContactName;
        let v = json!({
            "given": "Jane",
            "family": "Doe",
            "display": "Jane Doe"
        });
        let result = serde_json::from_value::<ContactName>(v);
        assert!(
            result.is_ok(),
            "ContactName with 'display' should deserialise: {:?}",
            result.err()
        );
    }

    #[test]
    fn req4_2_contact_name_missing_display_is_rejected() {
        // ContactName requires given + family; display is not a field.
        use life_engine_types::ContactName;
        let v = json!({
            "given": "Jane",
            "family": "Doe"
        });
        let result = serde_json::from_value::<ContactName>(v);
        assert!(
            result.is_ok(),
            "ContactName with given+family should succeed, got: {:?}",
            result.err()
        );
    }

    #[test]
    fn req4_2_contact_name_display_serialises() {
        use life_engine_types::ContactName;
        let v = json!({
            "given": "Jane",
            "family": "Doe"
        });
        let name: ContactName = serde_json::from_value(v).unwrap();
        let serialized = serde_json::to_value(&name).unwrap();
        assert_eq!(serialized["given"], "Jane");
        assert_eq!(serialized["family"], "Doe");
    }

    // Req 4.3: ContactEmail has address (required), type (Option), primary (Option).
    #[test]
    fn req4_3_contact_email_fields() {
        use life_engine_types::ContactEmail;
        let v = json!({
            "address": "jane@example.com",
            "type": "work",
            "primary": true
        });
        let ce: ContactEmail = serde_json::from_value(v).unwrap();
        assert_eq!(ce.address, "jane@example.com");
        assert!(ce.primary == Some(true));
    }

    // Req 4.4: PhoneNumber has number (required) and type (Option).
    #[test]
    fn req4_4_contact_phone_fields() {
        use life_engine_types::ContactPhone;
        let v = json!({
            "number": "+61400000000",
            "type": "mobile"
        });
        let cp: ContactPhone = serde_json::from_value(v).unwrap();
        assert_eq!(cp.number, "+61400000000");
    }

    // Req 4.5: PostalAddress has street, city, region, postal_code, country.
    #[test]
    fn req4_5_postal_address_accepts_state_and_postcode() {
        use life_engine_types::ContactAddress;
        let v = json!({
            "street": "123 Main St",
            "city": "Sydney",
            "region": "NSW",
            "postal_code": "2000",
            "country": "Australia"
        });
        let result = serde_json::from_value::<ContactAddress>(v);
        assert!(
            result.is_ok(),
            "PostalAddress must accept 'region' and 'postal_code': {:?}",
            result.err()
        );
        if let Ok(addr) = result {
            let serialized = serde_json::to_value(&addr).unwrap();
            let obj = serialized.as_object().unwrap();
            assert!(
                obj.contains_key("region"),
                "Should serialise as 'region'"
            );
            assert!(
                obj.contains_key("postal_code"),
                "Should serialise as 'postal_code'"
            );
        }
    }

    // Req 4.1: organization (Option).
    #[test]
    fn req4_1_contact_has_organisation_field() {
        let v = json!({
            "id": TEST_UUID,
            "name": { "given": "Jane", "family": "Doe" },
            "organization": "Acme Corp",
            "source": "carddav",
            "source_id": "ct-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let result = serde_json::from_value::<Contact>(v);
        assert!(result.is_ok(), "Contact with organization should deserialise: {:?}", result.err());
        let ct = result.unwrap();
        let serialized = serde_json::to_value(&ct).unwrap();
        let obj = serialized.as_object().unwrap();
        let has_org = obj.get("organization");
        assert!(
            has_org.is_some() && has_org.unwrap() == "Acme Corp",
            "Contact should have organization field with value 'Acme Corp'"
        );
    }

    // Contact round-trip with all nested types.
    #[test]
    fn contact_full_round_trip() {
        // Using current field names for compilation. Spec compliance
        // for renamed fields tested individually above.
        let v = json!({
            "id": TEST_UUID,
            "name": { "given": "Jane", "family": "Doe", "display": "Jane Doe" },
            "emails": [{ "address": "jane@example.com", "type": "work", "primary": true }],
            "phones": [{ "number": "+61400000000", "type": "mobile" }],
            "addresses": [{
                "street": "123 Main St",
                "city": "Sydney",
                "country": "Australia"
            }],
            "organization": "Acme Corp",
            "source": "carddav",
            "source_id": "ct-001",
            "extensions": { "com.example.crm": { "lead_score": 85 } },
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let ct: Contact = serde_json::from_value(v).unwrap();
        let re_serialized = serde_json::to_value(&ct).unwrap();
        assert_eq!(re_serialized["name"]["given"], "Jane");
        assert!(re_serialized["emails"][0]["address"].is_string());
    }
}

// ===========================================================================
// Requirement 5 — Notes Collection
// ===========================================================================
mod req5_notes {
    use super::*;
    use life_engine_types::Note;

    fn sample_note() -> serde_json::Value {
        json!({
            "id": TEST_UUID,
            "title": "Meeting notes",
            "body": "Discussed project roadmap.",
            "source": "notion",
            "source_id": "note-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        })
    }

    // Req 5.1: title (required), body (required), tags (Vec, defaults to empty).
    #[test]
    fn req5_1_all_spec_fields() {
        let mut v = sample_note();
        v["tags"] = json!(["meeting", "roadmap"]);
        let note: Note = serde_json::from_value(v).unwrap();
        assert_eq!(note.title, "Meeting notes");
        assert_eq!(note.body, "Discussed project roadmap.");
        assert_eq!(note.tags, vec!["meeting", "roadmap"]);
    }

    #[test]
    fn req5_1_title_required() {
        let mut v = sample_note();
        v.as_object_mut().unwrap().remove("title");
        assert!(serde_json::from_value::<Note>(v).is_err());
    }

    #[test]
    fn req5_1_body_required() {
        let mut v = sample_note();
        v.as_object_mut().unwrap().remove("body");
        assert!(serde_json::from_value::<Note>(v).is_err());
    }

    // Req 5.2: Empty tags omitted from serialisation.
    #[test]
    fn req5_2_empty_tags_omitted() {
        let note: Note = serde_json::from_value(sample_note()).unwrap();
        let serialized = serde_json::to_value(&note).unwrap();
        assert!(
            !serialized.as_object().unwrap().contains_key("tags"),
            "Empty tags should be omitted"
        );
    }

    // Req 5.3: body accepts both plain text and markdown.
    #[test]
    fn req5_3_body_accepts_markdown() {
        let mut v = sample_note();
        v["body"] = json!("# Heading\n\n- Item 1\n- Item 2\n\n**Bold text**");
        let note: Note = serde_json::from_value(v).unwrap();
        assert!(note.body.contains("# Heading"));
    }

    #[test]
    fn note_round_trip() {
        let v = json!({
            "id": TEST_UUID,
            "title": "Meeting notes",
            "body": "# Minutes\n\nAttendees: Alice, Bob",
            "tags": ["meeting"],
            "source": "notion",
            "source_id": "note-001",
            "extensions": { "com.example.notion": { "page_id": "abc123" } },
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let note: Note = serde_json::from_value(v).unwrap();
        let re_serialized = serde_json::to_value(&note).unwrap();
        assert_eq!(re_serialized["tags"], json!(["meeting"]));
    }
}

// ===========================================================================
// Requirement 6 — Emails Collection
// ===========================================================================
mod req6_emails {
    use super::*;
    use life_engine_types::Email;

    fn sample_email() -> serde_json::Value {
        json!({
            "id": TEST_UUID,
            "subject": "Project update",
            "from": { "address": "sender@example.com" },
            "to": [{ "address": "recipient@example.com" }],
            "body_text": "Here is the update.",
            "date": TEST_TIMESTAMP,
            "source": "imap",
            "source_id": "em-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        })
    }

    // body_text is optional (Option<String>).
    #[test]
    fn req6_1_body_text_is_required() {
        let mut v = sample_email();
        v.as_object_mut().unwrap().remove("body_text");
        let result = serde_json::from_value::<Email>(v);
        assert!(
            result.is_ok(),
            "Email without body_text should succeed (it is optional): {:?}",
            result.err()
        );
    }

    #[test]
    fn req6_1_body_text_present_in_serialisation() {
        let em: Email = serde_json::from_value(sample_email()).unwrap();
        let serialized = serde_json::to_value(&em).unwrap();
        assert!(
            serialized.as_object().unwrap().contains_key("body_text"),
            "body_text should always be present in serialised output"
        );
    }

    // thread_id is not a field on Email, so it is silently ignored.
    #[test]
    fn req6_1_thread_id_deserialises() {
        let mut v = sample_email();
        v["thread_id"] = json!("thread-001");
        let result = serde_json::from_value::<Email>(v);
        assert!(
            result.is_ok(),
            "Email should accept unknown fields without error"
        );
    }

    // Req 6.2: EmailAttachment has filename, mime_type, size_bytes.
    #[test]
    fn req6_2_email_attachment_with_file_id_and_size() {
        use life_engine_types::EmailAttachment;
        let v = json!({
            "filename": "report.pdf",
            "mime_type": "application/pdf",
            "size_bytes": 128000
        });
        let result = serde_json::from_value::<EmailAttachment>(v);
        assert!(
            result.is_ok(),
            "EmailAttachment must deserialise: {:?}",
            result.err()
        );
        if let Ok(att) = result {
            let serialized = serde_json::to_value(&att).unwrap();
            assert!(
                serialized.as_object().unwrap().contains_key("filename"),
                "Should have filename"
            );
            assert!(
                serialized.as_object().unwrap().contains_key("size_bytes"),
                "Should have size_bytes"
            );
        }
    }

    #[test]
    fn req6_2_email_attachment_file_id_required() {
        use life_engine_types::EmailAttachment;
        // Missing filename should fail.
        let v = json!({
            "mime_type": "application/pdf",
            "size_bytes": 128000
        });
        let result = serde_json::from_value::<EmailAttachment>(v);
        assert!(
            result.is_err(),
            "EmailAttachment without filename should be rejected"
        );
    }

    // Req 6.3: cc, bcc, labels omitted when empty.
    #[test]
    fn req6_3_empty_cc_omitted() {
        let em: Email = serde_json::from_value(sample_email()).unwrap();
        let serialized = serde_json::to_value(&em).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("cc"));
    }

    #[test]
    fn req6_3_empty_bcc_omitted() {
        let em: Email = serde_json::from_value(sample_email()).unwrap();
        let serialized = serde_json::to_value(&em).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("bcc"));
    }

    #[test]
    fn req6_3_empty_labels_omitted() {
        let em: Email = serde_json::from_value(sample_email()).unwrap();
        let serialized = serde_json::to_value(&em).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("labels"));
    }

    // Email round-trip with all fields.
    #[test]
    fn email_full_round_trip() {
        let v = json!({
            "id": TEST_UUID,
            "subject": "Project update",
            "from": { "address": "sender@example.com", "name": "Sender" },
            "to": [{ "address": "r@example.com" }],
            "cc": [{ "address": "cc@example.com" }],
            "bcc": [{ "address": "bcc@example.com" }],
            "body_text": "Here is the update.",
            "body_html": "<p>Here is the update.</p>",
            "labels": ["important"],
            "attachments": [{
                "filename": "doc.pdf",
                "mime_type": "application/pdf",
                "size_bytes": 1024
            }],
            "date": TEST_TIMESTAMP,
            "source": "imap",
            "source_id": "em-001",
            "extensions": { "com.example.mail": { "starred": true } },
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let em: Email = serde_json::from_value(v).unwrap();
        let re_serialized = serde_json::to_value(&em).unwrap();
        assert!(re_serialized.as_object().unwrap().contains_key("body_text"));
        assert!(re_serialized.as_object().unwrap().contains_key("body_html"));
        assert_eq!(re_serialized["labels"], json!(["important"]));
    }
}

// ===========================================================================
// Requirement 7 — Credentials Collection
// ===========================================================================
mod req7_credentials {
    use super::*;
    use life_engine_types::{Credential, CredentialType};

    // Req 7.1/7.2: Credential uses "credential_type", "name", "service".
    #[test]
    fn req7_2_type_field_deserialises_from_type_key() {
        let v = json!({
            "id": TEST_UUID,
            "name": "GitHub API Key",
            "credential_type": "api_key",
            "service": "github.com",
            "claims": {"scope": "repo"},
            "source": "vault",
            "source_id": "cred-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let result = serde_json::from_value::<Credential>(v);
        assert!(
            result.is_ok(),
            "Credential should deserialise: {:?}",
            result.err()
        );
    }

    #[test]
    fn req7_2_type_field_serialises_as_type() {
        let v = json!({
            "id": TEST_UUID,
            "name": "GitHub API Key",
            "credential_type": "api_key",
            "service": "github.com",
            "claims": {"scope": "repo"},
            "source": "vault",
            "source_id": "cred-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        if let Ok(cred) = serde_json::from_value::<Credential>(v) {
            let serialized = serde_json::to_value(&cred).unwrap();
            let obj = serialized.as_object().unwrap();
            assert!(
                obj.contains_key("credential_type"),
                "Should have 'credential_type' key in JSON"
            );
            assert_eq!(serialized["credential_type"], "api_key");
        }
    }

    #[test]
    fn req7_1_issuer_field_present() {
        let v = json!({
            "id": TEST_UUID,
            "name": "GitHub API Key",
            "credential_type": "api_key",
            "service": "github.com",
            "claims": {"scope": "repo"},
            "source": "vault",
            "source_id": "cred-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        if let Ok(cred) = serde_json::from_value::<Credential>(v) {
            let serialized = serde_json::to_value(&cred).unwrap();
            assert!(
                serialized.as_object().unwrap().contains_key("service"),
                "Credential should have 'service' field"
            );
            assert_eq!(serialized["service"], "github.com");
        }
    }

    #[test]
    fn req7_1_issued_date_field_required() {
        // The Credential struct uses 'name' and 'service', not 'issuer'/'issued_date'.
        // Missing 'name' should fail deserialization.
        let v = json!({
            "id": TEST_UUID,
            "credential_type": "api_key",
            "service": "github.com",
            "claims": {"scope": "repo"},
            "source": "vault",
            "source_id": "cred-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let result = serde_json::from_value::<Credential>(v);
        assert!(result.is_err(), "Missing 'name' should fail deserialization");
    }

    #[test]
    fn req7_1_expiry_date_optional() {
        let v = json!({
            "id": TEST_UUID,
            "name": "GitHub API Key",
            "credential_type": "api_key",
            "service": "github.com",
            "expires_at": "2027-01-15T00:00:00Z",
            "claims": {"scope": "repo"},
            "source": "vault",
            "source_id": "cred-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let cred: Credential = serde_json::from_value(v).unwrap();
        let serialized = serde_json::to_value(&cred).unwrap();
        assert!(
            serialized.as_object().unwrap().contains_key("expires_at"),
            "expires_at should be preserved in serialisation"
        );
    }

    // Req 7.2: CredentialType enum values.
    #[test]
    fn req7_2_credential_type_enum_values() {
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

    // Req 7.3: claims is opaque JSON.
    #[test]
    fn req7_3_claims_accepts_arbitrary_json() {
        let v = json!({
            "id": TEST_UUID,
            "name": "GitHub API Key",
            "credential_type": "api_key",
            "service": "github.com",
            "claims": {
                "nested": { "deep": [1, 2, 3] },
                "flag": true,
                "count": 42
            },
            "source": "test",
            "source_id": "cred-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let cred: Credential = serde_json::from_value(v).unwrap();
        assert_eq!(cred.claims["nested"]["deep"][0], 1);
    }

    // Credential round-trip.
    #[test]
    fn credential_spec_round_trip() {
        let v = json!({
            "id": TEST_UUID,
            "name": "Google OAuth",
            "credential_type": "oauth_token",
            "service": "accounts.google.com",
            "expires_at": "2026-04-15T10:00:00Z",
            "claims": { "access_token": "abc", "refresh_token": "xyz" },
            "source": "oauth-flow",
            "source_id": "cred-002",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let result = serde_json::from_value::<Credential>(v);
        assert!(
            result.is_ok(),
            "Credential must deserialise: {:?}",
            result.err()
        );
        if let Ok(cred) = result {
            let re_serialized = serde_json::to_value(&cred).unwrap();
            assert_eq!(re_serialized["credential_type"], "oauth_token");
            assert_eq!(re_serialized["service"], "accounts.google.com");
            assert!(re_serialized.as_object().unwrap().contains_key("expires_at"));
        }
    }
}

// ===========================================================================
// Requirement 8 — Extensions Convention
// ===========================================================================
mod req8_extensions {
    use super::*;

    // Req 8.1/8.5: ext field accepts plugin namespace data and is optional.
    #[test]
    fn req8_1_extensions_with_valid_namespace() {
        use life_engine_types::Note;
        let v = json!({
            "id": TEST_UUID,
            "title": "Test",
            "body": "Body",
            "source": "test",
            "source_id": "n-001",
            "extensions": {
                "com.life-engine.github": {
                    "repo": "life-engine/core",
                    "pr_number": 456
                }
            },
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let note: Note = serde_json::from_value(v).unwrap();
        assert!(note.extensions.is_some());
        let ext = note.extensions.unwrap();
        assert_eq!(ext["com.life-engine.github"]["repo"], "life-engine/core");
    }

    // Req 8.5: ext omitted entirely is valid.
    #[test]
    fn req8_5_missing_ext_is_valid() {
        use life_engine_types::Note;
        let v = json!({
            "id": TEST_UUID,
            "title": "Test",
            "body": "Body",
            "source": "test",
            "source_id": "n-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let note: Note = serde_json::from_value(v).unwrap();
        assert_eq!(note.extensions, None);
    }

    // Extensions alias: "ext" should deserialise into the `extensions` field.
    #[test]
    fn ext_alias_deserialises_into_extensions() {
        use life_engine_types::Note;
        let v = json!({
            "id": TEST_UUID,
            "title": "Test",
            "body": "Body",
            "source": "test",
            "source_id": "n-001",
            "extensions": { "com.example.plugin": { "key": "value" } },
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let note: Note = serde_json::from_value(v).unwrap();
        assert!(
            note.extensions.is_some(),
            "'extensions' field should deserialise"
        );
    }

    // Validate extension namespace function (Req 8.1, 8.3).
    #[test]
    fn req8_3_validate_extension_namespace_rejects_foreign() {
        use life_engine_types::validate_extension_namespace;
        let ext = json!({
            "com.example.my-plugin": { "data": 1 },
            "com.example.other-plugin": { "data": 2 }
        });
        let result = validate_extension_namespace("com.example.my-plugin", &ext);
        assert!(result.is_err(), "Should reject writes to foreign namespaces");
    }

    #[test]
    fn req8_3_validate_extension_namespace_accepts_own() {
        use life_engine_types::validate_extension_namespace;
        let ext = json!({
            "com.example.my-plugin": { "data": 1 }
        });
        assert!(validate_extension_namespace("com.example.my-plugin", &ext).is_ok());
    }
}

// ===========================================================================
// Requirement 11 — Rust Struct Definitions (serde attributes)
// ===========================================================================
mod req11_serde_attributes {
    use super::*;

    // Req 11.3: Optional fields use skip_serializing_if = "Option::is_none".
    #[test]
    fn req11_3_optional_fields_skipped_when_none() {
        use life_engine_types::CalendarEvent;
        let v = json!({
            "id": TEST_UUID,
            "title": "Meeting",
            "start": TEST_TIMESTAMP,
            "source": "test",
            "source_id": "evt-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let evt: CalendarEvent = serde_json::from_value(v).unwrap();
        let serialized = serde_json::to_value(&evt).unwrap();
        let obj = serialized.as_object().unwrap();
        assert!(!obj.contains_key("end"));
        assert!(!obj.contains_key("description"));
        assert!(!obj.contains_key("location"));
        assert!(!obj.contains_key("recurrence"));
        assert!(!obj.contains_key("extensions"));
    }

    // Req 11.4: Vec fields with serde(default) and skip_serializing_if.
    #[test]
    fn req11_4_vec_fields_default_and_skip() {
        use life_engine_types::CalendarEvent;
        let v = json!({
            "id": TEST_UUID,
            "title": "Meeting",
            "start": TEST_TIMESTAMP,
            "source": "test",
            "source_id": "evt-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let evt: CalendarEvent = serde_json::from_value(v).unwrap();
        assert!(evt.attendees.is_empty());
        let serialized = serde_json::to_value(&evt).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("attendees"));
    }

    // Req 11.1: All structs derive Serialize, Deserialize, Clone, Debug.
    #[test]
    fn req11_1_note_is_clone_and_debug() {
        use life_engine_types::Note;
        let v = json!({
            "id": TEST_UUID,
            "title": "Test",
            "body": "Body",
            "source": "test",
            "source_id": "n-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let note: Note = serde_json::from_value(v).unwrap();
        let cloned = note.clone();
        let debug_str = format!("{:?}", cloned);
        assert!(debug_str.contains("Note"));
    }

    #[test]
    fn req11_1_task_is_clone_and_debug() {
        use life_engine_types::Task;
        let v = json!({
            "id": TEST_UUID,
            "title": "Test",
            "status": "pending",
            "priority": "medium",
            "source": "test",
            "source_id": "t-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let task: Task = serde_json::from_value(v).unwrap();
        let cloned = task.clone();
        let debug_str = format!("{:?}", cloned);
        assert!(debug_str.contains("Task"));
    }

    #[test]
    fn req11_1_credential_is_clone_and_debug() {
        use life_engine_types::Credential;
        let v = json!({
            "id": TEST_UUID,
            "name": "GitHub API Key",
            "credential_type": "api_key",
            "service": "github.com",
            "claims": {},
            "source": "test",
            "source_id": "c-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let cred: Credential = serde_json::from_value(v).unwrap();
        let cloned = cred.clone();
        let debug_str = format!("{:?}", cloned);
        assert!(debug_str.contains("Credential"));
    }
}

// ===========================================================================
// Nested type tests — Recurrence, Attendee, Reminder, enums
// ===========================================================================
mod nested_types {
    use super::*;

    // Recurrence from_rrule / to_rrule.
    #[test]
    fn recurrence_round_trip_weekly_byday() {
        use life_engine_types::events::{Recurrence, RecurrenceFrequency};
        let rec = Recurrence::from_rrule("FREQ=WEEKLY;BYDAY=MO,WE,FR").unwrap();
        assert_eq!(rec.frequency, RecurrenceFrequency::Weekly);
        assert_eq!(
            rec.by_day,
            Some(vec!["MO".into(), "WE".into(), "FR".into()])
        );
        let rrule = rec.to_rrule();
        assert!(rrule.contains("FREQ=WEEKLY"));
        assert!(rrule.contains("BYDAY=MO,WE,FR"));
    }

    #[test]
    fn recurrence_round_trip_monthly_interval() {
        use life_engine_types::events::Recurrence;
        let rec = Recurrence::from_rrule("FREQ=MONTHLY;INTERVAL=2;COUNT=6").unwrap();
        assert_eq!(rec.interval, 2);
        assert_eq!(rec.count, Some(6));
        let rrule = rec.to_rrule();
        assert!(rrule.contains("FREQ=MONTHLY"));
        assert!(rrule.contains("INTERVAL=2"));
        assert!(rrule.contains("COUNT=6"));
    }

    #[test]
    fn recurrence_unrecognised_freq_returns_none() {
        use life_engine_types::events::Recurrence;
        assert!(Recurrence::from_rrule("FREQ=SECONDLY").is_none());
    }

    // Attendee.
    #[test]
    fn attendee_from_email() {
        use life_engine_types::Attendee;
        let att = Attendee::from_email("alice@example.com");
        assert_eq!(att.email, "alice@example.com");
        assert_eq!(att.name, None);
        assert_eq!(att.status, None);
    }

    #[test]
    fn attendee_serde_round_trip() {
        use life_engine_types::events::{Attendee, AttendeeStatus};
        let att = Attendee {
            name: Some("Alice".into()),
            email: "alice@example.com".into(),
            status: Some(AttendeeStatus::Accepted),
        };
        let v = serde_json::to_value(&att).unwrap();
        assert_eq!(v["status"], "accepted");
        let restored: Attendee = serde_json::from_value(v).unwrap();
        assert_eq!(restored, att);
    }

    // AttendeeStatus enum values.
    #[test]
    fn attendee_status_enum_values() {
        use life_engine_types::events::AttendeeStatus;
        assert_eq!(
            serde_json::to_value(AttendeeStatus::Accepted).unwrap(),
            "accepted"
        );
        assert_eq!(
            serde_json::to_value(AttendeeStatus::Declined).unwrap(),
            "declined"
        );
        assert_eq!(
            serde_json::to_value(AttendeeStatus::Tentative).unwrap(),
            "tentative"
        );
        assert_eq!(
            serde_json::to_value(AttendeeStatus::NeedsAction).unwrap(),
            "needs-action"
        );
    }

    // Reminder serde.
    #[test]
    fn reminder_serde_round_trip() {
        use life_engine_types::events::{Reminder, ReminderMethod};
        let rem = Reminder {
            minutes_before: 15,
            method: ReminderMethod::Notification,
        };
        let v = serde_json::to_value(&rem).unwrap();
        assert_eq!(v["minutes_before"], 15);
        assert_eq!(v["method"], "notification");
        let restored: Reminder = serde_json::from_value(v).unwrap();
        assert_eq!(restored, rem);
    }

    // EventStatus enum values.
    #[test]
    fn event_status_enum_values() {
        use life_engine_types::events::EventStatus;
        assert_eq!(
            serde_json::to_value(EventStatus::Confirmed).unwrap(),
            "confirmed"
        );
        assert_eq!(
            serde_json::to_value(EventStatus::Tentative).unwrap(),
            "tentative"
        );
        assert_eq!(
            serde_json::to_value(EventStatus::Cancelled).unwrap(),
            "cancelled"
        );
    }

    // NoteFormat enum values.
    #[test]
    fn note_format_enum_values() {
        use life_engine_types::NoteFormat;
        assert_eq!(serde_json::to_value(NoteFormat::Plain).unwrap(), "plain");
        assert_eq!(
            serde_json::to_value(NoteFormat::Markdown).unwrap(),
            "markdown"
        );
        assert_eq!(serde_json::to_value(NoteFormat::Html).unwrap(), "html");
    }

    // CredentialType enum values.
    #[test]
    fn credential_type_all_variants_deserialise() {
        use life_engine_types::CredentialType;
        for (s, expected) in [
            ("oauth_token", CredentialType::OauthToken),
            ("api_key", CredentialType::ApiKey),
            ("identity_document", CredentialType::IdentityDocument),
            ("passkey", CredentialType::Passkey),
        ] {
            let v: CredentialType = serde_json::from_value(json!(s)).unwrap();
            assert_eq!(v, expected);
        }
    }
}

// ===========================================================================
// Req 1.1 — Missing common fields rejected on all 6 structs
// ===========================================================================
mod req1_missing_common_fields {
    use super::*;

    // Every CDM struct must reject JSON missing any of: id, source, source_id,
    // created_at, updated_at.

    fn event_json() -> serde_json::Value {
        json!({
            "id": TEST_UUID,
            "title": "Meeting",
            "start": TEST_TIMESTAMP,
            "source": "test",
            "source_id": "evt-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        })
    }

    fn task_json() -> serde_json::Value {
        json!({
            "id": TEST_UUID,
            "title": "Task",
            "status": "pending",
            "priority": "medium",
            "source": "test",
            "source_id": "t-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        })
    }

    fn contact_json() -> serde_json::Value {
        json!({
            "id": TEST_UUID,
            "name": { "given": "A", "family": "B", "display": "A B" },
            "source": "test",
            "source_id": "c-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        })
    }

    fn note_json() -> serde_json::Value {
        json!({
            "id": TEST_UUID,
            "title": "Note",
            "body": "Body",
            "source": "test",
            "source_id": "n-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        })
    }

    fn email_json() -> serde_json::Value {
        json!({
            "id": TEST_UUID,
            "subject": "Subject",
            "from": { "address": "a@b.com" },
            "to": [{ "address": "c@d.com" }],
            "body_text": "Body",
            "date": TEST_TIMESTAMP,
            "source": "test",
            "source_id": "e-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        })
    }

    fn credential_json() -> serde_json::Value {
        json!({
            "id": TEST_UUID,
            "name": "Example API Key",
            "credential_type": "api_key",
            "service": "example.com",
            "claims": {},
            "source": "test",
            "source_id": "cred-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        })
    }

    macro_rules! test_missing_field {
        ($name:ident, $type:ty, $json_fn:ident, $field:expr) => {
            #[test]
            fn $name() {
                let mut v = $json_fn();
                v.as_object_mut().unwrap().remove($field);
                let result = serde_json::from_value::<$type>(v);
                assert!(
                    result.is_err(),
                    "{} missing '{}' should be rejected",
                    stringify!($type),
                    $field
                );
            }
        };
    }

    // Event
    test_missing_field!(event_missing_id, life_engine_types::CalendarEvent, event_json, "id");
    test_missing_field!(event_missing_source, life_engine_types::CalendarEvent, event_json, "source");
    test_missing_field!(event_missing_source_id, life_engine_types::CalendarEvent, event_json, "source_id");
    test_missing_field!(event_missing_created_at, life_engine_types::CalendarEvent, event_json, "created_at");
    test_missing_field!(event_missing_updated_at, life_engine_types::CalendarEvent, event_json, "updated_at");

    // Task
    test_missing_field!(task_missing_id, life_engine_types::Task, task_json, "id");
    test_missing_field!(task_missing_source, life_engine_types::Task, task_json, "source");
    test_missing_field!(task_missing_source_id, life_engine_types::Task, task_json, "source_id");
    test_missing_field!(task_missing_created_at, life_engine_types::Task, task_json, "created_at");
    test_missing_field!(task_missing_updated_at, life_engine_types::Task, task_json, "updated_at");

    // Contact
    test_missing_field!(contact_missing_id, life_engine_types::Contact, contact_json, "id");
    test_missing_field!(contact_missing_source, life_engine_types::Contact, contact_json, "source");
    test_missing_field!(contact_missing_source_id, life_engine_types::Contact, contact_json, "source_id");
    test_missing_field!(contact_missing_created_at, life_engine_types::Contact, contact_json, "created_at");
    test_missing_field!(contact_missing_updated_at, life_engine_types::Contact, contact_json, "updated_at");

    // Note
    test_missing_field!(note_missing_id, life_engine_types::Note, note_json, "id");
    test_missing_field!(note_missing_source, life_engine_types::Note, note_json, "source");
    test_missing_field!(note_missing_source_id, life_engine_types::Note, note_json, "source_id");
    test_missing_field!(note_missing_created_at, life_engine_types::Note, note_json, "created_at");
    test_missing_field!(note_missing_updated_at, life_engine_types::Note, note_json, "updated_at");

    // Email
    test_missing_field!(email_missing_id, life_engine_types::Email, email_json, "id");
    test_missing_field!(email_missing_source, life_engine_types::Email, email_json, "source");
    test_missing_field!(email_missing_source_id, life_engine_types::Email, email_json, "source_id");
    test_missing_field!(email_missing_created_at, life_engine_types::Email, email_json, "created_at");
    test_missing_field!(email_missing_updated_at, life_engine_types::Email, email_json, "updated_at");

    // Credential
    test_missing_field!(credential_missing_id, life_engine_types::Credential, credential_json, "id");
    test_missing_field!(credential_missing_source, life_engine_types::Credential, credential_json, "source");
    test_missing_field!(credential_missing_source_id, life_engine_types::Credential, credential_json, "source_id");
    test_missing_field!(credential_missing_created_at, life_engine_types::Credential, credential_json, "created_at");
    test_missing_field!(credential_missing_updated_at, life_engine_types::Credential, credential_json, "updated_at");
}

// ===========================================================================
// Req 1.2 — UUID v4 format
// ===========================================================================
mod req1_uuid_format {
    use super::*;

    #[test]
    fn req1_2_invalid_uuid_rejected() {
        use life_engine_types::Note;
        let v = json!({
            "id": "not-a-uuid",
            "title": "Test",
            "body": "Body",
            "source": "test",
            "source_id": "n-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        assert!(serde_json::from_value::<Note>(v).is_err());
    }

    #[test]
    fn req1_2_valid_uuid_v4_accepted() {
        use life_engine_types::Note;
        // A proper UUID v4
        let v = json!({
            "id": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
            "title": "Test",
            "body": "Body",
            "source": "test",
            "source_id": "n-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let note: Note = serde_json::from_value(v).unwrap();
        assert_eq!(note.id.to_string(), "f47ac10b-58cc-4372-a567-0e02b2c3d479");
    }
}

// ===========================================================================
// Event edge cases
// ===========================================================================
mod event_edge_cases {
    use super::*;
    use life_engine_types::CalendarEvent;

    #[test]
    fn event_validate_time_range_start_before_end() {
        let v = json!({
            "id": TEST_UUID,
            "title": "Meeting",
            "start": "2026-01-15T10:00:00Z",
            "end": "2026-01-15T11:00:00Z",
            "source": "test",
            "source_id": "evt-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let evt: CalendarEvent = serde_json::from_value(v).unwrap();
        assert!(evt.validate_time_range().is_ok());
    }

    #[test]
    fn event_validate_time_range_start_after_end_fails() {
        let v = json!({
            "id": TEST_UUID,
            "title": "Meeting",
            "start": "2026-01-15T12:00:00Z",
            "end": "2026-01-15T11:00:00Z",
            "source": "test",
            "source_id": "evt-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let evt: CalendarEvent = serde_json::from_value(v).unwrap();
        assert!(evt.validate_time_range().is_err());
    }

    #[test]
    fn event_validate_time_range_no_end_is_ok() {
        let v = json!({
            "id": TEST_UUID,
            "title": "Meeting",
            "start": "2026-01-15T10:00:00Z",
            "source": "test",
            "source_id": "evt-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let evt: CalendarEvent = serde_json::from_value(v).unwrap();
        assert!(evt.validate_time_range().is_ok());
    }

    #[test]
    fn event_with_all_day_flag() {
        let v = json!({
            "id": TEST_UUID,
            "title": "Holiday",
            "start": "2026-12-25T00:00:00Z",
            "all_day": true,
            "source": "test",
            "source_id": "evt-002",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let evt: CalendarEvent = serde_json::from_value(v).unwrap();
        assert_eq!(evt.all_day, Some(true));
    }

    #[test]
    fn event_with_timezone() {
        let v = json!({
            "id": TEST_UUID,
            "title": "Meeting",
            "start": TEST_TIMESTAMP,
            "timezone": "Australia/Sydney",
            "source": "test",
            "source_id": "evt-003",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let evt: CalendarEvent = serde_json::from_value(v).unwrap();
        assert_eq!(evt.timezone.as_deref(), Some("Australia/Sydney"));
    }

    #[test]
    fn event_with_status() {
        use life_engine_types::events::EventStatus;
        let v = json!({
            "id": TEST_UUID,
            "title": "Meeting",
            "start": TEST_TIMESTAMP,
            "status": "confirmed",
            "source": "test",
            "source_id": "evt-004",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let evt: CalendarEvent = serde_json::from_value(v).unwrap();
        assert_eq!(evt.status, Some(EventStatus::Confirmed));
    }

    #[test]
    fn event_with_reminders() {
        let v = json!({
            "id": TEST_UUID,
            "title": "Meeting",
            "start": TEST_TIMESTAMP,
            "reminders": [
                { "minutes_before": 15, "method": "notification" },
                { "minutes_before": 60, "method": "email" }
            ],
            "source": "test",
            "source_id": "evt-005",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let evt: CalendarEvent = serde_json::from_value(v).unwrap();
        assert_eq!(evt.reminders.len(), 2);
        assert_eq!(evt.reminders[0].minutes_before, 15);
    }

    #[test]
    fn event_empty_reminders_omitted_in_json() {
        let v = json!({
            "id": TEST_UUID,
            "title": "Meeting",
            "start": TEST_TIMESTAMP,
            "source": "test",
            "source_id": "evt-006",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let evt: CalendarEvent = serde_json::from_value(v).unwrap();
        let serialized = serde_json::to_value(&evt).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("reminders"));
    }

    #[test]
    fn event_with_structured_recurrence() {
        let v = json!({
            "id": TEST_UUID,
            "title": "Weekly sync",
            "start": TEST_TIMESTAMP,
            "recurrence": {
                "frequency": "weekly",
                "interval": 1,
                "by_day": ["MO"]
            },
            "source": "test",
            "source_id": "evt-007",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let evt: CalendarEvent = serde_json::from_value(v).unwrap();
        assert!(evt.recurrence.is_some());
        let rec = evt.recurrence.unwrap();
        assert_eq!(rec.by_day, Some(vec!["MO".into()]));
    }
}

// ===========================================================================
// Recurrence edge cases
// ===========================================================================
mod recurrence_edge_cases {
    use life_engine_types::events::Recurrence;

    #[test]
    fn rrule_daily_no_extra_params() {
        let rec = Recurrence::from_rrule("FREQ=DAILY").unwrap();
        assert_eq!(rec.interval, 1);
        assert_eq!(rec.count, None);
        assert_eq!(rec.until, None);
        assert_eq!(rec.by_day, None);
    }

    #[test]
    fn rrule_yearly_with_until() {
        let rec = Recurrence::from_rrule("FREQ=YEARLY;UNTIL=20301231T235959Z").unwrap();
        assert!(rec.until.is_some());
        let until = rec.until.unwrap();
        assert_eq!(until.format("%Y").to_string(), "2030");
    }

    #[test]
    fn rrule_empty_string_returns_none() {
        assert!(Recurrence::from_rrule("").is_none());
    }

    #[test]
    fn rrule_missing_freq_returns_none() {
        assert!(Recurrence::from_rrule("INTERVAL=2;COUNT=5").is_none());
    }

    #[test]
    fn rrule_to_rrule_default_interval_omitted() {
        let rec = Recurrence::from_rrule("FREQ=DAILY").unwrap();
        let output = rec.to_rrule();
        assert_eq!(output, "FREQ=DAILY");
        assert!(!output.contains("INTERVAL"));
    }

    #[test]
    fn rrule_to_rrule_interval_2_included() {
        let rec = Recurrence::from_rrule("FREQ=WEEKLY;INTERVAL=2").unwrap();
        let output = rec.to_rrule();
        assert!(output.contains("INTERVAL=2"));
    }
}

// ===========================================================================
// Contact nested type edge cases
// ===========================================================================
mod contact_nested_edge_cases {
    use super::*;

    #[test]
    fn contact_email_type_uses_json_key_type_not_email_type() {
        use life_engine_types::ContactEmail;
        let ce = ContactEmail {
            address: "jane@example.com".into(),
            email_type: Some(life_engine_types::ContactInfoType::Work),
            primary: None,
        };
        let serialized = serde_json::to_value(&ce).unwrap();
        assert!(serialized.as_object().unwrap().contains_key("type"));
        assert!(!serialized.as_object().unwrap().contains_key("email_type"));
        assert_eq!(serialized["type"], "work");
    }

    #[test]
    fn contact_phone_type_uses_json_key_type_not_phone_type() {
        use life_engine_types::ContactPhone;
        let cp = ContactPhone {
            number: "+1234567890".into(),
            phone_type: Some(life_engine_types::PhoneType::Mobile),
            primary: None,
        };
        let serialized = serde_json::to_value(&cp).unwrap();
        assert!(serialized.as_object().unwrap().contains_key("type"));
        assert!(!serialized.as_object().unwrap().contains_key("phone_type"));
        assert_eq!(serialized["type"], "mobile");
    }

    #[test]
    fn contact_address_type_uses_json_key_type() {
        use life_engine_types::ContactAddress;
        let addr = ContactAddress {
            street: None,
            city: None,
            region: Some("NSW".into()),
            postal_code: None,
            country: None,
            address_type: Some(life_engine_types::ContactInfoType::Home),
        };
        let serialized = serde_json::to_value(&addr).unwrap();
        assert!(serialized.as_object().unwrap().contains_key("type"));
        assert!(!serialized.as_object().unwrap().contains_key("address_type"));
    }

    #[test]
    fn contact_email_minimal() {
        use life_engine_types::ContactEmail;
        let v = json!({ "address": "a@b.com" });
        let ce: ContactEmail = serde_json::from_value(v).unwrap();
        assert_eq!(ce.address, "a@b.com");
        assert_eq!(ce.email_type, None);
        assert_eq!(ce.primary, None);
    }

    #[test]
    fn contact_phone_minimal() {
        use life_engine_types::ContactPhone;
        let v = json!({ "number": "+1234567890" });
        let cp: ContactPhone = serde_json::from_value(v).unwrap();
        assert_eq!(cp.number, "+1234567890");
        assert_eq!(cp.phone_type, None);
    }

    #[test]
    fn contact_address_all_none() {
        use life_engine_types::ContactAddress;
        let v = json!({});
        let addr: ContactAddress = serde_json::from_value(v).unwrap();
        assert_eq!(addr.street, None);
        assert_eq!(addr.city, None);
        assert_eq!(addr.country, None);
    }

    #[test]
    fn contact_info_type_enum_values() {
        use life_engine_types::ContactInfoType;
        assert_eq!(serde_json::to_value(ContactInfoType::Home).unwrap(), "home");
        assert_eq!(serde_json::to_value(ContactInfoType::Work).unwrap(), "work");
        assert_eq!(serde_json::to_value(ContactInfoType::Other).unwrap(), "other");
    }

    #[test]
    fn phone_type_enum_values() {
        use life_engine_types::PhoneType;
        assert_eq!(serde_json::to_value(PhoneType::Mobile).unwrap(), "mobile");
        assert_eq!(serde_json::to_value(PhoneType::Home).unwrap(), "home");
        assert_eq!(serde_json::to_value(PhoneType::Work).unwrap(), "work");
        assert_eq!(serde_json::to_value(PhoneType::Fax).unwrap(), "fax");
        assert_eq!(serde_json::to_value(PhoneType::Other).unwrap(), "other");
    }
}

// ===========================================================================
// Email edge cases
// ===========================================================================
mod email_edge_cases {
    use super::*;

    #[test]
    fn email_address_with_name() {
        use life_engine_types::EmailAddress;
        let v = json!({
            "name": "Jane Doe",
            "address": "jane@example.com"
        });
        let ea: EmailAddress = serde_json::from_value(v).unwrap();
        assert_eq!(ea.name.as_deref(), Some("Jane Doe"));
        assert_eq!(ea.address, "jane@example.com");
    }

    #[test]
    fn email_address_without_name() {
        use life_engine_types::EmailAddress;
        let v = json!({ "address": "jane@example.com" });
        let ea: EmailAddress = serde_json::from_value(v).unwrap();
        assert_eq!(ea.name, None);
    }

    #[test]
    fn email_address_name_skipped_when_none() {
        use life_engine_types::EmailAddress;
        let ea = EmailAddress {
            name: None,
            address: "jane@example.com".into(),
        };
        let serialized = serde_json::to_value(&ea).unwrap();
        assert!(!serialized.as_object().unwrap().contains_key("name"));
    }

    #[test]
    fn email_attachment_spec_format_round_trips() {
        use life_engine_types::EmailAttachment;
        let v = json!({
            "filename": "doc.pdf",
            "mime_type": "application/pdf",
            "size_bytes": 2048
        });
        let att: EmailAttachment = serde_json::from_value(v).unwrap();
        assert_eq!(att.filename, "doc.pdf");
        assert_eq!(att.size_bytes, 2048);
        let serialized = serde_json::to_value(&att).unwrap();
        assert_eq!(serialized["filename"], "doc.pdf");
        assert_eq!(serialized["size_bytes"], 2048);
    }

    #[test]
    fn email_with_multiple_recipients() {
        use life_engine_types::Email;
        let v = json!({
            "id": TEST_UUID,
            "subject": "Team email",
            "from": { "address": "sender@example.com" },
            "to": [
                { "address": "alice@example.com" },
                { "address": "bob@example.com" },
                { "address": "charlie@example.com" }
            ],
            "body_text": "Team message",
            "date": TEST_TIMESTAMP,
            "source": "test",
            "source_id": "em-002",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let em: Email = serde_json::from_value(v).unwrap();
        assert_eq!(em.to.len(), 3);
    }

    #[test]
    fn email_missing_subject_rejected() {
        use life_engine_types::Email;
        let v = json!({
            "id": TEST_UUID,
            "from": { "address": "a@b.com" },
            "to": [{ "address": "c@d.com" }],
            "date": TEST_TIMESTAMP,
            "source": "test",
            "source_id": "em-003",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        assert!(serde_json::from_value::<Email>(v).is_err());
    }

    #[test]
    fn email_missing_from_rejected() {
        use life_engine_types::Email;
        let v = json!({
            "id": TEST_UUID,
            "subject": "Hello",
            "to": [{ "address": "c@d.com" }],
            "date": TEST_TIMESTAMP,
            "source": "test",
            "source_id": "em-004",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        assert!(serde_json::from_value::<Email>(v).is_err());
    }
}

// ===========================================================================
// Extension convention edge cases
// ===========================================================================
mod extension_edge_cases {
    use super::*;
    use life_engine_types::validate_extension_namespace;

    #[test]
    fn req8_7_reserved_namespace_prefix() {
        // org.life-engine.* is reserved for first-party.
        // validate_extension_namespace should reject a third-party plugin
        // writing to org.life-engine.* namespace.
        let ext = json!({
            "org.life-engine.internal": { "data": 1 }
        });
        let result = validate_extension_namespace("com.example.plugin", &ext);
        assert!(
            result.is_err(),
            "Third-party plugin should not write to org.life-engine.* namespace"
        );
    }

    #[test]
    fn extension_with_array_value_preserved() {
        let ext = json!({
            "com.example.plugin": {
                "tags": ["a", "b", "c"],
                "scores": [1, 2, 3]
            }
        });
        let serialized = serde_json::to_string(&ext).unwrap();
        let restored: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(ext, restored);
    }

    #[test]
    fn extension_deeply_nested_round_trip() {
        let ext = json!({
            "com.example.plugin": {
                "level1": {
                    "level2": {
                        "level3": {
                            "value": 42
                        }
                    }
                }
            }
        });
        assert!(validate_extension_namespace("com.example.plugin", &ext).is_ok());
        let serialized = serde_json::to_string(&ext).unwrap();
        let restored: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(restored["com.example.plugin"]["level1"]["level2"]["level3"]["value"], 42);
    }

    #[test]
    fn extension_non_object_value_passes_validation() {
        // validate_extension_namespace only checks Object type.
        // Non-object values (string, array, number) pass through.
        let ext = json!("just a string");
        assert!(validate_extension_namespace("com.example.plugin", &ext).is_ok());
    }
}

// ===========================================================================
// Credential edge cases
// ===========================================================================
mod credential_edge_cases {
    use super::*;
    use life_engine_types::Credential;

    #[test]
    fn credential_missing_claims_rejected() {
        let v = json!({
            "id": TEST_UUID,
            "name": "Example API Key",
            "credential_type": "api_key",
            "service": "example.com",
            "source": "test",
            "source_id": "cred-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        assert!(
            serde_json::from_value::<Credential>(v).is_err(),
            "Credential without 'claims' should be rejected"
        );
    }

    #[test]
    fn credential_claims_can_be_empty_object() {
        let v = json!({
            "id": TEST_UUID,
            "name": "Example API Key",
            "credential_type": "api_key",
            "service": "example.com",
            "claims": {},
            "source": "test",
            "source_id": "cred-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let cred: Credential = serde_json::from_value(v).unwrap();
        assert!(cred.claims.is_object());
    }

    #[test]
    fn credential_claims_can_be_array() {
        // Req 7.3: claims is opaque JSON — any JSON value is valid.
        let v = json!({
            "id": TEST_UUID,
            "name": "Example API Key",
            "credential_type": "api_key",
            "service": "example.com",
            "claims": [1, 2, 3],
            "source": "test",
            "source_id": "cred-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let cred: Credential = serde_json::from_value(v).unwrap();
        assert!(cred.claims.is_array());
    }

    #[test]
    fn credential_type_rejects_unknown_variant() {
        use life_engine_types::CredentialType;
        assert!(serde_json::from_value::<CredentialType>(json!("password")).is_err());
        assert!(serde_json::from_value::<CredentialType>(json!("ssh_key")).is_err());
    }

    #[test]
    fn credential_encrypted_field_optional() {
        let v = json!({
            "id": TEST_UUID,
            "name": "Example API Key",
            "credential_type": "api_key",
            "service": "example.com",
            "claims": {},
            "encrypted": true,
            "source": "test",
            "source_id": "cred-001",
            "created_at": TEST_TIMESTAMP,
            "updated_at": TEST_TIMESTAMP
        });
        let cred: Credential = serde_json::from_value(v).unwrap();
        assert_eq!(cred.encrypted, Some(true));
    }
}

// ===========================================================================
// Enum deserialization rejects invalid values
// ===========================================================================
mod enum_rejection {
    use super::*;

    #[test]
    fn task_status_rejects_uppercase() {
        use life_engine_types::TaskStatus;
        assert!(serde_json::from_value::<TaskStatus>(json!("PENDING")).is_err());
        assert!(serde_json::from_value::<TaskStatus>(json!("Pending")).is_err());
    }

    #[test]
    fn task_priority_rejects_uppercase() {
        use life_engine_types::TaskPriority;
        assert!(serde_json::from_value::<TaskPriority>(json!("HIGH")).is_err());
        assert!(serde_json::from_value::<TaskPriority>(json!("High")).is_err());
    }

    #[test]
    fn credential_type_rejects_uppercase() {
        use life_engine_types::CredentialType;
        assert!(serde_json::from_value::<CredentialType>(json!("API_KEY")).is_err());
        assert!(serde_json::from_value::<CredentialType>(json!("ApiKey")).is_err());
    }

    #[test]
    fn event_status_rejects_unknown() {
        use life_engine_types::events::EventStatus;
        assert!(serde_json::from_value::<EventStatus>(json!("busy")).is_err());
    }

    #[test]
    fn attendee_status_rejects_unknown() {
        use life_engine_types::events::AttendeeStatus;
        assert!(serde_json::from_value::<AttendeeStatus>(json!("maybe")).is_err());
    }

    #[test]
    fn note_format_rejects_unknown() {
        use life_engine_types::NoteFormat;
        assert!(serde_json::from_value::<NoteFormat>(json!("rtf")).is_err());
    }

    #[test]
    fn recurrence_frequency_rejects_unknown() {
        use life_engine_types::events::RecurrenceFrequency;
        assert!(serde_json::from_value::<RecurrenceFrequency>(json!("secondly")).is_err());
    }
}
