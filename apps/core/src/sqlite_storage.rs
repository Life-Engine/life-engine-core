//! SQLite implementation of the [`StorageAdapter`] trait.
//!
//! Uses `rusqlite` (bundled SQLite) with WAL mode for concurrent readers.
//! A broadcast channel publishes [`ChangeEvent`]s on every mutation.
//!
//! Canonical schema definitions: `docs/schemas/plugin-data.schema.json` and `docs/schemas/audit-log.schema.json`.
//!
//! # SQLCipher
//!
//! When opened with [`SqliteStorage::open_encrypted`], the database is
//! encrypted via SQLCipher. The passphrase is stretched to a 256-bit
//! key using Argon2id (see [`crate::rekey::derive_key_for_db`]) and applied
//! with `PRAGMA key` immediately after connection open.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{broadcast, Mutex};

use crate::storage::{
    ComparisonOp, Pagination, QueryFilters, QueryResult, Record, SortDirection, SortOptions,
    StorageAdapter,
};

// ──────────────────────────────────────────────────────────────
// Change events
// ──────────────────────────────────────────────────────────────

/// The kind of mutation that occurred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
    /// A new record was created.
    Created,
    /// An existing record was updated.
    Updated,
    /// A record was deleted.
    Deleted,
}

/// Notification emitted when a record is created, updated, or deleted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeEvent {
    /// The collection that changed.
    pub collection: String,
    /// The affected record's ID.
    pub record_id: String,
    /// What happened.
    pub change_type: ChangeType,
}

// ──────────────────────────────────────────────────────────────
// SqliteStorage
// ──────────────────────────────────────────────────────────────

/// Broadcast channel capacity for change events.
const CHANGE_CHANNEL_CAPACITY: usize = 256;

/// SQLite-backed storage adapter.
///
/// # Concurrency (F-046)
///
/// A single `Mutex<Connection>` serialises all database access.  An `RwLock`
/// cannot be used because `rusqlite::Connection` is `Send` but *not* `Sync`,
/// so concurrent readers sharing `&Connection` would not compile.
///
/// If this becomes a bottleneck under concurrent load, consider:
///
/// - **Connection pooling** via `r2d2_sqlite` (multiple connections, each
///   behind its own lock, with WAL mode enabling true concurrent reads).
/// - **Sharding** hot read paths into a separate read-only `Connection`.
pub struct SqliteStorage {
    conn: Arc<Mutex<Connection>>,
    change_tx: broadcast::Sender<ChangeEvent>,
}

