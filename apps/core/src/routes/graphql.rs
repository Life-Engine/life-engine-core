//! GraphQL API for Life Engine — alternative to the REST API.
//!
//! Auto-generates GraphQL types from the Canonical Data Model (CDM) and
//! resolves queries, mutations, and subscriptions via the shared
//! `StorageAdapter` (same storage layer as REST).

use crate::message_bus::{BusEvent, MessageBus};
use crate::routes::health::AppState;
use crate::storage::{
    ComparisonFilter, ComparisonOp, FieldFilter, Pagination, QueryFilters, SortDirection,
    SortOptions, StorageAdapter, TextFilter,
};

use axum::extract::State;
use axum::response::IntoResponse;

use async_graphql::*;
use chrono::{DateTime, Utc};
use futures::Stream;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

/// Default plugin ID for API-level data access (matches REST API).
const CORE_PLUGIN_ID: &str = "core";

// ---------------------------------------------------------------------------
// GraphQL enum types
// ---------------------------------------------------------------------------

/// Task priority levels matching CDM TaskPriority.
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum GqlTaskPriority {
    Low,
    Medium,
    High,
    Urgent,
}

/// Task status values matching CDM TaskStatus.
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum GqlTaskStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

/// Credential type matching CDM CredentialType.
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum GqlCredentialType {
    OauthToken,
    ApiKey,
    IdentityDocument,
    Passkey,
}

// ---------------------------------------------------------------------------
// GraphQL output types (auto-generated from CDM schemas)
// ---------------------------------------------------------------------------

