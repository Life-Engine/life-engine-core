//! Database schema definitions and migrations.

/// DDL for the `plugin_data` table — the universal document store.
///
/// All plugin data is stored in this single table. Rows are scoped by
/// `plugin_id` and `collection`. The `data` column holds JSON-serialized
/// records, queryable via SQLite's `json_extract()`.
///
/// Indexes:
/// - `idx_plugin_collection` — composite index for per-plugin collection queries.
/// - `idx_collection` — single-column index for cross-plugin canonical queries.
pub const PLUGIN_DATA_DDL: &str = "\
CREATE TABLE IF NOT EXISTS plugin_data (
    id          TEXT PRIMARY KEY,
    plugin_id   TEXT NOT NULL,
    collection  TEXT NOT NULL,
    data        TEXT NOT NULL,
    version     INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_plugin_collection
    ON plugin_data(plugin_id, collection);

CREATE INDEX IF NOT EXISTS idx_collection
    ON plugin_data(collection);
";

/// DDL for the `audit_log` table — append-only security event log.
///
/// Records security-relevant events such as authentication attempts,
/// credential access, plugin lifecycle events, and data exports.
/// Entries are retained for 90 days before cleanup.
///
/// Index:
/// - `idx_audit_timestamp` — for time-range queries and retention cleanup.
pub const AUDIT_LOG_DDL: &str = "\
CREATE TABLE IF NOT EXISTS audit_log (
    id          TEXT PRIMARY KEY,
    timestamp   TEXT NOT NULL,
    event_type  TEXT NOT NULL,
    plugin_id   TEXT,
    details     TEXT NOT NULL,
    created_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_timestamp
    ON audit_log(timestamp);
";

/// Retention period for audit log entries, in days.
pub const AUDIT_RETENTION_DAYS: u32 = 90;

/// DDL for the `quarantine` table — records that failed migration transforms.
///
/// When an individual record cannot be migrated (WASM transform failure or
/// schema validation failure), it is stored here for admin review and retry.
///
/// Index:
/// - `idx_quarantine_plugin_collection` — for listing quarantined records per plugin/collection.
pub const QUARANTINE_DDL: &str = "\
CREATE TABLE IF NOT EXISTS quarantine (
    id              TEXT PRIMARY KEY,
    record_data     TEXT NOT NULL,
    plugin_id       TEXT NOT NULL,
    collection      TEXT NOT NULL,
    from_version    TEXT NOT NULL,
    to_version      TEXT NOT NULL,
    error_message   TEXT NOT NULL,
    timestamp       TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_quarantine_plugin_collection
    ON quarantine(plugin_id, collection);
";

/// DDL for the `migration_log` table — records every migration run.
///
/// Each entry captures the plugin, collection, version range, outcome counts,
/// timing, and backup path for auditability. Failures that prevent execution
/// entirely (as opposed to per-record failures in quarantine) are also logged
/// here with zero counts and an error message in `error`.
///
/// Index:
/// - `idx_migration_log_plugin_collection` — for querying migration history per plugin/collection.
pub const MIGRATION_LOG_DDL: &str = "\
CREATE TABLE IF NOT EXISTS migration_log (
    id                    TEXT PRIMARY KEY,
    plugin_id             TEXT NOT NULL,
    collection            TEXT NOT NULL,
    from_version          TEXT NOT NULL,
    to_version            TEXT NOT NULL,
    records_migrated      INTEGER NOT NULL,
    records_quarantined   INTEGER NOT NULL,
    duration_ms           INTEGER NOT NULL,
    backup_path           TEXT,
    error                 TEXT,
    timestamp             TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_migration_log_plugin_collection
    ON migration_log(plugin_id, collection);
";

/// DDL for the `schema_versions` table — tracks the current schema version
/// for each canonical and plugin-owned collection.
///
/// During startup, Core compares each collection's stored version against
/// the declared version in the types crate. If the stored version is behind,
/// canonical migration transforms run through the migration execution engine.
///
/// Index:
/// - Primary key on `(plugin_id, collection)` — one version per plugin/collection pair.
pub const SCHEMA_VERSIONS_DDL: &str = "\
CREATE TABLE IF NOT EXISTS schema_versions (
    plugin_id   TEXT NOT NULL,
    collection  TEXT NOT NULL,
    version     INTEGER NOT NULL DEFAULT 1,
    updated_at  TEXT NOT NULL,
    PRIMARY KEY (plugin_id, collection)
);
";

