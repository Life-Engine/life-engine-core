//! Built-in system CRUD step handler.
//!
//! Implements `PluginExecutor` for the `system.crud` plugin, providing
//! pass-through CRUD operations that delegate directly to `StorageContext`.
//! This is the default executor for system workflows — it runs with
//! `CallerIdentity::System` and auto-granted capabilities.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::{debug, warn};

use life_engine_traits::storage::QueryDescriptor;
use life_engine_traits::storage_context::{CallerIdentity, StorageContext};
use life_engine_traits::{EngineError, Severity};
use life_engine_types::{PipelineMessage, SchemaValidated, TypedPayload};

use crate::executor::PluginExecutor;

/// The plugin ID used in workflow YAML files for system CRUD operations.
pub const SYSTEM_CRUD_PLUGIN_ID: &str = "system.crud";

/// Built-in handler for system CRUD actions.
///
/// Registered as a special plugin that delegates directly to `StorageContext`
/// with `CallerIdentity::System` (bypasses capability checks). This enables
/// the default system workflows (collection.list, collection.get, etc.) to
/// work out of the box without any external plugins.
pub struct SystemCrudHandler {
    storage: Arc<StorageContext>,
}

impl SystemCrudHandler {
    /// Create a new system CRUD handler wrapping the given storage context.
    pub fn new(storage: Arc<StorageContext>) -> Self {
        Self { storage }
    }
}

/// Error type for system step failures.
#[derive(Debug)]
struct SystemStepError {
    message: String,
    code: String,
    severity: Severity,
}

impl std::fmt::Display for SystemStepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for SystemStepError {}

impl EngineError for SystemStepError {
    fn code(&self) -> &str {
        &self.code
    }

    fn severity(&self) -> Severity {
        self.severity
    }

    fn source_module(&self) -> &str {
        "system-crud"
    }
}

/// Extract a string field from the pipeline payload JSON.
///
/// System workflows receive their input from the transport layer via
/// `build_initial_message`, which places the request body as the payload.
/// Path params (`:collection`, `:id`) and query params are extracted
/// from nested `params` or `query` objects if present, falling back
/// to top-level keys.
fn extract_param(payload: &Value, key: &str) -> Option<String> {
    payload
        .get("params")
        .and_then(|p| p.get(key))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            payload
                .get(key)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
}

fn system_error(message: impl Into<String>, code: &str) -> Box<dyn EngineError> {
    Box::new(SystemStepError {
        message: message.into(),
        code: code.to_string(),
        severity: Severity::Fatal,
    })
}

/// Extract the inner JSON value from a `TypedPayload`.
///
/// For `Custom` payloads (the common case for system workflows), this
/// extracts the inner value from the `SchemaValidated` wrapper. For `Cdm`
/// payloads, it serializes the CDM type to JSON.
fn payload_to_value(payload: &TypedPayload) -> Value {
    match payload {
        TypedPayload::Custom(validated) => {
            // SchemaValidated implements Deref<Target = T>, and #[serde(transparent)].
            serde_json::to_value(&**validated).unwrap_or_default()
        }
        TypedPayload::Cdm(cdm) => serde_json::to_value(&**cdm).unwrap_or_default(),
    }
}

/// Wrap a `serde_json::Value` in a `TypedPayload::Custom`.
fn value_to_payload(value: Value) -> Result<TypedPayload, Box<dyn EngineError>> {
    let schema = json!({"type": "object"});
    let validated = SchemaValidated::new(value, &schema)
        .map_err(|e| system_error(e.to_string(), "PAYLOAD_VALIDATION"))?;
    Ok(TypedPayload::Custom(validated))
}

