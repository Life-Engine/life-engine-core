# ADR-004: SQLite and SQLCipher as default storage

## Status
Accepted

## Context

Life Engine is a self-hosted personal data sovereignty platform. Users store emails, contacts, calendar events, tasks, files, credentials, and notes. This data is sensitive. Storage must be:

- Encrypted at rest, so that if the host machine is compromised (stolen disk, unauthorised access to file system), raw data cannot be read.
- Local-first, meaning the database lives on the user's own hardware without requiring a separate database server process.
- Accessible from both the Rust Core process and the Tauri App client (the App maintains its own local read replica for offline-first performance).
- Capable of structured querying (relational joins, full-text search, indexing) rather than a raw key-value store.
- Embeddable in a single binary deployment — no separate database daemon to install or manage.

Additionally, the storage layer must be abstracted behind a trait so that alternative backends (PostgreSQL for power users with existing database infrastructure, S3 for archival storage) can be added in future phases without changing Core's business logic.

## Decision

The primary storage backend for Core and for the App's local replica is SQLite, accessed via the `rusqlite` crate. The `SQLCipher` feature of `rusqlite` is enabled, providing transparent full-database AES-256-CBC encryption. The encryption key is derived from the user's master password using Argon2id, so the encrypted database is useless without the password.

Credentials stored inside the database receive a second layer of individual encryption even within the already-encrypted database file, following the Defence in Depth principle. OAuth refresh tokens are encrypted at rest; access tokens are held in memory only and never written to disk.

The storage layer is abstracted behind a `StorageAdapter` Rust trait so that Core code never calls SQLite APIs directly. The concrete `SqliteAdapter` struct implements this trait. Future adapters (PostgreSQL, read-through S3) implement the same trait without requiring changes to Core.

## Consequences

Positive consequences:

- No external database server required. Self-hosters install one binary, not a binary plus a database daemon.
- SQLCipher encryption is transparent to application code — queries are identical to unencrypted SQLite.
- Argon2id key derivation means brute-forcing the encryption key from the database file alone requires cracking the master password.
- SQLite's file-based model means backups are as simple as copying a file.
- The `rusqlite` crate is mature, widely used, and has excellent `SQLCipher` support.
- Full-text search (FTS5) is available as a SQLite extension, enabling search across emails, notes, and contacts without a separate search engine.
- SQLite supports WAL mode, enabling concurrent reads while a write is in progress, which is important for the App's local replica pattern.

Negative consequences:

- SQLite is not designed for high-concurrency write workloads. A single writer per database at a time is a constraint. For personal use this is acceptable; for a shared family engine with many concurrent sync operations, write throughput could become a bottleneck in later phases.
- SQLCipher is a fork of SQLite maintained by Zetetic. It tracks upstream SQLite with some lag and introduces a dependency on a third-party fork rather than the canonical SQLite source.
- The `StorageAdapter` abstraction adds an indirection layer. Contributors must work through the trait interface rather than writing SQL directly, which can feel limiting for complex queries.
- Migrating the database schema requires a migration framework (e.g., `rusqlite_migration`). Migration errors on user databases are hard to reverse without a backup.

## Alternatives Considered

**PostgreSQL** is a production-grade relational database with better concurrency and replication support. It was rejected as the default because it requires a separate server process, is complex to configure for non-technical self-hosters, and conflicts with the single-binary deployment goal. PostgreSQL remains available as an optional backend via the `StorageAdapter` trait for users who already have PostgreSQL infrastructure.

**Firestore / cloud databases** were not considered for the primary store as they contradict the data sovereignty goal. Storing user data in a cloud service controlled by a third party is the exact problem Life Engine is designed to avoid.

**Custom file format** (e.g., a JSON file per collection) was evaluated for simplicity. It was rejected because custom formats lack indexing, relational queries, transactional writes, and the full-text search capabilities needed by the email and notes features. Re-implementing these features on top of a custom format is reinventing the database wheel.

**SurrealDB or EdgeDB** were evaluated as modern embedded databases. Both were rejected because their Rust SDKs were less mature than `rusqlite` at evaluation time, and their query models introduce novel paradigms that contributors would need to learn alongside everything else.
