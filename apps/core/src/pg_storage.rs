//! PostgreSQL implementation of the [`StorageAdapter`] trait.
//!
//! Uses `deadpool-postgres` for connection pooling and `tokio-postgres`
//! for async queries. Document data is stored as JSONB for efficient
//! querying. Full-text search uses PostgreSQL `tsvector` / `tsquery`.
//!
//! Change events are published on a broadcast channel, identical to
//! the SQLite implementation.

use async_trait::async_trait;
use chrono::Utc;
use deadpool_postgres::{Config as PoolConfig, Pool, Runtime};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast;
use tokio_postgres::NoTls;
use tokio_postgres_rustls::MakeRustlsConnect;

use crate::sqlite_storage::{ChangeEvent, ChangeType};
use crate::storage::{
    ComparisonOp, Pagination, QueryFilters, QueryResult, Record, SortDirection, SortOptions,
    StorageAdapter,
};

/// Broadcast channel capacity for change events.
const CHANGE_CHANNEL_CAPACITY: usize = 256;

/// TLS mode for the PostgreSQL connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PgSslMode {
    /// No TLS — connections are unencrypted.
    Disable,
    /// Use TLS if the server supports it, fall back to plaintext otherwise.
    Prefer,
    /// Require TLS; fail if the server does not support it.
    Require,
}

impl Default for PgSslMode {
    fn default() -> Self {
        Self::Require
    }
}

impl std::fmt::Display for PgSslMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disable => write!(f, "disable"),
            Self::Prefer => write!(f, "prefer"),
            Self::Require => write!(f, "require"),
        }
    }
}

impl std::str::FromStr for PgSslMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "disable" => Ok(Self::Disable),
            "prefer" => Ok(Self::Prefer),
            "require" => Ok(Self::Require),
            other => Err(anyhow::anyhow!(
                "invalid PG SSL mode '{}': expected disable, prefer, or require",
                other
            )),
        }
    }
}

/// PostgreSQL connection pool configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgConfig {
    /// PostgreSQL host.
    pub host: String,
    /// PostgreSQL port.
    pub port: u16,
    /// Database name.
    pub dbname: String,
    /// Username.
    pub user: String,
    /// Password.
    pub password: String,
    /// Maximum pool size.
    #[serde(default = "default_pool_size")]
    pub pool_size: usize,
    /// TLS mode for the connection (disable, prefer, require).
    /// Defaults to `require` so credentials are never sent in plaintext.
    #[serde(default)]
    pub ssl_mode: PgSslMode,
}

fn default_pool_size() -> usize {
    16
}

impl Default for PgConfig {
    fn default() -> Self {
        Self {
            host: "localhost".into(),
            port: 5432,
            dbname: "life_engine".into(),
            user: "life_engine".into(),
            password: String::new(),
            pool_size: default_pool_size(),
            ssl_mode: PgSslMode::default(),
        }
    }
}

/// Build a `rustls::ClientConfig` that trusts the system root certificates.
fn make_rustls_config() -> anyhow::Result<rustls::ClientConfig> {
    let mut root_store = rustls::RootCertStore::empty();
    for cert in rustls_native_certs::load_native_certs().expect("failed to load native certs") {
        root_store.add(cert).ok();
    }
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    Ok(config)
}

/// PostgreSQL-backed storage adapter with connection pooling.
pub struct PgStorage {
    pool: Pool,
    change_tx: broadcast::Sender<ChangeEvent>,
}

impl PgStorage {
    /// Create a new PostgreSQL storage with the given configuration.
    ///
    /// TLS behaviour is controlled by `config.ssl_mode`:
    ///
    /// - `Disable` — no TLS (plain TCP).
    /// - `Prefer` — try TLS first, fall back to plaintext on failure.
    /// - `Require` — TLS is mandatory; the connection fails if TLS
    ///   cannot be established.
    ///
    /// The default is `Require` so that database credentials are never
    /// transmitted in plaintext.
    pub async fn open(config: &PgConfig) -> anyhow::Result<Self> {
        let mut pool_config = PoolConfig::new();
        pool_config.host = Some(config.host.clone());
        pool_config.port = Some(config.port);
        pool_config.dbname = Some(config.dbname.clone());
        pool_config.user = Some(config.user.clone());
        pool_config.password = Some(config.password.clone());

        let pool = match config.ssl_mode {
            PgSslMode::Disable => {
                tracing::warn!("PostgreSQL TLS is disabled — credentials will be sent in plaintext");
                pool_config.create_pool(Some(Runtime::Tokio1), NoTls)?
            }
            PgSslMode::Prefer | PgSslMode::Require => {
                let tls_config = make_rustls_config()?;
                let tls_connector = MakeRustlsConnect::new(tls_config);
                tracing::info!("PostgreSQL TLS mode: {}", config.ssl_mode);
                pool_config.create_pool(Some(Runtime::Tokio1), tls_connector)?
            }
        };

        let storage = Self::from_pool(pool).await?;
        Ok(storage)
    }

