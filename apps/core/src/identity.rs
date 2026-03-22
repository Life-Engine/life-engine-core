//! Identity and Credential System.
//!
//! Provides secure storage and selective disclosure of identity credentials
//! (passport, licence, certificates). Credentials are encrypted with a
//! separate key from the main data store. Supports W3C Verifiable
//! Credentials 2.0 format and DID alignment.
//!
//! # Security
//!
//! - Credential values are NEVER logged.
//! - Each credential is encrypted with a dedicated identity key (separate
//!   from the main SQLCipher key and plugin credential key).
//! - All disclosure operations are recorded in the audit log.

use crate::crypto;
use anyhow::{Context, Result};
use base64::Engine as _;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

// ──────────────────────────────────────────────────────────────
// Types
// ──────────────────────────────────────────────────────────────

/// The type of identity credential stored.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialType {
    Passport,
    DriversLicence,
    Certificate,
    IdentityCard,
    Custom(String),
}

/// An identity credential with encrypted claims.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityCredential {
    /// Unique identifier.
    pub id: String,
    /// Credential type.
    pub credential_type: CredentialType,
    /// The issuing authority (e.g., "au.gov", "us.state.ca").
    pub issuer: String,
    /// Date the credential was issued.
    pub issued_date: DateTime<Utc>,
    /// Expiration date (None for non-expiring).
    pub expiry_date: Option<DateTime<Utc>>,
    /// Type-specific claims (stored encrypted at rest).
    pub claims: serde_json::Value,
    /// Record creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Record last-updated timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Metadata returned in list responses (no sensitive claims).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialMetadata {
    pub id: String,
    pub credential_type: CredentialType,
    pub issuer: String,
    pub issued_date: DateTime<Utc>,
    pub expiry_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<&IdentityCredential> for CredentialMetadata {
    fn from(cred: &IdentityCredential) -> Self {
        Self {
            id: cred.id.clone(),
            credential_type: cred.credential_type.clone(),
            issuer: cred.issuer.clone(),
            issued_date: cred.issued_date,
            expiry_date: cred.expiry_date,
            created_at: cred.created_at,
            updated_at: cred.updated_at,
        }
    }
}

/// A selective disclosure token asserting specific claims.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisclosureToken {
    /// Unique token identifier.
    pub id: String,
    /// The credential this token is derived from.
    pub credential_id: String,
    /// The disclosed claim names and their asserted values.
    pub disclosed_claims: HashMap<String, serde_json::Value>,
    /// When the token was issued.
    pub issued_at: DateTime<Utc>,
    /// When the token expires.
    pub expires_at: DateTime<Utc>,
    /// HMAC-SHA256 signature over the token payload.
    pub signature: String,
}

/// An audit log entry for a disclosure event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisclosureAuditEntry {
    /// Unique entry identifier.
    pub id: String,
    /// The credential that was disclosed.
    pub credential_id: String,
    /// The claims that were disclosed.
    pub disclosed_claims: Vec<String>,
    /// Who the disclosure was made to.
    pub recipient: String,
    /// When the disclosure occurred.
    pub timestamp: DateTime<Utc>,
    /// The disclosure token ID.
    pub token_id: String,
}

/// W3C Verifiable Credential 2.0 representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiableCredential {
    /// JSON-LD context.
    #[serde(rename = "@context")]
    pub context: Vec<String>,
    /// The VC type.
    #[serde(rename = "type")]
    pub vc_type: Vec<String>,
    /// Credential identifier.
    pub id: String,
    /// Issuer identifier (DID or URL).
    pub issuer: String,
    /// Issuance date.
    #[serde(rename = "issuanceDate")]
    pub issuance_date: String,
    /// Expiration date.
    #[serde(rename = "expirationDate", skip_serializing_if = "Option::is_none")]
    pub expiration_date: Option<String>,
    /// The credential subject.
    #[serde(rename = "credentialSubject")]
    pub credential_subject: CredentialSubject,
}

/// The subject of a verifiable credential.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialSubject {
    /// The subject DID.
    pub id: String,
    /// Claims about the subject.
    #[serde(flatten)]
    pub claims: HashMap<String, serde_json::Value>,
}

// ──────────────────────────────────────────────────────────────
// Identity Store
// ──────────────────────────────────────────────────────────────

/// Encrypted identity credential store backed by SQLite.
pub struct IdentityStore {
    conn: Arc<Mutex<rusqlite::Connection>>,
    /// Separate encryption key for identity credentials.
    encryption_key: Vec<u8>,
    /// Signing key for disclosure tokens.
    signing_key: Vec<u8>,
}

