//! StorageBackend trait implementation for SQLite.

use chrono::Utc;
use rusqlite::{params, params_from_iter};
use serde_json;
use uuid::Uuid;

use life_engine_types::{
    CdmType, FilterOp, MessageMetadata, PipelineMessage, SortDirection, StorageMutation,
    StorageQuery, TypedPayload,
};

use crate::error::StorageError;
use crate::SqliteStorage;

/// Maximum number of records a single query may return.
const MAX_LIMIT: u32 = 1000;

/// Parse a JSON `data` column value into a `CdmType` based on collection name.
///
/// For canonical collections the JSON is deserialized into the appropriate
/// Rust struct. For private (non-canonical) collections the raw JSON is
/// returned as a `TypedPayload::Custom` via `SchemaValidated`.
fn parse_payload(collection: &str, data_json: &str) -> Result<TypedPayload, StorageError> {
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
            let v = serde_json::from_str(data_json)?;
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

impl SqliteStorage {
    /// Execute a read query, translating `StorageQuery` into SQL.
    pub fn execute_query(&self, query: StorageQuery) -> Result<Vec<PipelineMessage>, StorageError> {
        let mut sql = String::from(
            "SELECT id, plugin_id, collection, data, version, created_at, updated_at \
             FROM plugin_data WHERE plugin_id = ?1 AND collection = ?2",
        );

        // Collect bind parameters. The first two are always plugin_id and collection.
        let mut bind_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        bind_values.push(Box::new(query.plugin_id.clone()));
        bind_values.push(Box::new(query.collection.clone()));

        let mut param_idx = 3u32;

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

            let payload = parse_payload(&collection, &data_json)?;

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

                self.conn.execute(
                    "INSERT INTO plugin_data \
                     (id, plugin_id, collection, data, version, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6)",
                    params![id, plugin_id, collection, data_json, now, now],
                )?;

                Ok(())
            }
            StorageMutation::Update {
                plugin_id,
                collection: _,
                id,
                data,
                expected_version,
            } => {
                let now = Utc::now().to_rfc3339();
                let data_json = serialize_payload(&data.payload)?;
                let id_str = id.to_string();
                let version_i64 = expected_version as i64;

                let rows_affected = self.conn.execute(
                    "UPDATE plugin_data \
                     SET data = ?1, version = version + 1, updated_at = ?2 \
                     WHERE id = ?3 AND plugin_id = ?4 AND version = ?5",
                    params![data_json, now, id_str, plugin_id, version_i64],
                )?;

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
                collection: _,
                id,
            } => {
                let id_str = id.to_string();

                self.conn.execute(
                    "DELETE FROM plugin_data WHERE id = ?1 AND plugin_id = ?2",
                    params![id_str, plugin_id],
                )?;

                Ok(())
            }
        }
    }
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
        SqliteStorage { conn }
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
    fn execute_filters_by_plugin_id() {
        let storage = setup_db();
        let task_json = serde_json::json!({
            "id": "00000000-0000-0000-0000-000000000001",
            "title": "Task A",
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
        insert_row(&storage, "id-1", "plugin-a", "tasks", &task_json.to_string());
        insert_row(&storage, "id-2", "plugin-b", "tasks", &task_json.to_string());

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
}