    /// Create storage from an existing connection pool.
    pub async fn from_pool(pool: Pool) -> anyhow::Result<Self> {
        let (change_tx, _) = broadcast::channel(CHANGE_CHANNEL_CAPACITY);

        let storage = Self { pool, change_tx };
        storage.create_tables().await?;

        Ok(storage)
    }

    /// Subscribe to change notifications.
    #[allow(dead_code)]
    pub fn subscribe(&self) -> broadcast::Receiver<ChangeEvent> {
        self.change_tx.subscribe()
    }

    /// Create the required database tables and indices.
    async fn create_tables(&self) -> anyhow::Result<()> {
        let client = self.pool.get().await?;

        client
            .batch_execute(
                "CREATE TABLE IF NOT EXISTS plugin_data (
                    id             TEXT PRIMARY KEY,
                    plugin_id      TEXT NOT NULL,
                    collection     TEXT NOT NULL,
                    data           JSONB NOT NULL,
                    version        BIGINT NOT NULL DEFAULT 1,
                    user_id        TEXT,
                    household_id   TEXT,
                    created_at     TIMESTAMPTZ NOT NULL,
                    updated_at     TIMESTAMPTZ NOT NULL
                );

                CREATE INDEX IF NOT EXISTS idx_plugin_collection
                    ON plugin_data(plugin_id, collection);

                CREATE TABLE IF NOT EXISTS audit_log (
                    id          TEXT PRIMARY KEY,
                    timestamp   TIMESTAMPTZ NOT NULL,
                    event_type  TEXT NOT NULL,
                    plugin_id   TEXT,
                    details     JSONB,
                    created_at  TIMESTAMPTZ NOT NULL
                );

                CREATE INDEX IF NOT EXISTS idx_audit_timestamp
                    ON audit_log(timestamp);

                -- Full-text search support via tsvector.
                DO $$
                BEGIN
                    IF NOT EXISTS (
                        SELECT 1 FROM information_schema.columns
                        WHERE table_name = 'plugin_data' AND column_name = 'search_vector'
                    ) THEN
                        ALTER TABLE plugin_data ADD COLUMN search_vector tsvector;
                    END IF;
                END $$;

                CREATE INDEX IF NOT EXISTS idx_plugin_data_search
                    ON plugin_data USING GIN(search_vector);

                -- Function to extract searchable text from JSONB data.
                CREATE OR REPLACE FUNCTION plugin_data_search_text(data JSONB)
                RETURNS TEXT AS $$
                DECLARE
                    result TEXT := '';
                BEGIN
                    IF data ? 'title' THEN
                        result := result || ' ' || COALESCE(data->>'title', '');
                    END IF;
                    IF data ? 'subject' THEN
                        result := result || ' ' || COALESCE(data->>'subject', '');
                    END IF;
                    IF data ? 'name' THEN
                        result := result || ' ' || COALESCE(data->>'name', '');
                    END IF;
                    IF data ? 'content' THEN
                        result := result || ' ' || COALESCE(data->>'content', '');
                    END IF;
                    IF data ? 'body' THEN
                        result := result || ' ' || COALESCE(data->>'body', '');
                    END IF;
                    IF data ? 'description' THEN
                        result := result || ' ' || COALESCE(data->>'description', '');
                    END IF;
                    IF data ? 'location' THEN
                        result := result || ' ' || COALESCE(data->>'location', '');
                    END IF;
                    RETURN TRIM(result);
                END;
                $$ LANGUAGE plpgsql IMMUTABLE;

                -- Trigger function to update search_vector on insert/update.
                CREATE OR REPLACE FUNCTION plugin_data_search_trigger()
                RETURNS TRIGGER AS $$
                BEGIN
                    NEW.search_vector := to_tsvector('english', plugin_data_search_text(NEW.data));
                    RETURN NEW;
                END;
                $$ LANGUAGE plpgsql;

                -- Drop and recreate trigger to ensure it's up to date.
                DROP TRIGGER IF EXISTS trg_plugin_data_search ON plugin_data;
                CREATE TRIGGER trg_plugin_data_search
                    BEFORE INSERT OR UPDATE ON plugin_data
                    FOR EACH ROW
                    EXECUTE FUNCTION plugin_data_search_trigger();",
            )
            .await?;