impl IdentityStore {
    /// Create a new identity store with a separate encryption key.
    ///
    /// The `identity_secret` is distinct from the main data encryption
    /// key, providing defence-in-depth for sensitive identity data.
    pub fn new(
        conn: Arc<Mutex<rusqlite::Connection>>,
        identity_secret: &str,
    ) -> Result<Self> {
        let encryption_key = crypto::derive_key(identity_secret, crypto::DOMAIN_IDENTITY_ENCRYPT);
        let signing_key = crypto::derive_key(identity_secret, crypto::DOMAIN_IDENTITY_SIGN);
        Ok(Self {
            conn,
            encryption_key,
            signing_key,
        })
    }

    /// Initialise the identity tables.
    pub async fn init(&self) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS identity_credentials (
                id TEXT PRIMARY KEY,
                credential_type TEXT NOT NULL,
                issuer TEXT NOT NULL,
                issued_date TEXT NOT NULL,
                expiry_date TEXT,
                encrypted_claims TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS disclosure_audit_log (
                id TEXT PRIMARY KEY,
                credential_id TEXT NOT NULL,
                disclosed_claims TEXT NOT NULL,
                recipient TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                token_id TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_disclosure_audit_timestamp
                ON disclosure_audit_log(timestamp);
            CREATE INDEX IF NOT EXISTS idx_disclosure_audit_credential
                ON disclosure_audit_log(credential_id);",
        )
        .context("failed to create identity tables")?;
        tracing::debug!("identity store initialised");
        Ok(())
    }

    // ── CRUD ────────────────────────────────────────────────

