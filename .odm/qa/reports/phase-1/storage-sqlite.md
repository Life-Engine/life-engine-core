# Storage-SQLite Crate Review

Reviewer: Storage and Database Expert
Date: 2026-03-28
Scope: `packages/storage-sqlite/` and related app-level storage files

## Summary

The `storage-sqlite` crate is a well-structured, security-conscious SQLite/SQLCipher storage backend. The architecture follows sound principles: encrypted-at-rest via SQLCipher, per-credential defence-in-depth encryption via AES-256-GCM, WAL journal mode for concurrent reads, append-only audit logging with retention cleanup, and JSON Schema validation on all writes. Test coverage is comprehensive across all modules.

Several issues were identified, ranging from a critical SQL injection vector in the migration executor to moderate design concerns around single-connection concurrency and missing transaction boundaries. The crate is in a strong position but needs targeted fixes before production use.

## File-by-File Analysis

### Cargo.toml

The dependency set is appropriate. `rusqlite` for SQLite access, `jsonschema` for validation, `zeroize` for key material cleanup, `tar`/`flate2` for export archives. No unnecessary dependencies observed. The `sha2` crate is used in `blob_fs.rs` but is not listed in `Cargo.toml` -- this will cause a compile error unless it comes transitively through another dependency (likely `life-engine-crypto`).

### src/lib.rs

Well-designed entry point. The `SqliteStorage` struct properly encapsulates the connection, private schema registry, and master key. Key security practices are present:

- `master_key` is zeroized on `Drop`
- The `master_key()` accessor is `pub(crate)` -- not leaked to external consumers
- The `rekey()` method correctly verifies the new key works before updating the in-memory copy
- SQLCipher PRAGMA key is set immediately after connection open, before any other statement

Issues found:

- **PRAGMA key quoting** (line 77): The format string `PRAGMA key = 'x"{hex_key}"';` produces `PRAGMA key = 'x"abcd..."';`. The correct SQLCipher syntax is `PRAGMA key = "x'abcd...'";` (double quotes outside, single quotes around the hex literal). The current quoting may work with some SQLCipher versions but is non-standard and fragile. The same issue exists at line 147 for `PRAGMA rekey`.
- **No connection pooling**: A single `Connection` means all operations are serialized. This is acknowledged but worth tracking as an architectural concern for multi-plugin concurrent access.

### src/backend.rs

The core read/write implementation. This is the largest and most complex file.

Positive observations:

- Parameterized queries throughout -- all user-supplied values go through `?N` bind parameters
- Optimistic concurrency via `WHERE version = ?` on updates
- Schema validation runs on every insert and update before the data touches the database
- Credential encryption is applied transparently before storage
- Audit logging for credential operations (read, insert, update, delete)
- Extension merge logic preserves existing extensions on update when the caller omits them
- `MAX_LIMIT` of 1000 prevents unbounded result sets

Issues found:

- **Sort field injection** (line 183): The `ORDER BY` clause interpolates `s.field` directly into the SQL string via `format!("json_extract(data, '$.{}') {}", s.field, dir)`. While `dir` is constrained to "ASC"/"DESC" by the enum, `s.field` comes from the caller and is not sanitized. A malicious field value like `x') ; DROP TABLE plugin_data; --` would result in SQL injection. This should use parameterized binding for the JSON path, consistent with how filters are handled.
- **No transaction wrapping on mutations** (lines 259-421): The doc comment says "Each mutation is wrapped in a transaction" but the code does not actually call `BEGIN`/`COMMIT`. For inserts this is a single statement so autocommit suffices, but for updates that include extension merging (a read then a write), there is a TOCTOU race: another writer could modify the record between the extension read and the update. A real transaction (`conn.execute("BEGIN IMMEDIATE", [])` ... `conn.execute("COMMIT", [])`) would fix this.
- **Delete does not check collection scope**: The delete mutation uses `WHERE id = ?1 AND plugin_id = ?2` but does not filter by `collection`. A plugin could delete a record from a different collection if it knows the ID. Adding `AND collection = ?3` would enforce stricter scoping.
- **Delete silently succeeds on miss**: When `rows_affected == 0` for a delete, the function returns `Ok(())`. This means deleting a non-existent record is not distinguishable from a successful delete. For credential deletes, the audit event is correctly skipped, but the caller gets no signal that nothing happened.
- **`version` cast** (line 353): `expected_version as i64` performs a silent truncation if `expected_version` is a `u64` greater than `i64::MAX`. Use `i64::try_from(expected_version)` and return an error on overflow.

