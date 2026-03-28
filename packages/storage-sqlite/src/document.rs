//! `DocumentStorageAdapter` implementation backed by SQLite/SQLCipher.
//!
//! Uses the existing `plugin_data` table as the document store. Each document
//! is stored as a JSON blob in the `data` column, with system columns for id,
//! collection, plugin_id, version, created_at, and updated_at.
//!
//! All rusqlite calls are offloaded to `spawn_blocking` so the async trait
//! methods never block the Tokio runtime.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tracing::warn;
use uuid::Uuid;

use life_engine_traits::storage::{
    AdapterCapabilities, ChangeEvent, CollectionDescriptor, DocumentList,
    DocumentStorageAdapter, FilterNode, FilterOperator, HealthCheck, HealthReport,
    HealthStatus, QueryDescriptor, SortDirection, StorageError,
};

/// A `DocumentStorageAdapter` backed by SQLite/SQLCipher.
///
/// Wraps a shared connection inside `Arc<Mutex<Connection>>` so it can be
/// sent across async tasks. A default `plugin_id` is used for documents
/// created through this adapter (the `StorageContext` layer above sets the
/// real plugin scope).
pub struct SqliteDocumentAdapter {
    conn: Arc<Mutex<Connection>>,
    /// Default plugin_id used when the caller does not embed one in the document.
    default_plugin_id: String,
}

impl std::fmt::Debug for SqliteDocumentAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteDocumentAdapter").finish_non_exhaustive()
    }
}

impl SqliteDocumentAdapter {
    /// Create a new adapter from a shared connection.
    pub fn new(conn: Arc<Mutex<Connection>>, default_plugin_id: String) -> Self {
        Self {
            conn,
            default_plugin_id,
        }
    }

    /// Create a new adapter from a raw connection (takes ownership).
    pub fn from_connection(conn: Connection, default_plugin_id: String) -> Self {
        Self {
            conn: Arc::new(Mutex::new(conn)),
            default_plugin_id,
        }
    }

    /// Return a clone of the shared connection handle.
    pub fn shared_connection(&self) -> Arc<Mutex<Connection>> {
        Arc::clone(&self.conn)
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Validate that a collection name contains only safe characters.
fn validate_identifier(name: &str) -> Result<(), StorageError> {
    if name.is_empty() {
        return Err(StorageError::ValidationFailed {
            message: "collection name must not be empty".into(),
            field: None,
        });
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return Err(StorageError::ValidationFailed {
            message: format!("invalid collection name: {name}"),
            field: None,
        });
    }
    Ok(())
}

/// Build a complete document `Value` from a database row.
fn row_to_document(
    id: &str,
    data_json: &str,
    created_at: &str,
    updated_at: &str,
) -> Result<Value, StorageError> {
    let mut doc: Value = serde_json::from_str(data_json).map_err(|e| StorageError::Internal {
        message: format!("corrupt JSON in database: {e}"),
    })?;
    if let Some(obj) = doc.as_object_mut() {
        obj.insert("id".into(), json!(id));
        obj.insert("created_at".into(), json!(created_at));
        obj.insert("updated_at".into(), json!(updated_at));
    }
    Ok(doc)
}

/// Encode a cursor from an offset value.
fn encode_cursor(offset: u64) -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    URL_SAFE_NO_PAD.encode(offset.to_string().as_bytes())
}

/// Decode an offset from a cursor string.
fn decode_cursor(cursor: &str) -> Result<u64, StorageError> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    let bytes = URL_SAFE_NO_PAD
        .decode(cursor)
        .map_err(|_| StorageError::ValidationFailed {
            message: "invalid pagination cursor".into(),
            field: None,
        })?;
    let s = String::from_utf8(bytes).map_err(|_| StorageError::ValidationFailed {
        message: "invalid pagination cursor".into(),
        field: None,
    })?;
    s.parse::<u64>()
        .map_err(|_| StorageError::ValidationFailed {
            message: "invalid pagination cursor".into(),
            field: None,
        })
}

// ── Filter → SQL translation ────────────────────────────────────────

/// Context for building a parameterised WHERE clause from a `FilterNode` tree.
struct WhereBuilder {
    bind_values: Vec<Box<dyn rusqlite::types::ToSql>>,
    param_idx: u32,
}

