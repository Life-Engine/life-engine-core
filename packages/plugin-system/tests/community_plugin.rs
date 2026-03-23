//! Community plugin loading integration tests (WP 8.20).
//!
//! End-to-end tests validating that community (third-party) plugins are
//! discovered, approved, and loaded through the same pipeline as first-party
//! plugins, with capability enforcement as the only differentiator.
//!
//! Covers requirements 9.1, 9.2, and 9.3 from the plugin-system spec.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use life_engine_plugin_system::host_functions::logging::LogRateLimiter;
use life_engine_plugin_system::injection::injected_function_names;
use life_engine_plugin_system::loader::{load_plugins, LoaderConfig};
use life_engine_traits::{Capability, EngineError, StorageBackend};
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

fn manifest_no_capabilities(id: &str) -> String {
    format!(
        r#"
[plugin]
id = "{id}"
name = "Test Plugin {id}"
version = "1.0.0"

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
// Test 1: Community plugin discovered via same mechanism as first-party (9.1)
// ===========================================================================

#[test]
fn community_plugin_discovered_in_same_plugins_directory() {
    let tmp = TempDir::new().unwrap();
    let first_party_dir = tmp.path().join("first-party");
    let community_dir = tmp.path().join("community");
    std::fs::create_dir_all(&first_party_dir).unwrap();
    std::fs::create_dir_all(&community_dir).unwrap();

    let wasm = echo_wasm_module();

    // Place a community plugin in its own directory
    create_plugin_dir(
        &community_dir,
        "community-scanner",
        &manifest_no_capabilities("community-scanner"),
        &wasm,
    );

    // No capabilities needed — plugin declares none
    let config = make_config(HashMap::new());

    let handles = load_plugins(
        &community_dir,
        &first_party_dir,
        &config,
        mock_storage(),
        mock_event_bus(),
        log_limiter(),
    )
    .unwrap();

    // Community plugin is discovered and loaded through the same pipeline
    assert_eq!(
        handles.len(),
        1,
        "community plugin should be discovered and loaded"
    );
    assert_eq!(handles[0].manifest.plugin.id, "community-scanner");
    assert_eq!(handles[0].manifest.plugin.version, "1.0.0");
}

// ===========================================================================
// Test 2: Community plugin requires explicit capability approval (9.2)
// ===========================================================================

#[test]
fn community_plugin_rejected_without_explicit_approval() {
    let tmp = TempDir::new().unwrap();
    let first_party_dir = tmp.path().join("first-party");
    let community_dir = tmp.path().join("community");
    std::fs::create_dir_all(&first_party_dir).unwrap();
    std::fs::create_dir_all(&community_dir).unwrap();

    let wasm = echo_wasm_module();

    // Community plugin declares storage:read but has no config approval
    create_plugin_dir(
        &community_dir,
        "unapproved-plugin",
        &manifest_with_capabilities("unapproved-plugin", &["storage:read"]),
        &wasm,
    );

    // Empty approved capabilities — nothing approved
    let config = make_config(HashMap::new());

    let handles = load_plugins(
        &community_dir,
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
        "community plugin with unapproved capabilities must not load"
    );
}

// ===========================================================================
// Test 3: Approved community plugin gets same host functions as first-party (9.3)
// ===========================================================================

#[test]
fn approved_community_plugin_gets_same_host_functions_as_first_party() {
    let tmp = TempDir::new().unwrap();
    let first_party_dir = tmp.path().join("first-party");
    let community_dir = tmp.path().join("community");
    std::fs::create_dir_all(&first_party_dir).unwrap();
    std::fs::create_dir_all(&community_dir).unwrap();

    let wasm = echo_wasm_module();
    let declared_caps = &["storage:read", "storage:write", "http:outbound"];

    // --- First-party plugin with the same capabilities ---
    create_plugin_dir(
        &first_party_dir,
        "fp-plugin",
        &manifest_with_capabilities("fp-plugin", declared_caps),
        &wasm,
    );

    let fp_config = make_config(HashMap::new());
    let fp_handles = load_plugins(
        &first_party_dir,
        &first_party_dir,
        &fp_config,
        mock_storage(),
        mock_event_bus(),
        log_limiter(),
    )
    .unwrap();

    assert_eq!(fp_handles.len(), 1);
    let fp_fn_names = injected_function_names(&fp_handles[0].capabilities);

    // --- Community plugin with the same capabilities, explicitly approved ---
    create_plugin_dir(
        &community_dir,
        "comm-plugin",
        &manifest_with_capabilities("comm-plugin", declared_caps),
        &wasm,
    );

    let mut approved = HashMap::new();
    approved.insert(
        "comm-plugin".to_string(),
        vec![
            Capability::StorageRead,
            Capability::StorageWrite,
            Capability::HttpOutbound,
        ],
    );
    let comm_config = make_config(approved);

    let comm_handles = load_plugins(
        &community_dir,
        &first_party_dir,
        &comm_config,
        mock_storage(),
        mock_event_bus(),
        log_limiter(),
    )
    .unwrap();

    assert_eq!(
        comm_handles.len(),
        1,
        "approved community plugin should load"
    );
    let comm_fn_names = injected_function_names(&comm_handles[0].capabilities);

    // Both should get the exact same set of host functions
    assert_eq!(
        fp_fn_names, comm_fn_names,
        "approved community plugin must get the same host functions as a first-party plugin with the same capabilities"
    );

    // Verify the expected functions are present
    assert!(comm_fn_names.contains(&"host_log"));
    assert!(comm_fn_names.contains(&"host_storage_read"));
    assert!(comm_fn_names.contains(&"host_storage_write"));
    assert!(comm_fn_names.contains(&"host_http_request"));
}

// ===========================================================================
// Test 4: Community plugin coexists with first-party plugins
// ===========================================================================

#[test]
fn community_and_first_party_plugins_coexist() {
    let tmp = TempDir::new().unwrap();
    let shared_dir = tmp.path().join("plugins");
    std::fs::create_dir_all(&shared_dir).unwrap();

    let wasm = echo_wasm_module();

    // First-party plugin (in same dir as first_party_dir)
    create_plugin_dir(
        &shared_dir,
        "builtin-plugin",
        &manifest_with_capabilities("builtin-plugin", &["storage:read", "storage:write"]),
        &wasm,
    );

    // Another first-party plugin
    create_plugin_dir(
        &shared_dir,
        "core-plugin",
        &manifest_no_capabilities("core-plugin"),
        &wasm,
    );

    // Simulate a community plugin in the same directory — it's first-party
    // because it's inside the first_party_dir. For a true third-party test,
    // we need separate dirs. But this tests coexistence in a shared plugins dir.
    create_plugin_dir(
        &shared_dir,
        "addon-plugin",
        &manifest_with_capabilities("addon-plugin", &["config:read"]),
        &wasm,
    );

    let config = make_config(HashMap::new());

    let handles = load_plugins(
        &shared_dir,
        &shared_dir,
        &config,
        mock_storage(),
        mock_event_bus(),
        log_limiter(),
    )
    .unwrap();

    assert_eq!(handles.len(), 3, "all three plugins should load");
    let ids: Vec<&str> = handles
        .iter()
        .map(|h| h.manifest.plugin.id.as_str())
        .collect();
    assert!(ids.contains(&"builtin-plugin"));
    assert!(ids.contains(&"core-plugin"));
    assert!(ids.contains(&"addon-plugin"));
}

// ===========================================================================
// Test 5: Community plugin partial approval — only approved caps granted
// ===========================================================================

#[test]
fn community_plugin_partial_approval_rejects_entirely() {
    let tmp = TempDir::new().unwrap();
    let first_party_dir = tmp.path().join("first-party");
    let community_dir = tmp.path().join("community");
    std::fs::create_dir_all(&first_party_dir).unwrap();
    std::fs::create_dir_all(&community_dir).unwrap();

    let wasm = echo_wasm_module();

    // Plugin declares three capabilities
    create_plugin_dir(
        &community_dir,
        "partial-plugin",
        &manifest_with_capabilities(
            "partial-plugin",
            &["storage:read", "storage:write", "http:outbound"],
        ),
        &wasm,
    );

    // Only approve two of the three
    let mut approved = HashMap::new();
    approved.insert(
        "partial-plugin".to_string(),
        vec![Capability::StorageRead, Capability::StorageWrite],
    );
    let config = make_config(approved);

    let handles = load_plugins(
        &community_dir,
        &first_party_dir,
        &config,
        mock_storage(),
        mock_event_bus(),
        log_limiter(),
    )
    .unwrap();

    // Plugin is rejected entirely because http:outbound is unapproved
    assert_eq!(
        handles.len(),
        0,
        "plugin with partially approved capabilities must be rejected entirely"
    );
}

// ===========================================================================
// Test 6: Community plugin approval then full loading lifecycle
// ===========================================================================

#[test]
fn community_plugin_approval_enables_full_loading() {
    let tmp = TempDir::new().unwrap();
    let first_party_dir = tmp.path().join("first-party");
    let community_dir = tmp.path().join("community");
    std::fs::create_dir_all(&first_party_dir).unwrap();
    std::fs::create_dir_all(&community_dir).unwrap();

    let wasm = echo_wasm_module();

    create_plugin_dir(
        &community_dir,
        "approved-comm",
        &manifest_with_capabilities(
            "approved-comm",
            &["storage:read", "events:emit", "config:read"],
        ),
        &wasm,
    );

    // Approve all declared capabilities
    let mut approved = HashMap::new();
    approved.insert(
        "approved-comm".to_string(),
        vec![
            Capability::StorageRead,
            Capability::EventsEmit,
            Capability::ConfigRead,
        ],
    );
    let config = make_config(approved);

    let handles = load_plugins(
        &community_dir,
        &first_party_dir,
        &config,
        mock_storage(),
        mock_event_bus(),
        log_limiter(),
    )
    .unwrap();

    assert_eq!(handles.len(), 1, "fully approved community plugin should load");

    let caps = &handles[0].capabilities;
    assert!(caps.has(Capability::StorageRead));
    assert!(caps.has(Capability::EventsEmit));
    assert!(caps.has(Capability::ConfigRead));
    assert_eq!(caps.len(), 3);

    // Verify host function injection matches approved capabilities
    let fn_names = injected_function_names(caps);
    assert!(fn_names.contains(&"host_log"), "host_log always injected");
    assert!(fn_names.contains(&"host_storage_read"));
    assert!(fn_names.contains(&"host_events_emit"));
    assert!(fn_names.contains(&"host_config_read"));
    assert_eq!(fn_names.len(), 4, "3 capability functions + host_log");

    // Unapproved functions must NOT be present
    assert!(!fn_names.contains(&"host_storage_write"));
    assert!(!fn_names.contains(&"host_http_request"));
    assert!(!fn_names.contains(&"host_events_subscribe"));
}