impl SqliteStorage {
    /// Open (or create) a database at the given path.
    #[allow(dead_code)]
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        Self::from_connection(conn)
    }

    /// Open an in-memory database (useful for testing).
    pub fn open_in_memory() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::from_connection(conn)
    }

    /// Open (or create) an encrypted database at the given path.
    ///
    /// Derives a 256-bit key from the passphrase using Argon2id, then
    /// applies it via `PRAGMA key` immediately after opening. The key
    /// must be set before any other database operation.
    ///
    /// # Errors
    ///
    /// Returns an error if key derivation fails or the database cannot
    /// be opened with the derived key (e.g. wrong passphrase).
    #[allow(dead_code)]
    pub fn open_encrypted(
        path: &Path,
        passphrase: &str,
        argon2_settings: &crate::config::Argon2Settings,
    ) -> anyhow::Result<Self> {
        let hex_key = crate::rekey::derive_key_for_db(passphrase, path, argon2_settings)?;

        // F-045: Validate that the derived key contains only hex characters
        // before interpolating into the PRAGMA statement. This prevents any
        // unexpected characters from breaking or hijacking the PRAGMA.
        if hex_key.is_empty()
            || !hex_key
                .chars()
                .all(|c| c.is_ascii_hexdigit())
        {
            anyhow::bail!("derived key contains non-hex characters");
        }

        let conn = Connection::open(path)?;

        // PRAGMA key must be the very first statement after open.
        let pragma_key = format!("PRAGMA key = \"x'{hex_key}'\";");
        conn.execute_batch(&pragma_key)?;

        // Verify the key is correct by reading from the database.
        conn.execute_batch("SELECT count(*) FROM sqlite_master;")
            .map_err(|e| anyhow::anyhow!(
                "database is not readable (wrong passphrase?): {e}"
            ))?;

        Self::from_connection(conn)
    }

    /// Open (or create) an encrypted database using a pre-derived 32-byte key.
    ///
    /// Unlike [`open_encrypted`] which takes a passphrase and derives the key
    /// internally, this method accepts a key already derived via
    /// `life_engine_crypto::derive_key()`. This is the preferred path for
    /// Core startup where key derivation happens as a separate step.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened or the key is wrong.
    pub fn open_with_key(path: &Path, key: &[u8; 32]) -> anyhow::Result<Self> {
        let hex_key = hex::encode(key);

        let conn = Connection::open(path)?;

        // PRAGMA key must be the very first statement after open.
        let pragma_key = format!("PRAGMA key = \"x'{hex_key}'\";");
        conn.execute_batch(&pragma_key)?;

        // Verify the key is correct by reading the database header.
        conn.execute_batch("SELECT count(*) FROM sqlite_master;")
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("not a database") || msg.contains("file is encrypted") {
                    anyhow::anyhow!(
                        "unable to decrypt database — check passphrase. \
                         If the passphrase changed, the database cannot be opened \
                         with the new key."
                    )
                } else {
                    anyhow::anyhow!("database verification failed: {e}")
                }
            })?;

        Self::from_connection(conn)
    }

    /// Subscribe to change notifications.  Returns a broadcast receiver.
    #[allow(dead_code)]
    pub fn subscribe(&self) -> broadcast::Receiver<ChangeEvent> {
        self.change_tx.subscribe()
    }

    /// Get a lock on the underlying SQLite connection.
    ///
    /// Used by the migration module to run raw queries for
    /// discovering collections.
    pub async fn connection(&self) -> tokio::sync::MutexGuard<'_, Connection> {
        self.conn.lock().await
    }

    /// Run a closure with a reference to the database connection.
    pub async fn with_conn<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&Connection) -> T,
    {
        let conn = self.conn.lock().await;
        f(&conn)
    }

    // ── private ──────────────────────────────────────────────

    fn from_connection(conn: Connection) -> anyhow::Result<Self> {
        // Enable WAL mode for concurrent readers.
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;

        Self::create_tables(&conn)?;
        AuditLogger::cleanup_old_entries(&conn)?;

        let (change_tx, _) = broadcast::channel(CHANGE_CHANNEL_CAPACITY);

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            change_tx,
        })
    }

    fn create_tables(conn: &Connection) -> anyhow::Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS plugin_data (
                id             TEXT PRIMARY KEY,
                plugin_id      TEXT NOT NULL,
                collection     TEXT NOT NULL,
                data           TEXT NOT NULL,
                version        INTEGER NOT NULL DEFAULT 1,
                user_id        TEXT,
                household_id   TEXT,
                created_at     TEXT NOT NULL,
                updated_at     TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_plugin_collection
                ON plugin_data(plugin_id, collection);

            CREATE TABLE IF NOT EXISTS audit_log (
                id               TEXT PRIMARY KEY,
                timestamp        TEXT NOT NULL,
                event_type       TEXT NOT NULL,
                collection       TEXT,
                document_id      TEXT,
                identity_subject TEXT,
                plugin_id        TEXT,
                details          TEXT,
                created_at       TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_audit_timestamp
                ON audit_log(timestamp);

            CREATE INDEX IF NOT EXISTS idx_audit_event_type
                ON audit_log(event_type);

            CREATE TABLE IF NOT EXISTS schema_versions (
                plugin_id   TEXT NOT NULL,
                collection  TEXT NOT NULL,
                version     INTEGER NOT NULL DEFAULT 1,
                updated_at  TEXT NOT NULL,
                PRIMARY KEY (plugin_id, collection)
            );

            CREATE TABLE IF NOT EXISTS federation_peers (
                id               TEXT PRIMARY KEY,
                name             TEXT NOT NULL,
                endpoint         TEXT NOT NULL,
                collections      TEXT NOT NULL DEFAULT '[]',
                ca_cert_path     TEXT,
                client_cert_path TEXT,
                client_key_path  TEXT,
                status           TEXT NOT NULL DEFAULT 'pending',
                last_sync_at     TEXT,
                last_sync_records INTEGER,
                created_at       TEXT NOT NULL,
                updated_at       TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS federation_sync_cursors (
                peer_id    TEXT NOT NULL,
                collection TEXT NOT NULL,
                cursor     TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (peer_id, collection),
                FOREIGN KEY (peer_id) REFERENCES federation_peers(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS households (
                id                  TEXT PRIMARY KEY,
                name                TEXT NOT NULL,
                shared_collections  TEXT NOT NULL DEFAULT '[]',
                created_at          TEXT NOT NULL,
                updated_at          TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS household_members (
                household_id TEXT NOT NULL,
                user_id      TEXT NOT NULL,
                display_name TEXT NOT NULL,
                email        TEXT,
                role         TEXT NOT NULL DEFAULT 'member',
                joined_at    TEXT NOT NULL,
                PRIMARY KEY (household_id, user_id),
                FOREIGN KEY (household_id) REFERENCES households(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_household_members_user
                ON household_members(user_id);

            CREATE TABLE IF NOT EXISTS household_invites (
                id           TEXT PRIMARY KEY,
                household_id TEXT NOT NULL,
                email        TEXT NOT NULL,
                role         TEXT NOT NULL DEFAULT 'member',
                invited_by   TEXT NOT NULL,
                created_at   TEXT NOT NULL,
                expires_at   TEXT NOT NULL,
                accepted     INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (household_id) REFERENCES households(id) ON DELETE CASCADE
            );",
        )?;
        Ok(())
    }

    /// Publish a change event (best-effort; ignores the case of no receivers).
    fn publish(&self, event: ChangeEvent) {
        let _ = self.change_tx.send(event);
    }
}

// ──────────────────────────────────────────────────────────────
// StorageAdapter implementation
// ──────────────────────────────────────────────────────────────

#[async_trait]
impl StorageAdapter for SqliteStorage {
    async fn get(
        &self,
        plugin_id: &str,
        collection: &str,
        id: &str,
    ) -> anyhow::Result<Option<Record>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, plugin_id, collection, data, version, user_id, household_id, created_at, updated_at
             FROM plugin_data
             WHERE id = ?1 AND plugin_id = ?2 AND collection = ?3",
        )?;

        let result = stmt
            .query_row(params![id, plugin_id, collection], |row| {
                Ok(row_to_record(row))
            })
            .optional()?;

        match result {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    async fn create(
        &self,
        plugin_id: &str,
        collection: &str,
        data: Value,
    ) -> anyhow::Result<Record> {
        let id = uuid::Uuid::new_v4().to_string();
        self.create_with_id(plugin_id, collection, &id, data).await
    }

    async fn create_with_id(
        &self,
        plugin_id: &str,
        collection: &str,
        id: &str,
        data: Value,
    ) -> anyhow::Result<Record> {
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let data_str = serde_json::to_string(&data)?;

        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO plugin_data (id, plugin_id, collection, data, version, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6)",
            params![id, plugin_id, collection, data_str, now_str, now_str],
        )?;

        AuditLogger::log_event_full(
            &conn,
            "system.storage.created",
            Some(collection),
            Some(id),
            None,
            Some(plugin_id),
            None,
        )?;

        // Release the lock before publishing to avoid holding it while
        // broadcast receivers run.
        drop(conn);

        self.publish(ChangeEvent {
            collection: collection.into(),
            record_id: id.to_string(),
            change_type: ChangeType::Created,
        });

        Ok(Record {
            id: id.to_string(),
            plugin_id: plugin_id.into(),
            collection: collection.into(),
            data,
            version: 1,
            user_id: None,
            household_id: None,
            created_at: now,
            updated_at: now,
        })
    }

    async fn update(
        &self,
        plugin_id: &str,
        collection: &str,
        id: &str,
        data: Value,
        version: i64,
    ) -> Result<Record, crate::storage::StorageError> {
        use crate::storage::StorageError;

        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let data_str = serde_json::to_string(&data)
            .map_err(|e| StorageError::Other(e.into()))?;
        let new_version = version + 1;

        let conn = self.conn.lock().await;
        let rows = conn.execute(
            "UPDATE plugin_data
             SET data = ?1, version = ?2, updated_at = ?3
             WHERE id = ?4 AND plugin_id = ?5 AND collection = ?6 AND version = ?7",
            params![data_str, new_version, now_str, id, plugin_id, collection, version],
        ).map_err(|e| StorageError::Other(e.into()))?;

        if rows == 0 {
            // Distinguish between "not found" and "version mismatch" by
            // checking whether the record exists at all.
            let exists: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM plugin_data WHERE id = ?1 AND plugin_id = ?2 AND collection = ?3)",
                    params![id, plugin_id, collection],
                    |row| row.get(0),
                )
                .map_err(|e| StorageError::Other(e.into()))?;
            return if exists {
                Err(StorageError::VersionMismatch)
            } else {
                Err(StorageError::NotFound)
            };
        }

        // Fetch created_at from the existing row.
        let created_at: String = conn.query_row(
            "SELECT created_at FROM plugin_data WHERE id = ?1",
            params![id],
            |row| row.get(0),
        ).map_err(|e| StorageError::Other(e.into()))?;

        AuditLogger::log_event_full(
            &conn,
            "system.storage.updated",
            Some(collection),
            Some(id),
            None,
            Some(plugin_id),
            Some(&serde_json::json!({
                "new_version": new_version,
            })),
        ).map_err(StorageError::Other)?;

        drop(conn);

        self.publish(ChangeEvent {
            collection: collection.into(),
            record_id: id.into(),
            change_type: ChangeType::Updated,
        });

        Ok(Record {
            id: id.into(),
            plugin_id: plugin_id.into(),
            collection: collection.into(),
            data,
            version: new_version,
            user_id: None,
            household_id: None,
            created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
                .map_err(|e| StorageError::Other(e.into()))?
                .with_timezone(&Utc),
            updated_at: now,
        })
    }

    async fn query(
        &self,
        plugin_id: &str,
        collection: &str,
        filters: QueryFilters,
        sort: Option<SortOptions>,
        pagination: Pagination,
    ) -> anyhow::Result<QueryResult> {
        let pagination = pagination.clamped();

        let conn = self.conn.lock().await;

        // Build WHERE clause.
        let mut where_parts: Vec<String> = vec![
            "plugin_id = ?".into(),
            "collection = ?".into(),
        ];
        let mut bind_values: Vec<Value> =
            vec![Value::String(plugin_id.into()), Value::String(collection.into())];

        build_filter_sql(&filters, &mut where_parts, &mut bind_values);

        let where_clause = where_parts.join(" AND ");

        // Count total before pagination.
        let count_sql = format!("SELECT COUNT(*) FROM plugin_data WHERE {where_clause}");
        let total = execute_count(&conn, &count_sql, &bind_values)?;

        // Build data query.
        let order_clause = match &sort {
            Some(opts) => {
                let dir = match opts.sort_dir {
                    SortDirection::Asc => "ASC",
                    SortDirection::Desc => "DESC",
                };
                if !opts.sort_by.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    anyhow::bail!("invalid sort_by field name: {}", opts.sort_by);
                }
                format!("ORDER BY json_extract(data, '$.{}') {dir}", opts.sort_by)
            }
            None => String::new(),
        };

        let data_sql = format!(
            "SELECT id, plugin_id, collection, data, version, user_id, household_id, created_at, updated_at
             FROM plugin_data
             WHERE {where_clause}
             {order_clause}
             LIMIT ? OFFSET ?"
        );

        // Append limit/offset to bind values.
        let mut full_binds = bind_values;
        full_binds.push(Value::Number(pagination.limit.into()));
        full_binds.push(Value::Number(pagination.offset.into()));

        let records = execute_query(&conn, &data_sql, &full_binds)?;

        Ok(QueryResult {
            records,
            total,
            limit: pagination.limit,
            offset: pagination.offset,
        })
    }

    async fn delete(
        &self,
        plugin_id: &str,
        collection: &str,
        id: &str,
    ) -> anyhow::Result<bool> {
        let conn = self.conn.lock().await;
        let rows = conn.execute(
            "DELETE FROM plugin_data WHERE id = ?1 AND plugin_id = ?2 AND collection = ?3",
            params![id, plugin_id, collection],
        )?;

        if rows > 0 {
            AuditLogger::log_event_full(
                &conn,
                "system.storage.deleted",
                Some(collection),
                Some(id),
                None,
                Some(plugin_id),
                None,
            )?;

            drop(conn);

            self.publish(ChangeEvent {
                collection: collection.into(),
                record_id: id.into(),
                change_type: ChangeType::Deleted,
            });

            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn list(
        &self,
        plugin_id: &str,
        collection: &str,
        sort: Option<SortOptions>,
        pagination: Pagination,
    ) -> anyhow::Result<QueryResult> {
        self.query(plugin_id, collection, QueryFilters::default(), sort, pagination)
            .await
    }
}

// ──────────────────────────────────────────────────────────────
// SQL helpers
// ──────────────────────────────────────────────────────────────

/// Convert a rusqlite Row into a Record.
fn row_to_record(row: &rusqlite::Row<'_>) -> anyhow::Result<Record> {
    let created_str: String = row.get(7)?;
    let updated_str: String = row.get(8)?;
    let data_str: String = row.get(3)?;

    Ok(Record {
        id: row.get(0)?,
        plugin_id: row.get(1)?,
        collection: row.get(2)?,
        data: serde_json::from_str(&data_str)?,
        version: row.get(4)?,
        user_id: row.get(5)?,
        household_id: row.get(6)?,
        created_at: chrono::DateTime::parse_from_rfc3339(&created_str)?.with_timezone(&Utc),
        updated_at: chrono::DateTime::parse_from_rfc3339(&updated_str)?.with_timezone(&Utc),
    })
}

/// Trait to allow `query_row` to return `Option`.
use rusqlite::OptionalExtension;

/// Validate that a field name contains only safe characters for use in SQL.
///
/// Allows alphanumeric characters, underscores, and dots (for nested JSON paths).
fn validate_field_name(field: &str) -> Result<(), anyhow::Error> {
    if field.is_empty() || !field.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.') {
        anyhow::bail!("invalid filter field name: {}", field);
    }
    Ok(())
}

/// Escape LIKE/ILIKE metacharacters (`%`, `_`, `\`) in a search term.
fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Recursively build filter SQL fragments from [`QueryFilters`].
fn build_filter_sql(
    filters: &QueryFilters,
    parts: &mut Vec<String>,
    binds: &mut Vec<Value>,
) {
    // Equality filters.
    for f in &filters.equality {
        if validate_field_name(&f.field).is_err() {
            continue;
        }
        parts.push(format!("json_extract(data, '$.{}') = ?", f.field));
        binds.push(f.value.clone());
    }

    // Comparison filters.
    for f in &filters.comparison {
        if validate_field_name(&f.field).is_err() {
            continue;
        }
        let op = match f.operator {
            ComparisonOp::Gte => ">=",
            ComparisonOp::Lte => "<=",
            ComparisonOp::Gt => ">",
            ComparisonOp::Lt => "<",
        };
        parts.push(format!("json_extract(data, '$.{}') {op} ?", f.field));
        binds.push(f.value.clone());
    }

    // Text search.
    for f in &filters.text_search {
        if validate_field_name(&f.field).is_err() {
            continue;
        }
        let escaped = escape_like(&f.contains);
        parts.push(format!("json_extract(data, '$.{}') LIKE ? ESCAPE '\\'", f.field));
        binds.push(Value::String(format!("%{escaped}%")));
    }

    // Logical AND groups.
    for group in &filters.and {
        let mut inner_parts = Vec::new();
        build_filter_sql(group, &mut inner_parts, binds);
        if !inner_parts.is_empty() {
            parts.push(format!("({})", inner_parts.join(" AND ")));
        }
    }

    // Logical OR groups.
    for group in &filters.or {
        let mut inner_parts = Vec::new();
        build_filter_sql(group, &mut inner_parts, binds);
        if !inner_parts.is_empty() {
            parts.push(format!("({})", inner_parts.join(" OR ")));
        }
    }
}

/// Execute a COUNT query with JSON bind values.
fn execute_count(conn: &Connection, sql: &str, binds: &[Value]) -> anyhow::Result<u64> {
    let mut stmt = conn.prepare(sql)?;
    let params: Vec<Box<dyn rusqlite::types::ToSql>> = binds
        .iter()
        .map(json_value_to_sql)
        .collect();
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|b| b.as_ref()).collect();
    let count: i64 = stmt.query_row(param_refs.as_slice(), |row| row.get(0))?;
    Ok(count as u64)
}

/// Execute a SELECT query and return Records.
fn execute_query(conn: &Connection, sql: &str, binds: &[Value]) -> anyhow::Result<Vec<Record>> {
    let mut stmt = conn.prepare(sql)?;
    let params: Vec<Box<dyn rusqlite::types::ToSql>> = binds
        .iter()
        .map(json_value_to_sql)
        .collect();
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|b| b.as_ref()).collect();

    let rows = stmt.query_map(param_refs.as_slice(), |row| Ok(row_to_record(row)))?;

    let mut records = Vec::new();
    for row_result in rows {
        records.push(row_result??);
    }
    Ok(records)
}

