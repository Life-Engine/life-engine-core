//! Host function injection gating.
//!
//! Builds the set of Extism `Function` objects for a plugin based on its
//! approved capabilities. This is the first layer of capability enforcement:
//! unapproved host functions simply don't exist in the WASM sandbox, so the
//! plugin cannot call them at all.
//!
//! `host_log` is ALWAYS injected regardless of capabilities — all plugins can log.

use std::sync::Arc;

use extism::{Function, PTR, UserData};
use life_engine_traits::{Capability, StorageBackend};
use life_engine_workflow_engine::WorkflowEventEmitter;
use tracing::debug;

use crate::capability::ApprovedCapabilities;
use crate::host_functions::config::ConfigHostContext;
use crate::host_functions::events::EventsHostContext;
use crate::host_functions::http::HttpHostContext;
use crate::host_functions::logging::{LogRateLimiter, LoggingHostContext};
use crate::host_functions::storage::StorageHostContext;

/// Resources needed to construct host function contexts.
pub struct InjectionDeps {
    /// Shared storage backend.
    pub storage: Arc<dyn StorageBackend>,
    /// Shared workflow event emitter.
    pub event_bus: Arc<dyn WorkflowEventEmitter>,
    /// Shared log rate limiter.
    pub log_rate_limiter: Arc<LogRateLimiter>,
    /// Per-plugin config section (if any).
    pub plugin_config: Option<serde_json::Value>,
}

/// Builds the set of Extism host functions for a plugin based on its approved
/// capabilities.
///
/// The mapping is:
/// - `storage:read` -> `host_storage_read`
/// - `storage:write` -> `host_storage_write`
/// - `http:outbound` -> `host_http_request`
/// - `events:emit` -> `host_events_emit`
/// - `events:subscribe` -> `host_events_subscribe`
/// - `config:read` -> `host_config_read`
/// - (always) -> `host_log`
pub fn build_host_functions(
    plugin_id: &str,
    capabilities: &ApprovedCapabilities,
    deps: &InjectionDeps,
) -> Vec<Function> {
    let mut functions = Vec::new();

    // Always inject host_log — no capability required
    functions.push(build_log_function(plugin_id, &deps.log_rate_limiter));

    if capabilities.has(Capability::StorageRead) {
        functions.push(build_storage_read_function(
            plugin_id,
            capabilities,
            &deps.storage,
        ));
        debug!(plugin_id = %plugin_id, "injected host_storage_read");
    }

    if capabilities.has(Capability::StorageWrite) {
        functions.push(build_storage_write_function(
            plugin_id,
            capabilities,
            &deps.storage,
        ));
        debug!(plugin_id = %plugin_id, "injected host_storage_write");
    }

    if capabilities.has(Capability::StorageDelete) {
        functions.push(build_storage_delete_function(
            plugin_id,
            capabilities,
            &deps.storage,
        ));
        debug!(plugin_id = %plugin_id, "injected host_storage_delete");
    }

    if capabilities.has(Capability::HttpOutbound) {
        functions.push(build_http_request_function(plugin_id, capabilities));
        debug!(plugin_id = %plugin_id, "injected host_http_request");
    }

    if capabilities.has(Capability::EventsEmit) {
        functions.push(build_events_emit_function(
            plugin_id,
            capabilities,
            &deps.event_bus,
        ));
        debug!(plugin_id = %plugin_id, "injected host_events_emit");
    }

    if capabilities.has(Capability::EventsSubscribe) {
        functions.push(build_events_subscribe_function(
            plugin_id,
            capabilities,
            &deps.event_bus,
        ));
        debug!(plugin_id = %plugin_id, "injected host_events_subscribe");
    }

    if capabilities.has(Capability::ConfigRead) {
        functions.push(build_config_read_function(
            plugin_id,
            capabilities,
            &deps.plugin_config,
        ));
        debug!(plugin_id = %plugin_id, "injected host_config_read");
    }

    debug!(
        plugin_id = %plugin_id,
        count = functions.len(),
        "host function injection complete"
    );

    functions
}

