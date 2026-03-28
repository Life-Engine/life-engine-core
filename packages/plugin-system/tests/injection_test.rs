//! Integration tests for host function injection gating (WP 8.15).
//!
//! Validates that the injection layer correctly maps approved capabilities
//! to host functions and that `host_log` is always present.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use life_engine_plugin_system::injection::{build_host_functions, injected_function_names, InjectionDeps};
use life_engine_plugin_system::capability::ApprovedCapabilities;
use life_engine_plugin_system::host_functions::logging::LogRateLimiter;
use life_engine_traits::{Capability, EngineError, StorageBackend};
use life_engine_types::{PipelineMessage, StorageMutation, StorageQuery};
use life_engine_workflow_engine::WorkflowEventEmitter;

// --- Mock StorageBackend ---

struct MockStorage;

#[async_trait]
impl StorageBackend for MockStorage {
    async fn execute(
        &self,
        _query: StorageQuery,
    ) -> Result<Vec<PipelineMessage>, Box<dyn EngineError>> {
        Ok(vec![])
    }

    async fn mutate(&self, _op: StorageMutation) -> Result<(), Box<dyn EngineError>> {
        Ok(())
    }

    async fn init(
        _config: toml::Value,
        _key: [u8; 32],
    ) -> Result<Self, Box<dyn EngineError>> {
        Ok(MockStorage)
    }
}

// --- Mock EventBus ---

struct MockEventBus {
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
    async fn emit(&self, event_name: &str, payload: serde_json::Value, _depth: u32) {
        self.emit_calls
            .lock()
            .unwrap()
            .push((event_name.to_string(), payload));
    }
}

// --- Helpers ---

fn make_capabilities(caps: &[Capability]) -> ApprovedCapabilities {
    let set: HashSet<Capability> = caps.iter().copied().collect();
    ApprovedCapabilities::new(set)
}

fn make_deps() -> InjectionDeps {
    InjectionDeps {
        storage: Arc::new(MockStorage),
        event_bus: Arc::new(MockEventBus::new()),
        log_rate_limiter: Arc::new(LogRateLimiter::new()),
        plugin_config: None,
        blob_storage: None,
        allowed_domains: None,
        declared_emit_events: None,
        declared_subscribe_events: None,
        execution_depth: 0,
    }
}

// --- Tests ---

#[test]
fn plugin_with_storage_read_only_gets_storage_read_and_logging() {
    let caps = make_capabilities(&[Capability::StorageRead]);
    let deps = make_deps();
    let functions = build_host_functions("test-plugin", &caps, &deps);

    let names: Vec<&str> = functions.iter().map(|f| f.name()).collect();
    assert!(names.contains(&"host_log"), "host_log must always be present");
    assert!(names.contains(&"host_storage_read"), "storage:read should inject host_storage_read");
    assert!(!names.contains(&"host_storage_write"), "storage:write should NOT be injected");
    assert_eq!(functions.len(), 2); // host_log + host_storage_read
}

#[test]
fn plugin_with_storage_write_without_read_does_not_get_read() {
    let caps = make_capabilities(&[Capability::StorageWrite]);
    let deps = make_deps();
    let functions = build_host_functions("test-plugin", &caps, &deps);

    let names: Vec<&str> = functions.iter().map(|f| f.name()).collect();
    assert!(names.contains(&"host_log"));
    assert!(names.contains(&"host_storage_write"));
    assert!(!names.contains(&"host_storage_read"));
    assert_eq!(functions.len(), 2);
}

#[test]
fn plugin_with_no_capabilities_gets_only_logging() {
    let caps = ApprovedCapabilities::empty();
    let deps = make_deps();
    let functions = build_host_functions("test-plugin", &caps, &deps);

    let names: Vec<&str> = functions.iter().map(|f| f.name()).collect();
    assert_eq!(names, vec!["host_log"]);
    assert_eq!(functions.len(), 1);
}

#[test]
fn plugin_with_all_capabilities_gets_all_host_functions() {
    let caps = make_capabilities(&[
        Capability::StorageRead,
        Capability::StorageWrite,
        Capability::HttpOutbound,
        Capability::EventsEmit,
        Capability::EventsSubscribe,
        Capability::ConfigRead,
    ]);
    let deps = make_deps();
    let functions = build_host_functions("test-plugin", &caps, &deps);

    let names: Vec<&str> = functions.iter().map(|f| f.name()).collect();
    assert_eq!(functions.len(), 7); // 6 capabilities + host_log
    assert!(names.contains(&"host_log"));
    assert!(names.contains(&"host_storage_read"));
    assert!(names.contains(&"host_storage_write"));
    assert!(names.contains(&"host_http_request"));
    assert!(names.contains(&"host_events_emit"));
    assert!(names.contains(&"host_events_subscribe"));
    assert!(names.contains(&"host_config_read"));
}

#[test]
fn host_log_is_always_present_regardless_of_capabilities() {
    let test_cases: Vec<Vec<Capability>> = vec![
        vec![],
        vec![Capability::StorageRead],
        vec![Capability::HttpOutbound],
        vec![Capability::EventsEmit, Capability::EventsSubscribe],
        vec![Capability::ConfigRead],
        vec![
            Capability::StorageRead,
            Capability::StorageWrite,
            Capability::HttpOutbound,
            Capability::EventsEmit,
            Capability::EventsSubscribe,
            Capability::ConfigRead,
        ],
    ];

    let deps = make_deps();

    for cap_list in test_cases {
        let caps = make_capabilities(&cap_list);
        let functions = build_host_functions("test-plugin", &caps, &deps);
        let names: Vec<&str> = functions.iter().map(|f| f.name()).collect();
        assert!(
            names.contains(&"host_log"),
            "host_log missing for capabilities: {cap_list:?}"
        );
    }
}

#[test]
fn all_injected_functions_have_life_engine_namespace() {
    let caps = make_capabilities(&[
        Capability::StorageRead,
        Capability::StorageWrite,
        Capability::HttpOutbound,
        Capability::EventsEmit,
        Capability::EventsSubscribe,
        Capability::ConfigRead,
    ]);
    let deps = make_deps();
    let functions = build_host_functions("test-plugin", &caps, &deps);

    for func in &functions {
        assert_eq!(
            func.namespace(),
            Some("life_engine"),
            "function '{}' should have 'life_engine' namespace",
            func.name()
        );
    }
}

#[test]
fn injected_function_names_matches_build_host_functions() {
    let caps = make_capabilities(&[Capability::StorageRead, Capability::HttpOutbound]);
    let deps = make_deps();

    let expected_names = injected_function_names(&caps);
    let functions = build_host_functions("test-plugin", &caps, &deps);
    let actual_names: Vec<&str> = functions.iter().map(|f| f.name()).collect();

    assert_eq!(expected_names, actual_names);
}
