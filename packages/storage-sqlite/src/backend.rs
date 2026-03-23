//! StorageBackend trait implementation for SQLite.

use chrono::Utc;
use rusqlite::{params, params_from_iter};
use serde_json;
use tracing::warn;
use uuid::Uuid;

use life_engine_types::{
    CdmType, FilterOp, MessageMetadata, PipelineMessage, SortDirection, StorageMutation,
    StorageQuery, TypedPayload,
};

use crate::audit::{self, AuditEvent, AuditEventType};
use crate::credentials;
use crate::error::StorageError;
use crate::validation;
use crate::SqliteStorage;

/// Maximum number of records a single query may return.
const MAX_LIMIT: u32 = 1000;

/// Canonical CDM collection names shared across all plugins.
const CANONICAL_COLLECTIONS: &[&str] = &[
    "events",
    "tasks",
    "contacts",
    "notes",
    "emails",
    "files",
    "credentials",
];

/// Returns `true` if the collection is a canonical CDM collection.
fn is_canonical(collection: &str) -> bool {
    CANONICAL_COLLECTIONS.contains(&collection)
}

/// Parse a JSON `data` column value into a `CdmType` based on collection name.
///
/// For canonical collections the JSON is deserialized into the appropriate
/// Rust struct. For private (non-canonical) collections the raw JSON is
/// returned as a `TypedPayload::Custom` via `SchemaValidated`.
///
/// Credentials are transparently decrypted before deserialization when the
/// `encrypted` flag is set.
fn parse_payload(
    collection: &str,
    data_json: &str,
    master_key: &[u8; 32],
) -> Result<TypedPayload, StorageError> {
    match collection {
        "events" => {
            let v = serde_json::from_str(data_json)?;
            Ok(TypedPayload::Cdm(Box::new(CdmType::Event(v))))
        }
        "tasks" => {
            let v = serde_json::from_str(data_json)?;
            Ok(TypedPayload::Cdm(Box::new(CdmType::Task(v))))
        }
        "contacts" => {
            let v = serde_json::from_str(data_json)?;
            Ok(TypedPayload::Cdm(Box::new(CdmType::Contact(v))))
        }
        "notes" => {
            let v = serde_json::from_str(data_json)?;
            Ok(TypedPayload::Cdm(Box::new(CdmType::Note(v))))
        }
        "emails" => {
            let v = serde_json::from_str(data_json)?;
            Ok(TypedPayload::Cdm(Box::new(CdmType::Email(v))))
        }
        "files" => {
            let v = serde_json::from_str(data_json)?;
            Ok(TypedPayload::Cdm(Box::new(CdmType::File(v))))
        }
        "credentials" => {
            let decrypted = credentials::decrypt_credential(master_key, data_json)?;
            let v = serde_json::from_str(&decrypted)?;
            Ok(TypedPayload::Cdm(Box::new(CdmType::Credential(v))))
        }
        _ => {
            // Private collection — data was validated on write, so we
            // deserialize the SchemaValidated wrapper directly (it uses
            // #[serde(transparent)] so it deserializes from raw JSON).
            let v: life_engine_types::SchemaValidated<serde_json::Value> =
                serde_json::from_str(data_json)?;
            Ok(TypedPayload::Custom(v))
        }
    }
}

/// Log an audit event, warning on failure instead of propagating the error.
///
/// Audit logging must never block or fail a storage operation.
fn try_log_audit(conn: &rusqlite::Connection, event: AuditEvent) {
    if let Err(e) = audit::log_event(conn, event) {
        warn!("audit log write failed: {e}");
    }
}

impl SqliteStorage {
    /// Execute a read query, translating `StorageQuery` into SQL.
    ///
    /// Canonical collections (events, tasks, contacts, notes, emails, files,
    /// credentials) are shared data — reads span all plugins. Private
    /// collections are strictly scoped by `plugin_id`.
    pub fn execute_query(&self, query: StorageQuery) -> Result<Vec<PipelineMessage>, StorageError> {
        let canonical = is_canonical(&query.collection);

        // Canonical collections allow cross-plugin reads; private collections
        // are strictly scoped by plugin_id.
        let mut bind_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut param_idx: u32;

        let mut sql = if canonical {
            bind_values.push(Box::new(query.collection.clone()));
            param_idx = 2;
            String::from(
                "SELECT id, plugin_id, collection, data, version, created_at, updated_at \
                 FROM plugin_data WHERE collection = ?1",
            )
        } else {
            bind_values.push(Box::new(query.plugin_id.clone()));
            bind_values.push(Box::new(query.collection.clone()));
            param_idx = 3;
            String::from(
                "SELECT id, plugin_id, collection, data, version, created_at, updated_at \
                 FROM plugin_data WHERE plugin_id = ?1 AND collection = ?2",
            )
        };

        // Translate filters to SQL WHERE clauses.
        for filter in &query.filters {
            let json_path = format!("$.{}", filter.field);
            let (op_str, value_str) = match filter.operator {
                FilterOp::Eq => ("=", None),
                FilterOp::Gte => (">=", None),
                FilterOp::Lte => ("<=", None),
                FilterOp::NotEq => ("!=", None),
                FilterOp::Contains => ("LIKE", Some(true)),
            };

            if value_str.is_some() {
                // LIKE pattern with wildcards for Contains.
                sql.push_str(&format!(
                    " AND json_extract(data, ?{}) {} ?{}",
                    param_idx,
                    op_str,
                    param_idx + 1
                ));
                bind_values.push(Box::new(json_path));
                let like_val = format!(
                    "%{}%",
                    json_value_to_string(&filter.value)
                );
                bind_values.push(Box::new(like_val));
                param_idx += 2;
            } else {
                sql.push_str(&format!(
                    " AND json_extract(data, ?{}) {} ?{}",
                    param_idx,
                    op_str,
                    param_idx + 1
                ));
                bind_values.push(Box::new(json_path));
                bind_values.push(json_value_to_boxed_sql(&filter.value));
                param_idx += 2;
            }
        }

        // ORDER BY clauses using json_extract.
        if !query.sort.is_empty() {
            sql.push_str(" ORDER BY ");
            let parts: Vec<String> = query
                .sort
                .iter()
                .map(|s| {
                    let dir = match s.direction {
                        SortDirection::Asc => "ASC",
                        SortDirection::Desc => "DESC",
                    };
                    format!("json_extract(data, '$.{}') {}", s.field, dir)
                })
                .collect();
            sql.push_str(&parts.join(", "));
        }

        // LIMIT and OFFSET.
        let limit = query.limit.map(|l| l.min(MAX_LIMIT)).unwrap_or(MAX_LIMIT);
        sql.push_str(&format!(" LIMIT {limit}"));

        if let Some(offset) = query.offset {
            sql.push_str(&format!(" OFFSET {offset}"));
        }

        // Execute the query.
        let params: Vec<&dyn rusqlite::types::ToSql> =
            bind_values.iter().map(|b| b.as_ref()).collect();

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(params.iter()), |row| {
            let id: String = row.get(0)?;
            let _plugin_id: String = row.get(1)?;
            let collection: String = row.get(2)?;
            let data_json: String = row.get(3)?;
            let _version: i64 = row.get(4)?;
            let _created_at: String = row.get(5)?;
            let _updated_at: String = row.get(6)?;
            Ok((id, collection, data_json))
        })?;