/// Returns the list of host function names that would be injected for the given
/// capabilities (useful for testing without constructing full deps).
pub fn injected_function_names(capabilities: &ApprovedCapabilities) -> Vec<&'static str> {
    let mut names = vec!["host_log"];

    if capabilities.has(Capability::StorageRead) {
        names.push("host_storage_read");
    }
    if capabilities.has(Capability::StorageWrite) {
        names.push("host_storage_write");
    }
    if capabilities.has(Capability::StorageDelete) {
        names.push("host_storage_delete");
    }
    if capabilities.has(Capability::StorageBlobRead) {
        names.push("host_blob_retrieve");
    }
    if capabilities.has(Capability::StorageBlobWrite) {
        names.push("host_blob_store");
    }
    if capabilities.has(Capability::StorageBlobDelete) {
        names.push("host_blob_delete");
    }
    if capabilities.has(Capability::HttpOutbound) {
        names.push("host_http_request");
    }
    if capabilities.has(Capability::EventsEmit) {
        names.push("host_events_emit");
    }
    if capabilities.has(Capability::EventsSubscribe) {
        names.push("host_events_subscribe");
    }
    if capabilities.has(Capability::ConfigRead) {
        names.push("host_config_read");
    }

    names
}

// --- Individual function builders ---

fn build_log_function(plugin_id: &str, rate_limiter: &Arc<LogRateLimiter>) -> Function {
    let ctx = LoggingHostContext {
        plugin_id: plugin_id.to_string(),
        rate_limiter: Arc::clone(rate_limiter),
    };

    Function::new(
        "host_log",
        [PTR],
        [PTR],
        UserData::new(ctx),
        |plugin, inputs, outputs, user_data| {
            let data = user_data.get()?;
            let ctx = data.lock().unwrap();
            let input: Vec<u8> = plugin.memory_get_val(&inputs[0])?;

            let result = crate::host_functions::logging::host_log(&ctx, &input);

            match result {
                Ok(output) => {
                    plugin.memory_set_val(&mut outputs[0], output)?;
                    Ok(())
                }
                Err(e) => Err(extism::Error::msg(e.to_string())),
            }
        },
    )
    .with_namespace("life_engine")
}

fn build_storage_read_function(
    plugin_id: &str,
    capabilities: &ApprovedCapabilities,
    storage: &Arc<dyn StorageBackend>,
) -> Function {
    let ctx = StorageHostContext {
        plugin_id: plugin_id.to_string(),
        capabilities: capabilities.clone(),
        storage: Arc::clone(storage),
    };

    Function::new(
        "host_storage_read",
        [PTR],
        [PTR],
        UserData::new(ctx),
        |plugin, inputs, outputs, user_data| {
            let data = user_data.get()?;
            let ctx = data.lock().unwrap();
            let input: Vec<u8> = plugin.memory_get_val(&inputs[0])?;

            let rt = tokio::runtime::Handle::try_current()
                .map(|h| h.block_on(crate::host_functions::storage::host_storage_read(&ctx, &input)))
                .unwrap_or_else(|_| {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(crate::host_functions::storage::host_storage_read(&ctx, &input))
                });

            match rt {
                Ok(output) => {
                    plugin.memory_set_val(&mut outputs[0], output)?;
                    Ok(())
                }
                Err(e) => Err(extism::Error::msg(e.to_string())),
            }
        },
    )
    .with_namespace("life_engine")
}

fn build_storage_write_function(
    plugin_id: &str,
    capabilities: &ApprovedCapabilities,
    storage: &Arc<dyn StorageBackend>,
) -> Function {
    let ctx = StorageHostContext {
        plugin_id: plugin_id.to_string(),
        capabilities: capabilities.clone(),
        storage: Arc::clone(storage),
    };

    Function::new(
        "host_storage_write",
        [PTR],
        [PTR],
        UserData::new(ctx),
        |plugin, inputs, outputs, user_data| {
            let data = user_data.get()?;
            let ctx = data.lock().unwrap();
            let input: Vec<u8> = plugin.memory_get_val(&inputs[0])?;

            let rt = tokio::runtime::Handle::try_current()
                .map(|h| h.block_on(crate::host_functions::storage::host_storage_write(&ctx, &input)))
                .unwrap_or_else(|_| {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(crate::host_functions::storage::host_storage_write(&ctx, &input))
                });

            match rt {
                Ok(output) => {
                    plugin.memory_set_val(&mut outputs[0], output)?;
                    Ok(())
                }
                Err(e) => Err(extism::Error::msg(e.to_string())),
            }
        },
    )
    .with_namespace("life_engine")
}