### src/schema.rs

Clean, well-documented DDL definitions. All tables use `IF NOT EXISTS` for idempotent creation. Indexes are appropriate for the query patterns used elsewhere.

Issues found:

- **No `updated_at` index on `plugin_data`**: Queries that sort or filter by `updated_at` (common for sync/polling patterns) will require a full table scan. Consider adding `CREATE INDEX IF NOT EXISTS idx_plugin_data_updated ON plugin_data(updated_at)`.
- **Text-based timestamps**: All timestamp columns use `TEXT` rather than a numeric format. While RFC 3339 strings sort correctly as text, this prevents efficient range queries and date arithmetic at the SQLite level. This is a design choice that trades query flexibility for simplicity -- acceptable for now but worth noting.
- **No foreign key relationships**: The `quarantine` and `migration_log` tables reference `plugin_id` and `collection` but have no FK constraints to `plugin_data` or `schema_versions`. This is likely intentional (quarantined records may not exist in plugin_data), but worth documenting.

### src/error.rs

Comprehensive error enum with proper `thiserror` derivation and `EngineError` trait implementation. Error codes are unique and severity mapping is reasonable (NotFound = Warning, ConcurrencyConflict = Retryable, everything else = Fatal).

No issues found.

### src/audit.rs

Solid append-only audit log implementation.

Positive observations:

- No public update or delete-by-id functions -- append-only by design
- Parameterized queries throughout
- Cleanup uses the configured retention period
- Query supports flexible filtering by event type and time range
- All 16 event types round-trip correctly through string representation

Issues found:

- **`unwrap_or` fallback in cleanup** (line 260): `checked_sub_signed(...).unwrap_or_else(Utc::now)` means that if the subtraction overflows (impossible for 90 days, but defensive code should handle the general case), cleanup would delete nothing instead of everything. The current behavior is safe but the fallback masks potential bugs.
- **No pagination on query_events**: The function returns all matching entries. For a database with months of audit data, this could return tens of thousands of rows. A `LIMIT`/`OFFSET` parameter would be prudent.
- **Dynamic SQL assembly in query_events**: While the parameters are properly bound via `?N`, the WHERE clause construction uses string formatting for the parameter indices. This is safe because the indices are computed from `params.len()`, not from user input, but it adds unnecessary complexity. Consider using a fixed query with optional parameters bound as NULL.

### src/export.rs

Well-implemented multi-format export. Supports JSON, iCalendar, vCard, and mbox formats. Audit events are logged on export.

Positive observations:

