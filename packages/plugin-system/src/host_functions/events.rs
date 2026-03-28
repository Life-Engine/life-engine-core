//! Events emit and subscribe host functions for WASM plugins.
//!
//! These host functions allow plugins to emit events via the workflow engine's
//! event bus and to register interest in specific event names. Each function
//! checks the plugin's approved capabilities before delegating to the
//! `WorkflowEventEmitter` trait.

use std::sync::Arc;

use life_engine_traits::Capability;
use life_engine_workflow_engine::WorkflowEventEmitter;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::capability::ApprovedCapabilities;
use crate::error::PluginError;

/// Context passed to events host functions, containing the plugin's identity,
/// approved capabilities, and a reference to the event bus.
#[derive(Clone)]
pub struct EventsHostContext {
    /// The plugin ID making the events call.
    pub plugin_id: String,
    /// The plugin's approved capabilities.
    pub capabilities: ApprovedCapabilities,
    /// Shared reference to the workflow event emitter.
    pub event_bus: Arc<dyn WorkflowEventEmitter>,
    /// Event names this plugin declared in its manifest `[events.emit]` section.
    /// If `None`, no manifest validation is performed (backwards compatibility).
    pub declared_emit_events: Option<Vec<String>>,
    /// Current execution depth for cascading event tracking.
    pub execution_depth: u32,
}

/// Request payload for emitting an event from a plugin.
#[derive(Debug, Deserialize, Serialize)]
pub struct EmitRequest {
    /// The event name to emit.
    pub event_name: String,
    /// The event payload as a JSON value.
    pub payload: serde_json::Value,
}

/// Request payload for subscribing to an event from a plugin.
#[derive(Debug, Deserialize, Serialize)]
pub struct SubscribeRequest {
    /// The event name to subscribe to.
    pub event_name: String,
}

/// Emits an event on behalf of a plugin via the workflow event bus.
///
/// Deserializes the event name and payload from JSON bytes, checks the
/// `EventsEmit` capability, and delegates to the `WorkflowEventEmitter`.
pub async fn host_events_emit(
    ctx: &EventsHostContext,
    input: &[u8],
) -> Result<Vec<u8>, PluginError> {
    // Check capability
    if !ctx.capabilities.has(Capability::EventsEmit) {
        warn!(
            plugin_id = %ctx.plugin_id,
            "events:emit capability violation"
        );
        return Err(PluginError::RuntimeCapabilityViolation(format!(
            "plugin '{}' lacks capability 'events:emit'",
            ctx.plugin_id
        )));
    }

    // Deserialize the emit request from WASM input
    let request: EmitRequest = serde_json::from_slice(input).map_err(|e| {
        PluginError::ExecutionFailed(format!(
            "failed to deserialize emit request from plugin '{}': {e}",
            ctx.plugin_id
        ))
    })?;

    // Validate against manifest-declared emit events
    if let Some(ref declared) = ctx.declared_emit_events
        && !declared.contains(&request.event_name)
    {
        warn!(
            plugin_id = %ctx.plugin_id,
            event_name = %request.event_name,
            "event not declared in manifest"
        );
        return Err(PluginError::RuntimeCapabilityViolation(format!(
            "plugin '{}' attempted to emit undeclared event '{}'",
            ctx.plugin_id, request.event_name
        )));
    }

    debug!(
        plugin_id = %ctx.plugin_id,
        event_name = %request.event_name,
        depth = ctx.execution_depth,
        "emitting event"
    );

    // Build the event payload with source and depth set by the host
    let enriched_payload = serde_json::json!({
        "source": ctx.plugin_id,
        "depth": ctx.execution_depth,
        "payload": request.payload,
    });

    // Delegate to the event bus
    ctx.event_bus
        .emit(&request.event_name, enriched_payload)
        .await;

    // Return empty JSON object as success acknowledgement
    Ok(b"{}".to_vec())
}