fn build_storage_delete_function(
    plugin_id: &str,
    capabilities: &ApprovedCapabilities,
    storage: &Arc<dyn StorageBackend>,
) -> Function {
    let ctx = StorageHostContext {
        plugin_id: plugin_id.to_string(),
        capabilities: capabilities.clone(),
        storage: Arc::clone(storage),
    };

    Function::new(
        "host_storage_delete",
        [PTR],
        [PTR],
        UserData::new(ctx),
        |plugin, inputs, outputs, user_data| {
            let data = user_data.get()?;
            let ctx = data.lock().unwrap();
            let input: Vec<u8> = plugin.memory_get_val(&inputs[0])?;

            let rt = tokio::runtime::Handle::try_current()
                .map(|h| h.block_on(crate::host_functions::storage::host_storage_delete(&ctx, &input)))
                .unwrap_or_else(|_| {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(crate::host_functions::storage::host_storage_delete(&ctx, &input))
                });

            match rt {
                Ok(output) => {
                    plugin.memory_set_val(&mut outputs[0], output)?;
                    Ok(())
                }
                Err(e) => Err(extism::Error::msg(e.to_string())),
            }
        },
    )
    .with_namespace("life_engine")
}

fn build_http_request_function(
    plugin_id: &str,
    capabilities: &ApprovedCapabilities,
) -> Function {
    let ctx = HttpHostContext {
        plugin_id: plugin_id.to_string(),
        capabilities: capabilities.clone(),
        client: reqwest::Client::new(),
        allowed_domains: None,
    };

    Function::new(
        "host_http_request",
        [PTR],
        [PTR],
        UserData::new(ctx),
        |plugin, inputs, outputs, user_data| {
            let data = user_data.get()?;
            let ctx = data.lock().unwrap();
            let input: Vec<u8> = plugin.memory_get_val(&inputs[0])?;

            let rt = tokio::runtime::Handle::try_current()
                .map(|h| h.block_on(crate::host_functions::http::host_http_request(&ctx, &input)))
                .unwrap_or_else(|_| {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(crate::host_functions::http::host_http_request(&ctx, &input))
                });

            match rt {
                Ok(output) => {
                    plugin.memory_set_val(&mut outputs[0], output)?;
                    Ok(())
                }
                Err(e) => Err(extism::Error::msg(e.to_string())),
            }
        },
    )
    .with_namespace("life_engine")
}

fn build_events_emit_function(
    plugin_id: &str,
    capabilities: &ApprovedCapabilities,
    event_bus: &Arc<dyn WorkflowEventEmitter>,
) -> Function {
    let ctx = EventsHostContext {
        plugin_id: plugin_id.to_string(),
        capabilities: capabilities.clone(),
        event_bus: Arc::clone(event_bus),
        declared_emit_events: None,
        execution_depth: 0,
    };

    Function::new(
        "host_events_emit",
        [PTR],
        [PTR],
        UserData::new(ctx),
        |plugin, inputs, outputs, user_data| {
            let data = user_data.get()?;
            let ctx = data.lock().unwrap();
            let input: Vec<u8> = plugin.memory_get_val(&inputs[0])?;

            let rt = tokio::runtime::Handle::try_current()
                .map(|h| h.block_on(crate::host_functions::events::host_events_emit(&ctx, &input)))
                .unwrap_or_else(|_| {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(crate::host_functions::events::host_events_emit(&ctx, &input))
                });

            match rt {
                Ok(output) => {
                    plugin.memory_set_val(&mut outputs[0], output)?;
                    Ok(())
                }
                Err(e) => Err(extism::Error::msg(e.to_string())),
            }
        },
    )
    .with_namespace("life_engine")
}

fn build_events_subscribe_function(
    plugin_id: &str,
    capabilities: &ApprovedCapabilities,
    event_bus: &Arc<dyn WorkflowEventEmitter>,
) -> Function {
    let ctx = EventsHostContext {
        plugin_id: plugin_id.to_string(),
        capabilities: capabilities.clone(),
        event_bus: Arc::clone(event_bus),
        declared_emit_events: None,
        execution_depth: 0,
    };

    Function::new(
        "host_events_subscribe",
        [PTR],
        [PTR],
        UserData::new(ctx),
        |plugin, inputs, outputs, user_data| {
            let data = user_data.get()?;
            let ctx = data.lock().unwrap();
            let input: Vec<u8> = plugin.memory_get_val(&inputs[0])?;

            let rt = tokio::runtime::Handle::try_current()
                .map(|h| {
                    h.block_on(crate::host_functions::events::host_events_subscribe(
                        &ctx, &input,
                    ))
                })
                .unwrap_or_else(|_| {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(crate::host_functions::events::host_events_subscribe(
                        &ctx, &input,
                    ))
                });

            match rt {
                Ok(output) => {
                    plugin.memory_set_val(&mut outputs[0], output)?;
                    Ok(())
                }
                Err(e) => Err(extism::Error::msg(e.to_string())),
            }
        },
    )
    .with_namespace("life_engine")
}