#[async_trait]
impl PluginExecutor for SystemCrudHandler {
    async fn execute(
        &self,
        plugin_id: &str,
        action: &str,
        mut input: PipelineMessage,
    ) -> Result<PipelineMessage, Box<dyn EngineError>> {
        debug!(
            plugin_id = plugin_id,
            action = action,
            "system.crud step executing"
        );

        let caller = CallerIdentity::System;
        let payload = payload_to_value(&input.payload);

        let result_value = match action {
            "list" => {
                let collection = extract_param(&payload, "collection").ok_or_else(|| {
                    system_error("missing 'collection' parameter", "MISSING_PARAM")
                })?;

                let limit = extract_param(&payload, "limit")
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(100);
                let cursor = extract_param(&payload, "cursor");

                let descriptor = QueryDescriptor {
                    collection,
                    filter: None,
                    sort: Vec::new(),
                    pagination: life_engine_traits::storage::Pagination { limit, cursor },
                    fields: None,
                    text_search: None,
                };

                let result = self
                    .storage
                    .doc_query(&caller, descriptor, false)
                    .await
                    .map_err(|e| system_error(e.to_string(), "STORAGE_ERROR"))?;

                json!({
                    "documents": result.documents,
                    "total_count": result.total_count,
                    "next_cursor": result.next_cursor,
                })
            }

            "get" => {
                let collection = extract_param(&payload, "collection").ok_or_else(|| {
                    system_error("missing 'collection' parameter", "MISSING_PARAM")
                })?;
                let id = extract_param(&payload, "id").ok_or_else(|| {
                    system_error("missing 'id' parameter", "MISSING_PARAM")
                })?;

                self.storage
                    .doc_get(&caller, &collection, &id, false)
                    .await
                    .map_err(|e| system_error(e.to_string(), "STORAGE_ERROR"))?
            }

            "create" => {
                let collection = extract_param(&payload, "collection").ok_or_else(|| {
                    system_error("missing 'collection' parameter", "MISSING_PARAM")
                })?;

                // The document body is nested under "body" if the transport
                // placed it there; otherwise use the entire payload.
                let body = payload
                    .get("body")
                    .cloned()
                    .unwrap_or_else(|| payload.clone());

                let result = self
                    .storage
                    .doc_create(&caller, &collection, body, false)
                    .await
                    .map_err(|e| system_error(e.to_string(), "STORAGE_ERROR"))?;

                json!({
                    "created": true,
                    "document": result,
                })
            }

            "update" => {
                let collection = extract_param(&payload, "collection").ok_or_else(|| {
                    system_error("missing 'collection' parameter", "MISSING_PARAM")
                })?;
                let id = extract_param(&payload, "id").ok_or_else(|| {
                    system_error("missing 'id' parameter", "MISSING_PARAM")
                })?;

                let body = payload
                    .get("body")
                    .cloned()
                    .unwrap_or_else(|| payload.clone());

                self.storage
                    .doc_update(&caller, &collection, &id, body, false)
                    .await
                    .map_err(|e| system_error(e.to_string(), "STORAGE_ERROR"))?
            }

            "delete" => {
                let collection = extract_param(&payload, "collection").ok_or_else(|| {
                    system_error("missing 'collection' parameter", "MISSING_PARAM")
                })?;
                let id = extract_param(&payload, "id").ok_or_else(|| {
                    system_error("missing 'id' parameter", "MISSING_PARAM")
                })?;

                self.storage
                    .doc_delete(&caller, &collection, &id, false)
                    .await
                    .map_err(|e| system_error(e.to_string(), "STORAGE_ERROR"))?;

                json!({ "deleted": true })
            }

            "graphql_resolve" => {
                // GraphQL resolution is handled by the transport-graphql layer.
                // This step passes through the payload as-is.
                warn!("graphql_resolve is a pass-through stub; resolution handled by transport layer");
                payload
            }

            "health_check" => {
                let storage_ok = self
                    .storage
                    .doc_count(&caller, "__health_check", None, false)
                    .await
                    .is_ok();

                json!({
                    "status": if storage_ok { "healthy" } else { "degraded" },
                    "storage": if storage_ok { "ok" } else { "unreachable" },
                })
            }

            unknown => {
                return Err(system_error(
                    format!("unknown system.crud action: '{unknown}'"),
                    "UNKNOWN_ACTION",
                ));
            }
        };

        input.payload = value_to_payload(result_value)?;
        Ok(input)
    }
}

/// A composite plugin executor that routes `system.crud` actions to the
/// built-in handler and all other plugin IDs to the underlying executor.
pub struct CompositePluginExecutor {
    system_handler: SystemCrudHandler,
    plugin_executor: Arc<dyn PluginExecutor>,
}

impl CompositePluginExecutor {
    /// Create a new composite executor.
    pub fn new(
        storage: Arc<StorageContext>,
        plugin_executor: Arc<dyn PluginExecutor>,
    ) -> Self {
        Self {
            system_handler: SystemCrudHandler::new(storage),
            plugin_executor,
        }
    }
}

#[async_trait]
impl PluginExecutor for CompositePluginExecutor {
    async fn execute(
        &self,
        plugin_id: &str,
        action: &str,
        input: PipelineMessage,
    ) -> Result<PipelineMessage, Box<dyn EngineError>> {
        if plugin_id == SYSTEM_CRUD_PLUGIN_ID {
            self.system_handler.execute(plugin_id, action, input).await
        } else {
            self.plugin_executor
                .execute(plugin_id, action, input)
                .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_param_from_nested_params() {
        let payload = json!({
            "params": {
                "collection": "contacts",
                "id": "abc-123"
            }
        });
        assert_eq!(
            extract_param(&payload, "collection"),
            Some("contacts".to_string())
        );
        assert_eq!(
            extract_param(&payload, "id"),
            Some("abc-123".to_string())
        );
        assert_eq!(extract_param(&payload, "missing"), None);
    }

    #[test]
    fn extract_param_from_top_level() {
        let payload = json!({
            "collection": "tasks"
        });
        assert_eq!(
            extract_param(&payload, "collection"),
            Some("tasks".to_string())
        );
    }

    #[test]
    fn system_crud_plugin_id_is_consistent() {
        assert_eq!(SYSTEM_CRUD_PLUGIN_ID, "system.crud");
    }

    #[test]
    fn value_to_payload_wraps_object() {
        let val = json!({"foo": "bar"});
        let payload = value_to_payload(val).unwrap();
        let roundtrip = payload_to_value(&payload);
        assert_eq!(roundtrip["foo"], "bar");
    }
}
