//! Local token authentication provider.
//!
//! Implements the `AuthProvider` trait using SQLite-backed token storage
//! with SHA-256 hashed tokens and Argon2id master passphrase verification.
//! Tokens and the master passphrase hash are persisted in two tables:
//! `auth_tokens` and `auth_config`. An in-memory write-through cache keeps
//! reads fast and preserves backward compatibility with existing tests.

use crate::auth::types::{AuthError, AuthIdentity, TokenInfo, TokenRequest, TokenResponse};
use crate::auth::AuthProvider;

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use rand::Rng;
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Default token expiry in days.
const DEFAULT_EXPIRY_DAYS: u32 = 30;

/// A stored token entry (only the hash is kept, never the raw token).
#[derive(Debug, Clone)]
pub(crate) struct StoredToken {
    /// Unique token identifier.
    pub(crate) id: String,
    /// SHA-256 hash of the raw token (hex-encoded).
    pub(crate) token_hash: String,
    /// When the token was created.
    pub(crate) created_at: DateTime<Utc>,
    /// When the token expires.
    pub(crate) expires_at: DateTime<Utc>,
}

/// Shared mutable state for the local token provider.
///
/// Serves as a write-through cache backed by SQLite. On startup,
/// existing tokens and the passphrase hash are loaded from the database.
#[derive(Debug)]
pub(crate) struct TokenState {
    /// The Argon2id hash of the master passphrase. `None` if not yet set.
    pub(crate) passphrase_hash: Option<String>,
    /// All stored tokens keyed by token ID.
    pub(crate) tokens: HashMap<String, StoredToken>,
}

/// Local token authentication provider.
///
/// Generates bearer tokens validated against SHA-256 hashes. The master
/// passphrase is verified using Argon2id. On first use, the first
/// `generate_token` call sets the master passphrase.
///
/// Tokens and the passphrase hash are persisted in SQLite via a
/// write-through cache. The in-memory `state` is the authoritative
/// source for reads; writes update both the cache and the database.
#[derive(Debug, Clone)]
pub struct LocalTokenProvider {
    /// In-memory write-through cache of tokens and passphrase hash.
    pub(crate) state: Arc<RwLock<TokenState>>,
    /// SQLite connection for durable storage.
    db: Arc<tokio::sync::Mutex<Connection>>,
}

impl LocalTokenProvider {
    /// Create a new local token provider backed by an in-memory SQLite database.
    ///
    /// Suitable for testing and backward compatibility. Data is lost when the
    /// provider is dropped.
    pub fn new() -> Self {
        let conn = Connection::open_in_memory()
            .expect("failed to open in-memory SQLite for auth");
        Self::from_connection(conn)
            .expect("failed to initialise in-memory auth database")
    }

    /// Create a local token provider backed by a file-based SQLite database.
    ///
    /// Tokens and the master passphrase hash persist across restarts.
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        Self::from_connection(conn)
    }

    /// Shared constructor: configures pragmas, creates tables, and loads
    /// existing state from the database into the in-memory cache.
    fn from_connection(conn: Connection) -> anyhow::Result<Self> {
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        Self::create_tables(&conn)?;

        // Load existing passphrase hash from auth_config.
        let passphrase_hash: Option<String> = conn
            .query_row(
                "SELECT value FROM auth_config WHERE key = 'passphrase_hash'",
                [],
                |row| row.get(0),
            )
            .ok();

        // Load existing tokens from auth_tokens.
        let tokens = {
            let mut stmt = conn.prepare(
                "SELECT id, token_hash, created_at, expires_at FROM auth_tokens",
            )?;
            let mut tokens = HashMap::new();
            let rows = stmt.query_map([], |row| {
                let id: String = row.get(0)?;
                let token_hash: String = row.get(1)?;
                let created_at_str: String = row.get(2)?;
                let expires_at_str: String = row.get(3)?;
                Ok((id, token_hash, created_at_str, expires_at_str))
            })?;
            for row in rows {
                let (id, token_hash, created_at_str, expires_at_str) = row?;
                let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());
                let expires_at = DateTime::parse_from_rfc3339(&expires_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());
                tokens.insert(
                    id.clone(),
                    StoredToken {
                        id,
                        token_hash,
                        created_at,
                        expires_at,
                    },
                );
            }
            tokens
        };

        Ok(Self {
            state: Arc::new(RwLock::new(TokenState {
                passphrase_hash,
                tokens,
            })),
            db: Arc::new(tokio::sync::Mutex::new(conn)),
        })
    }

    /// Create the `auth_tokens` and `auth_config` tables if they do not exist.
    fn create_tables(conn: &Connection) -> anyhow::Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS auth_tokens (
                id          TEXT PRIMARY KEY,
                token_hash  TEXT NOT NULL,
                created_at  TEXT NOT NULL,
                expires_at  TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS auth_config (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )?;
        Ok(())
    }

    /// Hash a raw token with SHA-256 and return the hex-encoded digest.
    fn hash_token(raw: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(raw.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Generate a cryptographically random 32-byte token, hex-encoded.
    fn generate_raw_token() -> String {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill(&mut bytes);
        hex::encode(bytes)
    }

    /// Hash a passphrase using Argon2id.
    fn hash_passphrase(passphrase: &str) -> Result<String, AuthError> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        argon2
            .hash_password(passphrase.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|e| AuthError::Internal(format!("failed to hash passphrase: {e}")))
    }

    /// Verify a passphrase against an Argon2id hash.
    fn verify_passphrase(passphrase: &str, hash: &str) -> Result<(), AuthError> {
        let parsed = PasswordHash::new(hash)
            .map_err(|e| AuthError::Internal(format!("invalid stored hash: {e}")))?;
        Argon2::default()
            .verify_password(passphrase.as_bytes(), &parsed)
            .map_err(|_| AuthError::InvalidCredentials)
    }
}