- All queries use parameterized statements
- Export format detection is based on collection name, not user input
- Tar archive is properly finalized and compressed
- Audit logging on export is best-effort (doesn't fail the export if audit write fails)

Issues found:

- **Column index offset bug** (lines 185-187): In `query_all_plugin_data`, after reading `data_str` at index 3, the code uses `row.get(3 + 1)`, `row.get(3 + 2)`, `row.get(3 + 3)` for version, created_at, updated_at. While this produces correct indices (4, 5, 6), the `3 + N` pattern is fragile and confusing. The same pattern appears in `query_plugin_data_by_plugin`. Use explicit indices (4, 5, 6) for clarity.
- **`to_ical_datetime` is fragile**: The function strips all non-digit, non-T, non-Z characters and truncates. This works for simple RFC 3339 timestamps but will produce incorrect output for timestamps with timezone offsets like `+05:30` (the digits from the offset would be included). Consider using `chrono::DateTime::parse_from_rfc3339` for robust parsing.
- **No LIMIT on full export queries**: `query_all_plugin_data` and `query_all_audit_log` load all rows into memory. For large databases, this could cause OOM. Consider streaming or chunked processing.
- **Error variant misuse**: Several errors use `StorageError::InitFailed` for tar/gzip failures. A more appropriate variant like `StorageError::Io` (which already exists) would be more semantically correct.

### src/blob_fs.rs

Filesystem-based blob storage with atomic writes, SHA-256 checksums, and metadata sidecars.

Positive observations:

- Atomic write via temp-file-then-rename prevents partial files
- Checksum verification on read detects corruption
- Sidecar rollback if metadata write fails after blob write
- `created_at` is preserved on overwrite
- Health check probes both existence and writability of the root directory

Issues found:

- **`sha2` crate not in Cargo.toml**: The file imports `sha2::{Digest, Sha256}` but `sha2` is not listed in `Cargo.toml`. This presumably compiles through a transitive dependency, but it should be an explicit dependency for correctness and to prevent breakage if the transitive path changes.
- **`mime_guess` crate not in Cargo.toml**: Same issue -- `mime_guess::from_path` is used but not declared as a direct dependency.
- **TOCTOU in delete** (line 198): `if !blob_path.exists()` followed by `std::fs::remove_file` has a race condition where the file could be deleted between the check and the remove. Use `remove_file` directly and handle `NotFound` from the error.
- **Path traversal risk in `blob_path`** (line 36): `self.root.join(key.as_str())` joins the key directly to the root path. If `BlobKey` validation does not prevent `..` segments, a malicious key like `../../etc/passwd` could escape the blob root. Verify that `BlobKey::new()` rejects path traversal sequences. If it doesn't, add a check here.
- **Non-atomic two-file write** (lines 156-161): The blob data and metadata sidecar are written as two separate atomic operations. If the process crashes after writing the blob but before writing the sidecar, the blob will exist without metadata. On next read, the metadata lookup will fail with NotFound even though the blob data exists. Consider writing both to a staging directory and renaming the directory atomically, or accepting this as a known limitation.
- **`async` functions doing synchronous I/O**: All the `BlobStorageAdapter` methods are `async fn` but perform blocking filesystem operations (`std::fs::read`, `std::fs::write`, `std::fs::remove_file`). In a Tokio runtime, this will block the executor thread. These should use `tokio::fs` or `spawn_blocking`.

### src/health.rs

Clean health check implementation with four independent checks (connectivity, WAL mode, encryption, file size).

No significant issues found. The design correctly reports Degraded (not Unhealthy) when SQLCipher is not available, which is appropriate for development environments using plain SQLite.

### src/credentials.rs

Defence-in-depth per-credential encryption using HMAC-derived keys and AES-256-GCM.

Positive observations:

- Each credential gets a unique derived key via `HMAC(master_key, credential_id)`
- The `encrypted` flag allows backward-compatible migration of unencrypted credentials
- Round-trip tests verify claims are not visible in the encrypted output

Issues found:

- **Derived key is deterministic**: `HMAC(master_key, credential_id)` produces the same key for the same credential every time. This means re-encrypting the same credential produces different ciphertexts (because AES-GCM uses a random nonce), but the key derivation itself is deterministic. This is fine for the current use case but means that if an attacker knows the credential_id and can observe multiple encryptions, they gain no additional information -- which is correct.
- **No key rotation for per-credential keys**: When `SqliteStorage::rekey()` changes the master key, the per-credential derived keys also change (because they're derived from the master key). However, the encrypted credential data stored in the database was encrypted with keys derived from the *old* master key. There is no re-encryption of individual credentials after a rekey. This means that after a master key rotation, reading any credential will fail with a decryption error because the new derived key won't match the old ciphertext. This is a **critical correctness issue** that must be addressed before key rotation is used in production.

### src/validation.rs

JSON Schema validation for canonical and private collections.

Positive observations:

- Schemas are compiled once at startup via `LazyLock`
- Private collection schemas are keyed by `(plugin_id, collection)` for namespace isolation
- Cross-plugin writes to private collections are correctly rejected
- Canonical collection names cannot be registered as private schemas

Issues found:

- **Duplicate `CANONICAL_COLLECTIONS` constant**: This constant is defined in both `validation.rs` (line 26) and `backend.rs` (line 24). If a new canonical collection is added, both must be updated. Extract to a single source of truth (e.g., in `schema.rs` or a shared module).
- **Duplicate `is_canonical` function**: Similarly, `is_canonical()` exists in both `validation.rs` (line 37) and `backend.rs` (line 36). The backend imports from validation in some places but also has its own copy.
- **Only first validation error reported** (line 96): `errors[0].clone()` discards additional validation errors. Consider joining all errors for better diagnostics.

### src/config.rs and src/types.rs

Both files are empty stubs (contain only a module doc comment). These are placeholders for future use and pose no issues.

### src/migration/mod.rs

Simple re-export module. No issues.

### src/migration/executor.rs

Migration executor that creates JSON-extract-based indexes from collection descriptors.

Issues found:

- **SQL injection in index creation** (lines 77-83): The field name `field.name` and descriptor name `descriptor.name` are interpolated directly into the SQL string: `format!("CREATE INDEX IF NOT EXISTS {idx_name} ON plugin_data(collection, json_extract(data, '$.{field}'))")`. If a plugin provides a malicious field name like `x')); DROP TABLE plugin_data; --`, this becomes an SQL injection attack. Since migration descriptors come from plugin manifests, this is an attack surface for malicious plugins. The same issue applies to composite index creation (lines 97-103) and index name construction (lines 76, 92). Field names and collection names must be validated to contain only alphanumeric characters and underscores.
- **Index name collision**: The index name `idx_col_{collection}_{field}` could collide if two collections have names that produce the same concatenated string. This is unlikely but theoretically possible.

### src/migration/quarantine.rs

Correct and well-tested quarantine implementation. All queries use parameterized statements. CRUD operations are complete (insert, list, retry, delete).

No issues found.

### src/migration/log.rs

Clean migration log implementation. All queries are parameterized. The `log_failure` convenience function correctly records zero counts.

No issues found.

### src/migration/backup.rs

Pre-migration backup using SQLite's online backup API with integrity verification.

Positive observations:

- Uses SQLite's backup API (non-blocking for concurrent readers)
- Verifies backup with `PRAGMA integrity_check` after creation
- Removes corrupt backups automatically
- Restore verifies backup integrity before overwriting

Issues found:

- **Backup does not handle encrypted databases**: `create_backup` opens the source database with `Connection::open(db_path)` without setting the SQLCipher key. For an encrypted database, this will produce a backup of the encrypted file without being able to verify it. The `PRAGMA integrity_check` on the backup will fail because the backup connection doesn't have the key. The backup function needs access to the encryption key to properly open the source and verify the backup.
- **Restore uses `std::fs::copy`** (line 93): This is a simple file copy, not SQLite's backup API. For an encrypted database this is fine (it copies the encrypted bytes), but it doesn't handle the case where the database is currently open and has WAL transactions pending. Restoring while the database is open could corrupt it.
- **No backup retention/cleanup**: Old backups accumulate in the `backups/` directory with no cleanup mechanism. Over time this could consume significant disk space.

### src/migration/version.rs

Schema version tracking with `stamp_version` for per-record migration stamping.

Issues found:

- **Semver parsing is naive** (line 63-67): `new_version.split('.').next()` extracts only the major version number. This means "2.0.0" and "2.5.0" both stamp as version 2. If migrations need to distinguish minor versions, this will lose information.

### src/tests/mod.rs

Comprehensive integration tests covering initialization, key verification, rekey, WAL mode, foreign keys, idempotent schema creation, and credential encryption. Tests use `tempfile::TempDir` for isolation.

No issues found. The test coverage is thorough.

## App-Level Storage Files

### apps/core/src/sqlite_storage.rs

This is a separate, app-level `SqliteStorage` that wraps a `Mutex<Connection>` and implements a `StorageAdapter` trait. Key observations:

- **Schema divergence from the crate**: The app-level DDL includes `user_id` and `household_id` columns not present in the crate's schema. The app-level `audit_log.details` column is nullable (no `NOT NULL`), while the crate's schema makes it `NOT NULL`. The app-level schema also lacks the `quarantine`, `migration_log`, and `schema_versions` tables.
- **PRAGMA key quoting differs**: The app uses `PRAGMA key = "x'{hex_key}'"` (double-quote outside, single-quote inside) while the crate uses `PRAGMA key = 'x"{hex_key}"'` (single-quote outside, double-quote inside). Only one of these is correct per the SQLCipher docs; they should be consistent.
- **Mutex-based concurrency**: Uses `Arc<Mutex<Connection>>` which serializes all access. This is documented with suggestions for r2d2 pooling.

### apps/core/src/storage_migration.rs

SQLite-to-PostgreSQL migration with atomic transactions, batch processing, and record count verification. Well-structured with progress callbacks. Uses `ON CONFLICT DO NOTHING` for idempotent migration.

- **Offset truncation** (line 100): `offset as u32` truncates a `u64` offset to `u32`, which could skip records if a collection has more than ~4 billion records. Unlikely but technically incorrect.

### apps/core/src/pg_storage.rs

PostgreSQL storage adapter with connection pooling, TLS support, full-text search via tsvector, and proper schema creation. Defaults to TLS-required which is the correct security posture.

No significant issues in the portions reviewed.

## Problems Summary

### Critical

- **SQL injection in migration executor**: Field names and collection names from plugin manifests are interpolated directly into DDL statements (`src/migration/executor.rs` lines 77-83, 97-103). A malicious plugin could execute arbitrary SQL.
- **Sort field injection in backend queries**: The `ORDER BY` clause in `execute_query` interpolates `s.field` directly into SQL (`src/backend.rs` line 183). Callers providing untrusted sort fields can inject SQL.
- **Credential re-encryption missing after rekey**: After `SqliteStorage::rekey()`, per-credential derived keys change but stored ciphertext remains encrypted under old keys (`src/credentials.rs` + `src/lib.rs`). Reading any credential after a master key rotation will fail.

### Major

- **Backup does not handle encrypted databases**: `create_backup` opens the database without the encryption key, causing integrity check failures on encrypted databases (`src/migration/backup.rs` lines 32-53).
- **PRAGMA key quoting inconsistency**: The SQLCipher PRAGMA key syntax differs between the crate (`'x"..."'`) and the app-level code (`"x'...'"`) and may not match the correct SQLCipher syntax (`src/lib.rs` line 77, `apps/core/src/sqlite_storage.rs` line 127).
- **Missing transaction boundaries on mutations**: The `execute_mutation` method does not wrap operations in explicit transactions, creating TOCTOU races for updates with extension merging (`src/backend.rs` lines 259-421).
- **Schema divergence between crate and app**: The `packages/storage-sqlite` schema and the `apps/core/src/sqlite_storage.rs` schema define different columns (`user_id`, `household_id`), different nullability constraints, and different table sets. This will cause confusion and potential data issues during migration.
- **Async functions performing blocking I/O in blob_fs**: All `BlobStorageAdapter` methods use synchronous `std::fs` operations inside `async fn`, blocking the Tokio executor thread (`src/blob_fs.rs`).

### Minor

- **Duplicate `CANONICAL_COLLECTIONS` constant**: Defined in both `validation.rs` and `backend.rs`. Should be a single source of truth.
- **Duplicate `is_canonical` function**: Exists in both modules.
- **Missing `sha2` and `mime_guess` in Cargo.toml**: Used in `blob_fs.rs` but not declared as direct dependencies.
- **Column index readability** in `export.rs`: Uses `3 + 1`, `3 + 2`, `3 + 3` instead of explicit indices 4, 5, 6.
- **`to_ical_datetime` fragility**: Will produce incorrect output for timestamps with timezone offsets.
- **No pagination on `query_events`**: Returns all matching audit entries without limit.
- **No `updated_at` index on `plugin_data`**: Missing index for a likely query pattern.
- **Error variant misuse in export**: Uses `InitFailed` for I/O errors that should use the `Io` variant.
- **Delete does not filter by collection**: The delete mutation in backend.rs does not include `collection` in the WHERE clause.
- **Version cast truncation**: `expected_version as i64` silently truncates on overflow.
- **TOCTOU in blob delete**: Checks existence before attempting removal.
- **Path traversal risk in blob keys**: `blob_path` joins user-provided keys without verifying absence of `..` segments.
- **No backup retention/cleanup**: Old pre-migration backups accumulate without cleanup.
- **Naive semver parsing in stamp_version**: Only extracts the major version number.

## Recommendations

1. **Fix SQL injection vectors immediately**: Validate field names and collection names in the migration executor to allow only `[a-zA-Z0-9_]` characters. For the sort field in `execute_query`, either use parameterized JSON path binding or validate against an allowlist.

2. **Implement credential re-encryption on rekey**: When the master key changes, iterate all records in the `credentials` collection, decrypt with old-key-derived keys, and re-encrypt with new-key-derived keys, all within a transaction.

3. **Standardize PRAGMA key syntax**: Audit both the crate and app-level code against the SQLCipher documentation and use a single, correct quoting format everywhere.

4. **Pass encryption key to backup functions**: The `create_backup` function should accept the encryption key to properly open and verify encrypted databases.

5. **Wrap mutations in explicit transactions**: Use `BEGIN IMMEDIATE` / `COMMIT` for update operations that involve read-then-write patterns (extension merging).

6. **Consolidate schema definitions**: Either have the app-level storage derive its schema from the crate, or document and version the differences. The current divergence will cause migration failures.

7. **Use `tokio::fs` or `spawn_blocking` in blob_fs**: Replace all synchronous `std::fs` calls in async methods with their async equivalents.

8. **Add input validation for blob keys**: Reject keys containing `..`, absolute paths, or null bytes before constructing filesystem paths.

9. **Extract `CANONICAL_COLLECTIONS` to a single location**: Move to `schema.rs` or a shared constants module and import from both `validation.rs` and `backend.rs`.

10. **Add pagination to `query_events`**: Accept `limit` and `offset` parameters to prevent unbounded result sets on large audit logs.