fn build_config_read_function(
    plugin_id: &str,
    capabilities: &ApprovedCapabilities,
    plugin_config: &Option<serde_json::Value>,
) -> Function {
    let ctx = ConfigHostContext {
        plugin_id: plugin_id.to_string(),
        capabilities: capabilities.clone(),
        plugin_config: plugin_config.clone(),
    };

    Function::new(
        "host_config_read",
        [PTR],
        [PTR],
        UserData::new(ctx),
        |plugin, _inputs, outputs, user_data| {
            let data = user_data.get()?;
            let ctx = data.lock().unwrap();

            match crate::host_functions::config::host_config_read(&ctx) {
                Ok(output) => {
                    plugin.memory_set_val(&mut outputs[0], output)?;
                    Ok(())
                }
                Err(e) => Err(extism::Error::msg(e.to_string())),
            }
        },
    )
    .with_namespace("life_engine")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn make_capabilities(caps: &[Capability]) -> ApprovedCapabilities {
        let set: HashSet<Capability> = caps.iter().copied().collect();
        ApprovedCapabilities::new(set)
    }

    #[test]
    fn no_capabilities_gets_only_logging() {
        let caps = ApprovedCapabilities::empty();
        let names = injected_function_names(&caps);
        assert_eq!(names, vec!["host_log"]);
    }

    #[test]
    fn storage_read_only_gets_read_not_write() {
        let caps = make_capabilities(&[Capability::StorageRead]);
        let names = injected_function_names(&caps);
        assert!(names.contains(&"host_log"));
        assert!(names.contains(&"host_storage_read"));
        assert!(!names.contains(&"host_storage_write"));
    }

    #[test]
    fn storage_write_without_read_does_not_get_read() {
        let caps = make_capabilities(&[Capability::StorageWrite]);
        let names = injected_function_names(&caps);
        assert!(names.contains(&"host_log"));
        assert!(names.contains(&"host_storage_write"));
        assert!(!names.contains(&"host_storage_read"));
    }

    #[test]
    fn all_capabilities_gets_all_functions() {
        let caps = make_capabilities(&[
            Capability::StorageRead,
            Capability::StorageWrite,
            Capability::StorageDelete,
            Capability::StorageBlobRead,
            Capability::StorageBlobWrite,
            Capability::StorageBlobDelete,
            Capability::HttpOutbound,
            Capability::EventsEmit,
            Capability::EventsSubscribe,
            Capability::ConfigRead,
        ]);
        let names = injected_function_names(&caps);
        assert_eq!(names.len(), 11); // 10 capabilities + host_log
        assert!(names.contains(&"host_log"));
        assert!(names.contains(&"host_storage_read"));
        assert!(names.contains(&"host_storage_write"));
        assert!(names.contains(&"host_storage_delete"));
        assert!(names.contains(&"host_blob_retrieve"));
        assert!(names.contains(&"host_blob_store"));
        assert!(names.contains(&"host_blob_delete"));
        assert!(names.contains(&"host_http_request"));
        assert!(names.contains(&"host_events_emit"));
        assert!(names.contains(&"host_events_subscribe"));
        assert!(names.contains(&"host_config_read"));
    }

    #[test]
    fn host_log_is_always_present() {
        // Test various combinations — host_log should always be first
        let combos: Vec<Vec<Capability>> = vec![
            vec![],
            vec![Capability::StorageRead],
            vec![Capability::HttpOutbound, Capability::EventsEmit],
            vec![Capability::ConfigRead],
        ];

        for caps_list in combos {
            let caps = make_capabilities(&caps_list);
            let names = injected_function_names(&caps);
            assert_eq!(names[0], "host_log", "host_log must always be first");
        }
    }
}