/// A task record (mirrors CDM Task).
#[derive(SimpleObject, Clone)]
pub struct GqlTask {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: GqlTaskStatus,
    pub priority: GqlTaskPriority,
    pub due_date: Option<DateTime<Utc>>,
    pub tags: Vec<String>,
    pub source: String,
    pub source_id: String,
    pub extensions: Option<async_graphql::Json<JsonValue>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Structured name for a contact.
#[derive(SimpleObject, Clone)]
pub struct GqlContactName {
    pub given: String,
    pub family: String,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
    pub middle: Option<String>,
}

/// An email address entry for a contact.
#[derive(SimpleObject, Clone)]
pub struct GqlEmailAddress {
    pub address: String,
    pub email_type: Option<String>,
    pub primary: Option<bool>,
}

/// A phone number entry for a contact.
#[derive(SimpleObject, Clone)]
pub struct GqlPhoneNumber {
    pub number: String,
    pub phone_type: Option<String>,
    pub primary: Option<bool>,
}

/// A postal address for a contact.
#[derive(SimpleObject, Clone)]
pub struct GqlPostalAddress {
    pub street: Option<String>,
    pub city: Option<String>,
    pub region: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub address_type: Option<String>,
}

/// A contact record (mirrors CDM Contact).
#[derive(SimpleObject, Clone)]
pub struct GqlContact {
    pub id: String,
    pub name: GqlContactName,
    pub emails: Vec<GqlEmailAddress>,
    pub phones: Vec<GqlPhoneNumber>,
    pub addresses: Vec<GqlPostalAddress>,
    pub organization: Option<String>,
    pub title: Option<String>,
    pub birthday: Option<String>,
    pub photo_url: Option<String>,
    pub notes: Option<String>,
    pub groups: Vec<String>,
    pub source: String,
    pub source_id: String,
    pub extensions: Option<async_graphql::Json<JsonValue>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A calendar event record (mirrors CDM CalendarEvent).
#[derive(Clone)]
pub struct GqlCalendarEvent {
    pub id: String,
    pub title: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub recurrence: Option<String>,
    pub attendees: Vec<String>,
    pub location: Option<String>,
    pub description: Option<String>,
    pub source: String,
    pub source_id: String,
    pub extensions: Option<async_graphql::Json<JsonValue>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// CalendarEvent with nested query support (attendees -> contacts).
#[Object]
impl GqlCalendarEvent {
    async fn id(&self) -> &str {
        &self.id
    }
    async fn title(&self) -> &str {
        &self.title
    }
    async fn start(&self) -> DateTime<Utc> {
        self.start
    }
    async fn end(&self) -> DateTime<Utc> {
        self.end
    }
    async fn recurrence(&self) -> Option<&str> {
        self.recurrence.as_deref()
    }
    async fn attendees(&self) -> &[String] {
        &self.attendees
    }
    async fn location(&self) -> Option<&str> {
        self.location.as_deref()
    }
    async fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
    async fn source(&self) -> &str {
        &self.source
    }
    async fn source_id(&self) -> &str {
        &self.source_id
    }
    async fn extensions(&self) -> Option<&async_graphql::Json<JsonValue>> {
        self.extensions.as_ref()
    }
    async fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    async fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    /// Resolve attendee email addresses to Contact records.
    ///
    /// Uses a single query with OR filters to batch-fetch all attendee
    /// contacts instead of issuing one query per attendee (N+1).
    async fn attendee_contacts(&self, ctx: &Context<'_>) -> Result<Vec<GqlContact>> {
        let storage = ctx
            .data::<Arc<crate::sqlite_storage::SqliteStorage>>()
            .map_err(|_| Error::new("storage not available"))?;

        if self.attendees.is_empty() {
            return Ok(vec![]);
        }

        // Build an OR filter matching any attendee email in a single query.
        let or_groups: Vec<QueryFilters> = self
            .attendees
            .iter()
            .map(|email| QueryFilters {
                text_search: vec![TextFilter {
                    field: "emails".into(),
                    contains: email.clone(),
                }],
                ..Default::default()
            })
            .collect();

        let filters = QueryFilters {
            or: or_groups,
            ..Default::default()
        };

        let result = storage
            .query(
                CORE_PLUGIN_ID,
                "contacts",
                filters,
                None,
                Pagination {
                    limit: self.attendees.len() as u32,
                    offset: 0,
                },
            )
            .await
            .map_err(|e| Error::new(format!("query failed: {e}")))?;

        let contacts = result
            .records
            .iter()
            .map(|r| record_to_contact(&r.data))
            .collect();
        Ok(contacts)
    }
}

/// An email attachment reference.
#[derive(SimpleObject, Clone)]
pub struct GqlEmailAttachment {
    pub file_id: String,
    pub filename: String,
    pub mime_type: String,
    pub size: u64,
}

/// An email record (mirrors CDM Email).
#[derive(Clone)]
pub struct GqlEmail {
    pub id: String,
    pub from: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body_text: String,
    pub body_html: Option<String>,
    pub thread_id: Option<String>,
    pub labels: Vec<String>,
    pub attachments: Vec<GqlEmailAttachment>,
    pub source: String,
    pub source_id: String,
    pub extensions: Option<async_graphql::Json<JsonValue>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Email with nested query support (attachments -> files).
#[Object]
impl GqlEmail {
    async fn id(&self) -> &str {
        &self.id
    }
    async fn from(&self) -> &str {
        &self.from
    }
    async fn to(&self) -> &[String] {
        &self.to
    }
    async fn cc(&self) -> &[String] {
        &self.cc
    }
    async fn bcc(&self) -> &[String] {
        &self.bcc
    }
    async fn subject(&self) -> &str {
        &self.subject
    }
    async fn body_text(&self) -> &str {
        &self.body_text
    }
    async fn body_html(&self) -> Option<&str> {
        self.body_html.as_deref()
    }
    async fn thread_id(&self) -> Option<&str> {
        self.thread_id.as_deref()
    }
    async fn labels(&self) -> &[String] {
        &self.labels
    }
    async fn attachments(&self) -> &[GqlEmailAttachment] {
        &self.attachments
    }
    async fn source(&self) -> &str {
        &self.source
    }
    async fn source_id(&self) -> &str {
        &self.source_id
    }
    async fn extensions(&self) -> Option<&async_graphql::Json<JsonValue>> {
        self.extensions.as_ref()
    }
    async fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    async fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    /// Resolve attachment file_ids to FileMetadata records.
    ///
    /// Uses a single OR-filter query to batch-fetch all attachment files
    /// instead of issuing one query per attachment (N+1).
    async fn attachment_files(&self, ctx: &Context<'_>) -> Result<Vec<GqlFile>> {
        let storage = ctx
            .data::<Arc<crate::sqlite_storage::SqliteStorage>>()
            .map_err(|_| Error::new("storage not available"))?;

        if self.attachments.is_empty() {
            return Ok(vec![]);
        }

        // Build an OR filter matching any attachment file_id in a single query.
        let or_groups: Vec<QueryFilters> = self
            .attachments
            .iter()
            .map(|att| QueryFilters {
                equality: vec![FieldFilter {
                    field: "id".into(),
                    value: serde_json::Value::String(att.file_id.clone()),
                }],
                ..Default::default()
            })
            .collect();

        let filters = QueryFilters {
            or: or_groups,
            ..Default::default()
        };

        let result = storage
            .query(
                CORE_PLUGIN_ID,
                "files",
                filters,
                None,
                Pagination {
                    limit: self.attachments.len() as u32,
                    offset: 0,
                },
            )
            .await
            .map_err(|e| Error::new(format!("query failed: {e}")))?;

        let files = result
            .records
            .iter()
            .map(|r| record_to_file(&r.data))
            .collect();
        Ok(files)
    }
}

/// A note record (mirrors CDM Note).
#[derive(SimpleObject, Clone)]
pub struct GqlNote {
    pub id: String,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    pub source: String,
    pub source_id: String,
    pub extensions: Option<async_graphql::Json<JsonValue>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A file metadata record (mirrors CDM FileMetadata).
#[derive(SimpleObject, Clone)]
pub struct GqlFile {
    pub id: String,
    pub name: String,
    pub mime_type: String,
    pub size: u64,
    pub path: String,
    pub checksum: Option<String>,
    pub source: String,
    pub source_id: String,
    pub extensions: Option<async_graphql::Json<JsonValue>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A credential record (mirrors CDM Credential).
#[derive(SimpleObject, Clone)]
pub struct GqlCredential {
    pub id: String,
    pub credential_type: GqlCredentialType,
    pub issuer: String,
    pub issued_date: String,
    pub expiry_date: Option<String>,
    pub claims: async_graphql::Json<JsonValue>,
    pub source: String,
    pub source_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// GraphQL input types for filtering, sorting, and pagination
// ---------------------------------------------------------------------------

/// Pagination arguments for list queries.
#[derive(InputObject, Default)]
pub struct PaginationInput {
    /// Maximum number of records (default 50, max 1000).
    pub limit: Option<u32>,
    /// Number of records to skip (default 0).
    pub offset: Option<u32>,
}

/// Sort direction.
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum GqlSortDirection {
    Asc,
    Desc,
}

/// Sort options for list queries.
#[derive(InputObject)]
pub struct SortInput {
    /// Field name to sort by.
    pub sort_by: String,
    /// Sort direction (default ASC).
    pub sort_dir: Option<GqlSortDirection>,
}

/// An equality filter condition.
#[derive(InputObject)]
pub struct EqualityFilterInput {
    /// JSON field path.
    pub field: String,
    /// Value to match (as JSON string).
    pub value: String,
}

/// A comparison filter condition.
#[derive(InputObject)]
pub struct ComparisonFilterInput {
    /// JSON field path.
    pub field: String,
    /// Operator: "gte", "lte", "gt", "lt".
    pub operator: String,
    /// Value to compare against (as JSON string).
    pub value: String,
}

/// A text search filter condition.
#[derive(InputObject)]
pub struct TextSearchFilterInput {
    /// JSON field path.
    pub field: String,
    /// Substring to search for.
    pub contains: String,
}

/// Combined filter input for queries.
#[derive(InputObject, Default)]
pub struct FilterInput {
    /// Equality conditions.
    #[graphql(default)]
    pub equality: Vec<EqualityFilterInput>,
    /// Comparison conditions.
    #[graphql(default)]
    pub comparison: Vec<ComparisonFilterInput>,
    /// Text search conditions.
    #[graphql(default)]
    pub text_search: Vec<TextSearchFilterInput>,
}

/// Paginated result wrapper for any collection.
#[derive(SimpleObject)]
#[graphql(concrete(name = "TaskConnection", params(GqlTask)))]
#[graphql(concrete(name = "ContactConnection", params(GqlContact)))]
#[graphql(concrete(name = "CalendarEventConnection", params(GqlCalendarEvent)))]
#[graphql(concrete(name = "EmailConnection", params(GqlEmail)))]
#[graphql(concrete(name = "NoteConnection", params(GqlNote)))]
#[graphql(concrete(name = "FileConnection", params(GqlFile)))]
#[graphql(concrete(name = "CredentialConnection", params(GqlCredential)))]
pub struct Connection<T: OutputType> {
    pub data: Vec<T>,
    pub total: u64,
    pub limit: u32,
    pub offset: u32,
}

// ---------------------------------------------------------------------------
// Input types for mutations
// ---------------------------------------------------------------------------

/// Input for creating/updating records (generic JSON).
#[derive(InputObject)]
pub struct RecordInput {
    /// The record data as a JSON string.
    pub data: String,
}

/// Input for updating a record (includes version for optimistic concurrency).
#[derive(InputObject)]
pub struct UpdateRecordInput {
    /// The record data as a JSON string.
    pub data: String,
    /// Expected version for optimistic concurrency.
    pub version: i64,
}

/// The raw storage record returned by mutations.
#[derive(SimpleObject, Clone)]
pub struct GqlRecord {
    pub id: String,
    pub plugin_id: String,
    pub collection: String,
    pub data: async_graphql::Json<JsonValue>,
    pub version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Conversion helpers (Record JSON -> GraphQL types)
// ---------------------------------------------------------------------------

fn record_to_task(data: &JsonValue) -> GqlTask {
    GqlTask {
        id: data["id"].as_str().unwrap_or_default().into(),
        title: data["title"].as_str().unwrap_or_default().into(),
        description: data["description"].as_str().map(Into::into),
        status: match data["status"].as_str().unwrap_or("pending") {
            "in_progress" => GqlTaskStatus::InProgress,
            "completed" => GqlTaskStatus::Completed,
            "cancelled" => GqlTaskStatus::Cancelled,
            _ => GqlTaskStatus::Pending,
        },
        priority: match data["priority"].as_str().unwrap_or("medium") {
            "low" => GqlTaskPriority::Low,
            "high" => GqlTaskPriority::High,
            "urgent" => GqlTaskPriority::Urgent,
            _ => GqlTaskPriority::Medium,
        },
        due_date: data["due_date"]
            .as_str()
            .and_then(|s| s.parse().ok()),
        tags: data["tags"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(Into::into)).collect())
            .unwrap_or_default(),
        source: data["source"].as_str().unwrap_or_default().into(),
        source_id: data["source_id"].as_str().unwrap_or_default().into(),
        extensions: data.get("extensions").and_then(|v| {
            if v.is_null() {
                None
            } else {
                Some(async_graphql::Json(v.clone()))
            }
        }),
        created_at: data["created_at"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Utc::now),
        updated_at: data["updated_at"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Utc::now),
    }
}

fn record_to_contact(data: &JsonValue) -> GqlContact {
    let name = &data["name"];
    GqlContact {
        id: data["id"].as_str().unwrap_or_default().into(),
        name: GqlContactName {
            given: name["given"].as_str().unwrap_or_default().into(),
            family: name["family"].as_str().unwrap_or_default().into(),
            prefix: name["prefix"].as_str().map(Into::into),
            suffix: name["suffix"].as_str().map(Into::into),
            middle: name["middle"].as_str().map(Into::into),
        },
        emails: data["emails"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|e| GqlEmailAddress {
                        address: e["address"].as_str().unwrap_or_default().into(),
                        email_type: e["type"].as_str().map(Into::into),
                        primary: e["primary"].as_bool(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        phones: data["phones"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|p| GqlPhoneNumber {
                        number: p["number"].as_str().unwrap_or_default().into(),
                        phone_type: p["type"].as_str().map(Into::into),
                        primary: p["primary"].as_bool(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        addresses: data["addresses"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|addr| GqlPostalAddress {
                        street: addr["street"].as_str().map(Into::into),
                        city: addr["city"].as_str().map(Into::into),
                        region: addr["region"].as_str().map(Into::into),
                        postal_code: addr["postal_code"].as_str().map(Into::into),
                        country: addr["country"].as_str().map(Into::into),
                        address_type: addr["type"].as_str().map(Into::into),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        organization: data["organization"].as_str().map(Into::into),
        title: data["title"].as_str().map(Into::into),
        birthday: data["birthday"].as_str().map(Into::into),
        photo_url: data["photo_url"].as_str().map(Into::into),
        notes: data["notes"].as_str().map(Into::into),
        groups: data["groups"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(Into::into)).collect())
            .unwrap_or_default(),
        source: data["source"].as_str().unwrap_or_default().into(),
        source_id: data["source_id"].as_str().unwrap_or_default().into(),
        extensions: data.get("extensions").and_then(|v| {
            if v.is_null() { None } else { Some(async_graphql::Json(v.clone())) }
        }),
        created_at: data["created_at"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Utc::now),
        updated_at: data["updated_at"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Utc::now),
    }
}

fn record_to_event(data: &JsonValue) -> GqlCalendarEvent {
    GqlCalendarEvent {
        id: data["id"].as_str().unwrap_or_default().into(),
        title: data["title"].as_str().unwrap_or_default().into(),
        start: data["start"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Utc::now),
        end: data["end"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Utc::now),
        recurrence: data["recurrence"].as_str().map(Into::into),
        attendees: data["attendees"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(Into::into)).collect())
            .unwrap_or_default(),
        location: data["location"].as_str().map(Into::into),
        description: data["description"].as_str().map(Into::into),
        source: data["source"].as_str().unwrap_or_default().into(),
        source_id: data["source_id"].as_str().unwrap_or_default().into(),
        extensions: data.get("extensions").and_then(|v| {
            if v.is_null() { None } else { Some(async_graphql::Json(v.clone())) }
        }),
        created_at: data["created_at"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Utc::now),
        updated_at: data["updated_at"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Utc::now),
    }
}

fn record_to_email(data: &JsonValue) -> GqlEmail {
    GqlEmail {
        id: data["id"].as_str().unwrap_or_default().into(),
        from: data["from"].as_str().unwrap_or_default().into(),
        to: data["to"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(Into::into)).collect())
            .unwrap_or_default(),
        cc: data["cc"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(Into::into)).collect())
            .unwrap_or_default(),
        bcc: data["bcc"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(Into::into)).collect())
            .unwrap_or_default(),
        subject: data["subject"].as_str().unwrap_or_default().into(),
        body_text: data["body_text"].as_str().unwrap_or_default().into(),
        body_html: data["body_html"].as_str().map(Into::into),
        thread_id: data["thread_id"].as_str().map(Into::into),
        labels: data["labels"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(Into::into)).collect())
            .unwrap_or_default(),
        attachments: data["attachments"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|att| GqlEmailAttachment {
                        file_id: att["file_id"].as_str().unwrap_or_default().into(),
                        filename: att["filename"].as_str().unwrap_or_default().into(),
                        mime_type: att["mime_type"].as_str().unwrap_or_default().into(),
                        size: att["size"].as_u64().unwrap_or(0),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        source: data["source"].as_str().unwrap_or_default().into(),
        source_id: data["source_id"].as_str().unwrap_or_default().into(),
        extensions: data.get("extensions").and_then(|v| {
            if v.is_null() { None } else { Some(async_graphql::Json(v.clone())) }
        }),
        created_at: data["created_at"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Utc::now),
        updated_at: data["updated_at"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Utc::now),
    }
}

fn record_to_note(data: &JsonValue) -> GqlNote {
    GqlNote {
        id: data["id"].as_str().unwrap_or_default().into(),
        title: data["title"].as_str().unwrap_or_default().into(),
        body: data["body"].as_str().unwrap_or_default().into(),
        tags: data["tags"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(Into::into)).collect())
            .unwrap_or_default(),
        source: data["source"].as_str().unwrap_or_default().into(),
        source_id: data["source_id"].as_str().unwrap_or_default().into(),
        extensions: data.get("extensions").and_then(|v| {
            if v.is_null() { None } else { Some(async_graphql::Json(v.clone())) }
        }),
        created_at: data["created_at"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Utc::now),
        updated_at: data["updated_at"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Utc::now),
    }
}

fn record_to_file(data: &JsonValue) -> GqlFile {
    GqlFile {
        id: data["id"].as_str().unwrap_or_default().into(),
        name: data["name"].as_str().unwrap_or_default().into(),
        mime_type: data["mime_type"].as_str().unwrap_or_default().into(),
        size: data["size"].as_u64().unwrap_or(0),
        path: data["path"].as_str().unwrap_or_default().into(),
        checksum: data["checksum"].as_str().map(Into::into),
        source: data["source"].as_str().unwrap_or_default().into(),
        source_id: data["source_id"].as_str().unwrap_or_default().into(),
        extensions: data.get("extensions").and_then(|v| {
            if v.is_null() { None } else { Some(async_graphql::Json(v.clone())) }
        }),
        created_at: data["created_at"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Utc::now),
        updated_at: data["updated_at"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Utc::now),
    }
}

fn record_to_credential(data: &JsonValue) -> GqlCredential {
    GqlCredential {
        id: data["id"].as_str().unwrap_or_default().into(),
        credential_type: match data["type"].as_str().unwrap_or("api_key") {
            "oauth_token" => GqlCredentialType::OauthToken,
            "identity_document" => GqlCredentialType::IdentityDocument,
            "passkey" => GqlCredentialType::Passkey,
            _ => GqlCredentialType::ApiKey,
        },
        issuer: data["issuer"].as_str().unwrap_or_default().into(),
        issued_date: data["issued_date"].as_str().unwrap_or_default().into(),
        expiry_date: data["expiry_date"].as_str().map(Into::into),
        claims: async_graphql::Json(data["claims"].clone()),
        source: data["source"].as_str().unwrap_or_default().into(),
        source_id: data["source_id"].as_str().unwrap_or_default().into(),
        created_at: data["created_at"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Utc::now),
        updated_at: data["updated_at"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Utc::now),
    }
}

fn record_to_gql_record(record: &crate::storage::Record) -> GqlRecord {
    GqlRecord {
        id: record.id.clone(),
        plugin_id: record.plugin_id.clone(),
        collection: record.collection.clone(),
        data: async_graphql::Json(record.data.clone()),
        version: record.version,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

// ---------------------------------------------------------------------------
// Filter conversion (GraphQL input -> StorageAdapter types)
// ---------------------------------------------------------------------------

fn convert_filter(input: &FilterInput) -> QueryFilters {
    QueryFilters {
        equality: input
            .equality
            .iter()
            .map(|f| FieldFilter {
                field: f.field.clone(),
                value: serde_json::from_str(&f.value)
                    .unwrap_or_else(|_| JsonValue::String(f.value.clone())),
            })
            .collect(),
        comparison: input
            .comparison
            .iter()
            .filter_map(|f| {
                let op = match f.operator.as_str() {
                    "gte" => ComparisonOp::Gte,
                    "lte" => ComparisonOp::Lte,
                    "gt" => ComparisonOp::Gt,
                    "lt" => ComparisonOp::Lt,
                    _ => return None,
                };
                Some(ComparisonFilter {
                    field: f.field.clone(),
                    operator: op,
                    value: serde_json::from_str(&f.value)
                        .unwrap_or_else(|_| JsonValue::String(f.value.clone())),
                })
            })
            .collect(),
        text_search: input
            .text_search
            .iter()
            .map(|f| TextFilter {
                field: f.field.clone(),
                contains: f.contains.clone(),
            })
            .collect(),
        and: vec![],
        or: vec![],
    }
}

fn convert_sort(input: &SortInput) -> SortOptions {
    SortOptions {
        sort_by: input.sort_by.clone(),
        sort_dir: match input.sort_dir {
            Some(GqlSortDirection::Desc) => SortDirection::Desc,
            _ => SortDirection::Asc,
        },
    }
}

fn convert_pagination(input: &PaginationInput) -> Pagination {
    Pagination {
        limit: input.limit.unwrap_or(50).min(1000),
        offset: input.offset.unwrap_or(0),
    }
}

// ---------------------------------------------------------------------------
// Generic collection query helper
// ---------------------------------------------------------------------------

async fn query_collection<T>(
    ctx: &Context<'_>,
    collection: &str,
    filter: Option<FilterInput>,
    sort: Option<SortInput>,
    pagination: Option<PaginationInput>,
    converter: fn(&JsonValue) -> T,
) -> Result<Connection<T>>
where
    T: OutputType + Send + Sync,
{
    let storage = ctx
        .data::<Arc<crate::sqlite_storage::SqliteStorage>>()
        .map_err(|_| Error::new("storage not available"))?;

    let pg = convert_pagination(&pagination.unwrap_or_default());
    let sort_opts = sort.as_ref().map(convert_sort);
    let filters = filter
        .as_ref()
        .map(convert_filter)
        .unwrap_or_default();

    let has_filters = !filters.equality.is_empty()
        || !filters.comparison.is_empty()
        || !filters.text_search.is_empty();

    let result = if has_filters {
        storage
            .query(CORE_PLUGIN_ID, collection, filters, sort_opts, pg)
            .await
    } else {
        storage
            .list(CORE_PLUGIN_ID, collection, sort_opts, pg)
            .await
    }
    .map_err(|e| Error::new(format!("query failed: {e}")))?;

    Ok(Connection {
        data: result.records.iter().map(|r| converter(&r.data)).collect(),
        total: result.total,
        limit: result.limit,
        offset: result.offset,
    })
}

async fn get_single<T>(
    ctx: &Context<'_>,
    collection: &str,
    id: &str,
    converter: fn(&JsonValue) -> T,
) -> Result<Option<T>>
where
    T: OutputType + Send + Sync,
{
    let storage = ctx
        .data::<Arc<crate::sqlite_storage::SqliteStorage>>()
        .map_err(|_| Error::new("storage not available"))?;

    match storage.get(CORE_PLUGIN_ID, collection, id).await {
        Ok(Some(record)) => Ok(Some(converter(&record.data))),
        Ok(None) => Ok(None),
        Err(e) => Err(Error::new(format!("get failed: {e}"))),
    }
}

// ---------------------------------------------------------------------------
// QueryRoot
// ---------------------------------------------------------------------------

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    // --- Tasks ---
    async fn tasks(
        &self,
        ctx: &Context<'_>,
        filter: Option<FilterInput>,
        sort: Option<SortInput>,
        pagination: Option<PaginationInput>,
    ) -> Result<Connection<GqlTask>> {
        query_collection(ctx, "tasks", filter, sort, pagination, record_to_task).await
    }

    async fn task(&self, ctx: &Context<'_>, id: String) -> Result<Option<GqlTask>> {
        get_single(ctx, "tasks", &id, record_to_task).await
    }

    // --- Contacts ---
    async fn contacts(
        &self,
        ctx: &Context<'_>,
        filter: Option<FilterInput>,
        sort: Option<SortInput>,
        pagination: Option<PaginationInput>,
    ) -> Result<Connection<GqlContact>> {
        query_collection(ctx, "contacts", filter, sort, pagination, record_to_contact).await
    }

    async fn contact(&self, ctx: &Context<'_>, id: String) -> Result<Option<GqlContact>> {
        get_single(ctx, "contacts", &id, record_to_contact).await
    }

    // --- Events ---
    async fn events(
        &self,
        ctx: &Context<'_>,
        filter: Option<FilterInput>,
        sort: Option<SortInput>,
        pagination: Option<PaginationInput>,
    ) -> Result<Connection<GqlCalendarEvent>> {
        query_collection(ctx, "events", filter, sort, pagination, record_to_event).await
    }

    async fn event(&self, ctx: &Context<'_>, id: String) -> Result<Option<GqlCalendarEvent>> {
        get_single(ctx, "events", &id, record_to_event).await
    }

    // --- Emails ---
    async fn emails(
        &self,
        ctx: &Context<'_>,
        filter: Option<FilterInput>,
        sort: Option<SortInput>,
        pagination: Option<PaginationInput>,
    ) -> Result<Connection<GqlEmail>> {
        query_collection(ctx, "emails", filter, sort, pagination, record_to_email).await
    }

    async fn email(&self, ctx: &Context<'_>, id: String) -> Result<Option<GqlEmail>> {
        get_single(ctx, "emails", &id, record_to_email).await
    }

    // --- Notes ---
    async fn notes(
        &self,
        ctx: &Context<'_>,
        filter: Option<FilterInput>,
        sort: Option<SortInput>,
        pagination: Option<PaginationInput>,
    ) -> Result<Connection<GqlNote>> {
        query_collection(ctx, "notes", filter, sort, pagination, record_to_note).await
    }

    async fn note(&self, ctx: &Context<'_>, id: String) -> Result<Option<GqlNote>> {
        get_single(ctx, "notes", &id, record_to_note).await
    }

    // --- Files ---
    async fn files(
        &self,
        ctx: &Context<'_>,
        filter: Option<FilterInput>,
        sort: Option<SortInput>,
        pagination: Option<PaginationInput>,
    ) -> Result<Connection<GqlFile>> {
        query_collection(ctx, "files", filter, sort, pagination, record_to_file).await
    }

    async fn file(&self, ctx: &Context<'_>, id: String) -> Result<Option<GqlFile>> {
        get_single(ctx, "files", &id, record_to_file).await
    }

    // --- Credentials ---
    async fn credentials(
        &self,
        ctx: &Context<'_>,
        filter: Option<FilterInput>,
        sort: Option<SortInput>,
        pagination: Option<PaginationInput>,
    ) -> Result<Connection<GqlCredential>> {
        query_collection(ctx, "credentials", filter, sort, pagination, record_to_credential).await
    }

    async fn credential(&self, ctx: &Context<'_>, id: String) -> Result<Option<GqlCredential>> {
        get_single(ctx, "credentials", &id, record_to_credential).await
    }
}

// ---------------------------------------------------------------------------
// MutationRoot
// ---------------------------------------------------------------------------

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    /// Create a record in any collection.
    async fn create_record(
        &self,
        ctx: &Context<'_>,
        collection: String,
        input: RecordInput,
    ) -> Result<GqlRecord> {
        let storage = ctx
            .data::<Arc<crate::sqlite_storage::SqliteStorage>>()
            .map_err(|_| Error::new("storage not available"))?;
        let bus = ctx
            .data::<Arc<MessageBus>>()
            .map_err(|_| Error::new("message bus not available"))?;

        let mut data: JsonValue = serde_json::from_str(&input.data)
            .map_err(|e| Error::new(format!("invalid JSON: {e}")))?;

        // Strip client-supplied identity fields to prevent spoofing.
        if let Some(obj) = data.as_object_mut() {
            obj.remove("_user_id");
            obj.remove("_household_id");
        }

        let record = storage
            .create(CORE_PLUGIN_ID, &collection, data)
            .await
            .map_err(|e| Error::new(format!("create failed: {e}")))?;

        bus.publish(BusEvent::NewRecords {
            collection: collection.clone(),
            count: 1,
        });
        bus.publish(BusEvent::RecordChanged {
            record: record.clone(),
        });

        Ok(record_to_gql_record(&record))
    }

    /// Update a record in any collection (optimistic concurrency via version).
    async fn update_record(
        &self,
        ctx: &Context<'_>,
        collection: String,
        id: String,
        input: UpdateRecordInput,
    ) -> Result<GqlRecord> {
        let storage = ctx
            .data::<Arc<crate::sqlite_storage::SqliteStorage>>()
            .map_err(|_| Error::new("storage not available"))?;
        let bus = ctx
            .data::<Arc<MessageBus>>()
            .map_err(|_| Error::new("message bus not available"))?;

        let mut data: JsonValue = serde_json::from_str(&input.data)
            .map_err(|e| Error::new(format!("invalid JSON: {e}")))?;

        // Strip client-supplied identity fields to prevent spoofing.
        if let Some(obj) = data.as_object_mut() {
            obj.remove("_user_id");
            obj.remove("_household_id");
        }

        let record = storage
            .update(CORE_PLUGIN_ID, &collection, &id, data, input.version)
            .await
            .map_err(|e| match e {
                crate::storage::StorageError::VersionMismatch => {
                    Error::new("version conflict: record was modified by another request")
                }
                crate::storage::StorageError::NotFound => {
                    Error::new(format!("record '{id}' not found in '{collection}'"))
                }
                other => Error::new(format!("update failed: {other}")),
            })?;

        bus.publish(BusEvent::NewRecords {
            collection: collection.clone(),
            count: 1,
        });
        bus.publish(BusEvent::RecordChanged {
            record: record.clone(),
        });

        Ok(record_to_gql_record(&record))
    }

    /// Delete a record from any collection.
    async fn delete_record(
        &self,
        ctx: &Context<'_>,
        collection: String,
        id: String,
    ) -> Result<bool> {
        let storage = ctx
            .data::<Arc<crate::sqlite_storage::SqliteStorage>>()
            .map_err(|_| Error::new("storage not available"))?;
        let bus = ctx
            .data::<Arc<MessageBus>>()
            .map_err(|_| Error::new("message bus not available"))?;

        let deleted = storage
            .delete(CORE_PLUGIN_ID, &collection, &id)
            .await
            .map_err(|e| Error::new(format!("delete failed: {e}")))?;

        if deleted {
            bus.publish(BusEvent::RecordDeleted {
                record_id: id,
                collection: collection.clone(),
            });
        }

        Ok(deleted)
    }
}

// ---------------------------------------------------------------------------
// SubscriptionRoot
// ---------------------------------------------------------------------------

pub struct SubscriptionRoot;

/// A subscription event delivered to GraphQL clients.
#[derive(SimpleObject, Clone)]
pub struct RecordChangeEvent {
    /// The type of change: "created", "updated", or "deleted".
    pub change_type: String,
    /// The collection that changed.
    pub collection: String,
    /// The record data (None for deletions).
    pub record: Option<GqlRecord>,
    /// The deleted record ID (only for deletions).
    pub deleted_id: Option<String>,
}

#[Subscription]
impl SubscriptionRoot {
    /// Subscribe to record changes, optionally filtered by collection.
    async fn record_changes(
        &self,
        ctx: &Context<'_>,
        collection: Option<String>,
    ) -> Result<impl Stream<Item = RecordChangeEvent>> {
        let bus = ctx
            .data::<Arc<MessageBus>>()
            .map_err(|_| Error::new("message bus not available"))?;

        let rx = bus.subscribe();
        let stream = BroadcastStream::new(rx);

        Ok(stream.filter_map(move |result| {
            match result {
                Ok(BusEvent::RecordChanged { record }) => {
                    if let Some(ref col) = collection {
                        if &record.collection != col {
                            return None;
                        }
                    }
                    Some(RecordChangeEvent {
                        change_type: "changed".into(),
                        collection: record.collection.clone(),
                        record: Some(record_to_gql_record(&record)),
                        deleted_id: None,
                    })
                }
                Ok(BusEvent::RecordDeleted { record_id, collection: del_collection }) => {
                    if let Some(ref col) = collection {
                        if &del_collection != col {
                            return None;
                        }
                    }
                    Some(RecordChangeEvent {
                        change_type: "deleted".into(),
                        collection: del_collection,
                        record: None,
                        deleted_id: Some(record_id),
                    })
                }
                Ok(BusEvent::NewRecords { collection: col, count: _ }) => {
                    if let Some(ref filter_col) = collection {
                        if &col != filter_col {
                            return None;
                        }
                    }
                    Some(RecordChangeEvent {
                        change_type: "new_records".into(),
                        collection: col,
                        record: None,
                        deleted_id: None,
                    })
                }
                _ => None,
            }
        }))
    }
}

// ---------------------------------------------------------------------------
// Schema construction and route handlers
// ---------------------------------------------------------------------------

pub type LifeEngineSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

/// Build the GraphQL schema with storage and message bus in context.
pub fn build_schema(
    storage: Arc<crate::sqlite_storage::SqliteStorage>,
    message_bus: Arc<MessageBus>,
) -> LifeEngineSchema {
    Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
        .data(storage)
        .data(message_bus)
        .finish()
}

/// Axum handler for GraphQL requests (POST /api/graphql).
pub async fn graphql_handler(
    State(state): axum::extract::State<AppState>,
    req: async_graphql_axum::GraphQLRequest,
) -> axum::response::Response {
    let storage = match state.storage {
        Some(s) => s,
        None => {
            return (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                axum::Json(serde_json::json!({"error": "storage not initialized"})),
            )
                .into_response();
        }
    };
    let schema = build_schema(storage, state.message_bus);
    let resp: async_graphql_axum::GraphQLResponse = schema.execute(req.into_inner()).await.into();
    resp.into_response()
}

/// Axum handler for GraphQL Playground (GET /api/graphql/playground).
///
/// Only available in debug builds. In release builds this returns 404
/// to prevent exposing an interactive query tool in production.
pub async fn graphql_playground() -> axum::response::Response {
    if cfg!(debug_assertions) {
        axum::response::Html(async_graphql::http::playground_source(
            async_graphql::http::GraphQLPlaygroundConfig::new("/api/graphql"),
        ))
        .into_response()
    } else {
        (
            axum::http::StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({"error": "playground is disabled in release builds"})),
        )
            .into_response()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite_storage::SqliteStorage;
    use serde_json::json;

    fn setup_test_schema() -> (LifeEngineSchema, Arc<SqliteStorage>, Arc<MessageBus>) {
        let storage = Arc::new(SqliteStorage::open_in_memory().unwrap());
        let bus = Arc::new(MessageBus::new());
        let schema = build_schema(Arc::clone(&storage), Arc::clone(&bus));
        (schema, storage, bus)
    }

    // -----------------------------------------------------------------------
    // TDD:RED — Auto-generated types match CDM schemas
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn task_type_has_all_cdm_fields() {
        let (schema, storage, _) = setup_test_schema();

        // Create a task via storage.
        storage
            .create(
                CORE_PLUGIN_ID,
                "tasks",
                json!({
                    "id": "task-001",
                    "title": "Test task",
                    "description": "A test",
                    "status": "pending",
                    "priority": "high",
                    "tags": ["test"],
                    "source": "test",
                    "source_id": "t1",
                    "created_at": "2026-03-22T00:00:00Z",
                    "updated_at": "2026-03-22T00:00:00Z"
                }),
            )
            .await
            .unwrap();

        let result = schema
            .execute(
                r#"{ tasks { data { id title description status priority tags source sourceId createdAt updatedAt } total } }"#,
            )
            .await;

        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let data = result.data.into_json().unwrap();
        let task = &data["tasks"]["data"][0];
        assert_eq!(task["title"], "Test task");
        assert_eq!(task["status"], "PENDING");
        assert_eq!(task["priority"], "HIGH");
        assert_eq!(task["tags"][0], "test");
    }

    #[tokio::test]
    async fn contact_type_has_all_cdm_fields() {
        let (schema, storage, _) = setup_test_schema();

        storage
            .create(
                CORE_PLUGIN_ID,
                "contacts",
                json!({
                    "id": "contact-001",
                    "name": {"given": "Jane", "family": "Doe"},
                    "emails": [{"address": "jane@example.com", "type": "work", "primary": true}],
                    "phones": [{"number": "+1234567890", "type": "mobile"}],
                    "organization": "Acme",
                    "source": "test",
                    "source_id": "c1",
                    "created_at": "2026-03-22T00:00:00Z",
                    "updated_at": "2026-03-22T00:00:00Z"
                }),
            )
            .await
            .unwrap();

        let result = schema
            .execute(
                r#"{ contacts { data { id name { given family } emails { address emailType primary } phones { number phoneType } organization source } total } }"#,
            )
            .await;

        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let data = result.data.into_json().unwrap();
        let contact = &data["contacts"]["data"][0];
        assert_eq!(contact["name"]["given"], "Jane");
        assert_eq!(contact["emails"][0]["address"], "jane@example.com");
    }

    #[tokio::test]
    async fn event_type_has_all_cdm_fields() {
        let (schema, storage, _) = setup_test_schema();

        storage
            .create(
                CORE_PLUGIN_ID,
                "events",
                json!({
                    "id": "event-001",
                    "title": "Meeting",
                    "start": "2026-03-22T09:00:00Z",
                    "end": "2026-03-22T10:00:00Z",
                    "attendees": ["user@example.com"],
                    "location": "Room A",
                    "source": "test",
                    "source_id": "e1",
                    "created_at": "2026-03-22T00:00:00Z",
                    "updated_at": "2026-03-22T00:00:00Z"
                }),
            )
            .await
            .unwrap();

        let result = schema
            .execute(
                r#"{ events { data { id title start end attendees location source } total } }"#,
            )
            .await;

        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let data = result.data.into_json().unwrap();
        assert_eq!(data["events"]["data"][0]["title"], "Meeting");
    }

    #[tokio::test]
    async fn all_seven_collections_are_queryable() {
        let (schema, _, _) = setup_test_schema();

        for query in [
            "{ tasks { total } }",
            "{ contacts { total } }",
            "{ events { total } }",
            "{ emails { total } }",
            "{ notes { total } }",
            "{ files { total } }",
            "{ credentials { total } }",
        ] {
            let result = schema.execute(query).await;
            assert!(
                result.errors.is_empty(),
                "query '{query}' failed: {:?}",
                result.errors
            );
        }
    }

    // -----------------------------------------------------------------------
    // TDD:RED — Queries return correct data for all collections
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn query_single_task_by_id() {
        let (schema, storage, _) = setup_test_schema();

        let record = storage
            .create(
                CORE_PLUGIN_ID,
                "tasks",
                json!({"title": "Find me", "status": "in_progress", "priority": "low", "source": "t", "source_id": "x"}),
            )
            .await
            .unwrap();

        let query = format!(r#"{{ task(id: "{}") {{ title status }} }}"#, record.id);
        let result = schema.execute(&query).await;
        assert!(result.errors.is_empty(), "{:?}", result.errors);

        let data = result.data.into_json().unwrap();
        assert_eq!(data["task"]["title"], "Find me");
        assert_eq!(data["task"]["status"], "IN_PROGRESS");
    }

    #[tokio::test]
    async fn query_nonexistent_returns_null() {
        let (schema, _, _) = setup_test_schema();

        let result = schema
            .execute(r#"{ task(id: "nonexistent") { title } }"#)
            .await;
        assert!(result.errors.is_empty());
        let data = result.data.into_json().unwrap();
        assert!(data["task"].is_null());
    }

    #[tokio::test]
    async fn query_notes_returns_correct_data() {
        let (schema, storage, _) = setup_test_schema();

        storage
            .create(
                CORE_PLUGIN_ID,
                "notes",
                json!({"title": "My note", "body": "Content", "tags": ["rust"], "source": "t", "source_id": "n1"}),
            )
            .await
            .unwrap();

        let result = schema
            .execute(r#"{ notes { data { title body tags } total } }"#)
            .await;
        assert!(result.errors.is_empty());
        let data = result.data.into_json().unwrap();
        assert_eq!(data["notes"]["total"], 1);
        assert_eq!(data["notes"]["data"][0]["title"], "My note");
    }

    // -----------------------------------------------------------------------
    // TDD:RED — Mutations create/update/delete correctly
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn mutation_create_record() {
        let (schema, _, _) = setup_test_schema();

        let result = schema
            .execute(
                r#"mutation {
                    createRecord(collection: "tasks", input: { data: "{\"title\": \"Created via GQL\", \"status\": \"pending\", \"priority\": \"medium\", \"source\": \"gql\", \"source_id\": \"g1\"}" }) {
                        id collection version
                    }
                }"#,
            )
            .await;

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let data = result.data.into_json().unwrap();
        assert_eq!(data["createRecord"]["collection"], "tasks");
        assert_eq!(data["createRecord"]["version"], 1);
        assert!(!data["createRecord"]["id"].as_str().unwrap().is_empty());
    }

    #[tokio::test]
    async fn mutation_update_record() {
        let (schema, storage, _) = setup_test_schema();

        let record = storage
            .create(
                CORE_PLUGIN_ID,
                "tasks",
                json!({"title": "Original", "source": "t", "source_id": "x"}),
            )
            .await
            .unwrap();

        let mutation = format!(
            r#"mutation {{
                updateRecord(collection: "tasks", id: "{}", input: {{ data: "{{\"title\": \"Updated\"}}", version: 1 }}) {{
                    id version data
                }}
            }}"#,
            record.id
        );

        let result = schema.execute(&mutation).await;
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let data = result.data.into_json().unwrap();
        assert_eq!(data["updateRecord"]["version"], 2);
    }

    #[tokio::test]
    async fn mutation_update_wrong_version_fails() {
        let (schema, storage, _) = setup_test_schema();

        let record = storage
            .create(
                CORE_PLUGIN_ID,
                "tasks",
                json!({"title": "Original", "source": "t", "source_id": "x"}),
            )
            .await
            .unwrap();

        let mutation = format!(
            r#"mutation {{
                updateRecord(collection: "tasks", id: "{}", input: {{ data: "{{\"title\": \"Bad\"}}", version: 99 }}) {{
                    id
                }}
            }}"#,
            record.id
        );

        let result = schema.execute(&mutation).await;
        assert!(!result.errors.is_empty());
        assert!(result.errors[0].message.contains("version conflict"));
    }

    #[tokio::test]
    async fn mutation_delete_record() {
        let (schema, storage, _) = setup_test_schema();

        let record = storage
            .create(
                CORE_PLUGIN_ID,
                "tasks",
                json!({"title": "Delete me", "source": "t", "source_id": "x"}),
            )
            .await
            .unwrap();

        let mutation = format!(
            r#"mutation {{ deleteRecord(collection: "tasks", id: "{}") }}"#,
            record.id
        );

        let result = schema.execute(&mutation).await;
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let data = result.data.into_json().unwrap();
        assert_eq!(data["deleteRecord"], true);

        // Verify deleted.
        let get_result = storage.get(CORE_PLUGIN_ID, "tasks", &record.id).await.unwrap();
        assert!(get_result.is_none());
    }

    #[tokio::test]
    async fn mutation_delete_nonexistent_returns_false() {
        let (schema, _, _) = setup_test_schema();

        let result = schema
            .execute(r#"mutation { deleteRecord(collection: "tasks", id: "nope") }"#)
            .await;
        assert!(result.errors.is_empty());
        let data = result.data.into_json().unwrap();
        assert_eq!(data["deleteRecord"], false);
    }

    // -----------------------------------------------------------------------
    // TDD:RED — Subscriptions deliver real-time updates
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn subscription_receives_record_change() {
        let (_, storage, bus) = setup_test_schema();
        let schema = build_schema(Arc::clone(&storage), Arc::clone(&bus));

        // Start subscription stream in a background task.
        let bus_clone = Arc::clone(&bus);
        let storage_clone = Arc::clone(&storage);
        let (tx, rx) = tokio::sync::oneshot::channel::<JsonValue>();

        let handle = tokio::spawn(async move {
            let mut stream = schema.execute_stream(
                "subscription { recordChanges { changeType collection } }",
            );

            // Skip responses that don't have data (initial setup or errors).
            // Signal readiness by publishing after a small delay.
            let delay_bus = Arc::clone(&bus_clone);
            let delay_storage = Arc::clone(&storage_clone);
            tokio::spawn(async move {
                // Small delay to ensure subscription stream is listening.
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                let record = delay_storage
                    .create(CORE_PLUGIN_ID, "tasks", json!({"title": "Sub test"}))
                    .await
                    .unwrap();
                delay_bus.publish(BusEvent::RecordChanged { record });
            });

            while let Some(resp) = stream.next().await {
                if resp.errors.is_empty() {
                    if let Ok(json) = resp.data.into_json() {
                        if !json.is_null() && json.get("recordChanges").is_some() {
                            let _ = tx.send(json);
                            return;
                        }
                    }
                }
            }
        });

        let data = tokio::time::timeout(std::time::Duration::from_secs(5), rx)
            .await
            .expect("subscription should deliver within timeout")
            .expect("should receive data");

        assert_eq!(data["recordChanges"]["changeType"], "changed");
        assert_eq!(data["recordChanges"]["collection"], "tasks");

        handle.abort();
    }

    // -----------------------------------------------------------------------
    // TDD:RED — Nested queries resolve relationships
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn event_attendee_contacts_resolves() {
        let (schema, storage, _) = setup_test_schema();

        // Create a contact.
        storage
            .create(
                CORE_PLUGIN_ID,
                "contacts",
                json!({
                    "name": {"given": "Alice", "family": "Smith"},
                    "emails": [{"address": "alice@example.com"}],
                    "source": "test",
                    "source_id": "c1"
                }),
            )
            .await
            .unwrap();

        // Create an event with that contact as attendee.
        storage
            .create(
                CORE_PLUGIN_ID,
                "events",
                json!({
                    "title": "Team sync",
                    "start": "2026-03-22T09:00:00Z",
                    "end": "2026-03-22T10:00:00Z",
                    "attendees": ["alice@example.com"],
                    "source": "test",
                    "source_id": "e1"
                }),
            )
            .await
            .unwrap();

        let result = schema
            .execute(
                r#"{ events { data { title attendeeContacts { name { given family } emails { address } } } } }"#,
            )
            .await;

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let data = result.data.into_json().unwrap();
        let contacts = &data["events"]["data"][0]["attendeeContacts"];
        assert_eq!(contacts[0]["name"]["given"], "Alice");
    }

    // -----------------------------------------------------------------------
    // TDD:RED — Filtering, sorting, pagination work correctly
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn query_with_pagination() {
        let (schema, storage, _) = setup_test_schema();

        for i in 0..5 {
            storage
                .create(
                    CORE_PLUGIN_ID,
                    "tasks",
                    json!({"title": format!("Task {i}"), "source": "t", "source_id": format!("t{i}")}),
                )
                .await
                .unwrap();
        }

        let result = schema
            .execute(r#"{ tasks(pagination: { limit: 2, offset: 0 }) { data { title } total limit offset } }"#)
            .await;

        assert!(result.errors.is_empty());
        let data = result.data.into_json().unwrap();
        assert_eq!(data["tasks"]["total"], 5);
        assert_eq!(data["tasks"]["limit"], 2);
        assert_eq!(data["tasks"]["data"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn query_with_sort() {
        let (schema, storage, _) = setup_test_schema();

        for title in ["Banana", "Apple", "Cherry"] {
            storage
                .create(
                    CORE_PLUGIN_ID,
                    "tasks",
                    json!({"title": title, "source": "t", "source_id": title}),
                )
                .await
                .unwrap();
        }

        let result = schema
            .execute(
                r#"{ tasks(sort: { sortBy: "created_at", sortDir: DESC }) { data { title } total } }"#,
            )
            .await;

        assert!(result.errors.is_empty());
        let data = result.data.into_json().unwrap();
        assert_eq!(data["tasks"]["total"], 3);
    }

    #[tokio::test]
    async fn query_with_text_filter() {
        let (schema, storage, _) = setup_test_schema();

        storage
            .create(
                CORE_PLUGIN_ID,
                "tasks",
                json!({"title": "Searchable item", "source": "t", "source_id": "s1"}),
            )
            .await
            .unwrap();
        storage
            .create(
                CORE_PLUGIN_ID,
                "tasks",
                json!({"title": "Other thing", "source": "t", "source_id": "s2"}),
            )
            .await
            .unwrap();

        let result = schema
            .execute(
                r#"{ tasks(filter: { textSearch: [{ field: "title", contains: "Searchable" }] }) { data { title } total } }"#,
            )
            .await;

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let data = result.data.into_json().unwrap();
        assert_eq!(data["tasks"]["total"], 1);
        assert_eq!(data["tasks"]["data"][0]["title"], "Searchable item");
    }

    #[tokio::test]
    async fn query_with_equality_filter() {
        let (schema, storage, _) = setup_test_schema();

        storage
            .create(
                CORE_PLUGIN_ID,
                "tasks",
                json!({"title": "A", "status": "pending", "source": "t", "source_id": "1"}),
            )
            .await
            .unwrap();
        storage
            .create(
                CORE_PLUGIN_ID,
                "tasks",
                json!({"title": "B", "status": "completed", "source": "t", "source_id": "2"}),
            )
            .await
            .unwrap();

        let result = schema
            .execute(
                r#"{ tasks(filter: { equality: [{ field: "status", value: "\"pending\"" }] }) { data { title } total } }"#,
            )
            .await;

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let data = result.data.into_json().unwrap();
        assert_eq!(data["tasks"]["total"], 1);
    }

    // -----------------------------------------------------------------------
    // Mutation bus event tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn create_mutation_publishes_bus_event() {
        let (schema, _, bus) = setup_test_schema();
        let mut rx = bus.subscribe();

        schema
            .execute(
                r#"mutation {
                    createRecord(collection: "tasks", input: { data: "{\"title\": \"Bus test\"}" }) { id }
                }"#,
            )
            .await;

        let event = rx.recv().await.expect("should receive event");
        match event {
            BusEvent::NewRecords { collection, .. } => assert_eq!(collection, "tasks"),
            _ => panic!("expected NewRecords event"),
        }
    }

    // -----------------------------------------------------------------------
    // Playground handler test
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn playground_returns_html() {
        use axum::response::IntoResponse;

        let resp = graphql_playground().await.into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
    }
}