/// All schema DDL statements in application order.
pub const ALL_DDL: &[&str] = &[
    PLUGIN_DATA_DDL,
    AUDIT_LOG_DDL,
    QUARANTINE_DDL,
    MIGRATION_LOG_DDL,
    SCHEMA_VERSIONS_DDL,
];

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn plugin_data_ddl_creates_table_and_indexes() {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(PLUGIN_DATA_DDL)
            .expect("plugin_data DDL should execute without error");

        // Verify table exists with expected columns.
        let columns: Vec<String> = conn
            .prepare("PRAGMA table_info(plugin_data)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(
            columns,
            vec!["id", "plugin_id", "collection", "data", "version", "created_at", "updated_at"]
        );

        // Verify indexes exist.
        let indexes: Vec<String> = conn
            .prepare("PRAGMA index_list(plugin_data)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert!(indexes.contains(&"idx_plugin_collection".to_string()));
        assert!(indexes.contains(&"idx_collection".to_string()));
    }

    #[test]
    fn audit_log_ddl_creates_table_and_index() {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(AUDIT_LOG_DDL)
            .expect("audit_log DDL should execute without error");

        // Verify table exists with expected columns.
        let columns: Vec<String> = conn
            .prepare("PRAGMA table_info(audit_log)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(
            columns,
            vec!["id", "timestamp", "event_type", "plugin_id", "details", "created_at"]
        );

        // Verify timestamp index exists.
        let indexes: Vec<String> = conn
            .prepare("PRAGMA index_list(audit_log)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert!(indexes.contains(&"idx_audit_timestamp".to_string()));
    }

    #[test]
    fn all_ddl_applies_idempotently() {
        let conn = Connection::open_in_memory().expect("open in-memory db");

        // Apply twice — should succeed both times due to IF NOT EXISTS.
        for ddl in ALL_DDL {
            conn.execute_batch(ddl).expect("DDL should apply");
        }
        for ddl in ALL_DDL {
            conn.execute_batch(ddl).expect("DDL should apply idempotently");
        }
    }

    #[test]
    fn plugin_data_version_defaults_to_one() {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(PLUGIN_DATA_DDL).unwrap();

        conn.execute(
            "INSERT INTO plugin_data (id, plugin_id, collection, data, created_at, updated_at) \
             VALUES ('test-id', 'plugin-a', 'events', '{}', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();

        let version: i64 = conn
            .query_row("SELECT version FROM plugin_data WHERE id = 'test-id'", [], |row| {
                row.get(0)
            })
            .unwrap();

        assert_eq!(version, 1);
    }

    #[test]
    fn quarantine_ddl_creates_table_and_index() {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(QUARANTINE_DDL)
            .expect("quarantine DDL should execute without error");

        let columns: Vec<String> = conn
            .prepare("PRAGMA table_info(quarantine)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(
            columns,
            vec![
                "id",
                "record_data",
                "plugin_id",
                "collection",
                "from_version",
                "to_version",
                "error_message",
                "timestamp"
            ]
        );

        let indexes: Vec<String> = conn
            .prepare("PRAGMA index_list(quarantine)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert!(indexes.contains(&"idx_quarantine_plugin_collection".to_string()));
    }

    #[test]
    fn migration_log_ddl_creates_table_and_index() {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(MIGRATION_LOG_DDL)
            .expect("migration_log DDL should execute without error");

        let columns: Vec<String> = conn
            .prepare("PRAGMA table_info(migration_log)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(
            columns,
            vec![
                "id",
                "plugin_id",
                "collection",
                "from_version",
                "to_version",
                "records_migrated",
                "records_quarantined",
                "duration_ms",
                "backup_path",
                "error",
                "timestamp"
            ]
        );

        let indexes: Vec<String> = conn
            .prepare("PRAGMA index_list(migration_log)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert!(indexes.contains(&"idx_migration_log_plugin_collection".to_string()));
    }

    #[test]
    fn schema_versions_ddl_creates_table() {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(SCHEMA_VERSIONS_DDL)
            .expect("schema_versions DDL should execute without error");

        let columns: Vec<String> = conn
            .prepare("PRAGMA table_info(schema_versions)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(columns, vec!["plugin_id", "collection", "version", "updated_at"]);
    }

    #[test]
    fn audit_log_plugin_id_is_nullable() {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(AUDIT_LOG_DDL).unwrap();

        // Insert with NULL plugin_id (e.g., auth events).
        conn.execute(
            "INSERT INTO audit_log (id, timestamp, event_type, plugin_id, details, created_at) \
             VALUES ('audit-1', '2026-01-01T00:00:00Z', 'auth_success', NULL, '{}', '2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();

        let plugin_id: Option<String> = conn
            .query_row("SELECT plugin_id FROM audit_log WHERE id = 'audit-1'", [], |row| {
                row.get(0)
            })
            .unwrap();

        assert!(plugin_id.is_none());
    }
}