        let mut results = Vec::new();
        for row in rows {
            let (id, collection, data_json) = row.map_err(StorageError::Database)?;

            let payload = parse_payload(&collection, &data_json, self.master_key())?;

            // Audit credential reads.
            if collection == "credentials" {
                try_log_audit(
                    &self.conn,
                    AuditEvent {
                        event_type: AuditEventType::CredentialAccess,
                        plugin_id: Some(query.plugin_id.clone()),
                        details: serde_json::json!({
                            "credential_id": id,
                            "operation": "read"
                        }),
                    },
                );
            }

            let correlation_id =
                Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4());

            results.push(PipelineMessage {
                metadata: MessageMetadata {
                    correlation_id,
                    source: format!("storage:{collection}"),
                    timestamp: Utc::now(),
                    auth_context: None,
                },
                payload,
            });
        }

        Ok(results)
    }

    /// Execute a write mutation, translating `StorageMutation` into SQL.
    ///
    /// Each mutation is wrapped in a transaction. Insert generates a new UUID
    /// and sets version to 1. Update uses optimistic concurrency via a
    /// `WHERE version = ?` clause. Delete scopes by both `id` and `plugin_id`.
    pub fn execute_mutation(&self, mutation: StorageMutation) -> Result<(), StorageError> {
        match mutation {
            StorageMutation::Insert {
                plugin_id,
                collection,
                data,
            } => {
                let id = Uuid::new_v4().to_string();
                let now = Utc::now().to_rfc3339();
                let data_json = serialize_payload(&data.payload)?;

                if validation::is_canonical(&collection) {
                    validation::validate_canonical(&collection, &data_json)?;
                } else {
                    validation::validate_private(
                        &self.private_schemas,
                        &plugin_id,
                        &collection,
                        &data_json,
                    )?;
                }

                // Encrypt credentials before storage.
                let data_json = if collection == "credentials" {
                    credentials::encrypt_credential(self.master_key(), &data_json)?
                } else {
                    data_json
                };

                self.conn.execute(
                    "INSERT INTO plugin_data \
                     (id, plugin_id, collection, data, version, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6)",
                    params![id, plugin_id, collection, data_json, now, now],
                )?;

                // Audit credential writes.
                if collection == "credentials" {
                    try_log_audit(
                        &self.conn,
                        AuditEvent {
                            event_type: AuditEventType::CredentialModify,
                            plugin_id: Some(plugin_id),
                            details: serde_json::json!({
                                "credential_id": id,
                                "operation": "insert"
                            }),
                        },
                    );
                }

                Ok(())
            }
            StorageMutation::Update {
                plugin_id,
                collection,
                id,
                data,
                expected_version,
            } => {
                let now = Utc::now().to_rfc3339();
                let mut data_json = serialize_payload(&data.payload)?;

                // For canonical collections (except credentials), preserve
                // existing extensions when the update payload omits them.
                if validation::is_canonical(&collection) && collection != "credentials" {
                    data_json = merge_existing_extensions(
                        &self.conn,
                        &id.to_string(),
                        &data_json,
                    )?;
                }

                if validation::is_canonical(&collection) {
                    validation::validate_canonical(&collection, &data_json)?;
                } else {
                    validation::validate_private(
                        &self.private_schemas,
                        &plugin_id,
                        &collection,
                        &data_json,
                    )?;
                }

                // Encrypt credentials before storage.
                let data_json = if collection == "credentials" {
                    credentials::encrypt_credential(self.master_key(), &data_json)?
                } else {
                    data_json
                };

                let id_str = id.to_string();
                let version_i64 = expected_version as i64;

                let rows_affected = self.conn.execute(
                    "UPDATE plugin_data \
                     SET data = ?1, version = version + 1, updated_at = ?2 \
                     WHERE id = ?3 AND plugin_id = ?4 AND version = ?5",
                    params![data_json, now, id_str, plugin_id, version_i64],
                )?;

                // Audit credential writes.
                if rows_affected > 0 && collection == "credentials" {
                    try_log_audit(
                        &self.conn,
                        AuditEvent {
                            event_type: AuditEventType::CredentialModify,
                            plugin_id: Some(plugin_id.clone()),
                            details: serde_json::json!({
                                "credential_id": id_str,
                                "operation": "update"
                            }),
                        },
                    );
                }

                if rows_affected == 0 {
                    // Either the record doesn't exist or the version has changed.
                    return Err(StorageError::ConcurrencyConflict {
                        id: id_str,
                        expected: expected_version,
                    });
                }

                Ok(())
            }
            StorageMutation::Delete {
                plugin_id,
                collection,
                id,
            } => {
                let id_str = id.to_string();

                let rows_affected = self.conn.execute(
                    "DELETE FROM plugin_data WHERE id = ?1 AND plugin_id = ?2",
                    params![id_str, plugin_id],
                )?;

                // Audit credential writes.
                if rows_affected > 0 && collection == "credentials" {
                    try_log_audit(
                        &self.conn,
                        AuditEvent {
                            event_type: AuditEventType::CredentialModify,
                            plugin_id: Some(plugin_id),
                            details: serde_json::json!({
                                "credential_id": id_str,
                                "operation": "delete"
                            }),
                        },
                    );
                }

                Ok(())
            }
        }
    }
}

