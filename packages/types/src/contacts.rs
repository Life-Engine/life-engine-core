//! Contact canonical data model.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Structured name for a contact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContactName {
    /// Given/first name.
    pub given: String,
    /// Family/last name.
    pub family: String,
    /// Full display name as the user prefers it.
    pub display: String,
}

/// An email address entry for a contact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmailAddress {
    pub address: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub email_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary: Option<bool>,
}

/// A phone number entry for a contact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PhoneNumber {
    pub number: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub phone_type: Option<String>,
}

/// A postal address for a contact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PostalAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub street: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postcode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
}

/// A contact in the Life Engine canonical data model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Contact {
    pub id: Uuid,
    pub name: ContactName,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub emails: Vec<EmailAddress>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phones: Vec<PhoneNumber>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub addresses: Vec<PostalAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organisation: Option<String>,
    pub source: String,
    pub source_id: String,
    /// Plugin-specific extension data, namespaced by plugin ID (reverse-domain format).
    /// Each key is a plugin's manifest `id` (e.g. `com.life-engine.todos`) and each value
    /// is an opaque JSON object owned by that plugin. See ADR-014.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