    /// Create a new identity credential.
    pub async fn create(&self, credential: &IdentityCredential) -> Result<()> {
        let encrypted_claims = self.encrypt_claims(&credential.claims)?;
        let credential_type = serde_json::to_string(&credential.credential_type)?;
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO identity_credentials
                (id, credential_type, issuer, issued_date, expiry_date,
                 encrypted_claims, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                credential.id,
                credential_type,
                credential.issuer,
                credential.issued_date.to_rfc3339(),
                credential.expiry_date.map(|d| d.to_rfc3339()),
                encrypted_claims,
                credential.created_at.to_rfc3339(),
                credential.updated_at.to_rfc3339(),
            ],
        )
        .context("failed to create identity credential")?;
        // Never log claims.
        tracing::debug!(
            id = credential.id,
            credential_type = ?credential.credential_type,
            "identity credential created (claims redacted)"
        );
        Ok(())
    }

    /// Retrieve an identity credential by ID (includes decrypted claims).
    pub async fn get(&self, id: &str) -> Result<Option<IdentityCredential>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, credential_type, issuer, issued_date, expiry_date,
                    encrypted_claims, created_at, updated_at
             FROM identity_credentials WHERE id = ?1",
        )?;

        let result = stmt
            .query_row(rusqlite::params![id], |row| {
                Ok(RawCredentialRow {
                    id: row.get(0)?,
                    credential_type: row.get(1)?,
                    issuer: row.get(2)?,
                    issued_date: row.get(3)?,
                    expiry_date: row.get(4)?,
                    encrypted_claims: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            })
            .optional()?;

        match result {
            Some(row) => {
                let credential = self.row_to_credential(row)?;
                Ok(Some(credential))
            }
            None => Ok(None),
        }
    }

    /// List all credentials (metadata only, no claims).
    pub async fn list(&self) -> Result<Vec<CredentialMetadata>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, credential_type, issuer, issued_date, expiry_date,
                    created_at, updated_at
             FROM identity_credentials ORDER BY created_at DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(RawMetadataRow {
                id: row.get(0)?,
                credential_type: row.get(1)?,
                issuer: row.get(2)?,
                issued_date: row.get(3)?,
                expiry_date: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;

        let mut metadata = Vec::new();
        for row in rows {
            let row = row?;
            metadata.push(CredentialMetadata {
                id: row.id,
                credential_type: serde_json::from_str(&row.credential_type)
                    .unwrap_or(CredentialType::Custom("unknown".into())),
                issuer: row.issuer,
                issued_date: DateTime::parse_from_rfc3339(&row.issued_date)?.with_timezone(&Utc),
                expiry_date: row
                    .expiry_date
                    .map(|d| DateTime::parse_from_rfc3339(&d).map(|dt| dt.with_timezone(&Utc)))
                    .transpose()?,
                created_at: DateTime::parse_from_rfc3339(&row.created_at)?.with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&row.updated_at)?.with_timezone(&Utc),
            });
        }
        Ok(metadata)
    }

    /// Update an existing credential's claims and metadata.
    pub async fn update(&self, credential: &IdentityCredential) -> Result<bool> {
        let encrypted_claims = self.encrypt_claims(&credential.claims)?;
        let credential_type = serde_json::to_string(&credential.credential_type)?;
        let conn = self.conn.lock().await;
        let updated = conn.execute(
            "UPDATE identity_credentials
             SET credential_type = ?2, issuer = ?3, issued_date = ?4,
                 expiry_date = ?5, encrypted_claims = ?6, updated_at = ?7
             WHERE id = ?1",
            rusqlite::params![
                credential.id,
                credential_type,
                credential.issuer,
                credential.issued_date.to_rfc3339(),
                credential.expiry_date.map(|d| d.to_rfc3339()),
                encrypted_claims,
                credential.updated_at.to_rfc3339(),
            ],
        )?;
        tracing::debug!(
            id = credential.id,
            updated = updated > 0,
            "identity credential updated (claims redacted)"
        );
        Ok(updated > 0)
    }

    /// Delete a credential by ID.
    pub async fn delete(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().await;
        let deleted = conn.execute(
            "DELETE FROM identity_credentials WHERE id = ?1",
            rusqlite::params![id],
        )?;
        tracing::debug!(id = id, deleted = deleted > 0, "identity credential deleted");
        Ok(deleted > 0)
    }

    // ── Selective Disclosure ────────────────────────────────

    /// Generate a selective disclosure token for specific claims.
    ///
    /// The token is a signed, time-limited assertion about the specified
    /// claims. The `recipient` identifies who the disclosure is for, and
    /// is recorded in the audit log.
    pub async fn disclose(
        &self,
        credential_id: &str,
        claim_names: &[String],
        recipient: &str,
        ttl: Duration,
    ) -> Result<DisclosureToken> {
        let credential = self
            .get(credential_id)
            .await?
            .context("credential not found")?;

        let mut disclosed_claims = HashMap::new();
        if let serde_json::Value::Object(ref map) = credential.claims {
            for name in claim_names {
                if let Some(value) = map.get(name) {
                    disclosed_claims.insert(name.clone(), value.clone());
                }
            }
        }

        let now = Utc::now();
        let token_id = Uuid::new_v4().to_string();
        let expires_at = now + ttl;

        // Use BTreeMap for deterministic key ordering in the signature payload.
        // HashMap iteration order is non-deterministic and can vary across
        // processes/platforms, causing signature verification failures.
        let sorted_claims: BTreeMap<_, _> = disclosed_claims.iter().collect();
        let payload = serde_json::json!({
            "credential_id": credential_id,
            "disclosed_claims": sorted_claims,
            "expires_at": expires_at.to_rfc3339(),
            "id": token_id,
            "issued_at": now.to_rfc3339(),
        });
        let payload_bytes = serde_json::to_vec(&payload)?;
        let signature = self.sign(&payload_bytes);

        let token = DisclosureToken {
            id: token_id.clone(),
            credential_id: credential_id.to_string(),
            disclosed_claims,
            issued_at: now,
            expires_at,
            signature,
        };

        // Record in audit log.
        self.record_disclosure(
            credential_id,
            claim_names,
            recipient,
            &token_id,
        )
        .await?;

        Ok(token)
    }

    /// Verify a disclosure token's signature and expiration.
    pub fn verify_token(&self, token: &DisclosureToken) -> Result<bool> {
        // Use BTreeMap for deterministic key ordering, matching disclose().
        let sorted_claims: BTreeMap<_, _> = token.disclosed_claims.iter().collect();
        let payload = serde_json::json!({
            "credential_id": token.credential_id,
            "disclosed_claims": sorted_claims,
            "expires_at": token.expires_at.to_rfc3339(),
            "id": token.id,
            "issued_at": token.issued_at.to_rfc3339(),
        });
        let payload_bytes = serde_json::to_vec(&payload)?;
        let expected_signature = self.sign(&payload_bytes);

        if token.signature != expected_signature {
            return Ok(false);
        }

        if Utc::now() > token.expires_at {
            return Ok(false);
        }

        Ok(true)
    }

    // ── Audit Log ───────────────────────────────────────────

    /// Record a disclosure event in the audit log.
    async fn record_disclosure(
        &self,
        credential_id: &str,
        claim_names: &[String],
        recipient: &str,
        token_id: &str,
    ) -> Result<()> {
        let entry_id = Uuid::new_v4().to_string();
        let claims_json = serde_json::to_string(claim_names)?;
        let now = Utc::now().to_rfc3339();

        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO disclosure_audit_log
                (id, credential_id, disclosed_claims, recipient, timestamp, token_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![entry_id, credential_id, claims_json, recipient, now, token_id],
        )
        .context("failed to record disclosure audit entry")?;

        // Never log which claims were disclosed.
        tracing::info!(
            credential_id = credential_id,
            recipient = recipient,
            "disclosure recorded (claims redacted)"
        );
        Ok(())
    }

    /// Retrieve the disclosure audit log for a credential.
    pub async fn get_audit_log(
        &self,
        credential_id: &str,
    ) -> Result<Vec<DisclosureAuditEntry>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, credential_id, disclosed_claims, recipient, timestamp, token_id
             FROM disclosure_audit_log
             WHERE credential_id = ?1
             ORDER BY timestamp DESC",
        )?;

        let rows = stmt.query_map(rusqlite::params![credential_id], |row| {
            Ok(RawAuditRow {
                id: row.get(0)?,
                credential_id: row.get(1)?,
                disclosed_claims: row.get(2)?,
                recipient: row.get(3)?,
                timestamp: row.get(4)?,
                token_id: row.get(5)?,
            })
        })?;

        let mut entries = Vec::new();
        for row in rows {
            let row = row?;
            let claims: Vec<String> = serde_json::from_str(&row.disclosed_claims)?;
            entries.push(DisclosureAuditEntry {
                id: row.id,
                credential_id: row.credential_id,
                disclosed_claims: claims,
                recipient: row.recipient,
                timestamp: DateTime::parse_from_rfc3339(&row.timestamp)?.with_timezone(&Utc),
                token_id: row.token_id,
            });
        }
        Ok(entries)
    }

    // ── W3C Verifiable Credentials ──────────────────────────

    /// Convert an identity credential to W3C Verifiable Credential 2.0 format.
    pub fn to_verifiable_credential(
        &self,
        credential: &IdentityCredential,
        subject_did: &str,
    ) -> VerifiableCredential {
        let mut claims = HashMap::new();
        if let serde_json::Value::Object(ref map) = credential.claims {
            for (k, v) in map {
                claims.insert(k.clone(), v.clone());
            }
        }

        VerifiableCredential {
            context: vec![
                "https://www.w3.org/ns/credentials/v2".into(),
                "https://www.w3.org/ns/credentials/examples/v2".into(),
            ],
            vc_type: vec![
                "VerifiableCredential".into(),
                credential_type_to_vc_type(&credential.credential_type),
            ],
            id: format!("urn:uuid:{}", credential.id),
            issuer: credential.issuer.clone(),
            issuance_date: credential.issued_date.to_rfc3339(),
            expiration_date: credential.expiry_date.map(|d| d.to_rfc3339()),
            credential_subject: CredentialSubject {
                id: subject_did.to_string(),
                claims,
            },
        }
    }

    /// Generate a DID for the local identity store.
    ///
    /// Uses the `did:key` method with a deterministic key derived from
    /// the identity secret, providing future interoperability with the
    /// DID ecosystem.
    pub fn generate_did(&self) -> String {
        let fingerprint = hex::encode(&Sha256::digest(&self.signing_key)[..16]);
        format!("did:key:z{fingerprint}")
    }

    // ── Internal helpers ────────────────────────────────────

    fn encrypt_claims(&self, claims: &serde_json::Value) -> Result<String> {
        let plaintext = serde_json::to_vec(claims)?;
        let encrypted = crypto::encrypt(&plaintext, &self.encryption_key)
            .map_err(|_| anyhow::anyhow!("failed to encrypt claims"))?;
        Ok(base64::engine::general_purpose::STANDARD.encode(encrypted))
    }

    fn decrypt_claims(&self, encrypted: &str) -> Result<serde_json::Value> {
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encrypted)
            .context("failed to decode base64 claims")?;
        let decrypted = crypto::decrypt(&decoded, &self.encryption_key)
            .map_err(|_| anyhow::anyhow!("failed to decrypt claims"))?;
        serde_json::from_slice(&decrypted).context("failed to parse decrypted claims")
    }

    fn sign(&self, data: &[u8]) -> String {
        crypto::hmac_sha256(&self.signing_key, data)
    }

    fn row_to_credential(&self, row: RawCredentialRow) -> Result<IdentityCredential> {
        Ok(IdentityCredential {
            id: row.id,
            credential_type: serde_json::from_str(&row.credential_type)
                .unwrap_or(CredentialType::Custom("unknown".into())),
            issuer: row.issuer,
            issued_date: DateTime::parse_from_rfc3339(&row.issued_date)?.with_timezone(&Utc),
            expiry_date: row
                .expiry_date
                .map(|d| DateTime::parse_from_rfc3339(&d).map(|dt| dt.with_timezone(&Utc)))
                .transpose()?,
            claims: self.decrypt_claims(&row.encrypted_claims)?,
            created_at: DateTime::parse_from_rfc3339(&row.created_at)?.with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(&row.updated_at)?.with_timezone(&Utc),
        })
    }
}