/// Convert a `serde_json::Value` to a boxed `ToSql` for rusqlite binding.
fn json_value_to_sql(v: &Value) -> Box<dyn rusqlite::types::ToSql> {
    match v {
        Value::String(s) => Box::new(s.clone()),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Box::new(i)
            } else if let Some(f) = n.as_f64() {
                Box::new(f)
            } else {
                Box::new(n.to_string())
            }
        }
        Value::Bool(b) => Box::new(*b),
        Value::Null => Box::new(rusqlite::types::Null),
        _ => Box::new(v.to_string()),
    }
}

// ──────────────────────────────────────────────────────────────
// AuditLogger
// ──────────────────────────────────────────────────────────────

/// A single audit log entry returned from queries.
#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub id: String,
    pub timestamp: String,
    pub event_type: String,
    pub collection: Option<String>,
    pub document_id: Option<String>,
    pub identity_subject: Option<String>,
    pub plugin_id: Option<String>,
    pub details: Option<String>,
}

/// Records security-relevant events in the `audit_log` table.
#[allow(dead_code)]
pub struct AuditLogger;

/// Number of days to retain audit entries.
const AUDIT_RETENTION_DAYS: i64 = 90;

#[allow(dead_code)]
impl AuditLogger {
    /// Insert an audit event with full field support.
    pub fn log_event(
        conn: &Connection,
        event_type: &str,
        plugin_id: Option<&str>,
        details: Option<&Value>,
    ) -> anyhow::Result<()> {
        Self::log_event_full(conn, event_type, None, None, None, plugin_id, details)
    }