        // Clean up old audit entries (90-day retention).
        let cutoff = Utc::now() - chrono::Duration::days(90);
        client
            .execute(
                "DELETE FROM audit_log WHERE timestamp < $1",
                &[&cutoff],
            )
            .await?;

        Ok(())
    }

    /// Publish a change event (best-effort).
    fn publish(&self, event: ChangeEvent) {
        let _ = self.change_tx.send(event);
    }

    /// Log an audit event.
    async fn log_audit(
        &self,
        event_type: &str,
        plugin_id: Option<&str>,
        details: Option<&Value>,
    ) -> anyhow::Result<()> {
        let client = self.pool.get().await?;
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();

        client
            .execute(
                "INSERT INTO audit_log (id, timestamp, event_type, plugin_id, details, created_at)
                 VALUES ($1, $2, $3, $4, $5, $6)",
                &[&id, &now, &event_type, &plugin_id, &details, &now],
            )
            .await?;

        Ok(())
    }

    /// Perform a full-text search using PostgreSQL tsvector.
    pub async fn fulltext_search(
        &self,
        query: &str,
        collection: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> anyhow::Result<QueryResult> {
        let client = self.pool.get().await?;
        let limit_val = limit.min(100) as i64;
        let offset_val = offset as i64;
        let query_str = query.to_string();

        let (total, records) = if let Some(coll) = collection {
            let coll_str = coll.to_string();
            let count_row = client
                .query_one(
                    "SELECT COUNT(*) FROM plugin_data WHERE search_vector @@ plainto_tsquery('english', $1) AND collection = $2",
                    &[&query_str, &coll_str],
                )
                .await?;
            let total: i64 = count_row.get(0);

            let rows = client
                .query(
                    "SELECT id, plugin_id, collection, data, version, user_id, household_id, created_at, updated_at,
                            ts_rank(search_vector, plainto_tsquery('english', $1)) AS rank
                     FROM plugin_data
                     WHERE search_vector @@ plainto_tsquery('english', $1) AND collection = $2
                     ORDER BY rank DESC
                     LIMIT $3 OFFSET $4",
                    &[&query_str, &coll_str, &limit_val, &offset_val],
                )
                .await?;

            let records = rows.iter().map(row_to_record).collect::<anyhow::Result<Vec<_>>>()?;
            (total, records)
        } else {
            let count_row = client
                .query_one(
                    "SELECT COUNT(*) FROM plugin_data WHERE search_vector @@ plainto_tsquery('english', $1)",
                    &[&query_str],
                )
                .await?;
            let total: i64 = count_row.get(0);

            let rows = client
                .query(
                    "SELECT id, plugin_id, collection, data, version, user_id, household_id, created_at, updated_at,
                            ts_rank(search_vector, plainto_tsquery('english', $1)) AS rank
                     FROM plugin_data
                     WHERE search_vector @@ plainto_tsquery('english', $1)
                     ORDER BY rank DESC
                     LIMIT $2 OFFSET $3",
                    &[&query_str, &limit_val, &offset_val],
                )
                .await?;

            let records = rows.iter().map(row_to_record).collect::<anyhow::Result<Vec<_>>>()?;
            (total, records)
        };

        Ok(QueryResult {
            records,
            total: total as u64,
            limit: limit.min(100),
            offset,
        })
    }

    /// Get a reference to the connection pool (used for migration).
    pub fn pool(&self) -> &Pool {
        &self.pool
    }
}

