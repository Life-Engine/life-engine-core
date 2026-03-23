//! Capability enforcement integration tests (WP 8.18).
//!
//! End-to-end tests that validate capability enforcement through the full
//! plugin loading pipeline: first-party plugins auto-grant all declared
//! capabilities, third-party plugins require explicit approval, unapproved
//! capabilities prevent loading (CAP_001), and modifying config to approve
//! a capability allows the plugin to load on the next attempt.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use life_engine_plugin_system::injection::injected_function_names;
use life_engine_plugin_system::loader::{LoaderConfig, load_plugins};
use life_engine_plugin_system::host_functions::logging::LogRateLimiter;
use life_engine_traits::{Capability, EngineError, Severity, StorageBackend};
use life_engine_types::{PipelineMessage, StorageMutation, StorageQuery};
use life_engine_workflow_engine::WorkflowEventEmitter;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// WASM fixture
// ---------------------------------------------------------------------------

/// A minimal WASM module with a `greet` export that echoes input.
fn echo_wasm_module() -> Vec<u8> {
    wat::parse_str(
        r#"
        (module
            (import "extism:host/env" "input_length" (func $input_length (result i64)))
            (import "extism:host/env" "input_load_u8" (func $input_load_u8 (param i64) (result i32)))
            (import "extism:host/env" "output_set" (func $output_set (param i64 i64)))
            (import "extism:host/env" "length" (func $length (param i64) (result i64)))
            (import "extism:host/env" "alloc" (func $alloc (param i64) (result i64)))
            (import "extism:host/env" "store_u8" (func $store_u8 (param i64 i32)))

            (memory (export "memory") 1)

            (func (export "greet") (result i32)
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
    )
    .expect("failed to compile echo WAT to WASM")
}

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

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
    async fn emit(&self, event_name: &str, payload: serde_json::Value) {
        self.emit_calls
            .lock()
            .unwrap()
            .push((event_name.to_string(), payload));
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn mock_storage() -> Arc<dyn StorageBackend> {
    Arc::new(MockStorage)
}

fn mock_event_bus() -> Arc<dyn WorkflowEventEmitter> {
    Arc::new(MockEventBus::new())
}

fn log_limiter() -> Arc<LogRateLimiter> {
    Arc::new(LogRateLimiter::new())
}

fn create_plugin_dir(
    parent: &Path,
    name: &str,
    manifest_toml: &str,
    wasm: &[u8],
) -> std::path::PathBuf {
    let dir = parent.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("manifest.toml"), manifest_toml).unwrap();
    std::fs::write(dir.join("plugin.wasm"), wasm).unwrap();
    dir
}

fn manifest_with_capabilities(id: &str, caps: &[&str]) -> String {
    let caps_str = caps
        .iter()
        .map(|c| format!("\"{c}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        r#"
[plugin]
id = "{id}"
name = "Test Plugin {id}"
version = "1.0.0"

[capabilities]
required = [{caps_str}]

[actions.greet]
description = "Greet action"
"#
    )
}

fn make_config(approved: HashMap<String, Vec<Capability>>) -> LoaderConfig {
    LoaderConfig {
        approved_capabilities: approved,
        plugin_configs: HashMap::new(),
    }
}

// ===========================================================================
// Test 1: First-party plugin gets all declared capabilities auto-granted
// ===========================================================================

#[test]
fn first_party_plugin_auto_granted_all_declared_capabilities() {
    let tmp = TempDir::new().unwrap();
    let first_party_dir = tmp.path().join("plugins");
    std::fs::create_dir_all(&first_party_dir).unwrap();

    let wasm = echo_wasm_module();
    create_plugin_dir(
        &first_party_dir,
        "first-party-plugin",
        &manifest_with_capabilities(
            "first-party-plugin",
            &[
                "storage:read",
                "storage:write",
                "http:outbound",
                "events:emit",
                "events:subscribe",
                "config:read",
            ],
        ),
        &wasm,
    );

    // No approved_capabilities in config — first-party doesn't need them
    let config = make_config(HashMap::new());
    let handles = load_plugins(
        &first_party_dir,
        &first_party_dir,
        &config,
        mock_storage(),
        mock_event_bus(),
        log_limiter(),
    )
    .unwrap();

    assert_eq!(handles.len(), 1, "first-party plugin should load");
    let caps = &handles[0].capabilities;
    assert!(caps.has(Capability::StorageRead));
    assert!(caps.has(Capability::StorageWrite));
    assert!(caps.has(Capability::HttpOutbound));
    assert!(caps.has(Capability::EventsEmit));
    assert!(caps.has(Capability::EventsSubscribe));
    assert!(caps.has(Capability::ConfigRead));
    assert_eq!(caps.len(), 6, "all 6 declared capabilities should be granted");

    // Verify all host functions would be injected
    let fn_names = injected_function_names(caps);
    assert_eq!(fn_names.len(), 7, "6 capability functions + host_log");
    assert!(fn_names.contains(&"host_log"));
    assert!(fn_names.contains(&"host_storage_read"));
    assert!(fn_names.contains(&"host_storage_write"));
    assert!(fn_names.contains(&"host_http_request"));
    assert!(fn_names.contains(&"host_events_emit"));
    assert!(fn_names.contains(&"host_events_subscribe"));
    assert!(fn_names.contains(&"host_config_read"));
}

// ===========================================================================
// Test 2: Third-party approved operations succeed
// ===========================================================================

#[test]
fn third_party_approved_capabilities_load_successfully() {
    let tmp = TempDir::new().unwrap();
    let first_party_dir = tmp.path().join("plugins");
    let third_party_dir = tmp.path().join("third-party");
    std::fs::create_dir_all(&first_party_dir).unwrap();
    std::fs::create_dir_all(&third_party_dir).unwrap();

    let wasm = echo_wasm_module();
    create_plugin_dir(
        &third_party_dir,
        "community-plugin",
        &manifest_with_capabilities("community-plugin", &["storage:read"]),
        &wasm,
    );

    let mut approved = HashMap::new();
    approved.insert(
        "community-plugin".to_string(),
        vec![Capability::StorageRead],
    );
    let config = make_config(approved);

    let handles = load_plugins(
        &third_party_dir,
        &first_party_dir,
        &config,
        mock_storage(),
        mock_event_bus(),
        log_limiter(),
    )
    .unwrap();

    assert_eq!(handles.len(), 1, "approved third-party plugin should load");
    let caps = &handles[0].capabilities;
    assert!(caps.has(Capability::StorageRead), "approved capability should be present");
    assert_eq!(caps.len(), 1);

    // Verify only approved host functions would be injected
    let fn_names = injected_function_names(caps);
    assert!(fn_names.contains(&"host_storage_read"));
    assert!(fn_names.contains(&"host_log"));
    assert_eq!(fn_names.len(), 2, "only host_log + host_storage_read");
}

// ===========================================================================
// Test 3: Third-party unapproved operations are not available (Fatal)
// ===========================================================================

#[test]
fn third_party_plugin_lacks_unapproved_capabilities() {
    let tmp = TempDir::new().unwrap();
    let first_party_dir = tmp.path().join("plugins");
    let third_party_dir = tmp.path().join("third-party");
    std::fs::create_dir_all(&first_party_dir).unwrap();
    std::fs::create_dir_all(&third_party_dir).unwrap();

    // Plugin declares only storage:read — that's all it can get
    let wasm = echo_wasm_module();
    create_plugin_dir(
        &third_party_dir,
        "limited-plugin",
        &manifest_with_capabilities("limited-plugin", &["storage:read"]),
        &wasm,
    );

    let mut approved = HashMap::new();
    approved.insert(
        "limited-plugin".to_string(),
        vec![Capability::StorageRead],
    );
    let config = make_config(approved);

    let handles = load_plugins(
        &third_party_dir,
        &first_party_dir,
        &config,
        mock_storage(),
        mock_event_bus(),
        log_limiter(),
    )
    .unwrap();

    assert_eq!(handles.len(), 1);
    let caps = &handles[0].capabilities;

    // The plugin does NOT have storage:write, http:outbound, etc.
    assert!(
        !caps.has(Capability::StorageWrite),
        "unapproved capability should not be present"
    );
    assert!(!caps.has(Capability::HttpOutbound));
    assert!(!caps.has(Capability::EventsEmit));
    assert!(!caps.has(Capability::EventsSubscribe));
    assert!(!caps.has(Capability::ConfigRead));

    // Injection gating: unapproved host functions are not injected
    let fn_names = injected_function_names(caps);
    assert!(
        !fn_names.contains(&"host_storage_write"),
        "host_storage_write must NOT be injected for unapproved capability"
    );
    assert!(!fn_names.contains(&"host_http_request"));
    assert!(!fn_names.contains(&"host_events_emit"));
    assert!(!fn_names.contains(&"host_events_subscribe"));
    assert!(!fn_names.contains(&"host_config_read"));
}

// ===========================================================================
// Test 4: Third-party with unapproved manifest capability refuses to load
//         entirely with CAP_001
// ===========================================================================

#[test]
fn third_party_unapproved_manifest_capability_refuses_to_load_with_cap_001() {
    let tmp = TempDir::new().unwrap();
    let first_party_dir = tmp.path().join("plugins");
    let third_party_dir = tmp.path().join("third-party");
    std::fs::create_dir_all(&first_party_dir).unwrap();
    std::fs::create_dir_all(&third_party_dir).unwrap();

    let wasm = echo_wasm_module();

    // Plugin declares storage:read AND storage:write
    create_plugin_dir(
        &third_party_dir,
        "greedy-plugin",
        &manifest_with_capabilities("greedy-plugin", &["storage:read", "storage:write"]),
        &wasm,
    );

    // Config only approves storage:read — storage:write is unapproved
    let mut approved = HashMap::new();
    approved.insert(
        "greedy-plugin".to_string(),
        vec![Capability::StorageRead],
    );
    let config = make_config(approved);

    let handles = load_plugins(
        &third_party_dir,
        &first_party_dir,
        &config,
        mock_storage(),
        mock_event_bus(),
        log_limiter(),
    )
    .unwrap();

    // Plugin should be rejected entirely — not loaded
    assert_eq!(
        handles.len(),
        0,
        "plugin with unapproved manifest capability must not load"
    );

    // Verify that the rejection is specifically CAP_001 by testing directly
    use life_engine_plugin_system::capability::check_capability_approval;
    use life_engine_plugin_system::manifest::{CapabilitySet, PluginManifest, PluginMeta};

    let manifest = PluginManifest {
        plugin: PluginMeta {
            id: "greedy-plugin".to_string(),
            name: "Test Plugin greedy-plugin".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
        },
        actions: HashMap::new(),
        capabilities: CapabilitySet {
            required: vec![Capability::StorageRead, Capability::StorageWrite],
        },
        config: None,
    };

    let plugin_path = third_party_dir.join("greedy-plugin");
    let result = check_capability_approval(
        &manifest,
        &plugin_path,
        &first_party_dir,
        &[Capability::StorageRead],
    );

    let err = result.unwrap_err();
    assert_eq!(err.code(), "CAP_001", "rejection must use CAP_001 error code");
    assert_eq!(err.severity(), Severity::Fatal, "CAP_001 must be Fatal");
    assert!(
        err.to_string().contains("storage:write"),
        "error should name the unapproved capability: {}",
        err
    );
    assert!(
        err.to_string().contains("greedy-plugin"),
        "error should name the plugin: {}",
        err
    );
}

// ===========================================================================
// Test 5: Modifying config to approve the capability allows load on restart
// ===========================================================================

#[test]
fn approving_capability_in_config_allows_plugin_to_load() {
    let tmp = TempDir::new().unwrap();
    let first_party_dir = tmp.path().join("plugins");
    let third_party_dir = tmp.path().join("third-party");
    std::fs::create_dir_all(&first_party_dir).unwrap();
    std::fs::create_dir_all(&third_party_dir).unwrap();

    let wasm = echo_wasm_module();
    create_plugin_dir(
        &third_party_dir,
        "pending-plugin",
        &manifest_with_capabilities("pending-plugin", &["storage:read", "storage:write"]),
        &wasm,
    );

    // --- First load: only storage:read approved → plugin rejected ---
    let mut approved = HashMap::new();
    approved.insert(
        "pending-plugin".to_string(),
        vec![Capability::StorageRead],
    );
    let config = make_config(approved);

    let handles = load_plugins(
        &third_party_dir,
        &first_party_dir,
        &config,
        mock_storage(),
        mock_event_bus(),
        log_limiter(),
    )
    .unwrap();

    assert_eq!(
        handles.len(),
        0,
        "plugin should be rejected on first load (storage:write not approved)"
    );

    // --- Second load: approve storage:write too → plugin loads ---
    let mut approved_full = HashMap::new();
    approved_full.insert(
        "pending-plugin".to_string(),
        vec![Capability::StorageRead, Capability::StorageWrite],
    );
    let config_updated = make_config(approved_full);

    let handles = load_plugins(
        &third_party_dir,
        &first_party_dir,
        &config_updated,
        mock_storage(),
        mock_event_bus(),
        log_limiter(),
    )
    .unwrap();

    assert_eq!(
        handles.len(),
        1,
        "plugin should load after approving the missing capability"
    );

    let caps = &handles[0].capabilities;
    assert!(caps.has(Capability::StorageRead));
    assert!(caps.has(Capability::StorageWrite));
    assert_eq!(caps.len(), 2);
}