    /// Insert an audit event with collection, document_id, and identity_subject.
    pub fn log_event_full(
        conn: &Connection,
        event_type: &str,
        collection: Option<&str>,
        document_id: Option<&str>,
        identity_subject: Option<&str>,
        plugin_id: Option<&str>,
        details: Option<&Value>,
    ) -> anyhow::Result<()> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let details_str = details.map(serde_json::to_string).transpose()?;

        conn.execute(
            "INSERT INTO audit_log (id, timestamp, event_type, collection, document_id, \
             identity_subject, plugin_id, details, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![id, now, event_type, collection, document_id, identity_subject, plugin_id, details_str, now],
        )?;
        Ok(())
    }

    /// Query audit log entries with pagination.
    ///
    /// Returns entries ordered by timestamp descending (newest first).
    /// Limit is capped at 1000 to prevent unbounded result sets.
    pub fn query_entries(
        conn: &Connection,
        limit: u32,
        offset: u32,
    ) -> anyhow::Result<(Vec<AuditEntry>, u64)> {
        let limit = limit.min(1000);
        let total: u64 = conn.query_row(
            "SELECT COUNT(*) FROM audit_log",
            [],
            |row| row.get(0),
        )?;

        let mut stmt = conn.prepare(
            "SELECT id, timestamp, event_type, collection, document_id, \
             identity_subject, plugin_id, details \
             FROM audit_log ORDER BY timestamp DESC LIMIT ?1 OFFSET ?2",
        )?;

        let entries = stmt
            .query_map(params![limit, offset], |row| {
                Ok(AuditEntry {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    event_type: row.get(2)?,
                    collection: row.get(3)?,
                    document_id: row.get(4)?,
                    identity_subject: row.get(5)?,
                    plugin_id: row.get(6)?,
                    details: row.get::<_, Option<String>>(7)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok((entries, total))
    }

    /// Delete audit entries older than the retention period.
    pub fn cleanup_old_entries(conn: &Connection) -> anyhow::Result<usize> {
        let cutoff = (Utc::now() - chrono::Duration::days(AUDIT_RETENTION_DAYS)).to_rfc3339();
        let deleted = conn.execute(
            "DELETE FROM audit_log WHERE timestamp < ?1",
            params![cutoff],
        )?;
        if deleted > 0 {
            tracing::info!(deleted, "audit log entries cleaned up");
        }
        Ok(deleted)
    }
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{
        ComparisonFilter, ComparisonOp, FieldFilter, Pagination, QueryFilters, SortDirection,
        SortOptions, StorageAdapter, TextFilter,
    };

    /// Helper: create an in-memory storage for tests.
    fn test_storage() -> SqliteStorage {
        SqliteStorage::open_in_memory().expect("failed to open in-memory db")
    }

    // ── CRUD ─────────────────────────────────────────────────

    #[tokio::test]
    async fn create_and_get_record() {
        let storage = test_storage();
        let data = serde_json::json!({"title": "Buy milk"});
        let created = storage.create("p1", "tasks", data.clone()).await.unwrap();

        assert_eq!(created.plugin_id, "p1");
        assert_eq!(created.collection, "tasks");
        assert_eq!(created.data, data);
        assert_eq!(created.version, 1);

        let fetched = storage
            .get("p1", "tasks", &created.id)
            .await
            .unwrap()
            .expect("record should exist");

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.data, data);
        assert_eq!(fetched.version, 1);
    }

    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let storage = test_storage();
        let result = storage.get("p1", "tasks", "no-such-id").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn update_record() {
        let storage = test_storage();
        let created = storage
            .create("p1", "tasks", serde_json::json!({"v": 1}))
            .await
            .unwrap();

        let updated = storage
            .update("p1", "tasks", &created.id, serde_json::json!({"v": 2}), 1)
            .await
            .unwrap();

        assert_eq!(updated.version, 2);
        assert_eq!(updated.data, serde_json::json!({"v": 2}));
        assert!(updated.updated_at >= created.updated_at);
    }

    #[tokio::test]
    async fn delete_record() {
        let storage = test_storage();
        let created = storage
            .create("p1", "tasks", serde_json::json!({}))
            .await
            .unwrap();

        assert!(storage.delete("p1", "tasks", &created.id).await.unwrap());
        assert!(storage.get("p1", "tasks", &created.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_nonexistent_returns_false() {
        let storage = test_storage();
        assert!(!storage.delete("p1", "tasks", "nope").await.unwrap());
    }

    // ── Optimistic locking ───────────────────────────────────

    #[tokio::test]
    async fn version_mismatch_rejects_update() {
        let storage = test_storage();
        let created = storage
            .create("p1", "tasks", serde_json::json!({"v": 1}))
            .await
            .unwrap();

        let result = storage
            .update("p1", "tasks", &created.id, serde_json::json!({"v": 2}), 999)
            .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("version mismatch"));
    }

    // ── Plugin isolation ─────────────────────────────────────

    #[tokio::test]
    async fn records_scoped_by_plugin_and_collection() {
        let storage = test_storage();
        storage.create("p1", "tasks", serde_json::json!({"a": 1})).await.unwrap();
        storage.create("p1", "notes", serde_json::json!({"a": 2})).await.unwrap();
        storage.create("p2", "tasks", serde_json::json!({"a": 3})).await.unwrap();

        let result = storage
            .query("p1", "tasks", QueryFilters::default(), None, Pagination::default())
            .await
            .unwrap();
        assert_eq!(result.total, 1);
        assert_eq!(result.records[0].data, serde_json::json!({"a": 1}));

        let result2 = storage
            .list("p2", "tasks", None, Pagination::default())
            .await
            .unwrap();
        assert_eq!(result2.total, 1);
    }

    // ── Query filters ────────────────────────────────────────

    #[tokio::test]
    async fn query_equality_filter() {
        let storage = test_storage();
        storage.create("p1", "tasks", serde_json::json!({"status": "open"})).await.unwrap();
        storage.create("p1", "tasks", serde_json::json!({"status": "closed"})).await.unwrap();

        let filters = QueryFilters {
            equality: vec![FieldFilter {
                field: "status".into(),
                value: serde_json::json!("open"),
            }],
            ..Default::default()
        };

        let result = storage
            .query("p1", "tasks", filters, None, Pagination::default())
            .await
            .unwrap();
        assert_eq!(result.total, 1);
        assert_eq!(result.records[0].data["status"], "open");
    }

    #[tokio::test]
    async fn query_comparison_filters() {
        let storage = test_storage();
        for i in 1..=5 {
            storage
                .create("p1", "items", serde_json::json!({"score": i}))
                .await
                .unwrap();
        }

        // $gte 3 => scores 3, 4, 5
        let filters = QueryFilters {
            comparison: vec![ComparisonFilter {
                field: "score".into(),
                operator: ComparisonOp::Gte,
                value: serde_json::json!(3),
            }],
            ..Default::default()
        };
        let result = storage
            .query("p1", "items", filters, None, Pagination::default())
            .await
            .unwrap();
        assert_eq!(result.total, 3);

        // $lt 3 => scores 1, 2
        let filters2 = QueryFilters {
            comparison: vec![ComparisonFilter {
                field: "score".into(),
                operator: ComparisonOp::Lt,
                value: serde_json::json!(3),
            }],
            ..Default::default()
        };
        let result2 = storage
            .query("p1", "items", filters2, None, Pagination::default())
            .await
            .unwrap();
        assert_eq!(result2.total, 2);
    }

    #[tokio::test]
    async fn query_text_search_contains() {
        let storage = test_storage();
        storage.create("p1", "notes", serde_json::json!({"title": "Rust programming"})).await.unwrap();
        storage.create("p1", "notes", serde_json::json!({"title": "Go programming"})).await.unwrap();
        storage.create("p1", "notes", serde_json::json!({"title": "Cooking recipes"})).await.unwrap();

        let filters = QueryFilters {
            text_search: vec![TextFilter {
                field: "title".into(),
                contains: "programming".into(),
            }],
            ..Default::default()
        };

        let result = storage
            .query("p1", "notes", filters, None, Pagination::default())
            .await
            .unwrap();
        assert_eq!(result.total, 2);
    }

    #[tokio::test]
    async fn query_and_logic() {
        let storage = test_storage();
        storage.create("p1", "tasks", serde_json::json!({"status": "open", "priority": 1})).await.unwrap();
        storage.create("p1", "tasks", serde_json::json!({"status": "open", "priority": 3})).await.unwrap();
        storage.create("p1", "tasks", serde_json::json!({"status": "closed", "priority": 1})).await.unwrap();

        // AND: status=open AND priority >= 2
        let filters = QueryFilters {
            and: vec![QueryFilters {
                equality: vec![FieldFilter {
                    field: "status".into(),
                    value: serde_json::json!("open"),
                }],
                comparison: vec![ComparisonFilter {
                    field: "priority".into(),
                    operator: ComparisonOp::Gte,
                    value: serde_json::json!(2),
                }],
                ..Default::default()
            }],
            ..Default::default()
        };

        let result = storage
            .query("p1", "tasks", filters, None, Pagination::default())
            .await
            .unwrap();
        assert_eq!(result.total, 1);
        assert_eq!(result.records[0].data["priority"], 3);
    }

    #[tokio::test]
    async fn query_or_logic() {
        let storage = test_storage();
        storage.create("p1", "tasks", serde_json::json!({"status": "open"})).await.unwrap();
        storage.create("p1", "tasks", serde_json::json!({"status": "closed"})).await.unwrap();
        storage.create("p1", "tasks", serde_json::json!({"status": "archived"})).await.unwrap();

        // OR: status=open OR status=closed
        let filters = QueryFilters {
            or: vec![QueryFilters {
                equality: vec![
                    FieldFilter {
                        field: "status".into(),
                        value: serde_json::json!("open"),
                    },
                    FieldFilter {
                        field: "status".into(),
                        value: serde_json::json!("closed"),
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        };

        let result = storage
            .query("p1", "tasks", filters, None, Pagination::default())
            .await
            .unwrap();
        // The OR group equality filters are joined with OR
        assert_eq!(result.total, 2);
    }

    // ── Sorting ──────────────────────────────────────────────

    #[tokio::test]
    async fn query_sort_ascending() {
        let storage = test_storage();
        storage.create("p1", "items", serde_json::json!({"name": "Charlie"})).await.unwrap();
        storage.create("p1", "items", serde_json::json!({"name": "Alice"})).await.unwrap();
        storage.create("p1", "items", serde_json::json!({"name": "Bob"})).await.unwrap();

        let result = storage
            .query(
                "p1",
                "items",
                QueryFilters::default(),
                Some(SortOptions {
                    sort_by: "name".into(),
                    sort_dir: SortDirection::Asc,
                }),
                Pagination::default(),
            )
            .await
            .unwrap();

        let names: Vec<&str> = result
            .records
            .iter()
            .map(|r| r.data["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, vec!["Alice", "Bob", "Charlie"]);
    }

    #[tokio::test]
    async fn query_sort_descending() {
        let storage = test_storage();
        storage.create("p1", "items", serde_json::json!({"score": 10})).await.unwrap();
        storage.create("p1", "items", serde_json::json!({"score": 30})).await.unwrap();
        storage.create("p1", "items", serde_json::json!({"score": 20})).await.unwrap();

        let result = storage
            .query(
                "p1",
                "items",
                QueryFilters::default(),
                Some(SortOptions {
                    sort_by: "score".into(),
                    sort_dir: SortDirection::Desc,
                }),
                Pagination::default(),
            )
            .await
            .unwrap();

        let scores: Vec<i64> = result
            .records
            .iter()
            .map(|r| r.data["score"].as_i64().unwrap())
            .collect();
        assert_eq!(scores, vec![30, 20, 10]);
    }

    // ── Pagination ───────────────────────────────────────────

    #[tokio::test]
    async fn pagination_limit_and_offset() {
        let storage = test_storage();
        for i in 0..10 {
            storage
                .create("p1", "items", serde_json::json!({"i": i}))
                .await
                .unwrap();
        }

        let result = storage
            .query(
                "p1",
                "items",
                QueryFilters::default(),
                Some(SortOptions {
                    sort_by: "i".into(),
                    sort_dir: SortDirection::Asc,
                }),
                Pagination { limit: 3, offset: 2 },
            )
            .await
            .unwrap();

        assert_eq!(result.total, 10);
        assert_eq!(result.records.len(), 3);
        assert_eq!(result.limit, 3);
        assert_eq!(result.offset, 2);

        // Items at indices 2, 3, 4
        let values: Vec<i64> = result
            .records
            .iter()
            .map(|r| r.data["i"].as_i64().unwrap())
            .collect();
        assert_eq!(values, vec![2, 3, 4]);
    }

    #[tokio::test]
    async fn pagination_total_count_independent_of_limit() {
        let storage = test_storage();
        for i in 0..5 {
            storage
                .create("p1", "items", serde_json::json!({"i": i}))
                .await
                .unwrap();
        }

        let result = storage
            .list("p1", "items", None, Pagination { limit: 2, offset: 0 })
            .await
            .unwrap();
        assert_eq!(result.total, 5);
        assert_eq!(result.records.len(), 2);
    }

    // ── Audit logging ────────────────────────────────────────

    #[tokio::test]
    async fn crud_operations_create_audit_entries() {
        let storage = test_storage();

        let created = storage
            .create("p1", "tasks", serde_json::json!({"t": 1}))
            .await
            .unwrap();
        storage
            .update("p1", "tasks", &created.id, serde_json::json!({"t": 2}), 1)
            .await
            .unwrap();
        storage.delete("p1", "tasks", &created.id).await.unwrap();

        let conn = storage.conn.lock().await;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get(0))
            .unwrap();
        // 3 events: create, update, delete
        assert_eq!(count, 3);

        // Verify event types.
        let mut stmt = conn
            .prepare("SELECT event_type FROM audit_log ORDER BY timestamp")
            .unwrap();
        let types: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(types, vec!["system.storage.created", "system.storage.updated", "system.storage.deleted"]);
    }

    #[tokio::test]
    async fn audit_retention_cleanup() {
        let storage = test_storage();
        let conn = storage.conn.lock().await;

        // Insert an old audit entry (100 days ago).
        let old_time = (Utc::now() - chrono::Duration::days(100)).to_rfc3339();
        conn.execute(
            "INSERT INTO audit_log (id, timestamp, event_type, collection, document_id, \
             identity_subject, plugin_id, details, created_at) \
             VALUES ('old', ?1, 'test', NULL, NULL, NULL, 'p1', NULL, ?1)",
            params![old_time],
        )
        .unwrap();

        // Insert a recent audit entry.
        let recent_time = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO audit_log (id, timestamp, event_type, collection, document_id, \
             identity_subject, plugin_id, details, created_at) \
             VALUES ('recent', ?1, 'test', NULL, NULL, NULL, 'p1', NULL, ?1)",
            params![recent_time],
        )
        .unwrap();

        let deleted = AuditLogger::cleanup_old_entries(&conn).unwrap();
        assert_eq!(deleted, 1);

        let remaining: i64 = conn
            .query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get(0))
            .unwrap();
        assert_eq!(remaining, 1);
    }

    // ── Change notifications ─────────────────────────────────

    #[tokio::test]
    async fn subscribe_receives_create_event() {
        let storage = test_storage();
        let mut rx = storage.subscribe();

        storage.create("p1", "tasks", serde_json::json!({})).await.unwrap();

        let event = rx.recv().await.unwrap();
        assert_eq!(event.collection, "tasks");
        assert_eq!(event.change_type, ChangeType::Created);
    }

    #[tokio::test]
    async fn subscribe_receives_update_event() {
        let storage = test_storage();
        let created = storage
            .create("p1", "tasks", serde_json::json!({"v": 1}))
            .await
            .unwrap();

        let mut rx = storage.subscribe();
        storage
            .update("p1", "tasks", &created.id, serde_json::json!({"v": 2}), 1)
            .await
            .unwrap();

        let event = rx.recv().await.unwrap();
        assert_eq!(event.change_type, ChangeType::Updated);
        assert_eq!(event.record_id, created.id);
    }

    #[tokio::test]
    async fn subscribe_receives_delete_event() {
        let storage = test_storage();
        let created = storage
            .create("p1", "tasks", serde_json::json!({}))
            .await
            .unwrap();

        let mut rx = storage.subscribe();
        storage.delete("p1", "tasks", &created.id).await.unwrap();

        let event = rx.recv().await.unwrap();
        assert_eq!(event.change_type, ChangeType::Deleted);
        assert_eq!(event.record_id, created.id);
    }

    // ── Concurrent access (WAL) ──────────────────────────────

    #[tokio::test]
    async fn concurrent_reads_during_writes() {
        let storage = Arc::new(test_storage());

        // Spawn multiple concurrent creates.
        let mut handles = Vec::new();
        for i in 0..10 {
            let s = Arc::clone(&storage);
            handles.push(tokio::spawn(async move {
                s.create("p1", "tasks", serde_json::json!({"i": i}))
                    .await
                    .unwrap()
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        // All 10 records should be present.
        let result = storage
            .list("p1", "tasks", None, Pagination { limit: 100, offset: 0 })
            .await
            .unwrap();
        assert_eq!(result.total, 10);
    }

    // ── Encryption round-trip ────────────────────────────────

    /// Low-cost Argon2 settings for fast test execution.
    fn test_argon2() -> crate::config::Argon2Settings {
        crate::config::Argon2Settings {
            memory_mb: 1,
            iterations: 1,
            parallelism: 1,
        }
    }

    #[tokio::test]
    async fn encrypted_roundtrip_write_close_reopen_read() {
        // Open encrypted, write a record, drop (close), reopen with
        // the same passphrase, and read the record back.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let settings = test_argon2();

        let id = {
            let storage = SqliteStorage::open_encrypted(
                tmp.path(),
                "my-secret-passphrase",
                &settings,
            )
            .expect("open_encrypted should succeed");

            let rec = storage
                .create("p1", "tasks", serde_json::json!({"title": "encrypted task"}))
                .await
                .unwrap();
            rec.id
        };
        // Storage dropped — connection closed.

        // Reopen with the same passphrase.
        let storage2 = SqliteStorage::open_encrypted(
            tmp.path(),
            "my-secret-passphrase",
            &settings,
        )
        .expect("reopen with correct passphrase should succeed");

        let record = storage2
            .get("p1", "tasks", &id)
            .await
            .unwrap()
            .expect("record should be readable after reopen");
        assert_eq!(record.data["title"], "encrypted task");
    }

    #[tokio::test]
    async fn encrypted_wrong_passphrase_fails() {
        // Open encrypted, close, reopen with WRONG passphrase — must fail.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let settings = test_argon2();

        {
            let storage = SqliteStorage::open_encrypted(
                tmp.path(),
                "correct-passphrase",
                &settings,
            )
            .expect("initial open should succeed");
            storage
                .create("p1", "tasks", serde_json::json!({"secret": true}))
                .await
                .unwrap();
        }

        // Reopen with wrong passphrase — MUST fail.
        let result = SqliteStorage::open_encrypted(
            tmp.path(),
            "wrong-passphrase",
            &settings,
        );
        assert!(
            result.is_err(),
            "opening with wrong passphrase should fail — \
             encryption is not being applied"
        );
    }

    #[tokio::test]
    async fn encrypted_different_argon2_settings_fails() {
        // Different Argon2 settings produce different derived keys,
        // so data written with one set cannot be read with another.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let settings_a = crate::config::Argon2Settings {
            memory_mb: 1,
            iterations: 1,
            parallelism: 1,
        };
        let settings_b = crate::config::Argon2Settings {
            memory_mb: 2,
            iterations: 1,
            parallelism: 1,
        };

        {
            let storage = SqliteStorage::open_encrypted(
                tmp.path(),
                "same-passphrase",
                &settings_a,
            )
            .expect("open with settings_a should succeed");
            storage
                .create("p1", "tasks", serde_json::json!({"v": 1}))
                .await
                .unwrap();
        }

        // Reopen with different Argon2 settings — MUST fail.
        let result = SqliteStorage::open_encrypted(
            tmp.path(),
            "same-passphrase",
            &settings_b,
        );
        assert!(
            result.is_err(),
            "opening with different Argon2 settings should fail — \
             the derived key should be different"
        );
    }
}