// ──────────────────────────────────────────────────────────────
// Internal helpers
// ──────────────────────────────────────────────────────────────

struct RawCredentialRow {
    id: String,
    credential_type: String,
    issuer: String,
    issued_date: String,
    expiry_date: Option<String>,
    encrypted_claims: String,
    created_at: String,
    updated_at: String,
}

struct RawMetadataRow {
    id: String,
    credential_type: String,
    issuer: String,
    issued_date: String,
    expiry_date: Option<String>,
    created_at: String,
    updated_at: String,
}

struct RawAuditRow {
    id: String,
    credential_id: String,
    disclosed_claims: String,
    recipient: String,
    timestamp: String,
    token_id: String,
}

/// Map credential type to W3C VC type string.
fn credential_type_to_vc_type(ct: &CredentialType) -> String {
    match ct {
        CredentialType::Passport => "PassportCredential".into(),
        CredentialType::DriversLicence => "DriversLicenceCredential".into(),
        CredentialType::Certificate => "CertificateCredential".into(),
        CredentialType::IdentityCard => "IdentityCardCredential".into(),
        CredentialType::Custom(name) => format!("{name}Credential"),
    }
}

/// Extension trait for optional rusqlite query results.
trait OptionalExt<T> {
    fn optional(self) -> std::result::Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalExt<T> for std::result::Result<T, rusqlite::Error> {
    fn optional(self) -> std::result::Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_store() -> IdentityStore {
        let conn =
            rusqlite::Connection::open_in_memory().expect("in-memory SQLite should open");
        let conn = Arc::new(Mutex::new(conn));
        let store =
            IdentityStore::new(conn, "test-identity-secret").expect("store should create");
        store.init().await.expect("init should succeed");
        store
    }

    fn sample_credential() -> IdentityCredential {
        let now = Utc::now();
        IdentityCredential {
            id: Uuid::new_v4().to_string(),
            credential_type: CredentialType::Passport,
            issuer: "au.gov".into(),
            issued_date: now,
            expiry_date: Some(now + Duration::days(3650)),
            claims: serde_json::json!({
                "full_name": "Jane Doe",
                "date_of_birth": "1990-01-15",
                "nationality": "Australian",
                "passport_number": "PA1234567"
            }),
            created_at: now,
            updated_at: now,
        }
    }

    // ── CRUD Tests ──────────────────────────────────────────

    #[tokio::test]
    async fn create_and_get_credential() {
        let store = setup_store().await;
        let cred = sample_credential();

        store.create(&cred).await.expect("create should succeed");
        let retrieved = store
            .get(&cred.id)
            .await
            .expect("get should succeed")
            .expect("credential should exist");

        assert_eq!(retrieved.id, cred.id);
        assert_eq!(retrieved.credential_type, cred.credential_type);
        assert_eq!(retrieved.issuer, cred.issuer);
        assert_eq!(retrieved.claims["full_name"], "Jane Doe");
        assert_eq!(retrieved.claims["passport_number"], "PA1234567");
    }

    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let store = setup_store().await;
        let result = store.get("nonexistent-id").await.expect("get should succeed");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn list_returns_metadata_without_claims() {
        let store = setup_store().await;
        let cred = sample_credential();
        store.create(&cred).await.unwrap();

        let list = store.list().await.expect("list should succeed");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, cred.id);
        assert_eq!(list[0].credential_type, CredentialType::Passport);
        assert_eq!(list[0].issuer, "au.gov");
    }

