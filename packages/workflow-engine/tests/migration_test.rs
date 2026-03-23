//! Integration tests for the migration execution engine.
//!
//! These tests exercise the full migration pipeline end-to-end: backup creation,
//! WASM transform execution, record version stamping, quarantine of failures,
//! migration logging, chain migrations, backup restore, and idempotency.

use std::path::Path;

use rusqlite::Connection;
use semver::Version;
use tempfile::TempDir;

use life_engine_storage_sqlite::migration::backup::restore_backup;
use life_engine_storage_sqlite::schema::{MIGRATION_LOG_DDL, PLUGIN_DATA_DDL, QUARANTINE_DDL};
use life_engine_workflow_engine::migration::{run_migrations, MigrationEntry};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn setup_db(dir: &Path) -> std::path::PathBuf {
    let db_path = dir.join("test.db");
    let conn = Connection::open(&db_path).unwrap();
    conn.execute_batch(PLUGIN_DATA_DDL).unwrap();
    conn.execute_batch(QUARANTINE_DDL).unwrap();
    conn.execute_batch(MIGRATION_LOG_DDL).unwrap();
    db_path
}

fn insert_record(
    db_path: &Path,
    id: &str,
    plugin_id: &str,
    collection: &str,
    data: &str,
    version: i64,
) {
    let conn = Connection::open(db_path).unwrap();
    conn.execute(
        "INSERT INTO plugin_data (id, plugin_id, collection, data, version, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            id,
            plugin_id,
            collection,
            data,
            version,
            "2026-01-01T00:00:00Z",
            "2026-01-01T00:00:00Z"
        ],
    )
    .unwrap();
}

fn record_version(db_path: &Path, id: &str) -> i64 {
    let conn = Connection::open(db_path).unwrap();
    conn.query_row(
        "SELECT version FROM plugin_data WHERE id = ?1",
        rusqlite::params![id],
        |row| row.get(0),
    )
    .unwrap()
}

fn record_data(db_path: &Path, id: &str) -> String {
    let conn = Connection::open(db_path).unwrap();
    conn.query_row(
        "SELECT data FROM plugin_data WHERE id = ?1",
        rusqlite::params![id],
        |row| row.get(0),
    )
    .unwrap()
}

fn count_rows(db_path: &Path, table: &str) -> i64 {
    let conn = Connection::open(db_path).unwrap();
    conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
        row.get(0)
    })
    .unwrap()
}

/// Identity WASM transform: copies input to output unchanged.
/// Exports a single function with the given name.
fn identity_wasm(export_name: &str) -> Vec<u8> {
    let wat = format!(
        r#"
        (module
            (import "extism:host/env" "input_length" (func $input_length (result i64)))
            (import "extism:host/env" "input_load_u8" (func $input_load_u8 (param i64) (result i32)))
            (import "extism:host/env" "output_set" (func $output_set (param i64 i64)))
            (import "extism:host/env" "alloc" (func $alloc (param i64) (result i64)))
            (import "extism:host/env" "store_u8" (func $store_u8 (param i64 i32)))

            (memory (export "memory") 1)

            (func (export "{export_name}") (result i32)
                (local $len i64)
                (local $offset i64)
                (local $i i64)
                (local $byte i32)

                (local.set $len (call $input_length))
                (local.set $offset (call $alloc (local.get $len)))

                (local.set $i (i64.const 0))
                (block $break
                    (loop $loop
                        (br_if $break (i64.ge_u (local.get $i) (local.get $len)))
                        (local.set $byte (call $input_load_u8 (local.get $i)))
                        (call $store_u8
                            (i64.add (local.get $offset) (local.get $i))
                            (local.get $byte)
                        )
                        (local.set $i (i64.add (local.get $i) (i64.const 1)))
                        (br $loop)
                    )
                )

                (call $output_set (local.get $offset) (local.get $len))
                (i32.const 0)
            )
        )
        "#,
    );
    wat::parse_str(&wat).expect("failed to compile WAT to WASM")
}