/// Registers a plugin's interest in a specific event name.
///
/// Deserializes the event name from JSON bytes and checks the
/// `EventsSubscribe` capability. The actual event delivery happens through
/// workflow triggers, not direct callbacks into WASM — subscribe is
/// declarative registration of interest.
pub async fn host_events_subscribe(
    ctx: &EventsHostContext,
    input: &[u8],
) -> Result<Vec<u8>, PluginError> {
    // Check capability
    if !ctx.capabilities.has(Capability::EventsSubscribe) {
        warn!(
            plugin_id = %ctx.plugin_id,
            "events:subscribe capability violation"
        );
        return Err(PluginError::RuntimeCapabilityViolation(format!(
            "plugin '{}' lacks capability 'events:subscribe'",
            ctx.plugin_id
        )));
    }

    // Deserialize the subscribe request from WASM input
    let request: SubscribeRequest = serde_json::from_slice(input).map_err(|e| {
        PluginError::ExecutionFailed(format!(
            "failed to deserialize subscribe request from plugin '{}': {e}",
            ctx.plugin_id
        ))
    })?;

    debug!(
        plugin_id = %ctx.plugin_id,
        event_name = %request.event_name,
        "registering event subscription"
    );

    // Emit a subscription registration event so the workflow engine can route
    // future occurrences of this event to workflows involving this plugin.
    ctx.event_bus
        .emit(
            "plugin.subscription.registered",
            serde_json::json!({
                "plugin_id": ctx.plugin_id,
                "event_name": request.event_name,
            }),
        )
        .await;

    // Return empty JSON object as success acknowledgement
    Ok(b"{}".to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashSet;
    use std::sync::Mutex;

    use async_trait::async_trait;

    // --- Mock event bus ---

    struct MockEventBus {
        /// Records of emit calls: (event_name, payload).
        emit_calls: Mutex<Vec<(String, serde_json::Value)>>,
    }

    impl MockEventBus {
        fn new() -> Self {
            Self {
                emit_calls: Mutex::new(vec![]),
            }
        }
    }

    #[async_trait]
    impl WorkflowEventEmitter for MockEventBus {
        async fn emit(&self, event_name: &str, payload: serde_json::Value) {
            self.emit_calls
                .lock()
                .unwrap()
                .push((event_name.to_string(), payload));
        }
    }

    // --- Helper functions ---

    fn make_capabilities(caps: &[Capability]) -> ApprovedCapabilities {
        let set: HashSet<Capability> = caps.iter().copied().collect();
        ApprovedCapabilities::new(set)
    }

    fn make_context(
        plugin_id: &str,
        caps: &[Capability],
        event_bus: Arc<dyn WorkflowEventEmitter>,
    ) -> EventsHostContext {
        EventsHostContext {
            plugin_id: plugin_id.to_string(),
            capabilities: make_capabilities(caps),
            event_bus,
            declared_emit_events: None,
            execution_depth: 0,
        }
    }

    fn make_context_with_declared_events(
        plugin_id: &str,
        caps: &[Capability],
        event_bus: Arc<dyn WorkflowEventEmitter>,
        declared_events: Vec<String>,
    ) -> EventsHostContext {
        EventsHostContext {
            plugin_id: plugin_id.to_string(),
            capabilities: make_capabilities(caps),
            event_bus,
            declared_emit_events: Some(declared_events),
            execution_depth: 0,
        }
    }

    fn make_emit_bytes(event_name: &str, payload: serde_json::Value) -> Vec<u8> {
        serde_json::to_vec(&EmitRequest {
            event_name: event_name.to_string(),
            payload,
        })
        .unwrap()
    }

    fn make_subscribe_bytes(event_name: &str) -> Vec<u8> {
        serde_json::to_vec(&SubscribeRequest {
            event_name: event_name.to_string(),
        })
        .unwrap()
    }

    // --- Tests ---

    #[tokio::test]
    async fn emit_succeeds_with_events_emit_capability() {
        let bus = Arc::new(MockEventBus::new());
        let ctx = make_context("test-plugin", &[Capability::EventsEmit], bus.clone());

        let payload = serde_json::json!({"key": "value", "count": 42});
        let input = make_emit_bytes("contact.created", payload.clone());
        let result = host_events_emit(&ctx, &input).await;

        assert!(result.is_ok(), "emit should succeed: {result:?}");
        assert_eq!(result.unwrap(), b"{}");

        // Verify the event bus was called correctly (payload is enriched with source/depth)
        let calls = bus.emit_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "contact.created");
        assert_eq!(calls[0].1["source"], "test-plugin");
        assert_eq!(calls[0].1["depth"], 0);
        assert_eq!(calls[0].1["payload"], payload);
    }

    #[tokio::test]
    async fn subscribe_succeeds_with_events_subscribe_capability() {
        let bus = Arc::new(MockEventBus::new());
        let ctx = make_context("test-plugin", &[Capability::EventsSubscribe], bus.clone());

        let input = make_subscribe_bytes("email.received");
        let result = host_events_subscribe(&ctx, &input).await;

        assert!(result.is_ok(), "subscribe should succeed: {result:?}");
        assert_eq!(result.unwrap(), b"{}");

        // Verify the event bus received the subscription registration
        let calls = bus.emit_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "plugin.subscription.registered");
        assert_eq!(calls[0].1["plugin_id"], "test-plugin");
        assert_eq!(calls[0].1["event_name"], "email.received");
    }

    #[tokio::test]
    async fn emit_without_events_emit_returns_capability_error() {
        let bus = Arc::new(MockEventBus::new());
        // Plugin has subscribe but NOT emit
        let ctx = make_context("test-plugin", &[Capability::EventsSubscribe], bus.clone());

        let input = make_emit_bytes("some.event", serde_json::json!({}));
        let result = host_events_emit(&ctx, &input).await;

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::RuntimeCapabilityViolation(_)));
        assert!(err.to_string().contains("events:emit"));
        assert!(err.to_string().contains("test-plugin"));

        // Verify no events were emitted
        let calls = bus.emit_calls.lock().unwrap();
        assert!(calls.is_empty());
    }

    #[tokio::test]
    async fn subscribe_without_events_subscribe_returns_capability_error() {
        let bus = Arc::new(MockEventBus::new());
        // Plugin has emit but NOT subscribe
        let ctx = make_context("test-plugin", &[Capability::EventsEmit], bus.clone());

        let input = make_subscribe_bytes("some.event");
        let result = host_events_subscribe(&ctx, &input).await;

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::RuntimeCapabilityViolation(_)));
        assert!(err.to_string().contains("events:subscribe"));
        assert!(err.to_string().contains("test-plugin"));

        // Verify no events were emitted
        let calls = bus.emit_calls.lock().unwrap();
        assert!(calls.is_empty());
    }

    #[tokio::test]
    async fn emitted_event_is_received_by_event_bus() {
        let bus = Arc::new(MockEventBus::new());
        let ctx = make_context("my-plugin", &[Capability::EventsEmit], bus.clone());

        let payload = serde_json::json!({
            "contact_id": "c-123",
            "name": "Alice",
            "source": "email-import"
        });
        let input = make_emit_bytes("contact.updated", payload.clone());
        let _ = host_events_emit(&ctx, &input).await;

        let calls = bus.emit_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "contact.updated");
        assert_eq!(calls[0].1["source"], "my-plugin");
        assert_eq!(calls[0].1["depth"], 0);
        assert_eq!(calls[0].1["payload"]["contact_id"], "c-123");
        assert_eq!(calls[0].1["payload"]["name"], "Alice");
    }

    #[tokio::test]
    async fn invalid_emit_input_returns_execution_error() {
        let bus = Arc::new(MockEventBus::new());
        let ctx = make_context("test-plugin", &[Capability::EventsEmit], bus);

        let result = host_events_emit(&ctx, b"not valid json").await;

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ExecutionFailed(_)));
        assert!(err.to_string().contains("deserialize"));
    }

    #[tokio::test]
    async fn invalid_subscribe_input_returns_execution_error() {
        let bus = Arc::new(MockEventBus::new());
        let ctx = make_context("test-plugin", &[Capability::EventsSubscribe], bus);

        let result = host_events_subscribe(&ctx, b"not valid json").await;

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ExecutionFailed(_)));
        assert!(err.to_string().contains("deserialize"));
    }

    #[tokio::test]
    async fn emit_undeclared_event_returns_capability_error() {
        let bus = Arc::new(MockEventBus::new());
        let ctx = make_context_with_declared_events(
            "test-plugin",
            &[Capability::EventsEmit],
            bus.clone(),
            vec!["contact.created".to_string(), "contact.updated".to_string()],
        );

        // Try to emit an event not in the declared list
        let input = make_emit_bytes("task.deleted", serde_json::json!({}));
        let result = host_events_emit(&ctx, &input).await;

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::RuntimeCapabilityViolation(_)));
        assert!(err.to_string().contains("undeclared event"));
        assert!(err.to_string().contains("task.deleted"));

        // Verify no events were emitted
        let calls = bus.emit_calls.lock().unwrap();
        assert!(calls.is_empty());
    }

    #[tokio::test]
    async fn emit_declared_event_succeeds() {
        let bus = Arc::new(MockEventBus::new());
        let ctx = make_context_with_declared_events(
            "test-plugin",
            &[Capability::EventsEmit],
            bus.clone(),
            vec!["contact.created".to_string()],
        );

        let input = make_emit_bytes("contact.created", serde_json::json!({"id": "c-1"}));
        let result = host_events_emit(&ctx, &input).await;

        assert!(result.is_ok(), "declared event should succeed: {result:?}");
    }

    #[tokio::test]
    async fn emitted_event_includes_source_and_depth() {
        let bus = Arc::new(MockEventBus::new());
        let mut ctx = make_context("my-plugin", &[Capability::EventsEmit], bus.clone());
        ctx.execution_depth = 2;

        let payload = serde_json::json!({"contact_id": "c-123"});
        let input = make_emit_bytes("contact.updated", payload);
        let _ = host_events_emit(&ctx, &input).await;

        let calls = bus.emit_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "contact.updated");
        assert_eq!(calls[0].1["source"], "my-plugin");
        assert_eq!(calls[0].1["depth"], 2);
        assert_eq!(calls[0].1["payload"]["contact_id"], "c-123");
    }
}