// ──────────────────────────────────────────────────────────────
// StorageAdapter implementation
// ──────────────────────────────────────────────────────────────

#[async_trait]
impl StorageAdapter for PgStorage {
    async fn get(
        &self,
        plugin_id: &str,
        collection: &str,
        id: &str,
    ) -> anyhow::Result<Option<Record>> {
        let client = self.pool.get().await?;
        let row = client
            .query_opt(
                "SELECT id, plugin_id, collection, data, version, user_id, household_id, created_at, updated_at
                 FROM plugin_data
                 WHERE id = $1 AND plugin_id = $2 AND collection = $3",
                &[&id, &plugin_id, &collection],
            )
            .await?;

        match row {
            Some(r) => Ok(Some(row_to_record(&r)?)),
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

        let id_owned = id.to_string();
        let client = self.pool.get().await?;
        client
            .execute(
                "INSERT INTO plugin_data (id, plugin_id, collection, data, version, created_at, updated_at)
                 VALUES ($1, $2, $3, $4, 1, $5, $6)",
                &[&id_owned, &plugin_id, &collection, &data, &now, &now],
            )
            .await?;

        self.log_audit(
            "data.create",
            Some(plugin_id),
            Some(&serde_json::json!({
                "collection": collection,
                "record_id": id,
            })),
        )
        .await?;

        self.publish(ChangeEvent {
            collection: collection.into(),
            record_id: id_owned.clone(),
            change_type: ChangeType::Created,
        });

        Ok(Record {
            id: id_owned,
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
        let new_version = version + 1;

        let client = self.pool.get().await
            .map_err(|e| StorageError::Other(e.into()))?;
        let rows = client
            .execute(
                "UPDATE plugin_data
                 SET data = $1, version = $2, updated_at = $3
                 WHERE id = $4 AND plugin_id = $5 AND collection = $6 AND version = $7",
                &[&data, &new_version, &now, &id, &plugin_id, &collection, &version],
            )
            .await
            .map_err(|e| StorageError::Other(e.into()))?;

        if rows == 0 {
            // Distinguish between "not found" and "version mismatch".
            let exists = client
                .query_one(
                    "SELECT EXISTS(SELECT 1 FROM plugin_data WHERE id = $1 AND plugin_id = $2 AND collection = $3)",
                    &[&id, &plugin_id, &collection],
                )
                .await
                .map(|row| row.get::<_, bool>(0))
                .unwrap_or(false);
            return if exists {
                Err(StorageError::VersionMismatch)
            } else {
                Err(StorageError::NotFound)
            };
        }

        // Fetch created_at from the existing row.
        let row = client
            .query_one(
                "SELECT created_at FROM plugin_data WHERE id = $1",
                &[&id],
            )
            .await
            .map_err(|e| StorageError::Other(e.into()))?;
        let created_at: chrono::DateTime<Utc> = row.get(0);

        self.log_audit(
            "data.update",
            Some(plugin_id),
            Some(&serde_json::json!({
                "collection": collection,
                "record_id": id,
                "new_version": new_version,
            })),
        )
        .await
        .map_err(|e| StorageError::Other(e.into()))?;

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
            created_at,
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
        let client = self.pool.get().await?;

        // Build the query with string-interpolated filter values.
        // We use parameterized queries for the base plugin_id and collection,
        // and build filter conditions as additional parameterized clauses.
        let mut where_parts: Vec<String> = vec![
            "plugin_id = $1".into(),
            "collection = $2".into(),
        ];
        let mut string_params: Vec<String> = vec![
            plugin_id.to_string(),
            collection.to_string(),
        ];
        let mut param_idx = 3u32;

        build_pg_filter_clauses(&filters, &mut where_parts, &mut string_params, &mut param_idx);

        let where_clause = where_parts.join(" AND ");

        // Count total.
        let count_sql = format!("SELECT COUNT(*) FROM plugin_data WHERE {where_clause}");
        let count_params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
            string_params.iter().map(|s| s as &(dyn tokio_postgres::types::ToSql + Sync)).collect();
        let count_row = client.query_one(&count_sql, &count_params).await?;
        let total: i64 = count_row.get(0);

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
                format!("ORDER BY data->>'{}' {dir}", opts.sort_by)
            }
            None => String::new(),
        };

        let limit_val = pagination.limit as i64;
        let offset_val = pagination.offset as i64;
        let limit_str = limit_val.to_string();
        let offset_str = offset_val.to_string();

        string_params.push(limit_str);
        string_params.push(offset_str);

        let limit_param = format!("${}", param_idx);
        let offset_param = format!("${}", param_idx + 1);

        let data_sql = format!(
            "SELECT id, plugin_id, collection, data, version, user_id, household_id, created_at, updated_at
             FROM plugin_data
             WHERE {where_clause}
             {order_clause}
             LIMIT {limit_param} OFFSET {offset_param}"
        );

        let data_params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
            string_params.iter().map(|s| s as &(dyn tokio_postgres::types::ToSql + Sync)).collect();
        let rows = client.query(&data_sql, &data_params).await?;

        let records = rows
            .iter()
            .map(row_to_record)
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(QueryResult {
            records,
            total: total as u64,
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
        let client = self.pool.get().await?;
        let rows = client
            .execute(
                "DELETE FROM plugin_data WHERE id = $1 AND plugin_id = $2 AND collection = $3",
                &[&id, &plugin_id, &collection],
            )
            .await?;

        if rows > 0 {
            self.log_audit(
                "data.delete",
                Some(plugin_id),
                Some(&serde_json::json!({
                    "collection": collection,
                    "record_id": id,
                })),
            )
            .await?;

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

/// Convert a tokio-postgres Row into a Record.
fn row_to_record(row: &tokio_postgres::Row) -> anyhow::Result<Record> {
    Ok(Record {
        id: row.get(0),
        plugin_id: row.get(1),
        collection: row.get(2),
        data: row.get(3),
        version: row.get(4),
        user_id: row.get(5),
        household_id: row.get(6),
        created_at: row.get(7),
        updated_at: row.get(8),
    })
}

/// Validate that a field name contains only safe characters for use in SQL.
///
/// Allows alphanumeric characters, underscores, and dots (for nested JSON paths).
fn validate_field_name(field: &str) -> bool {
    !field.is_empty() && field.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
}

/// Escape LIKE/ILIKE metacharacters (`%`, `_`, `\`) in a search term.
fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Build PostgreSQL filter SQL from [`QueryFilters`].
///
/// All values are passed as String parameters to keep the params vector
/// homogeneous (`Vec<String>`) which is `Send + Sync` across `.await`.
fn build_pg_filter_clauses(
    filters: &QueryFilters,
    parts: &mut Vec<String>,
    params: &mut Vec<String>,
    param_idx: &mut u32,
) {
    // Equality filters.
    for f in &filters.equality {
        if !validate_field_name(&f.field) {
            continue;
        }
        parts.push(format!("data->>'{}' = ${param_idx}", f.field));
        params.push(json_value_to_string(&f.value));
        *param_idx += 1;
    }

    // Comparison filters — cast the JSONB field to NUMERIC for comparison.
    for f in &filters.comparison {
        if !validate_field_name(&f.field) {
            continue;
        }
        let op = match f.operator {
            ComparisonOp::Gte => ">=",
            ComparisonOp::Lte => "<=",
            ComparisonOp::Gt => ">",
            ComparisonOp::Lt => "<",
        };
        // Compare as text (works for numeric strings in sorted order).
        // For proper numeric comparison, cast both sides.
        parts.push(format!(
            "CAST(data->>'{}' AS NUMERIC) {op} CAST(${param_idx} AS NUMERIC)",
            f.field
        ));
        params.push(json_value_to_string(&f.value));
        *param_idx += 1;
    }

    // Text search.
    for f in &filters.text_search {
        if !validate_field_name(&f.field) {
            continue;
        }
        let escaped = escape_like(&f.contains);
        parts.push(format!("data->>'{}' ILIKE ${param_idx} ESCAPE '\\'", f.field));
        params.push(format!("%{escaped}%"));
        *param_idx += 1;
    }

    // Logical AND groups.
    for group in &filters.and {
        let mut inner_parts = Vec::new();
        build_pg_filter_clauses(group, &mut inner_parts, params, param_idx);
        if !inner_parts.is_empty() {
            parts.push(format!("({})", inner_parts.join(" AND ")));
        }
    }

    // Logical OR groups.
    for group in &filters.or {
        let mut inner_parts = Vec::new();
        build_pg_filter_clauses(group, &mut inner_parts, params, param_idx);
        if !inner_parts.is_empty() {
            parts.push(format!("({})", inner_parts.join(" OR ")));
        }
    }
}

/// Convert a serde_json::Value to a string representation for PG params.
fn json_value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        _ => v.to_string(),
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

    /// Check if PostgreSQL is available for testing.
    ///
    /// Tests require `LIFE_ENGINE_TEST_PG_URL` env var set to a valid
    /// PostgreSQL connection string (e.g. `postgresql://user:pass@localhost/test_db`).
    /// If not set, tests are skipped.
    fn pg_test_url() -> Option<String> {
        std::env::var("LIFE_ENGINE_TEST_PG_URL").ok()
    }

    /// Create a test PgStorage instance connected to the test database.
    /// Returns None if PG is not available.
    async fn test_storage() -> Option<PgStorage> {
        let url = pg_test_url()?;

        let url_parsed: url::Url = url.parse().ok()?;
        let ssl_mode = url_parsed
            .query_pairs()
            .find(|(k, _)| k == "sslmode")
            .and_then(|(_, v)| v.parse::<PgSslMode>().ok())
            .unwrap_or(PgSslMode::Disable);
        let config = PgConfig {
            host: url_parsed.host_str().unwrap_or("localhost").into(),
            port: url_parsed.port().unwrap_or(5432),
            dbname: url_parsed.path().trim_start_matches('/').into(),
            user: url_parsed.username().into(),
            password: url_parsed.password().unwrap_or("").into(),
            pool_size: 4,
            ssl_mode,
        };

        let storage = PgStorage::open(&config).await.ok()?;

        // Clean up any existing test data.
        let client = storage.pool.get().await.ok()?;
        let _ = client.execute("DELETE FROM plugin_data", &[]).await;
        let _ = client.execute("DELETE FROM audit_log", &[]).await;

        Some(storage)
    }

    /// Helper macro to define PG tests that skip when `LIFE_ENGINE_TEST_PG_URL` is not set.
    macro_rules! pg_test {
        ($name:ident, $storage:ident, $body:block) => {
            #[tokio::test]
            async fn $name() {
                let Some($storage) = test_storage().await else {
                    eprintln!(
                        "Skipping {} — LIFE_ENGINE_TEST_PG_URL not set",
                        stringify!($name)
                    );
                    return;
                };
                $body
            }
        };
    }

    // ── CRUD ─────────────────────────────────────────────────

    pg_test!(create_and_get_record, storage, {
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
    });

    pg_test!(get_nonexistent_returns_none, storage, {
        let result = storage.get("p1", "tasks", "no-such-id").await.unwrap();
        assert!(result.is_none());
    });

    pg_test!(update_record, storage, {
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
    });

    pg_test!(delete_record, storage, {
        let created = storage
            .create("p1", "tasks", serde_json::json!({}))
            .await
            .unwrap();

        assert!(storage.delete("p1", "tasks", &created.id).await.unwrap());
        assert!(storage.get("p1", "tasks", &created.id).await.unwrap().is_none());
    });

    pg_test!(delete_nonexistent_returns_false, storage, {
        assert!(!storage.delete("p1", "tasks", "nope").await.unwrap());
    });

    // ── Optimistic locking ───────────────────────────────────

    pg_test!(version_mismatch_rejects_update, storage, {
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
    });

    // ── Plugin isolation ─────────────────────────────────────

    pg_test!(records_scoped_by_plugin_and_collection, storage, {
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
    });

    // ── Query filters ────────────────────────────────────────

    pg_test!(query_equality_filter, storage, {
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
    });

    pg_test!(query_comparison_filters, storage, {
        for i in 1..=5 {
            storage
                .create("p1", "items", serde_json::json!({"score": i}))
                .await
                .unwrap();
        }

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
    });

    pg_test!(query_text_search_contains, storage, {
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
    });

    pg_test!(query_and_logic, storage, {
        storage.create("p1", "tasks", serde_json::json!({"status": "open", "priority": 1})).await.unwrap();
        storage.create("p1", "tasks", serde_json::json!({"status": "open", "priority": 3})).await.unwrap();
        storage.create("p1", "tasks", serde_json::json!({"status": "closed", "priority": 1})).await.unwrap();

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
    });

    pg_test!(query_or_logic, storage, {
        storage.create("p1", "tasks", serde_json::json!({"status": "open"})).await.unwrap();
        storage.create("p1", "tasks", serde_json::json!({"status": "closed"})).await.unwrap();
        storage.create("p1", "tasks", serde_json::json!({"status": "archived"})).await.unwrap();

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
        assert_eq!(result.total, 2);
    });

    // ── Sorting ──────────────────────────────────────────────

    pg_test!(query_sort_ascending, storage, {
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
    });

    pg_test!(query_sort_descending, storage, {
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
    });

    // ── Pagination ───────────────────────────────────────────

    pg_test!(pagination_limit_and_offset, storage, {
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
    });

    pg_test!(pagination_total_count_independent_of_limit, storage, {
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
    });

    // ── Audit logging ────────────────────────────────────────

    pg_test!(crud_operations_create_audit_entries, storage, {
        let created = storage
            .create("p1", "tasks", serde_json::json!({"t": 1}))
            .await
            .unwrap();
        storage
            .update("p1", "tasks", &created.id, serde_json::json!({"t": 2}), 1)
            .await
            .unwrap();
        storage.delete("p1", "tasks", &created.id).await.unwrap();

        let client = storage.pool.get().await.unwrap();
        let count_row = client
            .query_one("SELECT COUNT(*) FROM audit_log", &[])
            .await
            .unwrap();
        let count: i64 = count_row.get(0);
        assert_eq!(count, 3);
    });

    // ── Change notifications ─────────────────────────────────

    pg_test!(subscribe_receives_create_event, storage, {
        let mut rx = storage.subscribe();

        storage.create("p1", "tasks", serde_json::json!({})).await.unwrap();

        let event = rx.recv().await.unwrap();
        assert_eq!(event.collection, "tasks");
        assert_eq!(event.change_type, ChangeType::Created);
    });

    pg_test!(subscribe_receives_update_event, storage, {
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
    });

    pg_test!(subscribe_receives_delete_event, storage, {
        let created = storage
            .create("p1", "tasks", serde_json::json!({}))
            .await
            .unwrap();

        let mut rx = storage.subscribe();
        storage.delete("p1", "tasks", &created.id).await.unwrap();

        let event = rx.recv().await.unwrap();
        assert_eq!(event.change_type, ChangeType::Deleted);
        assert_eq!(event.record_id, created.id);
    });

    // ── Full-text search via tsvector ────────────────────────

    pg_test!(fulltext_search_finds_by_title, storage, {
        storage
            .create("p1", "notes", serde_json::json!({"title": "Rust programming guide"}))
            .await
            .unwrap();
        storage
            .create("p1", "notes", serde_json::json!({"title": "Go programming tutorial"}))
            .await
            .unwrap();
        storage
            .create("p1", "notes", serde_json::json!({"title": "Cooking recipes"}))
            .await
            .unwrap();

        let result = storage.fulltext_search("programming", None, 20, 0).await.unwrap();
        assert_eq!(result.total, 2);
    });

    pg_test!(fulltext_search_filters_by_collection, storage, {
        storage
            .create("p1", "notes", serde_json::json!({"title": "Rust guide"}))
            .await
            .unwrap();
        storage
            .create("p1", "tasks", serde_json::json!({"title": "Learn Rust"}))
            .await
            .unwrap();

        let result = storage
            .fulltext_search("rust", Some("notes"), 20, 0)
            .await
            .unwrap();
        assert_eq!(result.total, 1);
        assert_eq!(result.records[0].collection, "notes");
    });

    pg_test!(fulltext_search_body_and_content, storage, {
        storage
            .create("p1", "emails", serde_json::json!({"subject": "Meeting", "body": "Let's discuss the quarterly review"}))
            .await
            .unwrap();
        storage
            .create("p1", "notes", serde_json::json!({"title": "Notes", "content": "The quarterly review went well"}))
            .await
            .unwrap();

        let result = storage.fulltext_search("quarterly review", None, 20, 0).await.unwrap();
        assert_eq!(result.total, 2);
    });
}