/// Identity WASM with multiple export names (for chain migration tests).
fn multi_export_identity_wasm(export_names: &[&str]) -> Vec<u8> {
    let funcs: String = export_names
        .iter()
        .map(|name| {
            format!(
                r#"
            (func (export "{name}") (result i32)
                (local $len i64)
                (local $offset i64)
                (local $i i64)
                (local $byte i32)

                (local.set $len (call $input_length))
                (local.set $offset (call $alloc (local.get $len)))

                (local.set $i (i64.const 0))
                (block $break
                    (loop $loop
                        (br_if $break (i64.ge_u (local.get $i) (local.get $len)))
                        (local.set $byte (call $input_load_u8 (local.get $i)))
                        (call $store_u8
                            (i64.add (local.get $offset) (local.get $i))
                            (local.get $byte)
                        )
                        (local.set $i (i64.add (local.get $i) (i64.const 1)))
                        (br $loop)
                    )
                )

                (call $output_set (local.get $offset) (local.get $len))
                (i32.const 0)
            )
            "#
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let wat = format!(
        r#"
        (module
            (import "extism:host/env" "input_length" (func $input_length (result i64)))
            (import "extism:host/env" "input_load_u8" (func $input_load_u8 (param i64) (result i32)))
            (import "extism:host/env" "output_set" (func $output_set (param i64 i64)))
            (import "extism:host/env" "alloc" (func $alloc (param i64) (result i64)))
            (import "extism:host/env" "store_u8" (func $store_u8 (param i64 i32)))

            (memory (export "memory") 1)

            {funcs}
        )
        "#,
    );
    wat::parse_str(&wat).expect("failed to compile WAT to WASM")
}

/// WASM that traps unconditionally (for quarantine tests).
fn trapping_wasm(export_name: &str) -> Vec<u8> {
    let wat = format!(
        r#"
        (module
            (memory (export "memory") 1)
            (func (export "{export_name}") (result i32)
                unreachable
            )
        )
        "#,
    );
    wat::parse_str(&wat).expect("failed to compile WAT to WASM")
}

// ---------------------------------------------------------------------------
// Test 1: Simple end-to-end migration
// ---------------------------------------------------------------------------

#[tokio::test]
async fn simple_migration_transforms_records_and_bumps_version() {
    let tmp = TempDir::new().unwrap();
    let data_dir = tmp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    let db_path = setup_db(tmp.path());

    // Insert records at v1.
    insert_record(
        &db_path,
        "r1",
        "test-plugin",
        "events",
        r#"{"title":"Meeting","priority":1}"#,
        1,
    );
    insert_record(
        &db_path,
        "r2",
        "test-plugin",
        "events",
        r#"{"title":"Standup","priority":2}"#,
        1,
    );

    let wasm_path = tmp.path().join("plugin.wasm");
    std::fs::write(&wasm_path, identity_wasm("migrate_v1_to_v2")).unwrap();

    let entries = vec![MigrationEntry {
        from: "1.x".to_string(),
        to: Version::new(2, 0, 0),
        transform: "migrate_v1_to_v2".to_string(),
        description: "Rename title to name".to_string(),
        collection: "events".to_string(),
    }];

    let result = run_migrations(&wasm_path, &entries, "test-plugin", &db_path, &data_dir)
        .await
        .unwrap();

    // Verify migration counts.
    assert_eq!(result.migrated, 2);
    assert_eq!(result.quarantined, 0);
    assert_eq!(result.entries_applied.len(), 1);

    // Verify records are now at version 2.
    assert_eq!(record_version(&db_path, "r1"), 2);
    assert_eq!(record_version(&db_path, "r2"), 2);

    // Verify backup was created.
    assert!(result.backup_path.exists(), "backup file should exist");

    // Verify migration log entry was created.
    assert_eq!(count_rows(&db_path, "migration_log"), 1);

    // Verify no records in quarantine.
    assert_eq!(count_rows(&db_path, "quarantine"), 0);
}

// ---------------------------------------------------------------------------
// Test 2: Quarantine — transform failures are quarantined
// ---------------------------------------------------------------------------

#[tokio::test]
async fn quarantine_records_when_transform_fails() {
    let tmp = TempDir::new().unwrap();
    let data_dir = tmp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    let db_path = setup_db(tmp.path());

    // Insert records at v1 — all will fail because the transform traps.
    insert_record(
        &db_path,
        "r1",
        "test-plugin",
        "events",
        r#"{"name":"Alice"}"#,
        1,
    );
    insert_record(
        &db_path,
        "r2",
        "test-plugin",
        "events",
        r#"{"name":"Bob"}"#,
        1,
    );
    insert_record(
        &db_path,
        "r3",
        "test-plugin",
        "events",
        r#"{"name":"Charlie"}"#,
        1,
    );

    let wasm_path = tmp.path().join("plugin.wasm");
    std::fs::write(&wasm_path, trapping_wasm("migrate_v1_to_v2")).unwrap();

    let entries = vec![MigrationEntry {
        from: "1.x".to_string(),
        to: Version::new(2, 0, 0),
        transform: "migrate_v1_to_v2".to_string(),
        description: "failing migration".to_string(),
        collection: "events".to_string(),
    }];

    let result = run_migrations(&wasm_path, &entries, "test-plugin", &db_path, &data_dir)
        .await
        .unwrap();

    // All records should be quarantined.
    assert_eq!(result.migrated, 0);
    assert_eq!(result.quarantined, 3);

    // Verify quarantine table has all 3 records.
    assert_eq!(count_rows(&db_path, "quarantine"), 3);

    // Verify quarantine entries have correct metadata.
    let conn = Connection::open(&db_path).unwrap();
    let (plugin_id, collection, from_ver, to_ver): (String, String, String, String) = conn
        .query_row(
            "SELECT plugin_id, collection, from_version, to_version FROM quarantine LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();

    assert_eq!(plugin_id, "test-plugin");
    assert_eq!(collection, "events");
    assert_eq!(from_ver, "1.x");
    assert_eq!(to_ver, "2.0.0");
}

// ---------------------------------------------------------------------------
// Test 3: Chain migration — v1 -> v2 -> v3 -> v4
// ---------------------------------------------------------------------------

#[tokio::test]
async fn chain_migration_applies_entries_in_sequence() {
    let tmp = TempDir::new().unwrap();
    let data_dir = tmp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    let db_path = setup_db(tmp.path());

    // Insert records at v1.
    insert_record(
        &db_path,
        "r1",
        "test-plugin",
        "tasks",
        r#"{"task":"buy milk"}"#,
        1,
    );
    insert_record(
        &db_path,
        "r2",
        "test-plugin",
        "tasks",
        r#"{"task":"fix bug"}"#,
        1,
    );

    let wasm_path = tmp.path().join("plugin.wasm");
    std::fs::write(
        &wasm_path,
        multi_export_identity_wasm(&[
            "migrate_v1_to_v2",
            "migrate_v2_to_v3",
            "migrate_v3_to_v4",
        ]),
    )
    .unwrap();

    let entries = vec![
        MigrationEntry {
            from: "1.x".to_string(),
            to: Version::new(2, 0, 0),
            transform: "migrate_v1_to_v2".to_string(),
            description: "v1 to v2".to_string(),
            collection: "tasks".to_string(),
        },
        MigrationEntry {
            from: "2.x".to_string(),
            to: Version::new(3, 0, 0),
            transform: "migrate_v2_to_v3".to_string(),
            description: "v2 to v3".to_string(),
            collection: "tasks".to_string(),
        },
        MigrationEntry {
            from: "3.x".to_string(),
            to: Version::new(4, 0, 0),
            transform: "migrate_v3_to_v4".to_string(),
            description: "v3 to v4".to_string(),
            collection: "tasks".to_string(),
        },
    ];

    let result = run_migrations(&wasm_path, &entries, "test-plugin", &db_path, &data_dir)
        .await
        .unwrap();

    // Each entry migrates 2 records, applied sequentially: v1->v2, v2->v3, v3->v4.
    assert_eq!(result.migrated, 6);
    assert_eq!(result.quarantined, 0);
    assert_eq!(result.entries_applied.len(), 3);

    // Records should be at version 4.
    assert_eq!(record_version(&db_path, "r1"), 4);
    assert_eq!(record_version(&db_path, "r2"), 4);

    // Data should be preserved (identity transforms).
    assert_eq!(record_data(&db_path, "r1"), r#"{"task":"buy milk"}"#);
    assert_eq!(record_data(&db_path, "r2"), r#"{"task":"fix bug"}"#);

    // Migration log should have one entry per migration entry.
    assert_eq!(count_rows(&db_path, "migration_log"), 3);
}

// ---------------------------------------------------------------------------
// Test 4: Backup — verify backup exists and can be restored
// ---------------------------------------------------------------------------

#[tokio::test]
async fn backup_is_created_and_restorable() {
    let tmp = TempDir::new().unwrap();
    let data_dir = tmp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    let db_path = setup_db(tmp.path());

    // Insert a record at v1 with known data.
    insert_record(
        &db_path,
        "r1",
        "test-plugin",
        "events",
        r#"{"original":true}"#,
        1,
    );

    let wasm_path = tmp.path().join("plugin.wasm");
    std::fs::write(&wasm_path, identity_wasm("migrate_v1_to_v2")).unwrap();

    let entries = vec![MigrationEntry {
        from: "1.x".to_string(),
        to: Version::new(2, 0, 0),
        transform: "migrate_v1_to_v2".to_string(),
        description: "test migration".to_string(),
        collection: "events".to_string(),
    }];

    let result = run_migrations(&wasm_path, &entries, "test-plugin", &db_path, &data_dir)
        .await
        .unwrap();

    // After migration, record is at v2.
    assert_eq!(record_version(&db_path, "r1"), 2);

    // Backup should exist.
    assert!(result.backup_path.exists());

    // Restore the backup.
    restore_backup(&result.backup_path, &db_path).unwrap();

    // After restore, record should be back at v1.
    assert_eq!(record_version(&db_path, "r1"), 1);
    assert_eq!(record_data(&db_path, "r1"), r#"{"original":true}"#);
}

// ---------------------------------------------------------------------------
// Test 5: Idempotency — running the same migration twice has no effect
// ---------------------------------------------------------------------------

#[tokio::test]
async fn idempotent_migration_does_not_duplicate() {
    let tmp = TempDir::new().unwrap();
    let data_dir = tmp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    let db_path = setup_db(tmp.path());

    insert_record(
        &db_path,
        "r1",
        "test-plugin",
        "events",
        r#"{"name":"Alice"}"#,
        1,
    );
    insert_record(
        &db_path,
        "r2",
        "test-plugin",
        "events",
        r#"{"name":"Bob"}"#,
        1,
    );

    let wasm_path = tmp.path().join("plugin.wasm");
    std::fs::write(&wasm_path, identity_wasm("migrate_v1_to_v2")).unwrap();

    let entries = vec![MigrationEntry {
        from: "1.x".to_string(),
        to: Version::new(2, 0, 0),
        transform: "migrate_v1_to_v2".to_string(),
        description: "test migration".to_string(),
        collection: "events".to_string(),
    }];

    // First run: should migrate 2 records.
    let result1 = run_migrations(&wasm_path, &entries, "test-plugin", &db_path, &data_dir)
        .await
        .unwrap();
    assert_eq!(result1.migrated, 2);

    // Second run: records are already at v2, so no v1 records match.
    let result2 = run_migrations(&wasm_path, &entries, "test-plugin", &db_path, &data_dir)
        .await
        .unwrap();
    assert_eq!(result2.migrated, 0);
    assert_eq!(result2.quarantined, 0);
    assert!(result2.entries_applied.is_empty());

    // Records should still be at v2 — not double-bumped.
    assert_eq!(record_version(&db_path, "r1"), 2);
    assert_eq!(record_version(&db_path, "r2"), 2);

    // Total plugin_data rows unchanged (no duplicates created).
    assert_eq!(count_rows(&db_path, "plugin_data"), 2);
}

// ---------------------------------------------------------------------------
// Test 6: Mixed collections — migrations only affect target collection
// ---------------------------------------------------------------------------

#[tokio::test]
async fn migration_only_affects_target_collection() {
    let tmp = TempDir::new().unwrap();
    let data_dir = tmp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    let db_path = setup_db(tmp.path());

    // Insert records in two different collections.
    insert_record(
        &db_path,
        "event-1",
        "test-plugin",
        "events",
        r#"{"type":"meeting"}"#,
        1,
    );
    insert_record(
        &db_path,
        "task-1",
        "test-plugin",
        "tasks",
        r#"{"type":"todo"}"#,
        1,
    );

    let wasm_path = tmp.path().join("plugin.wasm");
    std::fs::write(&wasm_path, identity_wasm("migrate_v1_to_v2")).unwrap();

    // Migration targets only the "events" collection.
    let entries = vec![MigrationEntry {
        from: "1.x".to_string(),
        to: Version::new(2, 0, 0),
        transform: "migrate_v1_to_v2".to_string(),
        description: "events only".to_string(),
        collection: "events".to_string(),
    }];

    let result = run_migrations(&wasm_path, &entries, "test-plugin", &db_path, &data_dir)
        .await
        .unwrap();

    assert_eq!(result.migrated, 1);

    // Events collection record should be at v2.
    assert_eq!(record_version(&db_path, "event-1"), 2);

    // Tasks collection record should remain at v1 — untouched.
    assert_eq!(record_version(&db_path, "task-1"), 1);
}

// ---------------------------------------------------------------------------
// Test 7: Different plugins — migrations are scoped to plugin_id
// ---------------------------------------------------------------------------

#[tokio::test]
async fn migration_is_scoped_to_plugin_id() {
    let tmp = TempDir::new().unwrap();
    let data_dir = tmp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    let db_path = setup_db(tmp.path());

    // Same collection, different plugins.
    insert_record(
        &db_path,
        "r1",
        "plugin-a",
        "events",
        r#"{"source":"a"}"#,
        1,
    );
    insert_record(
        &db_path,
        "r2",
        "plugin-b",
        "events",
        r#"{"source":"b"}"#,
        1,
    );

    let wasm_path = tmp.path().join("plugin.wasm");
    std::fs::write(&wasm_path, identity_wasm("migrate_v1_to_v2")).unwrap();

    let entries = vec![MigrationEntry {
        from: "1.x".to_string(),
        to: Version::new(2, 0, 0),
        transform: "migrate_v1_to_v2".to_string(),
        description: "plugin-a migration".to_string(),
        collection: "events".to_string(),
    }];

    // Run migration only for plugin-a.
    let result = run_migrations(&wasm_path, &entries, "plugin-a", &db_path, &data_dir)
        .await
        .unwrap();

    assert_eq!(result.migrated, 1);

    // plugin-a record migrated to v2.
    assert_eq!(record_version(&db_path, "r1"), 2);

    // plugin-b record untouched at v1.
    assert_eq!(record_version(&db_path, "r2"), 1);
}

// ---------------------------------------------------------------------------
// Test 8: No matching records — migration completes with zero counts
// ---------------------------------------------------------------------------

#[tokio::test]
async fn no_matching_records_completes_gracefully() {
    let tmp = TempDir::new().unwrap();
    let data_dir = tmp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    let db_path = setup_db(tmp.path());

    // Records at v2, but migration targets v1.
    insert_record(
        &db_path,
        "r1",
        "test-plugin",
        "events",
        r#"{"name":"Alice"}"#,
        2,
    );

    let wasm_path = tmp.path().join("plugin.wasm");
    std::fs::write(&wasm_path, identity_wasm("migrate_v1_to_v2")).unwrap();

    let entries = vec![MigrationEntry {
        from: "1.x".to_string(),
        to: Version::new(2, 0, 0),
        transform: "migrate_v1_to_v2".to_string(),
        description: "no-op migration".to_string(),
        collection: "events".to_string(),
    }];

    let result = run_migrations(&wasm_path, &entries, "test-plugin", &db_path, &data_dir)
        .await
        .unwrap();

    assert_eq!(result.migrated, 0);
    assert_eq!(result.quarantined, 0);
    assert!(result.entries_applied.is_empty());

    // Backup is still created (before we know there are no records).
    assert!(result.backup_path.exists());
}
