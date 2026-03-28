//! Rust SDK for building Life Engine Core plugins.
//!
//! This crate provides the traits and types that Core plugin authors
//! implement to extend Life Engine's functionality. It compiles for both
//! native targets (for testing) and `wasm32-wasip1` (for production plugin
//! builds).
//!
//! # Plugin Models
//!
//! Life Engine supports two plugin models through two traits:
//!
//! ## `CorePlugin` (recommended for new plugins)
//!
//! Defined in this crate (`plugin-sdk-rs`). Use `CorePlugin` for plugins
//! that need async I/O, HTTP route handling, event subscriptions, credential
//! storage, and the full lifecycle API (`on_load`/`on_unload`). All
//! built-in engine plugins (backup, connectors, DAV transports, etc.)
//! implement `CorePlugin`.
//!
//! ## `Plugin` (workflow-engine action model)
//!
//! Defined in `life-engine-traits` and re-exported here. Use `Plugin` for
//! lightweight, synchronous action plugins that the workflow engine invokes
//! via `execute(action, PipelineMessage)`. `Plugin` declares named actions
//! with optional JSON Schema validation and does not participate in the
//! async lifecycle or route system.
//!
//! ## Migration path
//!
//! New plugins should implement `CorePlugin`. The `Plugin` trait remains
//! stable for workflow-engine actions. If you have a `Plugin` implementation
//! that needs async capabilities, routes, or event handling, migrate to
//! `CorePlugin` — they share the same `Capability` enum and identity
//! conventions.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use life_engine_plugin_sdk::prelude::*;
//!
//! struct MyPlugin;
//!
//! #[async_trait]
//! impl CorePlugin for MyPlugin {
//!     fn id(&self) -> &str { "com.example.my-plugin" }
//!     fn display_name(&self) -> &str { "My Plugin" }
//!     fn version(&self) -> &str { "0.1.0" }
//!     fn capabilities(&self) -> Vec<Capability> { vec![Capability::StorageRead] }
//!     async fn on_load(&mut self, _ctx: &PluginContext) -> Result<()> { Ok(()) }
//!     async fn on_unload(&mut self) -> Result<()> { Ok(()) }
//!     fn routes(&self) -> Vec<PluginRoute> { vec![] }
//!     async fn handle_event(&self, _event: &CoreEvent) -> Result<()> { Ok(()) }
//! }
//! ```
//!
//! # Building for WASM
//!
//! Plugins are compiled to WASM for execution inside the Core sandbox:
//!
//! ```bash
//! # Install the WASM target (one-time)
//! rustup target add wasm32-wasip1
//!
//! # Build your plugin as a WASM module
//! cargo build --target wasm32-wasip1 --release
//! ```
//!
//! A reference `.cargo/config.toml` is included in this crate's directory
//! that sets `wasm32-wasip1` as the default build target. Plugin authors
//! can copy it into their own project to avoid passing `--target` on every
//! build.

pub mod context;
pub mod credential_store;
pub mod error;
pub mod lifecycle;
pub mod macros;
pub mod retry;
pub mod storage;
#[cfg(any(test, feature = "test-utils"))]
pub mod test;
pub mod traits;
pub mod types;
pub mod wasm_guest;

// Re-export core SDK types at crate root.
pub use context::{ActionContext, ConfigClient, EventClient, HttpClient, HttpResponse, StorageClient};
pub use credential_store::{CredentialStore, StoredCredential};
pub use error::PluginError;
pub use lifecycle::LifecycleHooks;
pub use macros::{PluginInvocation, PluginOutput};
pub use storage::StorageContext;
pub use traits::CorePlugin;
pub use types::{
    Capability, CollectionSchema, CoreEvent, CredentialAccess, HttpMethod, PluginContext,
    PluginRoute,
};

// Re-export async_trait so plugin authors don't need an extra dependency.
pub use async_trait::async_trait;

// Re-export anyhow::Result so plugin authors can use it directly.
pub use anyhow::Result;

// Re-export serde_json so plugin authors can use it (needed for handle_route, events, etc.)
pub use serde_json;

// Re-export all canonical data model types and pipeline types from the types crate
// so plugin authors only need one dependency.
pub use life_engine_types;
pub use life_engine_types::{
    // Canonical collection types
    Attendee, AttendeeStatus, CalendarEvent, Contact, ContactAddress, ContactEmail,
    ContactInfoType, ContactName, ContactPhone, Credential, CredentialType, Email, EmailAddress,
    EmailAttachment, EventStatus, FileMetadata, Note, NoteFormat, PhoneType, Recurrence,
    RecurrenceFrequency, Reminder, ReminderMethod, Task, TaskPriority, TaskStatus,
    // Pipeline message types
    CdmType, MessageMetadata, PipelineMessage, SchemaValidated, SchemaValidationError,
    TypedPayload,
    // Storage query and mutation types
    FilterOp, QueryFilter, SortDirection, SortField, StorageMutation, StorageQuery,
    // Extension namespace validation
    validate_extension_namespace, ExtensionError,
};

// Re-export all infrastructure traits and types from the traits crate.
pub use life_engine_traits;
pub use life_engine_traits::{
    Action, CapabilityViolation, EngineError, Plugin, Severity, StorageBackend,
};

/// Convenience prelude for plugin authors.
///
/// Import everything needed to implement a plugin:
/// ```rust,ignore
/// use life_engine_plugin_sdk::prelude::*;
/// ```
pub mod prelude {
    // Native plugin types (CorePlugin-based)
    pub use crate::credential_store::{CredentialStore, StoredCredential};
    pub use crate::traits::CorePlugin;
    pub use crate::types::{
        Capability, CollectionSchema, CoreEvent, CredentialAccess, HttpMethod, PluginContext,
        PluginRoute,
    };
    pub use anyhow::Result;
    pub use async_trait::async_trait;

    // Plugin action context and client traits
    pub use crate::context::{
        ActionContext, ConfigClient, EventClient, HttpClient, HttpResponse, StorageClient,
    };
    pub use crate::error::PluginError;
    pub use crate::lifecycle::LifecycleHooks;
    pub use crate::retry::RetryState;

    // CDM and pipeline types
    pub use life_engine_types::{
        CdmType, MessageMetadata, PipelineMessage, SchemaValidated, TypedPayload,
    };

    // Storage types and context
    pub use crate::storage::StorageContext;
    pub use life_engine_types::{
        FilterOp, QueryFilter, SortDirection, SortField, StorageMutation, StorageQuery,
    };

    // WASM plugin traits and types (from life-engine-traits)
    pub use life_engine_traits::{
        Action, CapabilityViolation, EngineError, Plugin, Severity, StorageBackend,
    };
}