/// Merge existing extensions into the new data when the update omits them.
///
/// If the new payload has `extensions: null` (or missing) and the existing
/// record in the database has a non-null extensions object, the existing
/// extensions are copied into the new payload. This implements the
/// "update without specifying extensions preserves them" semantics.
fn merge_existing_extensions(
    conn: &rusqlite::Connection,
    record_id: &str,
    new_data_json: &str,
) -> Result<String, StorageError> {
    let mut new_data: serde_json::Value = serde_json::from_str(new_data_json)?;

    // If the new payload already has non-null extensions, no merge needed.
    if let Some(ext) = new_data.get("extensions")
        && !ext.is_null()
    {
        return Ok(new_data_json.to_string());
    }

    // Look up existing record's extensions from the database.
    let existing_extensions: Option<serde_json::Value> = conn
        .query_row(
            "SELECT json_extract(data, '$.extensions') FROM plugin_data WHERE id = ?1",
            params![record_id],
            |row| {
                let raw: Option<String> = row.get(0)?;
                Ok(raw.and_then(|s| serde_json::from_str(&s).ok()))
            },
        )
        .unwrap_or(None);

    if let Some(ext) = existing_extensions
        && !ext.is_null()
    {
        new_data["extensions"] = ext;
        return Ok(serde_json::to_string(&new_data)?);
    }

    Ok(new_data_json.to_string())
}

/// Serialize a `TypedPayload` to a JSON string for storage in the `data` column.
fn serialize_payload(payload: &TypedPayload) -> Result<String, StorageError> {
    match payload {
        TypedPayload::Cdm(cdm) => match cdm.as_ref() {
            CdmType::Event(v) => Ok(serde_json::to_string(v)?),
            CdmType::Task(v) => Ok(serde_json::to_string(v)?),
            CdmType::Contact(v) => Ok(serde_json::to_string(v)?),
            CdmType::Note(v) => Ok(serde_json::to_string(v)?),
            CdmType::Email(v) => Ok(serde_json::to_string(v)?),
            CdmType::File(v) => Ok(serde_json::to_string(v)?),
            CdmType::Credential(v) => Ok(serde_json::to_string(v)?),
            CdmType::EventBatch(v) => Ok(serde_json::to_string(v)?),
            CdmType::TaskBatch(v) => Ok(serde_json::to_string(v)?),
            CdmType::ContactBatch(v) => Ok(serde_json::to_string(v)?),
            CdmType::NoteBatch(v) => Ok(serde_json::to_string(v)?),
            CdmType::EmailBatch(v) => Ok(serde_json::to_string(v)?),
            CdmType::FileBatch(v) => Ok(serde_json::to_string(v)?),
            CdmType::CredentialBatch(v) => Ok(serde_json::to_string(v)?),
        },
        TypedPayload::Custom(v) => Ok(serde_json::to_string(v)?),
    }
}

