//! Contact canonical data model.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Type label for contact information (emails, addresses).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ContactInfoType {
    Home,
    Work,
    Other,
}

/// Type label for phone numbers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PhoneType {
    Mobile,
    Home,
    Work,
    Fax,
    Other,
}

/// Structured name for a contact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContactName {
    /// Given/first name.
    pub given: String,
    /// Family/last name.
    pub family: String,
    /// Name prefix (e.g., Mr, Dr).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    /// Name suffix (e.g., Jr, III).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,
    /// Middle name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub middle: Option<String>,
}

/// An email address entry for a contact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContactEmail {
    /// Email address.
    pub address: String,
    /// Type of email (home, work, other).
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub email_type: Option<ContactInfoType>,
    /// Whether this is the primary email.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary: Option<bool>,
}

/// A phone number entry for a contact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContactPhone {
    /// Phone number.
    pub number: String,
    /// Type of phone (mobile, home, work, fax, other).
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub phone_type: Option<PhoneType>,
    /// Whether this is the primary phone number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary: Option<bool>,
}

/// A postal address for a contact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContactAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub street: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    /// Type of address (home, work, other).
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub address_type: Option<ContactInfoType>,
}

/// A contact in the Life Engine canonical data model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Contact {
    pub id: Uuid,
    pub name: ContactName,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub emails: Vec<ContactEmail>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phones: Vec<ContactPhone>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub addresses: Vec<ContactAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub birthday: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub photo_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<String>,
    pub source: String,
    pub source_id: String,
    /// Plugin-specific extension data, namespaced by plugin ID (reverse-domain format).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