impl Default for LocalTokenProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AuthProvider for LocalTokenProvider {
    async fn validate_token(&self, token: &str) -> Result<AuthIdentity, AuthError> {
        let token_hash = Self::hash_token(token);
        let state = self.state.read().await;

        let stored = state
            .tokens
            .values()
            .find(|t| t.token_hash == token_hash)
            .ok_or(AuthError::TokenNotFound)?;

        if Utc::now() >= stored.expires_at {
            return Err(AuthError::TokenExpired);
        }

        Ok(AuthIdentity {
            token_id: stored.id.clone(),
            user_id: None,
            household_id: None,
            role: None,
            created_at: stored.created_at,
            expires_at: stored.expires_at,
        })
    }

    async fn generate_token(
        &self,
        credentials: &TokenRequest,
    ) -> Result<TokenResponse, AuthError> {
        let mut state = self.state.write().await;

        // If no passphrase is set yet, this call sets it.
        match &state.passphrase_hash {
            None => {
                tracing::info!("master passphrase set for the first time");
                let hash = Self::hash_passphrase(&credentials.passphrase)?;
                // Persist the passphrase hash to SQLite.
                {
                    let db = self.db.lock().await;
                    db.execute(
                        "INSERT OR REPLACE INTO auth_config (key, value) VALUES ('passphrase_hash', ?1)",
                        params![hash],
                    )
                    .map_err(|e| AuthError::Internal(format!("failed to store passphrase hash: {e}")))?;
                }
                state.passphrase_hash = Some(hash);
            }
            Some(existing_hash) => {
                Self::verify_passphrase(&credentials.passphrase, existing_hash)?;
            }
        }

        let raw_token = Self::generate_raw_token();
        let token_hash = Self::hash_token(&raw_token);
        let now = Utc::now();
        let expiry_days = credentials.expires_in_days.unwrap_or(DEFAULT_EXPIRY_DAYS);
        let expires_at = now + Duration::days(i64::from(expiry_days));
        let token_id = uuid::Uuid::new_v4().to_string();

        let created_at_str = now.to_rfc3339();
        let expires_at_str = expires_at.to_rfc3339();

        // Persist the token to SQLite.
        {
            let db = self.db.lock().await;
            db.execute(
                "INSERT INTO auth_tokens (id, token_hash, created_at, expires_at) VALUES (?1, ?2, ?3, ?4)",
                params![token_id, token_hash, created_at_str, expires_at_str],
            )
            .map_err(|e| AuthError::Internal(format!("failed to store token: {e}")))?;
        }

        let stored = StoredToken {
            id: token_id.clone(),
            token_hash,
            created_at: now,
            expires_at,
        };
        state.tokens.insert(token_id.clone(), stored);

        tracing::info!(token_id = %token_id, "token generated");

        Ok(TokenResponse {
            token_id,
            token: raw_token,
            expires_at: expires_at_str,
        })
    }

    async fn revoke_token(&self, token_id: &str) -> Result<(), AuthError> {
        let mut state = self.state.write().await;
        if state.tokens.remove(token_id).is_none() {
            return Err(AuthError::TokenNotFound);
        }

        // Remove from SQLite.
        {
            let db = self.db.lock().await;
            db.execute(
                "DELETE FROM auth_tokens WHERE id = ?1",
                params![token_id],
            )
            .map_err(|e| AuthError::Internal(format!("failed to delete token: {e}")))?;
        }

        tracing::info!(token_id = %token_id, "token revoked");
        Ok(())
    }