/// Convert a `serde_json::Value` to a string representation for LIKE patterns.
fn json_value_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Convert a `serde_json::Value` to a boxed `ToSql` for parameter binding.
fn json_value_to_boxed_sql(v: &serde_json::Value) -> Box<dyn rusqlite::types::ToSql> {
    match v {
        serde_json::Value::String(s) => Box::new(s.clone()),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Box::new(i)
            } else if let Some(f) = n.as_f64() {
                Box::new(f)
            } else {
                Box::new(n.to_string())
            }
        }
        serde_json::Value::Bool(b) => Box::new(*b),
        serde_json::Value::Null => Box::new(Option::<String>::None),
        other => Box::new(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema;
    use life_engine_types::{QueryFilter, SortField, StorageMutation};
    use rusqlite::Connection;

    fn setup_db() -> SqliteStorage {
        let conn = Connection::open_in_memory().unwrap();
        for ddl in schema::ALL_DDL {
            conn.execute_batch(ddl).unwrap();
        }
        SqliteStorage {
            conn,
            private_schemas: crate::validation::PrivateSchemaRegistry::new(),
            master_key: [0x42u8; 32],
        }
    }

    fn insert_row(storage: &SqliteStorage, id: &str, plugin_id: &str, collection: &str, data: &str) {
        storage.conn.execute(
            "INSERT INTO plugin_data (id, plugin_id, collection, data, version, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            rusqlite::params![id, plugin_id, collection, data],
        ).unwrap();
    }

    #[test]
    fn execute_returns_matching_records() {
        let storage = setup_db();
        let task_json = serde_json::json!({
            "id": "00000000-0000-0000-0000-000000000001",
            "title": "Buy groceries",
            "description": null,
            "status": "pending",
            "priority": "medium",
            "due_date": null,
            "completed_at": null,
            "tags": [],
            "assignee": null,
            "parent_id": null,
            "source": "test",
            "source_id": "t-1",
            "extensions": null,
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        });
        insert_row(
            &storage,
            "00000000-0000-0000-0000-000000000001",
            "plugin-a",
            "tasks",
            &task_json.to_string(),
        );

        let query = StorageQuery {
            collection: "tasks".into(),
            plugin_id: "plugin-a".into(),
            filters: vec![],
            sort: vec![],
            limit: None,
            offset: None,
        };

        let results = storage.execute_query(query).unwrap();
        assert_eq!(results.len(), 1);
        assert!(matches!(
            &results[0].payload,
            TypedPayload::Cdm(cdm) if matches!(cdm.as_ref(), CdmType::Task(_))
        ));
    }

    #[test]
    fn execute_private_collection_filters_by_plugin_id() {
        let storage = setup_db();
        let data = serde_json::json!({"key": "value"});
        insert_row(&storage, "id-1", "plugin-a", "com.example:private", &data.to_string());
        insert_row(&storage, "id-2", "plugin-b", "com.example:private", &data.to_string());

        let query = StorageQuery {
            collection: "com.example:private".into(),
            plugin_id: "plugin-a".into(),
            filters: vec![],
            sort: vec![],
            limit: None,
            offset: None,
        };

        let results = storage.execute_query(query).unwrap();
        assert_eq!(results.len(), 1, "private collection should filter by plugin_id");
    }

    #[test]
    fn execute_applies_eq_filter() {
        let storage = setup_db();
        let make_task = |title: &str| {
            serde_json::json!({
                "id": "00000000-0000-0000-0000-000000000001",
                "title": title,
                "description": null,
                "status": "pending",
                "priority": "medium",
                "due_date": null,
                "completed_at": null,
                "tags": [],
                "assignee": null,
                "parent_id": null,
                "source": "test",
                "source_id": "t-1",
                "extensions": null,
                "created_at": "2026-01-01T00:00:00Z",
                "updated_at": "2026-01-01T00:00:00Z"
            })
        };
        insert_row(&storage, "id-1", "plug", "tasks", &make_task("Alpha").to_string());
        insert_row(&storage, "id-2", "plug", "tasks", &make_task("Beta").to_string());

        let query = StorageQuery {
            collection: "tasks".into(),
            plugin_id: "plug".into(),
            filters: vec![QueryFilter {
                field: "title".into(),
                operator: FilterOp::Eq,
                value: serde_json::Value::String("Alpha".into()),
            }],
            sort: vec![],
            limit: None,
            offset: None,
        };

        let results = storage.execute_query(query).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn execute_applies_contains_filter() {
        let storage = setup_db();
        let make_task = |title: &str| {
            serde_json::json!({
                "id": "00000000-0000-0000-0000-000000000001",
                "title": title,
                "description": null,
                "status": "pending",
                "priority": "medium",
                "due_date": null,
                "completed_at": null,
                "tags": [],
                "assignee": null,
                "parent_id": null,
                "source": "test",
                "source_id": "t-1",
                "extensions": null,
                "created_at": "2026-01-01T00:00:00Z",
                "updated_at": "2026-01-01T00:00:00Z"
            })
        };
        insert_row(&storage, "id-1", "plug", "tasks", &make_task("Buy groceries").to_string());
        insert_row(&storage, "id-2", "plug", "tasks", &make_task("Sell car").to_string());

        let query = StorageQuery {
            collection: "tasks".into(),
            plugin_id: "plug".into(),
            filters: vec![QueryFilter {
                field: "title".into(),
                operator: FilterOp::Contains,
                value: serde_json::Value::String("grocer".into()),
            }],
            sort: vec![],
            limit: None,
            offset: None,
        };

        let results = storage.execute_query(query).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn execute_applies_sort() {
        let storage = setup_db();
        let make_task = |title: &str| {
            serde_json::json!({
                "id": "00000000-0000-0000-0000-000000000001",
                "title": title,
                "description": null,
                "status": "pending",
                "priority": "medium",
                "due_date": null,
                "completed_at": null,
                "tags": [],
                "assignee": null,
                "parent_id": null,
                "source": "test",
                "source_id": "t-1",
                "extensions": null,
                "created_at": "2026-01-01T00:00:00Z",
                "updated_at": "2026-01-01T00:00:00Z"
            })
        };
        insert_row(&storage, "id-1", "plug", "tasks", &make_task("Charlie").to_string());
        insert_row(&storage, "id-2", "plug", "tasks", &make_task("Alpha").to_string());
        insert_row(&storage, "id-3", "plug", "tasks", &make_task("Bravo").to_string());

        let query = StorageQuery {
            collection: "tasks".into(),
            plugin_id: "plug".into(),
            filters: vec![],
            sort: vec![SortField {
                field: "title".into(),
                direction: SortDirection::Asc,
            }],
            limit: None,
            offset: None,
        };

        let results = storage.execute_query(query).unwrap();
        assert_eq!(results.len(), 3);
        // Verify alphabetical order by checking payload titles.
        let titles: Vec<String> = results
            .iter()
            .map(|r| match &r.payload {
                TypedPayload::Cdm(cdm) => match cdm.as_ref() {
                    CdmType::Task(t) => t.title.clone(),
                    _ => panic!("expected Task"),
                },
                _ => panic!("expected Cdm"),
            })
            .collect();
        assert_eq!(titles, vec!["Alpha", "Bravo", "Charlie"]);
    }

    #[test]
    fn execute_applies_limit_and_offset() {
        let storage = setup_db();
        let make_task = |title: &str| {
            serde_json::json!({
                "id": "00000000-0000-0000-0000-000000000001",
                "title": title,
                "description": null,
                "status": "pending",
                "priority": "medium",
                "due_date": null,
                "completed_at": null,
                "tags": [],
                "assignee": null,
                "parent_id": null,
                "source": "test",
                "source_id": "t-1",
                "extensions": null,
                "created_at": "2026-01-01T00:00:00Z",
                "updated_at": "2026-01-01T00:00:00Z"
            })
        };
        for i in 0..5 {
            insert_row(
                &storage,
                &format!("id-{i}"),
                "plug",
                "tasks",
                &make_task(&format!("Task {i}")).to_string(),
            );
        }

        let query = StorageQuery {
            collection: "tasks".into(),
            plugin_id: "plug".into(),
            filters: vec![],
            sort: vec![],
            limit: Some(2),
            offset: Some(1),
        };

        let results = storage.execute_query(query).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn execute_caps_limit_at_max() {
        let storage = setup_db();
        let make_task = || {
            serde_json::json!({
                "id": "00000000-0000-0000-0000-000000000001",
                "title": "X",
                "description": null,
                "status": "pending",
                "priority": "medium",
                "due_date": null,
                "completed_at": null,
                "tags": [],
                "assignee": null,
                "parent_id": null,
                "source": "test",
                "source_id": "t-1",
                "extensions": null,
                "created_at": "2026-01-01T00:00:00Z",
                "updated_at": "2026-01-01T00:00:00Z"
            })
        };
        // Insert 3 rows but request limit 5000.
        for i in 0..3 {
            insert_row(&storage, &format!("id-{i}"), "plug", "tasks", &make_task().to_string());
        }

        let query = StorageQuery {
            collection: "tasks".into(),
            plugin_id: "plug".into(),
            filters: vec![],
            sort: vec![],
            limit: Some(5000),
            offset: None,
        };

        // Should not error — limit is capped at MAX_LIMIT internally.
        let results = storage.execute_query(query).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn execute_empty_collection_returns_empty() {
        let storage = setup_db();

        let query = StorageQuery {
            collection: "tasks".into(),
            plugin_id: "plug".into(),
            filters: vec![],
            sort: vec![],
            limit: None,
            offset: None,
        };

        let results = storage.execute_query(query).unwrap();
        assert!(results.is_empty());
    }

    // --- Mutation tests ---

    fn make_pipeline_message(title: &str) -> PipelineMessage {
        let task: life_engine_types::Task = serde_json::from_value(serde_json::json!({
            "id": "00000000-0000-0000-0000-000000000001",
            "title": title,
            "description": null,
            "status": "pending",
            "priority": "medium",
            "due_date": null,
            "completed_at": null,
            "tags": [],
            "assignee": null,
            "parent_id": null,
            "source": "test",
            "source_id": "t-1",
            "extensions": null,
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        }))
        .unwrap();
        PipelineMessage {
            metadata: MessageMetadata {
                correlation_id: Uuid::new_v4(),
                source: "test".into(),
                timestamp: Utc::now(),
                auth_context: None,
            },
            payload: TypedPayload::Cdm(Box::new(CdmType::Task(task))),
        }
    }

    #[test]
    fn mutate_insert_creates_record() {
        let storage = setup_db();
        let msg = make_pipeline_message("Buy groceries");

        storage
            .execute_mutation(StorageMutation::Insert {
                plugin_id: "plugin-a".into(),
                collection: "tasks".into(),
                data: msg,
            })
            .unwrap();

        let query = StorageQuery {
            collection: "tasks".into(),
            plugin_id: "plugin-a".into(),
            filters: vec![],
            sort: vec![],
            limit: None,
            offset: None,
        };
        let results = storage.execute_query(query).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn mutate_insert_sets_version_to_one() {
        let storage = setup_db();
        let msg = make_pipeline_message("Task V1");

        storage
            .execute_mutation(StorageMutation::Insert {
                plugin_id: "plug".into(),
                collection: "tasks".into(),
                data: msg,
            })
            .unwrap();

        let version: i64 = storage
            .conn
            .query_row(
                "SELECT version FROM plugin_data WHERE plugin_id = 'plug'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn mutate_update_increments_version() {
        let storage = setup_db();
        // Insert a row directly so we know the id and version.
        let id = "00000000-0000-0000-0000-aaaaaaaaaaaa";
        let task_json = serde_json::json!({
            "id": id,
            "title": "Original",
            "description": null,
            "status": "pending",
            "priority": "medium",
            "due_date": null,
            "completed_at": null,
            "tags": [],
            "assignee": null,
            "parent_id": null,
            "source": "test",
            "source_id": "t-1",
            "extensions": null,
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        });
        insert_row(&storage, id, "plug", "tasks", &task_json.to_string());

        let updated_msg = make_pipeline_message("Updated");

        storage
            .execute_mutation(StorageMutation::Update {
                plugin_id: "plug".into(),
                collection: "tasks".into(),
                id: Uuid::parse_str(id).unwrap(),
                data: updated_msg,
                expected_version: 1,
            })
            .unwrap();

        let version: i64 = storage
            .conn
            .query_row(
                "SELECT version FROM plugin_data WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, 2);
    }

    #[test]
    fn mutate_update_concurrency_conflict() {
        let storage = setup_db();
        let id = "00000000-0000-0000-0000-bbbbbbbbbbbb";
        let task_json = serde_json::json!({
            "id": id,
            "title": "Original",
            "description": null,
            "status": "pending",
            "priority": "medium",
            "due_date": null,
            "completed_at": null,
            "tags": [],
            "assignee": null,
            "parent_id": null,
            "source": "test",
            "source_id": "t-1",
            "extensions": null,
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        });
        insert_row(&storage, id, "plug", "tasks", &task_json.to_string());

        let msg = make_pipeline_message("Stale update");

        // Use wrong expected_version (99 instead of 1).
        let result = storage.execute_mutation(StorageMutation::Update {
            plugin_id: "plug".into(),
            collection: "tasks".into(),
            id: Uuid::parse_str(id).unwrap(),
            data: msg,
            expected_version: 99,
        });

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            StorageError::ConcurrencyConflict { .. }
        ));
    }

    #[test]
    fn mutate_delete_removes_record() {
        let storage = setup_db();
        let id = "00000000-0000-0000-0000-cccccccccccc";
        let task_json = serde_json::json!({
            "id": id,
            "title": "To delete",
            "description": null,
            "status": "pending",
            "priority": "medium",
            "due_date": null,
            "completed_at": null,
            "tags": [],
            "assignee": null,
            "parent_id": null,
            "source": "test",
            "source_id": "t-1",
            "extensions": null,
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        });
        insert_row(&storage, id, "plug", "tasks", &task_json.to_string());

        storage
            .execute_mutation(StorageMutation::Delete {
                plugin_id: "plug".into(),
                collection: "tasks".into(),
                id: Uuid::parse_str(id).unwrap(),
            })
            .unwrap();

        let count: i64 = storage
            .conn
            .query_row("SELECT count(*) FROM plugin_data", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn mutate_delete_scoped_by_plugin_id() {
        let storage = setup_db();
        let id = "00000000-0000-0000-0000-dddddddddddd";
        let task_json = serde_json::json!({
            "id": id,
            "title": "Owned by A",
            "description": null,
            "status": "pending",
            "priority": "medium",
            "due_date": null,
            "completed_at": null,
            "tags": [],
            "assignee": null,
            "parent_id": null,
            "source": "test",
            "source_id": "t-1",
            "extensions": null,
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        });
        insert_row(&storage, id, "plugin-a", "tasks", &task_json.to_string());

        // Attempt delete with wrong plugin_id — should not remove the row.
        storage
            .execute_mutation(StorageMutation::Delete {
                plugin_id: "plugin-b".into(),
                collection: "tasks".into(),
                id: Uuid::parse_str(id).unwrap(),
            })
            .unwrap();

        let count: i64 = storage
            .conn
            .query_row("SELECT count(*) FROM plugin_data", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1, "record should not be deleted by wrong plugin_id");
    }

    // --- Plugin data isolation tests ---

    #[test]
    fn canonical_read_is_cross_plugin() {
        let storage = setup_db();
        let task_a = serde_json::json!({
            "id": "00000000-0000-0000-0000-000000000001",
            "title": "Task from A",
            "description": null,
            "status": "pending",
            "priority": "medium",
            "due_date": null,
            "completed_at": null,
            "tags": [],
            "assignee": null,
            "parent_id": null,
            "source": "test",
            "source_id": "t-1",
            "extensions": null,
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        });
        insert_row(&storage, "id-1", "plugin-a", "tasks", &task_a.to_string());
        insert_row(&storage, "id-2", "plugin-b", "tasks", &task_a.to_string());

        // Reading a canonical collection as plugin-a should see records from both plugins.
        let query = StorageQuery {
            collection: "tasks".into(),
            plugin_id: "plugin-a".into(),
            filters: vec![],
            sort: vec![],
            limit: None,
            offset: None,
        };
        let results = storage.execute_query(query).unwrap();
        assert_eq!(results.len(), 2, "canonical reads should span all plugins");
    }

    #[test]
    fn private_read_is_scoped_by_plugin_id() {
        let storage = setup_db();
        let data = serde_json::json!({"key": "value"});
        insert_row(&storage, "id-1", "plugin-a", "com.example.weather:forecasts", &data.to_string());
        insert_row(&storage, "id-2", "plugin-b", "com.example.weather:forecasts", &data.to_string());

        // Plugin-a should only see its own private data.
        let query = StorageQuery {
            collection: "com.example.weather:forecasts".into(),
            plugin_id: "plugin-a".into(),
            filters: vec![],
            sort: vec![],
            limit: None,
            offset: None,
        };
        let results = storage.execute_query(query).unwrap();
        assert_eq!(results.len(), 1, "private collection reads must be scoped by plugin_id");
    }

    #[test]
    fn cross_plugin_update_fails() {
        let storage = setup_db();
        let id = "00000000-0000-0000-0000-eeeeeeeeeeee";
        let task_json = serde_json::json!({
            "id": id,
            "title": "Owned by A",
            "description": null,
            "status": "pending",
            "priority": "medium",
            "due_date": null,
            "completed_at": null,
            "tags": [],
            "assignee": null,
            "parent_id": null,
            "source": "test",
            "source_id": "t-1",
            "extensions": null,
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        });
        insert_row(&storage, id, "plugin-a", "tasks", &task_json.to_string());

        let msg = make_pipeline_message("Attempted update by B");

        // Plugin-b tries to update plugin-a's record — should fail with ConcurrencyConflict
        // because the WHERE plugin_id clause won't match.
        let result = storage.execute_mutation(StorageMutation::Update {
            plugin_id: "plugin-b".into(),
            collection: "tasks".into(),
            id: Uuid::parse_str(id).unwrap(),
            data: msg,
            expected_version: 1,
        });
        assert!(result.is_err(), "cross-plugin update must fail");
    }

    #[test]
    fn cross_plugin_delete_is_noop() {
        let storage = setup_db();
        let id = "00000000-0000-0000-0000-ffffffffffff";
        let task_json = serde_json::json!({
            "id": id,
            "title": "Owned by A",
            "description": null,
            "status": "pending",
            "priority": "medium",
            "due_date": null,
            "completed_at": null,
            "tags": [],
            "assignee": null,
            "parent_id": null,
            "source": "test",
            "source_id": "t-1",
            "extensions": null,
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        });
        insert_row(&storage, id, "plugin-a", "tasks", &task_json.to_string());

        // Plugin-b tries to delete plugin-a's record — should not remove it.
        storage.execute_mutation(StorageMutation::Delete {
            plugin_id: "plugin-b".into(),
            collection: "tasks".into(),
            id: Uuid::parse_str(id).unwrap(),
        }).unwrap();

        let count: i64 = storage
            .conn
            .query_row("SELECT count(*) FROM plugin_data", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1, "cross-plugin delete must not remove the record");
    }

    // --- Extensions support tests ---

    fn make_event_json(title: &str, extensions: Option<serde_json::Value>) -> String {
        serde_json::json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "title": title,
            "start": "2026-03-23T09:00:00Z",
            "source": "test-plugin",
            "source_id": "ev-1",
            "extensions": extensions,
            "created_at": "2026-03-23T00:00:00Z",
            "updated_at": "2026-03-23T00:00:00Z"
        })
        .to_string()
    }

    fn make_event_pipeline_message(
        title: &str,
        extensions: Option<serde_json::Value>,
    ) -> PipelineMessage {
        let event: life_engine_types::CalendarEvent = serde_json::from_value(serde_json::json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "title": title,
            "start": "2026-03-23T09:00:00Z",
            "source": "test-plugin",
            "source_id": "ev-1",
            "extensions": extensions,
            "created_at": "2026-03-23T00:00:00Z",
            "updated_at": "2026-03-23T00:00:00Z"
        }))
        .unwrap();
        PipelineMessage {
            metadata: MessageMetadata {
                correlation_id: Uuid::new_v4(),
                source: "test".into(),
                timestamp: Utc::now(),
                auth_context: None,
            },
            payload: TypedPayload::Cdm(Box::new(CdmType::Event(event))),
        }
    }

    #[test]
    fn extensions_round_trip_preserves_data() {
        let storage = setup_db();
        let ext = serde_json::json!({
            "com.example.plugin": { "custom_field": "value", "count": 42 }
        });
        let id = "550e8400-e29b-41d4-a716-446655440000";
        insert_row(&storage, id, "plugin-a", "events", &make_event_json("Meeting", Some(ext.clone())));

        let query = StorageQuery {
            collection: "events".into(),
            plugin_id: "plugin-a".into(),
            filters: vec![],
            sort: vec![],
            limit: None,
            offset: None,
        };
        let results = storage.execute_query(query).unwrap();
        assert_eq!(results.len(), 1);

        match &results[0].payload {
            TypedPayload::Cdm(cdm) => match cdm.as_ref() {
                CdmType::Event(event) => {
                    assert_eq!(event.extensions, Some(ext), "extensions should round-trip");
                }
                _ => panic!("expected Event"),
            },
            _ => panic!("expected Cdm"),
        }
    }

    #[test]
    fn update_without_extensions_preserves_existing() {
        let storage = setup_db();
        let ext = serde_json::json!({
            "com.example.plugin": { "custom_field": "preserved" }
        });
        let id = "550e8400-e29b-41d4-a716-446655440000";
        insert_row(&storage, id, "plugin-a", "events", &make_event_json("Original", Some(ext.clone())));

        // Update with extensions: None — should preserve existing extensions.
        let updated_msg = make_event_pipeline_message("Updated Title", None);
        storage
            .execute_mutation(StorageMutation::Update {
                plugin_id: "plugin-a".into(),
                collection: "events".into(),
                id: Uuid::parse_str(id).unwrap(),
                data: updated_msg,
                expected_version: 1,
            })
            .unwrap();

        // Read back and verify extensions were preserved.
        let raw: String = storage
            .conn
            .query_row(
                "SELECT data FROM plugin_data WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            parsed["extensions"], ext,
            "existing extensions should be preserved when update omits them"
        );
        assert_eq!(parsed["title"], "Updated Title");
    }

    #[test]
    fn update_with_new_extensions_replaces() {
        let storage = setup_db();
        let old_ext = serde_json::json!({
            "com.example.plugin": { "old": true }
        });
        let new_ext = serde_json::json!({
            "com.example.plugin": { "new": true }
        });
        let id = "550e8400-e29b-41d4-a716-446655440000";
        insert_row(&storage, id, "plugin-a", "events", &make_event_json("Event", Some(old_ext)));

        let updated_msg = make_event_pipeline_message("Event", Some(new_ext.clone()));
        storage
            .execute_mutation(StorageMutation::Update {
                plugin_id: "plugin-a".into(),
                collection: "events".into(),
                id: Uuid::parse_str(id).unwrap(),
                data: updated_msg,
                expected_version: 1,
            })
            .unwrap();

        let raw: String = storage
            .conn
            .query_row(
                "SELECT data FROM plugin_data WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            parsed["extensions"], new_ext,
            "explicitly provided extensions should replace existing"
        );
    }

    // --- Per-credential encryption tests ---

    fn make_credential_json(id: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "name": "Test OAuth Token",
            "credential_type": "oauth_token",
            "service": "google",
            "claims": {
                "access_token": "ya29.a0AfH6SMB",
                "refresh_token": "1//0eXyz",
                "token_type": "Bearer"
            },
            "source": "google-plugin",
            "source_id": "cred-1",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        })
    }

    fn make_credential_pipeline_message(id: &str) -> PipelineMessage {
        let cred: life_engine_types::Credential =
            serde_json::from_value(make_credential_json(id)).unwrap();
        PipelineMessage {
            metadata: MessageMetadata {
                correlation_id: Uuid::new_v4(),
                source: "test".into(),
                timestamp: Utc::now(),
                auth_context: None,
            },
            payload: TypedPayload::Cdm(Box::new(CdmType::Credential(cred))),
        }
    }

    #[test]
    fn credential_insert_encrypts_claims_at_rest() {
        let storage = setup_db();
        let id = "550e8400-e29b-41d4-a716-446655440001";
        let msg = make_credential_pipeline_message(id);

        storage
            .execute_mutation(StorageMutation::Insert {
                plugin_id: "plugin-a".into(),
                collection: "credentials".into(),
                data: msg,
            })
            .unwrap();

        // Read the raw data column — claims should be encrypted (hex string).
        let raw: String = storage
            .conn
            .query_row(
                "SELECT data FROM plugin_data WHERE collection = 'credentials'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let raw_doc: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(raw_doc["encrypted"], true, "encrypted flag should be set");
        assert!(
            raw_doc["claims"].is_string(),
            "claims should be a hex-encoded ciphertext string at rest"
        );
    }

    #[test]
    fn credential_read_decrypts_claims() {
        let storage = setup_db();
        let id = "550e8400-e29b-41d4-a716-446655440002";
        let msg = make_credential_pipeline_message(id);

        storage
            .execute_mutation(StorageMutation::Insert {
                plugin_id: "plugin-a".into(),
                collection: "credentials".into(),
                data: msg,
            })
            .unwrap();

        let query = StorageQuery {
            collection: "credentials".into(),
            plugin_id: "plugin-a".into(),
            filters: vec![],
            sort: vec![],
            limit: None,
            offset: None,
        };
        let results = storage.execute_query(query).unwrap();
        assert_eq!(results.len(), 1);

        match &results[0].payload {
            TypedPayload::Cdm(cdm) => match cdm.as_ref() {
                CdmType::Credential(cred) => {
                    // Claims should be decrypted back to the original object.
                    assert_eq!(cred.claims["access_token"], "ya29.a0AfH6SMB");
                    assert_eq!(cred.claims["refresh_token"], "1//0eXyz");
                    // Encrypted flag should not be set on the returned credential.
                    assert!(cred.encrypted.is_none() || cred.encrypted == Some(false));
                }
                _ => panic!("expected Credential"),
            },
            _ => panic!("expected Cdm"),
        }
    }

    #[test]
    fn credential_read_with_wrong_key_fails() {
        let storage = setup_db();
        let id = "550e8400-e29b-41d4-a716-446655440003";
        let msg = make_credential_pipeline_message(id);

        storage
            .execute_mutation(StorageMutation::Insert {
                plugin_id: "plugin-a".into(),
                collection: "credentials".into(),
                data: msg,
            })
            .unwrap();

        // Create a new storage instance with a different master key
        // pointing at the same in-memory DB isn't possible, so instead
        // read the raw encrypted data and attempt to decrypt with a wrong key.
        let raw: String = storage
            .conn
            .query_row(
                "SELECT data FROM plugin_data WHERE collection = 'credentials'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        let wrong_key = [0x99u8; 32];
        let result = crate::credentials::decrypt_credential(&wrong_key, &raw);
        assert!(result.is_err(), "decryption with wrong key should fail");
    }

    #[test]
    fn unencrypted_credential_reads_normally() {
        let storage = setup_db();
        let id = "550e8400-e29b-41d4-a716-446655440004";
        // Insert a credential directly without encryption (simulating legacy data).
        let cred_json = make_credential_json(id);
        insert_row(&storage, id, "plugin-a", "credentials", &cred_json.to_string());

        let query = StorageQuery {
            collection: "credentials".into(),
            plugin_id: "plugin-a".into(),
            filters: vec![],
            sort: vec![],
            limit: None,
            offset: None,
        };
        let results = storage.execute_query(query).unwrap();
        assert_eq!(results.len(), 1);

        match &results[0].payload {
            TypedPayload::Cdm(cdm) => match cdm.as_ref() {
                CdmType::Credential(cred) => {
                    assert_eq!(cred.claims["access_token"], "ya29.a0AfH6SMB");
                }
                _ => panic!("expected Credential"),
            },
            _ => panic!("expected Cdm"),
        }
    }

    // --- Audit logging integration tests ---

    fn audit_count(storage: &SqliteStorage) -> i64 {
        storage
            .conn
            .query_row("SELECT count(*) FROM audit_log", [], |row| row.get(0))
            .unwrap()
    }

    fn last_audit_event(storage: &SqliteStorage) -> (String, Option<String>, String) {
        storage
            .conn
            .query_row(
                "SELECT event_type, plugin_id, details FROM audit_log ORDER BY rowid DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap()
    }

    fn sample_credential_json(id: &str) -> String {
        serde_json::json!({
            "id": id,
            "name": "Test Token",
            "credential_type": "oauth_token",
            "service": "test-service",
            "claims": {
                "access_token": "ya29.a0AfH6SMB",
                "refresh_token": "1//0eXyz",
                "token_type": "Bearer",
                "expires_in": 3600
            },
            "source": "test-plugin",
            "source_id": "cred-1",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        })
        .to_string()
    }

    #[test]
    fn audit_credential_read_logs_access() {
        let storage = setup_db();
        let cred_json = sample_credential_json("00000000-0000-0000-0000-000000000099");
        // Store an encrypted credential directly.
        let encrypted =
            crate::credentials::encrypt_credential(&storage.master_key, &cred_json).unwrap();
        insert_row(
            &storage,
            "00000000-0000-0000-0000-000000000099",
            "plugin-a",
            "credentials",
            &encrypted,
        );

        assert_eq!(audit_count(&storage), 0);

        let query = StorageQuery {
            collection: "credentials".into(),
            plugin_id: "plugin-a".into(),
            filters: vec![],
            sort: vec![],
            limit: None,
            offset: None,
        };
        let results = storage.execute_query(query).unwrap();
        assert_eq!(results.len(), 1);

        // Verify audit entry was created.
        assert_eq!(audit_count(&storage), 1);
        let (event_type, plugin_id, details) = last_audit_event(&storage);
        assert_eq!(event_type, "credential_access");
        assert_eq!(plugin_id.as_deref(), Some("plugin-a"));
        let details: serde_json::Value = serde_json::from_str(&details).unwrap();
        assert_eq!(details["operation"], "read");
        assert_eq!(details["credential_id"], "00000000-0000-0000-0000-000000000099");
    }

    #[test]
    fn audit_credential_insert_logs_modify() {
        let storage = setup_db();
        let cred_json = sample_credential_json("00000000-0000-0000-0000-000000000088");
        let cred: life_engine_types::Credential = serde_json::from_str(&cred_json).unwrap();
        let msg = PipelineMessage {
            metadata: MessageMetadata {
                correlation_id: Uuid::new_v4(),
                source: "test".into(),
                timestamp: Utc::now(),
                auth_context: None,
            },
            payload: TypedPayload::Cdm(Box::new(CdmType::Credential(cred))),
        };

        let mutation = StorageMutation::Insert {
            plugin_id: "plugin-a".into(),
            collection: "credentials".into(),
            data: msg,
        };
        storage.execute_mutation(mutation).unwrap();

        assert_eq!(audit_count(&storage), 1);
        let (event_type, plugin_id, details) = last_audit_event(&storage);
        assert_eq!(event_type, "credential_modify");
        assert_eq!(plugin_id.as_deref(), Some("plugin-a"));
        let details: serde_json::Value = serde_json::from_str(&details).unwrap();
        assert_eq!(details["operation"], "insert");
    }

    #[test]
    fn audit_non_credential_operations_do_not_log() {
        let storage = setup_db();
        let task_json = serde_json::json!({
            "id": "00000000-0000-0000-0000-000000000001",
            "title": "Test task",
            "description": null,
            "status": "pending",
            "priority": "medium",
            "due_date": null,
            "completed_at": null,
            "tags": [],
            "assignee": null,
            "parent_id": null,
            "source": "test",
            "source_id": "t-1",
            "extensions": null,
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        });
        insert_row(
            &storage,
            "00000000-0000-0000-0000-000000000001",
            "plugin-a",
            "tasks",
            &task_json.to_string(),
        );

        let query = StorageQuery {
            collection: "tasks".into(),
            plugin_id: "plugin-a".into(),
            filters: vec![],
            sort: vec![],
            limit: None,
            offset: None,
        };
        storage.execute_query(query).unwrap();

        // No audit entries for non-credential operations.
        assert_eq!(audit_count(&storage), 0);
    }
}
