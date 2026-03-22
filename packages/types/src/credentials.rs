//! Credential canonical data model.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The type of credential stored.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialType {
    OauthToken,
    ApiKey,
    IdentityDocument,
    Passkey,
}

/// A credential in the Life Engine canonical data model.
///
/// Credentials intentionally have NO extensions field — the `claims` object
/// serves that purpose for type-specific data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Credential {
    pub id: Uuid,
    pub name: String,
    pub credential_type: CredentialType,
    pub service: String,
    pub claims: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    pub source: String,
    pub source_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
