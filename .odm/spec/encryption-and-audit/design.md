<!--
domain: encryption-and-audit
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Encryption and Audit — Design

## Purpose

This document describes the technical design for Core's encryption-at-rest and audit logging subsystems. Database encryption uses SQLCipher with Argon2id key derivation. Credential storage adds per-record encryption via `packages/crypto`. Audit events flow through the event bus and persist to an `audit_log` table with 90-day retention.

## Crypto Crate — `packages/crypto`

All encryption primitives live in `packages/crypto`. No other crate imports raw cryptographic libraries directly.

### Public API

```rust
// packages/crypto/src/lib.rs

/// Derive a 32-byte key from a passphrase and salt using Argon2id.
/// Default params: 64 MB memory, 3 iterations, 4 parallelism.
pub fn derive_key(passphrase: &str, salt: &[u8], params: Option<Argon2Params>) -> Result<[u8; 32]>;

/// Encrypt plaintext using AES-256-GCM. Returns nonce (12 bytes) || ciphertext || tag.
pub fn encrypt(plaintext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>>;

/// Decrypt ciphertext produced by `encrypt`. Extracts nonce, decrypts, verifies tag.
pub fn decrypt(ciphertext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>>;

/// Compute HMAC-SHA256 tag over data.
pub fn hmac_sign(data: &[u8], key: &[u8; 32]) -> [u8; 32];

/// Verify HMAC-SHA256 tag. Constant-time comparison.
pub fn hmac_verify(data: &[u8], key: &[u8; 32], tag: &[u8; 32]) -> bool;
```

### Argon2id Parameters

```rust
pub struct Argon2Params {
    pub memory_kib: u32,    // default: 65536 (64 MB)
    pub iterations: u32,    // default: 3
    pub parallelism: u32,   // default: 4
    pub output_len: usize,  // default: 32
}
```

The default parameters are suitable for most devices. For low-resource environments, parameters are overridable via `config.toml`:

```toml
[crypto]
argon2_memory_kib = 32768
argon2_iterations = 2
argon2_parallelism = 2
```

### AES-256-GCM Wire Format

The `encrypt` function produces a single byte vector:

- Bytes 0..12 — Random nonce (96 bits)
- Bytes 12..N-16 — Ciphertext
- Bytes N-16..N — Authentication tag (128 bits)

The `decrypt` function reverses this layout. A corrupted or tampered ciphertext produces an authentication error.

## SQLCipher Database Encryption

### Startup Sequence

```rust
// packages/storage-sqlite/src/lib.rs

pub async fn open_database(path: &Path, passphrase: &str) -> Result<SqlitePool> {
    let salt = read_or_create_salt(path)?;
    let key = crypto::derive_key(passphrase, &salt, None)?;
    let hex_key = hex::encode(&key);

    let pool = SqlitePool::connect(&format!("sqlite:{}", path.display())).await?;

    // Apply derived key via SQLCipher PRAGMA
    sqlx::query(&format!("PRAGMA key = \"x'{}'\";", hex_key))
        .execute(&pool)
        .await?;

    // Enable WAL for concurrent reads during writes
    sqlx::query("PRAGMA journal_mode = WAL;")
        .execute(&pool)
        .await?;

    // Verify the key works by reading the schema
    sqlx::query("SELECT count(*) FROM sqlite_master;")
        .execute(&pool)
        .await
        .map_err(|_| Error::AuthenticationFailed)?;

    Ok(pool)
}
```

The salt is stored alongside the database file as `<db_name>.salt`. It is not secret — its purpose is to prevent precomputed key attacks.

### First Launch

On first launch, when no database file exists:

1. Prompt the user for a master passphrase
2. Generate a random 16-byte salt, write to `<db_name>.salt`
3. Derive the encryption key via Argon2id
4. Create the SQLCipher database with the derived key
5. Run DDL to create `plugin_data`, `audit_log`, and other core tables

## Credential Storage

### Encryption Layer

Credentials receive per-record encryption on top of SQLCipher's full-database encryption:

```rust
// Storing a credential
let credential_key = crypto::derive_key(passphrase, &credential_salt, None)?;
let encrypted = crypto::encrypt(credential_plaintext.as_bytes(), &credential_key)?;

storage
    .insert("credentials", &PipelineMessage {
        data: json!({
            "service": "github",
            "type": "oauth_refresh_token",
            "encrypted_value": base64::encode(&encrypted),
            "salt": base64::encode(&credential_salt),
        }),
        ..Default::default()
    })
    .execute()
    .await?;
```

### OAuth Token Lifecycle

- **Refresh tokens** — Encrypted at rest in the `credentials` collection using per-record encryption. Each token has its own salt.
- **Access tokens** — Held in an in-memory cache (`HashMap<String, AccessToken>`) keyed by service name. Never written to disk, database, or logs.
- **Rotation** — A background task checks token expiry and rotates access tokens using the stored refresh token before they expire. When the provider issues a new refresh token during rotation, the old one is replaced.

```rust
pub struct TokenCache {
    /// In-memory only — never persisted
    access_tokens: HashMap<String, AccessToken>,
}

pub struct AccessToken {
    pub token: String,
    pub expires_at: DateTime<Utc>,
}
```