impl WhereBuilder {
    fn new(start_idx: u32) -> Self {
        Self {
            bind_values: Vec::new(),
            param_idx: start_idx,
        }
    }

    /// Translate a `FilterNode` into a SQL expression, appending bind values.
    fn build(&mut self, node: &FilterNode) -> String {
        match node {
            FilterNode::And(children) => {
                if children.is_empty() {
                    return "1=1".into();
                }
                let parts: Vec<String> = children.iter().map(|c| self.build(c)).collect();
                format!("({})", parts.join(" AND "))
            }
            FilterNode::Or(children) => {
                if children.is_empty() {
                    return "1=0".into();
                }
                let parts: Vec<String> = children.iter().map(|c| self.build(c)).collect();
                format!("({})", parts.join(" OR "))
            }
            FilterNode::Not(child) => {
                let inner = self.build(child);
                format!("NOT ({inner})")
            }
            FilterNode::Comparison {
                field,
                operator,
                value,
            } => self.build_comparison(field, operator, value),
        }
    }

    fn build_comparison(&mut self, field: &str, op: &FilterOperator, value: &Value) -> String {
        let json_path = format!("$.{field}");
        let path_idx = self.param_idx;
        self.param_idx += 1;
        self.bind_values.push(Box::new(json_path));

        match op {
            FilterOperator::Exists => {
                format!("json_extract(data, ?{path_idx}) IS NOT NULL")
            }
            FilterOperator::In => {
                if let Some(arr) = value.as_array() {
                    if arr.is_empty() {
                        return "1=0".into();
                    }
                    let placeholders: Vec<String> = arr
                        .iter()
                        .map(|v| {
                            let idx = self.param_idx;
                            self.param_idx += 1;
                            self.bind_values.push(json_value_to_boxed_sql(v));
                            format!("?{idx}")
                        })
                        .collect();
                    format!(
                        "json_extract(data, ?{path_idx}) IN ({})",
                        placeholders.join(", ")
                    )
                } else {
                    "1=0".into()
                }
            }
            FilterOperator::NotIn => {
                if let Some(arr) = value.as_array() {
                    if arr.is_empty() {
                        return "1=1".into();
                    }
                    let placeholders: Vec<String> = arr
                        .iter()
                        .map(|v| {
                            let idx = self.param_idx;
                            self.param_idx += 1;
                            self.bind_values.push(json_value_to_boxed_sql(v));
                            format!("?{idx}")
                        })
                        .collect();
                    format!(
                        "json_extract(data, ?{path_idx}) NOT IN ({})",
                        placeholders.join(", ")
                    )
                } else {
                    "1=1".into()
                }
            }
            FilterOperator::Contains => {
                let val_idx = self.param_idx;
                self.param_idx += 1;
                let like_val = format!("%{}%", json_value_to_string(value));
                self.bind_values.push(Box::new(like_val));
                format!("json_extract(data, ?{path_idx}) LIKE ?{val_idx}")
            }
            FilterOperator::StartsWith => {
                let val_idx = self.param_idx;
                self.param_idx += 1;
                let like_val = format!("{}%", json_value_to_string(value));
                self.bind_values.push(Box::new(like_val));
                format!("json_extract(data, ?{path_idx}) LIKE ?{val_idx}")
            }
            _ => {
                let sql_op = match op {
                    FilterOperator::Eq => "=",
                    FilterOperator::Ne => "!=",
                    FilterOperator::Gt => ">",
                    FilterOperator::Gte => ">=",
                    FilterOperator::Lt => "<",
                    FilterOperator::Lte => "<=",
                    _ => unreachable!(),
                };
                let val_idx = self.param_idx;
                self.param_idx += 1;
                self.bind_values.push(json_value_to_boxed_sql(value));
                format!("json_extract(data, ?{path_idx}) {sql_op} ?{val_idx}")
            }
        }
    }
}

/// Convert a `serde_json::Value` to a boxed `ToSql` for rusqlite binding.
fn json_value_to_boxed_sql(value: &Value) -> Box<dyn rusqlite::types::ToSql> {
    match value {
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
        Value::Null => Box::new(Option::<String>::None),
        _ => Box::new(value.to_string()),
    }
}

/// Extract a simple string representation of a JSON value for LIKE patterns.
fn json_value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        _ => value.to_string(),
    }
}