    async fn list_tokens(&self) -> Result<Vec<TokenInfo>, AuthError> {
        let state = self.state.read().await;
        let now = Utc::now();
        let infos = state
            .tokens
            .values()
            .map(|t| TokenInfo {
                id: t.id.clone(),
                created_at: t.created_at.to_rfc3339(),
                expires_at: t.expires_at.to_rfc3339(),
                is_expired: now >= t.expires_at,
            })
            .collect();
        Ok(infos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn generate_token_with_valid_passphrase() {
        let provider = LocalTokenProvider::new();
        let req = TokenRequest {
            passphrase: "my-secret".into(),
            expires_in_days: Some(7),
        };
        let resp = provider.generate_token(&req).await.unwrap();
        assert!(!resp.token.is_empty());
        assert!(!resp.token_id.is_empty());
        assert!(!resp.expires_at.is_empty());
    }

    #[tokio::test]
    async fn first_call_sets_passphrase() {
        let provider = LocalTokenProvider::new();

        // First call sets the passphrase.
        let req = TokenRequest {
            passphrase: "initial".into(),
            expires_in_days: None,
        };
        provider.generate_token(&req).await.unwrap();

        // Second call with same passphrase succeeds.
        let req2 = TokenRequest {
            passphrase: "initial".into(),
            expires_in_days: None,
        };
        provider.generate_token(&req2).await.unwrap();

        // Third call with wrong passphrase fails.
        let req3 = TokenRequest {
            passphrase: "wrong".into(),
            expires_in_days: None,
        };
        let err = provider.generate_token(&req3).await.unwrap_err();
        assert!(matches!(err, AuthError::InvalidCredentials));
    }

    #[tokio::test]
    async fn validate_valid_token() {
        let provider = LocalTokenProvider::new();
        let req = TokenRequest {
            passphrase: "pass".into(),
            expires_in_days: Some(30),
        };
        let resp = provider.generate_token(&req).await.unwrap();

        let identity = provider.validate_token(&resp.token).await.unwrap();
        assert_eq!(identity.token_id, resp.token_id);
    }

    #[tokio::test]
    async fn validate_invalid_token_returns_not_found() {
        let provider = LocalTokenProvider::new();
        let err = provider
            .validate_token("nonexistent-token")
            .await
            .unwrap_err();
        assert!(matches!(err, AuthError::TokenNotFound));
    }

    #[tokio::test]
    async fn validate_expired_token() {
        let provider = LocalTokenProvider::new();

        // Generate a token, then manually expire it.
        let req = TokenRequest {
            passphrase: "pass".into(),
            expires_in_days: Some(1),
        };
        let resp = provider.generate_token(&req).await.unwrap();

        // Manually set expiry to the past.
        {
            let mut state = provider.state.write().await;
            let stored = state.tokens.get_mut(&resp.token_id).unwrap();
            stored.expires_at = Utc::now() - Duration::hours(1);
        }

        let err = provider.validate_token(&resp.token).await.unwrap_err();
        assert!(matches!(err, AuthError::TokenExpired));
    }

    #[tokio::test]
    async fn revoke_token_succeeds() {
        let provider = LocalTokenProvider::new();
        let req = TokenRequest {
            passphrase: "pass".into(),
            expires_in_days: None,
        };
        let resp = provider.generate_token(&req).await.unwrap();

        provider.revoke_token(&resp.token_id).await.unwrap();

        // Token should no longer validate.
        let err = provider.validate_token(&resp.token).await.unwrap_err();
        assert!(matches!(err, AuthError::TokenNotFound));
    }

    #[tokio::test]
    async fn revoke_nonexistent_token_returns_not_found() {
        let provider = LocalTokenProvider::new();
        let err = provider.revoke_token("no-such-id").await.unwrap_err();
        assert!(matches!(err, AuthError::TokenNotFound));
    }

    #[tokio::test]
    async fn list_tokens_returns_all() {
        let provider = LocalTokenProvider::new();

        // Generate two tokens.
        let req = TokenRequest {
            passphrase: "pass".into(),
            expires_in_days: Some(30),
        };
        provider.generate_token(&req).await.unwrap();

        let req2 = TokenRequest {
            passphrase: "pass".into(),
            expires_in_days: Some(7),
        };
        provider.generate_token(&req2).await.unwrap();

        let tokens = provider.list_tokens().await.unwrap();
        assert_eq!(tokens.len(), 2);
        for info in &tokens {
            assert!(!info.is_expired);
        }
    }

    #[tokio::test]
    async fn list_tokens_shows_expired() {
        let provider = LocalTokenProvider::new();
        let req = TokenRequest {
            passphrase: "pass".into(),
            expires_in_days: Some(1),
        };
        let resp = provider.generate_token(&req).await.unwrap();

        // Manually expire the token.
        {
            let mut state = provider.state.write().await;
            let stored = state.tokens.get_mut(&resp.token_id).unwrap();
            stored.expires_at = Utc::now() - Duration::hours(1);
        }

        let tokens = provider.list_tokens().await.unwrap();
        assert_eq!(tokens.len(), 1);
        assert!(tokens[0].is_expired);
    }

    #[test]
    fn hash_token_is_deterministic() {
        let hash1 = LocalTokenProvider::hash_token("test");
        let hash2 = LocalTokenProvider::hash_token("test");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn different_tokens_produce_different_hashes() {
        let hash1 = LocalTokenProvider::hash_token("token-a");
        let hash2 = LocalTokenProvider::hash_token("token-b");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn generate_raw_token_is_64_hex_chars() {
        let token = LocalTokenProvider::generate_raw_token();
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn generate_raw_tokens_are_unique() {
        let t1 = LocalTokenProvider::generate_raw_token();
        let t2 = LocalTokenProvider::generate_raw_token();
        assert_ne!(t1, t2);
    }

    #[test]
    fn default_provider() {
        let _provider = LocalTokenProvider::default();
    }

    #[tokio::test]
    async fn default_expiry_is_30_days() {
        let provider = LocalTokenProvider::new();
        let req = TokenRequest {
            passphrase: "pass".into(),
            expires_in_days: None,
        };
        let resp = provider.generate_token(&req).await.unwrap();
        let expires: DateTime<Utc> = resp.expires_at.parse().unwrap();
        let days_until_expiry = (expires - Utc::now()).num_days();
        // Should be approximately 30 days (allow 1 day tolerance).
        assert!(days_until_expiry >= 29 && days_until_expiry <= 30);
    }

    #[tokio::test]
    async fn token_persists_across_provider_instances() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("auth.db");

        // Create first provider, generate a token.
        let provider1 = LocalTokenProvider::open(&db_path).unwrap();
        let req = TokenRequest {
            passphrase: "persist-test".into(),
            expires_in_days: Some(30),
        };
        let resp = provider1.generate_token(&req).await.unwrap();
        let raw_token = resp.token.clone();
        let token_id = resp.token_id.clone();

        // Drop the first provider.
        drop(provider1);

        // Create a second provider on the same file.
        let provider2 = LocalTokenProvider::open(&db_path).unwrap();

        // The token should still be valid.
        let identity = provider2.validate_token(&raw_token).await.unwrap();
        assert_eq!(identity.token_id, token_id);
    }

    #[tokio::test]
    async fn passphrase_persists_across_provider_instances() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("auth.db");

        // Create first provider and set the passphrase.
        let provider1 = LocalTokenProvider::open(&db_path).unwrap();
        let req = TokenRequest {
            passphrase: "my-secret".into(),
            expires_in_days: Some(7),
        };
        provider1.generate_token(&req).await.unwrap();
        drop(provider1);

        // Create a second provider on the same file.
        let provider2 = LocalTokenProvider::open(&db_path).unwrap();

        // Wrong passphrase should fail.
        let bad_req = TokenRequest {
            passphrase: "wrong-secret".into(),
            expires_in_days: Some(7),
        };
        let err = provider2.generate_token(&bad_req).await.unwrap_err();
        assert!(matches!(err, AuthError::InvalidCredentials));

        // Correct passphrase should succeed.
        let good_req = TokenRequest {
            passphrase: "my-secret".into(),
            expires_in_days: Some(7),
        };
        provider2.generate_token(&good_req).await.unwrap();
    }

    #[tokio::test]
    async fn open_file_backed_database() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("auth-open-test.db");

        let provider = LocalTokenProvider::open(&db_path).unwrap();

        // Should work like a normal provider.
        let req = TokenRequest {
            passphrase: "file-backed".into(),
            expires_in_days: Some(14),
        };
        let resp = provider.generate_token(&req).await.unwrap();
        assert!(!resp.token.is_empty());

        let identity = provider.validate_token(&resp.token).await.unwrap();
        assert_eq!(identity.token_id, resp.token_id);

        let tokens = provider.list_tokens().await.unwrap();
        assert_eq!(tokens.len(), 1);

        // The database file should exist on disk.
        assert!(db_path.exists());
    }
}