## Audit Logging

### Event Bus Integration

Audit events are emitted via the event bus, not a separate logging system. `StorageContext` emits events after successful write operations:

```rust
// After a successful insert
event_bus.emit(Event {
    event_type: "system.storage.created".into(),
    payload: json!({
        "collection": collection,
        "record_id": id,
        "plugin_id": plugin_id,
    }),
    timestamp: Utc::now(),
});
```

An audit log subscriber listens on the event bus and persists events to the `audit_log` table:

```rust
pub struct AuditLogSubscriber {
    storage: Arc<dyn StorageBackend>,
}

impl EventSubscriber for AuditLogSubscriber {
    fn handles(&self) -> Vec<&str> {
        vec![
            "system.storage.created",
            "system.storage.updated",
            "system.storage.deleted",
            "system.blob.stored",
            "system.blob.deleted",
            "system.auth.attempt",
            "system.credential.accessed",
            "system.credential.rotated",
            "system.credential.revoked",
            "system.plugin.installed",
            "system.plugin.enabled",
            "system.plugin.disabled",
            "system.permission.granted",
            "system.permission.revoked",
            "system.connector.authorised",
            "system.connector.revoked",
        ]
    }

    async fn handle(&self, event: &Event) -> Result<()> {
        self.storage.mutate(StorageMutation::Insert {
            collection: "audit_log".into(),
            data: json!({
                "event_type": event.event_type,
                "timestamp": event.timestamp.to_rfc3339(),
                "details": event.payload,
            }),
        }).await
    }
}
```

### Audit Log Table

```sql
CREATE TABLE IF NOT EXISTS audit_log (
    id          TEXT PRIMARY KEY,
    event_type  TEXT NOT NULL,
    plugin_id   TEXT,
    details     TEXT NOT NULL,  -- JSON
    created_at  TEXT NOT NULL   -- RFC 3339
);

CREATE INDEX IF NOT EXISTS idx_audit_log_timestamp
    ON audit_log(created_at);

CREATE INDEX IF NOT EXISTS idx_audit_log_event_type
    ON audit_log(event_type);
```

### Storage Write Events

The five storage write event types emitted by `StorageContext`:

- `system.storage.created` — Emitted after a successful `insert()`. Payload includes `collection`, `record_id`, `plugin_id`.
- `system.storage.updated` — Emitted after a successful `update()`. Payload includes `collection`, `record_id`, `plugin_id`.
- `system.storage.deleted` — Emitted after a successful `delete()`. Payload includes `collection`, `record_id`, `plugin_id`.
- `system.blob.stored` — Emitted after a blob is written to blob storage. Payload includes `blob_key`, `plugin_id`.
- `system.blob.deleted` — Emitted after a blob is removed from blob storage. Payload includes `blob_key`, `plugin_id`.

Read operations do not emit audit events.

### Security Events

Additional event types logged for security-relevant actions:

- `system.auth.attempt` — Emitted on every authentication attempt (success or failure). Details include outcome and method.
- `system.credential.accessed` — Emitted when a credential is decrypted and returned to a plugin. Details include service name and plugin id.
- `system.credential.rotated` — Emitted when a token is rotated. Details include service name.
- `system.credential.revoked` — Emitted when a credential is revoked. Details include service name and plugin id.
- `system.plugin.installed` — Emitted when a plugin is installed.
- `system.plugin.enabled` — Emitted when a plugin is enabled.
- `system.plugin.disabled` — Emitted when a plugin is disabled.
- `system.permission.granted` — Emitted when a capability is granted to a plugin.
- `system.permission.revoked` — Emitted when a capability is revoked from a plugin.
- `system.connector.authorised` — Emitted when a connector completes authorisation.
- `system.connector.revoked` — Emitted when a connector's authorisation is revoked.

### Retention Policy

Audit logs are retained for 90 days (configurable via `audit_retention_days` in `config.toml`). A daily cleanup task deletes expired entries:

```rust
pub async fn cleanup_audit_log(storage: &dyn StorageBackend, retention_days: u32) -> Result<u64> {
    let cutoff = Utc::now() - Duration::days(retention_days as i64);
    let cutoff_str = cutoff.to_rfc3339();

    storage.mutate(StorageMutation::DeleteWhere {
        collection: "audit_log".into(),
        condition: json!({ "created_at": { "$lt": cutoff_str } }),
    }).await
}
```

Key properties of the retention policy:

- Default retention is 90 days, configurable via `config.toml`
- Cleanup runs once daily as a scheduled background task
- No telemetry, external reporting, or data export from audit logs
- Audit entries are encrypted at rest within the SQLCipher database

## Conventions

- All encryption operations go through `packages/crypto` — no direct use of `aes-gcm`, `argon2`, or similar crates elsewhere
- Credential plaintext is never written to logs, error messages, or debug output
- Access tokens exist only in memory; refresh tokens are always encrypted before persistence
- Audit events use the `system.*` namespace to distinguish them from plugin events
- The `audit_log` table is internal to Core; plugins cannot query or modify it directly