// ── DocumentStorageAdapter impl ─────────────────────────────────────

#[async_trait]
impl DocumentStorageAdapter for SqliteDocumentAdapter {
    async fn get(&self, collection: &str, id: &str) -> Result<Value, StorageError> {
        validate_identifier(collection)?;
        let conn = Arc::clone(&self.conn);
        let collection = collection.to_owned();
        let id = id.to_owned();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(|e| StorageError::Internal {
                message: format!("lock poisoned: {e}"),
            })?;
            let mut stmt = conn
                .prepare(
                    "SELECT id, data, created_at, updated_at FROM plugin_data \
                     WHERE collection = ?1 AND id = ?2",
                )
                .map_err(|e| StorageError::Internal {
                    message: e.to_string(),
                })?;

            let result = stmt.query_row(params![collection, id], |row| {
                let id: String = row.get(0)?;
                let data: String = row.get(1)?;
                let created_at: String = row.get(2)?;
                let updated_at: String = row.get(3)?;
                Ok((id, data, created_at, updated_at))
            });

            match result {
                Ok((id, data, created_at, updated_at)) => {
                    row_to_document(&id, &data, &created_at, &updated_at)
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => Err(StorageError::NotFound {
                    collection,
                    id,
                }),
                Err(e) => Err(StorageError::Internal {
                    message: e.to_string(),
                }),
            }
        })
        .await
        .map_err(|e| StorageError::Internal {
            message: format!("task join error: {e}"),
        })?
    }

    async fn create(&self, collection: &str, document: Value) -> Result<Value, StorageError> {
        validate_identifier(collection)?;
        let conn = Arc::clone(&self.conn);
        let collection = collection.to_owned();
        let plugin_id = self.default_plugin_id.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(|e| StorageError::Internal {
                message: format!("lock poisoned: {e}"),
            })?;

            let id = document
                .get("id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_owned())
                .unwrap_or_else(|| Uuid::new_v4().to_string());

            let now = Utc::now().to_rfc3339();

            // Strip system fields from the stored data blob.
            let mut data = document.clone();
            if let Some(obj) = data.as_object_mut() {
                obj.remove("id");
                obj.remove("created_at");
                obj.remove("updated_at");
            }
            let data_json = serde_json::to_string(&data).map_err(|e| StorageError::Internal {
                message: e.to_string(),
            })?;

            // Check for duplicate id.
            let exists: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM plugin_data WHERE collection = ?1 AND id = ?2)",
                    params![collection, id],
                    |row| row.get(0),
                )
                .map_err(|e| StorageError::Internal {
                    message: e.to_string(),
                })?;

            if exists {
                return Err(StorageError::AlreadyExists {
                    collection,
                    id,
                });
            }

            conn.execute(
                "INSERT INTO plugin_data (id, plugin_id, collection, data, version, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6)",
                params![id, plugin_id, collection, data_json, now, now],
            )
            .map_err(|e| StorageError::Internal {
                message: e.to_string(),
            })?;

            row_to_document(&id, &data_json, &now, &now)
        })
        .await
        .map_err(|e| StorageError::Internal {
            message: format!("task join error: {e}"),
        })?
    }

    async fn update(
        &self,
        collection: &str,
        id: &str,
        document: Value,
    ) -> Result<Value, StorageError> {
        validate_identifier(collection)?;
        let conn = Arc::clone(&self.conn);
        let collection = collection.to_owned();
        let id = id.to_owned();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(|e| StorageError::Internal {
                message: format!("lock poisoned: {e}"),
            })?;

            let now = Utc::now().to_rfc3339();

            // Strip system fields from the stored data blob.
            let mut data = document;
            if let Some(obj) = data.as_object_mut() {
                obj.remove("id");
                obj.remove("created_at");
                obj.remove("updated_at");
            }
            let data_json = serde_json::to_string(&data).map_err(|e| StorageError::Internal {
                message: e.to_string(),
            })?;

            let rows = conn
                .execute(
                    "UPDATE plugin_data SET data = ?1, version = version + 1, updated_at = ?2 \
                     WHERE collection = ?3 AND id = ?4",
                    params![data_json, now, collection, id],
                )
                .map_err(|e| StorageError::Internal {
                    message: e.to_string(),
                })?;

            if rows == 0 {
                return Err(StorageError::NotFound {
                    collection,
                    id,
                });
            }

            // Fetch created_at from the stored row.
            let created_at: String = conn
                .query_row(
                    "SELECT created_at FROM plugin_data WHERE collection = ?1 AND id = ?2",
                    params![collection, id],
                    |row| row.get(0),
                )
                .map_err(|e| StorageError::Internal {
                    message: e.to_string(),
                })?;

            row_to_document(&id, &data_json, &created_at, &now)
        })
        .await
        .map_err(|e| StorageError::Internal {
            message: format!("task join error: {e}"),
        })?
    }

    async fn partial_update(
        &self,
        collection: &str,
        id: &str,
        patch: Value,
    ) -> Result<Value, StorageError> {
        validate_identifier(collection)?;
        let conn = Arc::clone(&self.conn);
        let collection = collection.to_owned();
        let id = id.to_owned();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(|e| StorageError::Internal {
                message: format!("lock poisoned: {e}"),
            })?;

            // Fetch existing document.
            let (existing_data, created_at): (String, String) = conn
                .query_row(
                    "SELECT data, created_at FROM plugin_data WHERE collection = ?1 AND id = ?2",
                    params![collection, id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => StorageError::NotFound {
                        collection: collection.clone(),
                        id: id.clone(),
                    },
                    other => StorageError::Internal {
                        message: other.to_string(),
                    },
                })?;

            let mut doc: Value =
                serde_json::from_str(&existing_data).map_err(|e| StorageError::Internal {
                    message: format!("corrupt JSON in database: {e}"),
                })?;

            // JSON merge patch: merge the patch into the existing document.
            json_merge_patch(&mut doc, &patch);

            // Strip system fields before storing.
            if let Some(obj) = doc.as_object_mut() {
                obj.remove("id");
                obj.remove("created_at");
                obj.remove("updated_at");
            }

            let now = Utc::now().to_rfc3339();
            let data_json = serde_json::to_string(&doc).map_err(|e| StorageError::Internal {
                message: e.to_string(),
            })?;

            conn.execute(
                "UPDATE plugin_data SET data = ?1, version = version + 1, updated_at = ?2 \
                 WHERE collection = ?3 AND id = ?4",
                params![data_json, now, collection, id],
            )
            .map_err(|e| StorageError::Internal {
                message: e.to_string(),
            })?;

            row_to_document(&id, &data_json, &created_at, &now)
        })
        .await
        .map_err(|e| StorageError::Internal {
            message: format!("task join error: {e}"),
        })?
    }

    async fn delete(&self, collection: &str, id: &str) -> Result<(), StorageError> {
        validate_identifier(collection)?;
        let conn = Arc::clone(&self.conn);
        let collection = collection.to_owned();
        let id = id.to_owned();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(|e| StorageError::Internal {
                message: format!("lock poisoned: {e}"),
            })?;

            let rows = conn
                .execute(
                    "DELETE FROM plugin_data WHERE collection = ?1 AND id = ?2",
                    params![collection, id],
                )
                .map_err(|e| StorageError::Internal {
                    message: e.to_string(),
                })?;

            if rows == 0 {
                Err(StorageError::NotFound { collection, id })
            } else {
                Ok(())
            }
        })
        .await
        .map_err(|e| StorageError::Internal {
            message: format!("task join error: {e}"),
        })?
    }

    async fn query(&self, descriptor: QueryDescriptor) -> Result<DocumentList, StorageError> {
        validate_identifier(&descriptor.collection)?;
        let conn = Arc::clone(&self.conn);

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(|e| StorageError::Internal {
                message: format!("lock poisoned: {e}"),
            })?;

            let mut bind_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
            bind_values.push(Box::new(descriptor.collection.clone()));
            let mut sql = String::from(
                "SELECT id, data, created_at, updated_at FROM plugin_data WHERE collection = ?1",
            );

            // Apply filter.
            if let Some(ref filter) = descriptor.filter {
                let mut wb = WhereBuilder::new(2);
                let clause = wb.build(filter);
                sql.push_str(" AND ");
                sql.push_str(&clause);
                bind_values.extend(wb.bind_values);
            }

            // Count total before pagination.
            let count_sql = format!(
                "SELECT COUNT(*) FROM plugin_data WHERE collection = ?1{}",
                if descriptor.filter.is_some() {
                    sql.strip_prefix(
                        "SELECT id, data, created_at, updated_at FROM plugin_data WHERE collection = ?1",
                    )
                    .unwrap_or("")
                } else {
                    ""
                }
            );

            let params_refs: Vec<&dyn rusqlite::types::ToSql> =
                bind_values.iter().map(|b| b.as_ref()).collect();

            let total_count: u64 = conn
                .query_row(&count_sql, params_refs.as_slice(), |row| {
                    row.get::<_, i64>(0).map(|n| n as u64)
                })
                .map_err(|e| StorageError::Internal {
                    message: format!("count query failed: {e}"),
                })?;

            // Sort.
            if !descriptor.sort.is_empty() {
                sql.push_str(" ORDER BY ");
                let parts: Vec<String> = descriptor
                    .sort
                    .iter()
                    .map(|s| {
                        let dir = match s.direction {
                            SortDirection::Asc => "ASC",
                            SortDirection::Desc => "DESC",
                        };
                        // Use json_extract with a literal path (safe because
                        // field names are validated at the StorageContext level).
                        format!("json_extract(data, '$.{}') {dir}", s.field)
                    })
                    .collect();
                sql.push_str(&parts.join(", "));
            }

            // Pagination.
            let offset = descriptor
                .pagination
                .cursor
                .as_deref()
                .map(decode_cursor)
                .transpose()?
                .unwrap_or(0);
            let limit = descriptor.pagination.limit.min(1000);
            sql.push_str(&format!(" LIMIT {limit} OFFSET {offset}"));

            // Execute.
            let params_refs: Vec<&dyn rusqlite::types::ToSql> =
                bind_values.iter().map(|b| b.as_ref()).collect();
            let mut stmt = conn.prepare(&sql).map_err(|e| StorageError::Internal {
                message: format!("query prepare failed: {e}"),
            })?;

            let rows = stmt
                .query_map(params_refs.as_slice(), |row| {
                    let id: String = row.get(0)?;
                    let data: String = row.get(1)?;
                    let created_at: String = row.get(2)?;
                    let updated_at: String = row.get(3)?;
                    Ok((id, data, created_at, updated_at))
                })
                .map_err(|e| StorageError::Internal {
                    message: format!("query execution failed: {e}"),
                })?;

            let mut documents = Vec::new();
            for row in rows {
                let (id, data, created_at, updated_at) =
                    row.map_err(|e| StorageError::Internal {
                        message: e.to_string(),
                    })?;
                let mut doc = row_to_document(&id, &data, &created_at, &updated_at)?;

                // Apply field projection.
                if let Some(ref fields) = descriptor.fields {
                    if let Some(obj) = doc.as_object() {
                        let projected: serde_json::Map<String, Value> = obj
                            .iter()
                            .filter(|(k, _)| fields.contains(k))
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        doc = Value::Object(projected);
                    }
                }

                documents.push(doc);
            }

            let next_cursor = if offset + limit < total_count {
                Some(encode_cursor(offset + limit))
            } else {
                None
            };

            Ok(DocumentList {
                documents,
                total_count,
                next_cursor,
            })
        })
        .await
        .map_err(|e| StorageError::Internal {
            message: format!("task join error: {e}"),
        })?
    }

    async fn count(
        &self,
        collection: &str,
        filter: Option<FilterNode>,
    ) -> Result<u64, StorageError> {
        validate_identifier(collection)?;
        let conn = Arc::clone(&self.conn);
        let collection = collection.to_owned();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(|e| StorageError::Internal {
                message: format!("lock poisoned: {e}"),
            })?;

            let mut bind_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
            bind_values.push(Box::new(collection));

            let mut sql =
                String::from("SELECT COUNT(*) FROM plugin_data WHERE collection = ?1");

            if let Some(ref filter) = filter {
                let mut wb = WhereBuilder::new(2);
                let clause = wb.build(filter);
                sql.push_str(" AND ");
                sql.push_str(&clause);
                bind_values.extend(wb.bind_values);
            }

            let params_refs: Vec<&dyn rusqlite::types::ToSql> =
                bind_values.iter().map(|b| b.as_ref()).collect();

            let count: i64 = conn
                .query_row(&sql, params_refs.as_slice(), |row| row.get(0))
                .map_err(|e| StorageError::Internal {
                    message: format!("count query failed: {e}"),
                })?;

            Ok(count as u64)
        })
        .await
        .map_err(|e| StorageError::Internal {
            message: format!("task join error: {e}"),
        })?
    }

    async fn batch_create(
        &self,
        collection: &str,
        documents: Vec<Value>,
    ) -> Result<Vec<Value>, StorageError> {
        validate_identifier(collection)?;
        let conn = Arc::clone(&self.conn);
        let collection = collection.to_owned();
        let plugin_id = self.default_plugin_id.clone();

        tokio::task::spawn_blocking(move || {
            let mut conn = conn.lock().map_err(|e| StorageError::Internal {
                message: format!("lock poisoned: {e}"),
            })?;

            let tx = conn.transaction().map_err(|e| StorageError::TransactionFailed {
                reason: e.to_string(),
            })?;

            let now = Utc::now().to_rfc3339();
            let mut results = Vec::with_capacity(documents.len());

            for document in documents {
                let id = document
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_owned())
                    .unwrap_or_else(|| Uuid::new_v4().to_string());

                let mut data = document;
                if let Some(obj) = data.as_object_mut() {
                    obj.remove("id");
                    obj.remove("created_at");
                    obj.remove("updated_at");
                }
                let data_json =
                    serde_json::to_string(&data).map_err(|e| StorageError::Internal {
                        message: e.to_string(),
                    })?;

                tx.execute(
                    "INSERT INTO plugin_data (id, plugin_id, collection, data, version, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6)",
                    params![id, plugin_id, collection, data_json, now, now],
                )
                .map_err(|e| {
                    if e.to_string().contains("UNIQUE constraint") {
                        StorageError::AlreadyExists {
                            collection: collection.clone(),
                            id: id.clone(),
                        }
                    } else {
                        StorageError::Internal {
                            message: e.to_string(),
                        }
                    }
                })?;

                results.push(row_to_document(&id, &data_json, &now, &now)?);
            }

            tx.commit().map_err(|e| StorageError::TransactionFailed {
                reason: e.to_string(),
            })?;

            Ok(results)
        })
        .await
        .map_err(|e| StorageError::Internal {
            message: format!("task join error: {e}"),
        })?
    }

    async fn batch_update(
        &self,
        collection: &str,
        updates: Vec<(String, Value)>,
    ) -> Result<Vec<Value>, StorageError> {
        validate_identifier(collection)?;
        let conn = Arc::clone(&self.conn);
        let collection = collection.to_owned();

        tokio::task::spawn_blocking(move || {
            let mut conn = conn.lock().map_err(|e| StorageError::Internal {
                message: format!("lock poisoned: {e}"),
            })?;

            let tx = conn.transaction().map_err(|e| StorageError::TransactionFailed {
                reason: e.to_string(),
            })?;

            let now = Utc::now().to_rfc3339();
            let mut results = Vec::with_capacity(updates.len());

            for (id, document) in updates {
                let mut data = document;
                if let Some(obj) = data.as_object_mut() {
                    obj.remove("id");
                    obj.remove("created_at");
                    obj.remove("updated_at");
                }
                let data_json =
                    serde_json::to_string(&data).map_err(|e| StorageError::Internal {
                        message: e.to_string(),
                    })?;

                let rows = tx
                    .execute(
                        "UPDATE plugin_data SET data = ?1, version = version + 1, updated_at = ?2 \
                         WHERE collection = ?3 AND id = ?4",
                        params![data_json, now, collection, id],
                    )
                    .map_err(|e| StorageError::Internal {
                        message: e.to_string(),
                    })?;

                if rows == 0 {
                    return Err(StorageError::NotFound {
                        collection: collection.clone(),
                        id,
                    });
                }

                let created_at: String = tx
                    .query_row(
                        "SELECT created_at FROM plugin_data WHERE collection = ?1 AND id = ?2",
                        params![collection, id],
                        |row| row.get(0),
                    )
                    .map_err(|e| StorageError::Internal {
                        message: e.to_string(),
                    })?;

                results.push(row_to_document(&id, &data_json, &created_at, &now)?);
            }

            tx.commit().map_err(|e| StorageError::TransactionFailed {
                reason: e.to_string(),
            })?;

            Ok(results)
        })
        .await
        .map_err(|e| StorageError::Internal {
            message: format!("task join error: {e}"),
        })?
    }

    async fn batch_delete(
        &self,
        collection: &str,
        ids: Vec<String>,
    ) -> Result<(), StorageError> {
        validate_identifier(collection)?;
        if ids.is_empty() {
            return Ok(());
        }
        let conn = Arc::clone(&self.conn);
        let collection = collection.to_owned();

        tokio::task::spawn_blocking(move || {
            let mut conn = conn.lock().map_err(|e| StorageError::Internal {
                message: format!("lock poisoned: {e}"),
            })?;

            let tx = conn.transaction().map_err(|e| StorageError::TransactionFailed {
                reason: e.to_string(),
            })?;

            let placeholders: String = ids
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", i + 2))
                .collect::<Vec<_>>()
                .join(", ");

            let sql = format!(
                "DELETE FROM plugin_data WHERE collection = ?1 AND id IN ({placeholders})"
            );

            let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
            param_values.push(Box::new(collection.clone()));
            for id in &ids {
                param_values.push(Box::new(id.clone()));
            }
            let params_refs: Vec<&dyn rusqlite::types::ToSql> =
                param_values.iter().map(|b| b.as_ref()).collect();

            let deleted = tx
                .execute(&sql, params_refs.as_slice())
                .map_err(|e| StorageError::Internal {
                    message: e.to_string(),
                })?;

            if deleted != ids.len() {
                // Some ids were not found — roll back for atomicity.
                return Err(StorageError::Internal {
                    message: format!(
                        "batch_delete expected to delete {} rows but deleted {deleted}",
                        ids.len()
                    ),
                });
            }

            tx.commit().map_err(|e| StorageError::TransactionFailed {
                reason: e.to_string(),
            })?;

            Ok(())
        })
        .await
        .map_err(|e| StorageError::Internal {
            message: format!("task join error: {e}"),
        })?
    }

    async fn watch(
        &self,
        _collection: &str,
    ) -> Result<mpsc::Receiver<ChangeEvent>, StorageError> {
        // SQLite does not have native change notifications.
        // Return an empty channel — StorageContext will emit events on the write path.
        let (_tx, rx) = mpsc::channel(1);
        Ok(rx)
    }

    async fn migrate(&self, descriptor: CollectionDescriptor) -> Result<(), StorageError> {
        validate_identifier(&descriptor.name)?;
        let conn = Arc::clone(&self.conn);

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(|e| StorageError::Internal {
                message: format!("lock poisoned: {e}"),
            })?;

            // The plugin_data table is the universal store — no per-collection
            // DDL is needed. But we track field metadata in schema_versions and
            // create indexes for declared index hints.
            //
            // Check for existing schema version to detect breaking changes.
            let existing: Option<i64> = conn
                .query_row(
                    "SELECT version FROM schema_versions WHERE plugin_id = ?1 AND collection = ?2",
                    params![descriptor.plugin_id, descriptor.name],
                    |row| row.get(0),
                )
                .ok();

            if existing.is_some() {
                // Schema already registered — this is an idempotent re-migration.
                // In the future we would compare field definitions here and reject
                // breaking changes with SchemaConflict.
            }

            // Upsert schema version.
            let now = Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO schema_versions (plugin_id, collection, version, updated_at) \
                 VALUES (?1, ?2, 1, ?3) \
                 ON CONFLICT(plugin_id, collection) DO UPDATE SET updated_at = ?3",
                params![descriptor.plugin_id, descriptor.name, now],
            )
            .map_err(|e| StorageError::Internal {
                message: e.to_string(),
            })?;

            // Create advisory indexes for declared index hints.
            for hint in &descriptor.indexes {
                // Validate the index hint is a safe identifier.
                if !hint
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
                {
                    warn!(
                        index_hint = hint,
                        collection = descriptor.name,
                        "skipping invalid index hint"
                    );
                    continue;
                }

                let index_name = format!(
                    "idx_{}_{}_{}",
                    descriptor.plugin_id.replace('.', "_").replace('-', "_"),
                    descriptor.name.replace('.', "_").replace('-', "_"),
                    hint.replace('.', "_").replace('-', "_")
                );

                let sql = format!(
                    "CREATE INDEX IF NOT EXISTS {index_name} ON plugin_data(collection, json_extract(data, '$.{hint}'))"
                );

                conn.execute_batch(&sql).map_err(|e| {
                    warn!(
                        index = index_name,
                        error = %e,
                        "failed to create advisory index"
                    );
                    StorageError::Internal {
                        message: format!("index creation failed: {e}"),
                    }
                })?;
            }

            Ok(())
        })
        .await
        .map_err(|e| StorageError::Internal {
            message: format!("task join error: {e}"),
        })?
    }

    async fn health(&self) -> Result<HealthReport, StorageError> {
        let conn = Arc::clone(&self.conn);

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(|e| StorageError::Internal {
                message: format!("lock poisoned: {e}"),
            })?;

            let mut checks = Vec::new();

            // Connection check.
            let conn_check = match conn.execute_batch("SELECT 1") {
                Ok(()) => HealthCheck {
                    name: "connection".into(),
                    status: HealthStatus::Healthy,
                    message: None,
                },
                Err(e) => HealthCheck {
                    name: "connection".into(),
                    status: HealthStatus::Unhealthy,
                    message: Some(e.to_string()),
                },
            };
            checks.push(conn_check);

            // WAL mode check.
            let wal_check = match conn.query_row("PRAGMA journal_mode", [], |row| {
                row.get::<_, String>(0)
            }) {
                Ok(mode) if mode == "wal" => HealthCheck {
                    name: "wal_mode".into(),
                    status: HealthStatus::Healthy,
                    message: None,
                },
                Ok(mode) => HealthCheck {
                    name: "wal_mode".into(),
                    status: HealthStatus::Degraded,
                    message: Some(format!("expected WAL mode, got {mode}")),
                },
                Err(e) => HealthCheck {
                    name: "wal_mode".into(),
                    status: HealthStatus::Unhealthy,
                    message: Some(e.to_string()),
                },
            };
            checks.push(wal_check);

            // Write check.
            let write_check = match conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS _health_check (v INTEGER); \
                 DROP TABLE IF EXISTS _health_check",
            ) {
                Ok(()) => HealthCheck {
                    name: "write".into(),
                    status: HealthStatus::Healthy,
                    message: None,
                },
                Err(e) => HealthCheck {
                    name: "write".into(),
                    status: HealthStatus::Unhealthy,
                    message: Some(e.to_string()),
                },
            };
            checks.push(write_check);

            // Overall status = worst of individual checks.
            let status = checks
                .iter()
                .map(|c| c.status)
                .max()
                .unwrap_or(HealthStatus::Healthy);

            Ok(HealthReport {
                status,
                message: None,
                checks,
            })
        })
        .await
        .map_err(|e| StorageError::Internal {
            message: format!("task join error: {e}"),
        })?
    }

    fn capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities {
            indexing: true,
            transactions: true,
            full_text_search: false,
            watch: false,
            batch_operations: true,
            encryption: true, // SQLCipher provides at-rest encryption.
        }
    }
}