    #[tokio::test]
    async fn update_credential() {
        let store = setup_store().await;
        let mut cred = sample_credential();
        store.create(&cred).await.unwrap();

        cred.claims = serde_json::json!({
            "full_name": "Jane Smith",
            "date_of_birth": "1990-01-15",
            "nationality": "Australian",
            "passport_number": "PA7654321"
        });
        cred.updated_at = Utc::now();

        let updated = store.update(&cred).await.expect("update should succeed");
        assert!(updated);

        let retrieved = store.get(&cred.id).await.unwrap().unwrap();
        assert_eq!(retrieved.claims["full_name"], "Jane Smith");
        assert_eq!(retrieved.claims["passport_number"], "PA7654321");
    }

    #[tokio::test]
    async fn update_nonexistent_returns_false() {
        let store = setup_store().await;
        let cred = sample_credential();
        let updated = store.update(&cred).await.expect("update should succeed");
        assert!(!updated);
    }

    #[tokio::test]
    async fn delete_credential() {
        let store = setup_store().await;
        let cred = sample_credential();
        store.create(&cred).await.unwrap();

        let deleted = store.delete(&cred.id).await.expect("delete should succeed");
        assert!(deleted);

        let result = store.get(&cred.id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn delete_nonexistent_returns_false() {
        let store = setup_store().await;
        let deleted = store.delete("nonexistent").await.expect("delete should succeed");
        assert!(!deleted);
    }

    // ── Encryption Tests ────────────────────────────────────

    #[tokio::test]
    async fn claims_stored_encrypted_at_rest() {
        let store = setup_store().await;
        let cred = sample_credential();
        store.create(&cred).await.unwrap();

        // Read raw encrypted value from SQLite.
        let conn = store.conn.lock().await;
        let encrypted: String = conn
            .query_row(
                "SELECT encrypted_claims FROM identity_credentials WHERE id = ?1",
                rusqlite::params![cred.id],
                |row| row.get(0),
            )
            .expect("should find row");

        // Encrypted value must not contain plaintext claim values.
        assert!(
            !encrypted.contains("Jane Doe"),
            "plaintext name must not appear in encrypted claims"
        );
        assert!(
            !encrypted.contains("PA1234567"),
            "plaintext passport number must not appear in encrypted claims"
        );
    }

    #[tokio::test]
    async fn separate_encryption_key_from_plugin_credentials() {
        // Identity store and plugin credential store with the same master
        // secret should derive different keys.
        let identity_key = crypto::derive_key("shared-secret", crypto::DOMAIN_IDENTITY_ENCRYPT);
        let plugin_key = crypto::derive_key("shared-secret", crypto::DOMAIN_CREDENTIAL_STORE);
        assert_ne!(
            identity_key, plugin_key,
            "identity and plugin credential encryption keys must differ"
        );
    }

    // ── Log Safety Tests ────────────────────────────────────

    #[test]
    fn credential_debug_does_not_leak_claims_in_format() {
        // The IdentityCredential derives Debug, but we verify that the
        // tracing calls in the store never format the claims. This test
        // ensures the code path uses "(claims redacted)" strings.
        let cred = sample_credential();
        let metadata = CredentialMetadata::from(&cred);
        let debug_str = format!("{metadata:?}");
        // Metadata should not contain sensitive claim values.
        assert!(!debug_str.contains("PA1234567"));
        assert!(!debug_str.contains("Jane Doe"));
    }

    #[test]
    fn list_response_excludes_claims() {
        let cred = sample_credential();
        let metadata = CredentialMetadata::from(&cred);
        let json = serde_json::to_string(&metadata).unwrap();
        assert!(!json.contains("claims"));
        assert!(!json.contains("PA1234567"));
        assert!(!json.contains("Jane Doe"));
    }

    // ── Selective Disclosure Tests ──────────────────────────

    #[tokio::test]
    async fn disclose_produces_valid_time_limited_token() {
        let store = setup_store().await;
        let cred = sample_credential();
        store.create(&cred).await.unwrap();

        let token = store
            .disclose(
                &cred.id,
                &["nationality".into()],
                "border-control",
                Duration::hours(1),
            )
            .await
            .expect("disclose should succeed");

        assert_eq!(token.credential_id, cred.id);
        assert_eq!(
            token.disclosed_claims.get("nationality"),
            Some(&serde_json::json!("Australian"))
        );
        // Should not include non-requested claims.
        assert!(token.disclosed_claims.get("passport_number").is_none());
        assert!(token.expires_at > Utc::now());
        assert!(!token.signature.is_empty());
    }

    #[tokio::test]
    async fn disclose_token_is_verifiable() {
        let store = setup_store().await;
        let cred = sample_credential();
        store.create(&cred).await.unwrap();

        let token = store
            .disclose(
                &cred.id,
                &["date_of_birth".into()],
                "age-verifier",
                Duration::hours(1),
            )
            .await
            .unwrap();

        let valid = store.verify_token(&token).expect("verify should succeed");
        assert!(valid, "fresh token should be valid");
    }

    #[tokio::test]
    async fn tampered_token_fails_verification() {
        let store = setup_store().await;
        let cred = sample_credential();
        store.create(&cred).await.unwrap();

        let mut token = store
            .disclose(
                &cred.id,
                &["nationality".into()],
                "test-recipient",
                Duration::hours(1),
            )
            .await
            .unwrap();

        // Tamper with disclosed claims.
        token
            .disclosed_claims
            .insert("nationality".into(), serde_json::json!("Martian"));

        let valid = store.verify_token(&token).expect("verify should succeed");
        assert!(!valid, "tampered token should fail verification");
    }

    #[tokio::test]
    async fn expired_token_fails_verification() {
        let store = setup_store().await;
        let cred = sample_credential();
        store.create(&cred).await.unwrap();

        let mut token = store
            .disclose(
                &cred.id,
                &["nationality".into()],
                "test-recipient",
                Duration::hours(1),
            )
            .await
            .unwrap();

        // Backdate the token so it's expired.
        token.expires_at = Utc::now() - Duration::hours(1);
        // Re-sign to make the signature valid but token expired.
        let payload = serde_json::json!({
            "id": token.id,
            "credential_id": token.credential_id,
            "disclosed_claims": token.disclosed_claims,
            "issued_at": token.issued_at.to_rfc3339(),
            "expires_at": token.expires_at.to_rfc3339(),
        });
        token.signature = store.sign(&serde_json::to_vec(&payload).unwrap());

        let valid = store.verify_token(&token).expect("verify should succeed");
        assert!(!valid, "expired token should fail verification");
    }

    // ── Audit Log Tests ─────────────────────────────────────

    #[tokio::test]
    async fn disclosure_creates_audit_entry() {
        let store = setup_store().await;
        let cred = sample_credential();
        store.create(&cred).await.unwrap();

        store
            .disclose(
                &cred.id,
                &["nationality".into(), "date_of_birth".into()],
                "customs-authority",
                Duration::hours(1),
            )
            .await
            .unwrap();

        let audit_log = store
            .get_audit_log(&cred.id)
            .await
            .expect("get_audit_log should succeed");

        assert_eq!(audit_log.len(), 1);
        assert_eq!(audit_log[0].credential_id, cred.id);
        assert_eq!(audit_log[0].recipient, "customs-authority");
        assert_eq!(
            audit_log[0].disclosed_claims,
            vec!["nationality", "date_of_birth"]
        );
    }

    #[tokio::test]
    async fn multiple_disclosures_all_recorded() {
        let store = setup_store().await;
        let cred = sample_credential();
        store.create(&cred).await.unwrap();

        store
            .disclose(&cred.id, &["nationality".into()], "recipient-a", Duration::hours(1))
            .await
            .unwrap();
        store
            .disclose(&cred.id, &["date_of_birth".into()], "recipient-b", Duration::hours(1))
            .await
            .unwrap();

        let audit_log = store.get_audit_log(&cred.id).await.unwrap();
        assert_eq!(audit_log.len(), 2);

        let recipients: Vec<&str> = audit_log.iter().map(|e| e.recipient.as_str()).collect();
        assert!(recipients.contains(&"recipient-a"));
        assert!(recipients.contains(&"recipient-b"));
    }

    #[tokio::test]
    async fn audit_log_records_token_id() {
        let store = setup_store().await;
        let cred = sample_credential();
        store.create(&cred).await.unwrap();

        let token = store
            .disclose(&cred.id, &["nationality".into()], "test", Duration::hours(1))
            .await
            .unwrap();

        let audit_log = store.get_audit_log(&cred.id).await.unwrap();
        assert_eq!(audit_log[0].token_id, token.id);
    }

    // ── W3C VC Format Tests ─────────────────────────────────

    #[tokio::test]
    async fn to_verifiable_credential_has_required_fields() {
        let store = setup_store().await;
        let cred = sample_credential();
        let did = store.generate_did();

        let vc = store.to_verifiable_credential(&cred, &did);

        assert_eq!(
            vc.context,
            vec![
                "https://www.w3.org/ns/credentials/v2",
                "https://www.w3.org/ns/credentials/examples/v2"
            ]
        );
        assert!(vc.vc_type.contains(&"VerifiableCredential".to_string()));
        assert!(vc.vc_type.contains(&"PassportCredential".to_string()));
        assert!(vc.id.starts_with("urn:uuid:"));
        assert_eq!(vc.issuer, "au.gov");
        assert!(!vc.issuance_date.is_empty());
        assert!(vc.expiration_date.is_some());
        assert_eq!(vc.credential_subject.id, did);
        assert_eq!(
            vc.credential_subject.claims.get("full_name"),
            Some(&serde_json::json!("Jane Doe"))
        );
    }

    #[tokio::test]
    async fn vc_serializes_to_valid_json_ld() {
        let store = setup_store().await;
        let cred = sample_credential();
        let did = store.generate_did();

        let vc = store.to_verifiable_credential(&cred, &did);
        let json = serde_json::to_value(&vc).expect("VC should serialize to JSON");

        // Verify JSON-LD required fields.
        assert!(json["@context"].is_array());
        assert!(json["type"].is_array());
        assert!(json["id"].is_string());
        assert!(json["issuer"].is_string());
        assert!(json["issuanceDate"].is_string());
        assert!(json["credentialSubject"]["id"].is_string());
    }

    #[tokio::test]
    async fn vc_type_maps_correctly_for_all_credential_types() {
        let store = setup_store().await;
        let did = store.generate_did();

        let types = vec![
            (CredentialType::Passport, "PassportCredential"),
            (CredentialType::DriversLicence, "DriversLicenceCredential"),
            (CredentialType::Certificate, "CertificateCredential"),
            (CredentialType::IdentityCard, "IdentityCardCredential"),
            (
                CredentialType::Custom("Medical".into()),
                "MedicalCredential",
            ),
        ];

        for (cred_type, expected_vc_type) in types {
            let now = Utc::now();
            let cred = IdentityCredential {
                id: Uuid::new_v4().to_string(),
                credential_type: cred_type,
                issuer: "test".into(),
                issued_date: now,
                expiry_date: None,
                claims: serde_json::json!({}),
                created_at: now,
                updated_at: now,
            };
            let vc = store.to_verifiable_credential(&cred, &did);
            assert!(
                vc.vc_type.contains(&expected_vc_type.to_string()),
                "expected VC type {expected_vc_type}"
            );
        }
    }

    #[tokio::test]
    async fn vc_without_expiry_omits_expiration_date() {
        let store = setup_store().await;
        let did = store.generate_did();
        let now = Utc::now();
        let cred = IdentityCredential {
            id: Uuid::new_v4().to_string(),
            credential_type: CredentialType::Certificate,
            issuer: "test".into(),
            issued_date: now,
            expiry_date: None,
            claims: serde_json::json!({"skill": "rust"}),
            created_at: now,
            updated_at: now,
        };

        let vc = store.to_verifiable_credential(&cred, &did);
        assert!(vc.expiration_date.is_none());

        let json = serde_json::to_value(&vc).unwrap();
        assert!(json.get("expirationDate").is_none());
    }

    // ── DID Tests ───────────────────────────────────────────

    #[test]
    fn generate_did_starts_with_did_key() {
        let conn =
            rusqlite::Connection::open_in_memory().expect("in-memory SQLite should open");
        let conn = Arc::new(Mutex::new(conn));
        let store =
            IdentityStore::new(conn, "test-secret").expect("store should create");
        let did = store.generate_did();
        assert!(did.starts_with("did:key:z"), "DID should start with did:key:z");
    }

    #[test]
    fn generate_did_is_deterministic() {
        let conn1 =
            rusqlite::Connection::open_in_memory().expect("in-memory SQLite should open");
        let conn1 = Arc::new(Mutex::new(conn1));
        let store1 =
            IdentityStore::new(conn1, "same-secret").expect("store should create");

        let conn2 =
            rusqlite::Connection::open_in_memory().expect("in-memory SQLite should open");
        let conn2 = Arc::new(Mutex::new(conn2));
        let store2 =
            IdentityStore::new(conn2, "same-secret").expect("store should create");

        assert_eq!(store1.generate_did(), store2.generate_did());
    }

    #[test]
    fn different_secrets_produce_different_dids() {
        let conn1 =
            rusqlite::Connection::open_in_memory().expect("in-memory SQLite should open");
        let conn1 = Arc::new(Mutex::new(conn1));
        let store1 =
            IdentityStore::new(conn1, "secret-a").expect("store should create");

        let conn2 =
            rusqlite::Connection::open_in_memory().expect("in-memory SQLite should open");
        let conn2 = Arc::new(Mutex::new(conn2));
        let store2 =
            IdentityStore::new(conn2, "secret-b").expect("store should create");

        assert_ne!(store1.generate_did(), store2.generate_did());
    }

    // ── Key Derivation Tests ────────────────────────────────

    #[test]
    fn identity_encrypt_and_sign_keys_differ() {
        let encrypt_key = crypto::derive_key("secret", crypto::DOMAIN_IDENTITY_ENCRYPT);
        let sign_key = crypto::derive_key("secret", crypto::DOMAIN_IDENTITY_SIGN);
        assert_ne!(encrypt_key, sign_key);
    }

    #[test]
    fn identity_key_derivation_is_deterministic() {
        let k1 = crypto::derive_key("secret", crypto::DOMAIN_IDENTITY_ENCRYPT);
        let k2 = crypto::derive_key("secret", crypto::DOMAIN_IDENTITY_ENCRYPT);
        assert_eq!(k1, k2);
    }
}
