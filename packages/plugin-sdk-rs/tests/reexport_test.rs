//! Compile-only test verifying that all public types from life-engine-types
//! and life-engine-traits are accessible through the plugin SDK without
//! adding those crates as direct dependencies.

// CDM types
use life_engine_plugin_sdk::{
    Attendee, AttendeeStatus, CalendarEvent, Contact, ContactAddress, ContactEmail,
    ContactInfoType, ContactName, ContactPhone, Credential, CredentialType, Email, EmailAddress,
    EmailAttachment, EventStatus, FileMetadata, Note, NoteFormat, PhoneType, Recurrence,
    RecurrenceFrequency, Reminder, ReminderMethod, Task, TaskPriority, TaskStatus,
};

// Pipeline types
use life_engine_plugin_sdk::{
    CdmType, MessageMetadata, PipelineMessage, SchemaValidated, SchemaValidationError,
    TypedPayload,
};

// Storage query/mutation types
use life_engine_plugin_sdk::{
    FilterOp, QueryFilter, SortDirection, SortField, StorageMutation, StorageQuery,
};

// Extension namespace validation
use life_engine_plugin_sdk::{validate_extension_namespace, ExtensionError};

// Traits crate re-exports (Capability is the unified type)
use life_engine_plugin_sdk::{
    Action, Capability, CapabilityViolation, EngineError, Plugin, Severity, StorageBackend,
};

// Prelude re-exports everything needed
use life_engine_plugin_sdk::prelude;

// Full crate re-exports for qualified access
use life_engine_plugin_sdk::life_engine_traits;
use life_engine_plugin_sdk::life_engine_types;

#[test]
fn sdk_reexports_all_cdm_types() {
    // This test verifies compilation — if any type is missing, this won't compile.
    fn _assert_types_exist() {
        let _: Option<CalendarEvent> = None;
        let _: Option<Task> = None;
        let _: Option<Contact> = None;
        let _: Option<Note> = None;
        let _: Option<Email> = None;
        let _: Option<FileMetadata> = None;
        let _: Option<Credential> = None;
    }
}

#[test]
fn sdk_reexports_pipeline_types() {
    fn _assert_types_exist() {
        let _: Option<PipelineMessage> = None;
        let _: Option<MessageMetadata> = None;
        let _: Option<TypedPayload> = None;
        let _: Option<CdmType> = None;
    }
}

#[test]
fn sdk_reexports_storage_types() {
    fn _assert_types_exist() {
        let _: Option<StorageQuery> = None;
        let _: Option<StorageMutation> = None;
        let _: Option<QueryFilter> = None;
        let _: Option<FilterOp> = None;
        let _: Option<SortField> = None;
        let _: Option<SortDirection> = None;
    }
}

#[test]
fn sdk_reexports_traits_types() {
    fn _assert_types_exist() {
        let _: Option<Action> = None;
        let _: Option<Severity> = None;
        let _: Option<Capability> = None;
        let _: Option<CapabilityViolation> = None;
    }
}

#[test]
fn sdk_traits_accessible_via_qualified_path() {
    // Traits crate types can also be accessed via the full crate re-export
    let _: Option<life_engine_traits::Capability> = None;
    let _: Option<life_engine_traits::Action> = None;

    // Types crate types can also be accessed via the full crate re-export
    let _: Option<life_engine_types::StorageQuery> = None;
    let _: Option<life_engine_types::PipelineMessage> = None;
}