/// RFC 7386 JSON Merge Patch: recursively merge `patch` into `target`.
fn json_merge_patch(target: &mut Value, patch: &Value) {
    if let Value::Object(patch_obj) = patch {
        if !target.is_object() {
            *target = Value::Object(serde_json::Map::new());
        }
        if let Value::Object(target_obj) = target {
            for (key, value) in patch_obj {
                if value.is_null() {
                    target_obj.remove(key);
                } else {
                    let entry = target_obj
                        .entry(key.clone())
                        .or_insert(Value::Null);
                    json_merge_patch(entry, value);
                }
            }
        }
    } else {
        *target = patch.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_json_merge_patch() {
        let mut target = json!({"a": 1, "b": {"c": 2, "d": 3}});
        let patch = json!({"b": {"c": 99, "d": null}, "e": 5});
        json_merge_patch(&mut target, &patch);
        assert_eq!(target, json!({"a": 1, "b": {"c": 99}, "e": 5}));
    }

    #[test]
    fn test_validate_identifier() {
        assert!(validate_identifier("events").is_ok());
        assert!(validate_identifier("my_plugin.data").is_ok());
        assert!(validate_identifier("").is_err());
        assert!(validate_identifier("foo;DROP TABLE").is_err());
    }

    #[test]
    fn test_cursor_round_trip() {
        let offset = 42u64;
        let cursor = encode_cursor(offset);
        assert_eq!(decode_cursor(&cursor).unwrap(), offset);
    }
}
