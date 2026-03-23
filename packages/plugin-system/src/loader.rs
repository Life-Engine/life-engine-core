//! Plugin loader orchestrating the full loading flow.
//!
//! Coordinates discovery, manifest parsing, capability approval, host function
//! construction, and WASM loading to produce ready-to-use plugin handles.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use life_engine_traits::{Capability, StorageBackend};
use life_engine_workflow_engine::WorkflowEventEmitter;
use tracing::{info, warn};

use crate::capability::{check_capability_approval, ApprovedCapabilities};
use crate::discovery::scan_plugins_directory;
use crate::error::PluginError;
use crate::host_functions::logging::LogRateLimiter;
use crate::manifest::{parse_manifest, PluginManifest};
use crate::runtime::{load_plugin, PluginInstance};

/// A successfully loaded plugin with its instance, manifest, and approved capabilities.
#[derive(Debug)]
pub struct PluginHandle {
    /// The loaded WASM plugin instance.
    pub instance: PluginInstance,
    /// The parsed plugin manifest.
    pub manifest: PluginManifest,
    /// The set of approved capabilities for this plugin.
    pub capabilities: ApprovedCapabilities,
}

/// Configuration for the plugin loader.
///
/// This is a simplified config struct used by the loader until the full
/// Core config system is wired up (WP 8.13).
pub struct LoaderConfig {
    /// Per-plugin approved capabilities, keyed by plugin ID.
    pub approved_capabilities: HashMap<String, Vec<Capability>>,
    /// Per-plugin config sections, keyed by plugin ID.
    pub plugin_configs: HashMap<String, serde_json::Value>,
}

/// Loads all plugins from a directory, orchestrating the full loading flow.
///
/// For each discovered plugin subdirectory:
/// 1. Parses the manifest
/// 2. Checks capability approval (first-party auto-grant, third-party config check)
/// 3. Loads the WASM binary with capability-matched host functions
///
/// Plugins that fail any step are skipped with a warning — one bad plugin
/// does not prevent others from loading.
pub fn load_plugins(
    plugins_dir: &Path,
    first_party_dir: &Path,
    config: &LoaderConfig,
    _storage: Arc<dyn StorageBackend>,
    _event_bus: Arc<dyn WorkflowEventEmitter>,
    log_rate_limiter: Arc<LogRateLimiter>,
) -> Result<Vec<PluginHandle>, PluginError> {
    let discovered = scan_plugins_directory(plugins_dir)?;

    if discovered.is_empty() {
        info!(directory = %plugins_dir.display(), "no plugins discovered");
        return Ok(vec![]);
    }

    let mut handles = Vec::new();

    for plugin in &discovered {
        match load_single_plugin(plugin, first_party_dir, config, &log_rate_limiter) {
            Ok(handle) => {
                let cap_names: Vec<String> =
                    handle.capabilities.iter().map(|c| c.to_string()).collect();
                info!(
                    plugin_id = %handle.manifest.plugin.id,
                    version = %handle.manifest.plugin.version,
                    capabilities = %cap_names.join(", "),
                    "loaded plugin"
                );
                handles.push(handle);
            }
            Err(e) => {
                warn!(
                    directory = %plugin.path.display(),
                    error = %e,
                    "skipping plugin due to loading error"
                );
            }
        }
    }

    info!(
        loaded = handles.len(),
        total = discovered.len(),
        "plugin loading complete"
    );

    Ok(handles)
}

/// Attempts to load a single plugin through the full pipeline.
fn load_single_plugin(
    plugin: &crate::discovery::DiscoveredPlugin,
    first_party_dir: &Path,
    config: &LoaderConfig,
    _log_rate_limiter: &Arc<LogRateLimiter>,
) -> Result<PluginHandle, PluginError> {
    // Step 1: Parse manifest
    let manifest = parse_manifest(&plugin.manifest_path)?;
    let plugin_id = &manifest.plugin.id;

    // Step 2: Check capability approval
    let approved_caps = config
        .approved_capabilities
        .get(plugin_id)
        .map(|v| v.as_slice())
        .unwrap_or(&[]);

    let capabilities =
        check_capability_approval(&manifest, &plugin.path, first_party_dir, approved_caps)?;

    // Step 3: Build host functions based on approved capabilities
    // Host functions are registered as Extism Functions. For now, we pass an
    // empty list — the actual Extism host function wiring requires Extism
    // UserData and will be completed in WP 8.15 (Host Function Injection Gating).
    // The loader's job is to determine WHICH functions to inject; the actual
    // function objects are constructed by the injection layer.
    let host_functions = Vec::new();

    // Step 4: Load WASM binary
    let instance = load_plugin(&plugin.wasm_path, plugin_id, host_functions)?;

    Ok(PluginHandle {
        instance,
        manifest,
        capabilities,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Mutex;

    use async_trait::async_trait;
    use life_engine_traits::EngineError;
    use life_engine_types::{PipelineMessage, StorageMutation, StorageQuery};
    use tempfile::TempDir;

    // --- Minimal WASM module ---

    fn minimal_wasm_module() -> Vec<u8> {
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
        .expect("failed to compile WAT to WASM")
    }

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
        async fn emit(&self, event_name: &str, payload: serde_json::Value) {
            self.emit_calls
                .lock()
                .unwrap()
                .push((event_name.to_string(), payload));
        }
    }

    // --- Helpers ---

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

    fn valid_manifest(id: &str) -> String {
        format!(
            r#"
[plugin]
id = "{id}"
name = "Test Plugin {id}"
version = "1.0.0"
description = "A test plugin"

[actions.greet]
description = "Greet action"
"#
        )
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
"#
        )
    }

    fn make_config(
        approved: HashMap<String, Vec<Capability>>,
    ) -> LoaderConfig {
        LoaderConfig {
            approved_capabilities: approved,
            plugin_configs: HashMap::new(),
        }
    }

    fn mock_storage() -> Arc<dyn StorageBackend> {
        Arc::new(MockStorage)
    }

    fn mock_event_bus() -> Arc<dyn WorkflowEventEmitter> {
        Arc::new(MockEventBus::new())
    }

    fn log_limiter() -> Arc<LogRateLimiter> {
        Arc::new(LogRateLimiter::new())
    }

    // --- Tests ---

    #[test]
    fn valid_plugin_loads_successfully() {
        let tmp = TempDir::new().unwrap();
        let plugins_dir = tmp.path().join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();

        let wasm = minimal_wasm_module();
        create_plugin_dir(&plugins_dir, "my-plugin", &valid_manifest("my-plugin"), &wasm);

        let config = make_config(HashMap::new());
        let result = load_plugins(
            &plugins_dir,
            &plugins_dir, // first-party dir = plugins dir
            &config,
            mock_storage(),
            mock_event_bus(),
            log_limiter(),
        );

        let handles = result.unwrap();
        assert_eq!(handles.len(), 1);
        assert_eq!(handles[0].manifest.plugin.id, "my-plugin");
        assert_eq!(handles[0].manifest.plugin.version, "1.0.0");
    }

    #[test]
    fn missing_manifest_skips_with_warning() {
        let tmp = TempDir::new().unwrap();
        let plugins_dir = tmp.path().join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();

        // Create a directory with only plugin.wasm (no manifest.toml)
        let bad_dir = plugins_dir.join("bad-plugin");
        std::fs::create_dir_all(&bad_dir).unwrap();
        std::fs::write(bad_dir.join("plugin.wasm"), &minimal_wasm_module()).unwrap();

        // Create a valid plugin too
        let wasm = minimal_wasm_module();
        create_plugin_dir(&plugins_dir, "good-plugin", &valid_manifest("good-plugin"), &wasm);

        let config = make_config(HashMap::new());
        let result = load_plugins(
            &plugins_dir,
            &plugins_dir,
            &config,
            mock_storage(),
            mock_event_bus(),
            log_limiter(),
        );

        let handles = result.unwrap();
        // Only the valid plugin loaded; the one without manifest was skipped by scanner
        assert_eq!(handles.len(), 1);
        assert_eq!(handles[0].manifest.plugin.id, "good-plugin");
    }

    #[test]
    fn unapproved_third_party_capability_rejects_plugin() {
        let tmp = TempDir::new().unwrap();
        let plugins_dir = tmp.path().join("plugins");
        let third_party_dir = tmp.path().join("third-party");
        std::fs::create_dir_all(&plugins_dir).unwrap();
        std::fs::create_dir_all(&third_party_dir).unwrap();

        let wasm = minimal_wasm_module();
        create_plugin_dir(
            &third_party_dir,
            "ext-plugin",
            &manifest_with_capabilities("ext-plugin", &["storage:read", "storage:write"]),
            &wasm,
        );

        // Only approve storage:read, not storage:write
        let mut approved = HashMap::new();
        approved.insert(
            "ext-plugin".to_string(),
            vec![Capability::StorageRead],
        );
        let config = make_config(approved);

        let result = load_plugins(
            &third_party_dir,
            &plugins_dir, // first-party dir is different
            &config,
            mock_storage(),
            mock_event_bus(),
            log_limiter(),
        );

        let handles = result.unwrap();
        // Plugin should be skipped due to unapproved capability
        assert_eq!(handles.len(), 0);
    }

    #[test]
    fn corrupt_wasm_binary_skips_without_aborting() {
        let tmp = TempDir::new().unwrap();
        let plugins_dir = tmp.path().join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();

        // Create a plugin with corrupt WASM
        let corrupt_dir = plugins_dir.join("corrupt-plugin");
        std::fs::create_dir_all(&corrupt_dir).unwrap();
        std::fs::write(corrupt_dir.join("manifest.toml"), valid_manifest("corrupt-plugin")).unwrap();
        std::fs::write(corrupt_dir.join("plugin.wasm"), b"not valid wasm").unwrap();

        // Create a valid plugin
        let wasm = minimal_wasm_module();
        create_plugin_dir(&plugins_dir, "good-plugin", &valid_manifest("good-plugin"), &wasm);

        let config = make_config(HashMap::new());
        let result = load_plugins(
            &plugins_dir,
            &plugins_dir,
            &config,
            mock_storage(),
            mock_event_bus(),
            log_limiter(),
        );

        let handles = result.unwrap();
        // Corrupt plugin is skipped; good plugin loads
        assert_eq!(handles.len(), 1);
        assert_eq!(handles[0].manifest.plugin.id, "good-plugin");
    }

    #[test]
    fn multiple_plugins_load_independently() {
        let tmp = TempDir::new().unwrap();
        let plugins_dir = tmp.path().join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();

        let wasm = minimal_wasm_module();
        create_plugin_dir(&plugins_dir, "alpha-plugin", &valid_manifest("alpha-plugin"), &wasm);
        create_plugin_dir(&plugins_dir, "beta-plugin", &valid_manifest("beta-plugin"), &wasm);
        create_plugin_dir(&plugins_dir, "gamma-plugin", &valid_manifest("gamma-plugin"), &wasm);

        let config = make_config(HashMap::new());
        let result = load_plugins(
            &plugins_dir,
            &plugins_dir,
            &config,
            mock_storage(),
            mock_event_bus(),
            log_limiter(),
        );

        let handles = result.unwrap();
        assert_eq!(handles.len(), 3);

        let ids: Vec<&str> = handles.iter().map(|h| h.manifest.plugin.id.as_str()).collect();
        assert!(ids.contains(&"alpha-plugin"));
        assert!(ids.contains(&"beta-plugin"));
        assert!(ids.contains(&"gamma-plugin"));
    }

    #[test]
    fn first_party_plugins_get_all_capabilities_auto_granted() {
        let tmp = TempDir::new().unwrap();
        let plugins_dir = tmp.path().join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();

        let wasm = minimal_wasm_module();
        create_plugin_dir(
            &plugins_dir,
            "first-party",
            &manifest_with_capabilities(
                "first-party",
                &["storage:read", "storage:write", "http:outbound"],
            ),
            &wasm,
        );

        // No approved_capabilities in config — first-party doesn't need them
        let config = make_config(HashMap::new());
        let result = load_plugins(
            &plugins_dir,
            &plugins_dir, // first-party dir matches plugins dir
            &config,
            mock_storage(),
            mock_event_bus(),
            log_limiter(),
        );

        let handles = result.unwrap();
        assert_eq!(handles.len(), 1);

        let caps = &handles[0].capabilities;
        assert!(caps.has(Capability::StorageRead));
        assert!(caps.has(Capability::StorageWrite));
        assert!(caps.has(Capability::HttpOutbound));
        assert_eq!(caps.len(), 3);
    }

    #[test]
    fn empty_plugins_directory_returns_empty_vec() {
        let tmp = TempDir::new().unwrap();
        let plugins_dir = tmp.path().join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();

        let config = make_config(HashMap::new());
        let result = load_plugins(
            &plugins_dir,
            &plugins_dir,
            &config,
            mock_storage(),
            mock_event_bus(),
            log_limiter(),
        );

        let handles = result.unwrap();
        assert!(handles.is_empty());
    }

    #[test]
    fn failure_of_one_plugin_does_not_affect_others() {
        let tmp = TempDir::new().unwrap();
        let plugins_dir = tmp.path().join("plugins");
        let third_party_dir = tmp.path().join("third-party");
        std::fs::create_dir_all(&plugins_dir).unwrap();
        std::fs::create_dir_all(&third_party_dir).unwrap();

        let wasm = minimal_wasm_module();

        // Good first-party plugin
        create_plugin_dir(&plugins_dir, "good-plugin", &valid_manifest("good-plugin"), &wasm);

        // Bad plugin with corrupt WASM (also first-party)
        let bad_dir = plugins_dir.join("bad-plugin");
        std::fs::create_dir_all(&bad_dir).unwrap();
        std::fs::write(bad_dir.join("manifest.toml"), valid_manifest("bad-plugin")).unwrap();
        std::fs::write(bad_dir.join("plugin.wasm"), b"corrupt").unwrap();

        // Another good plugin
        create_plugin_dir(
            &plugins_dir,
            "other-plugin",
            &valid_manifest("other-plugin"),
            &wasm,
        );

        let config = make_config(HashMap::new());
        let result = load_plugins(
            &plugins_dir,
            &plugins_dir,
            &config,
            mock_storage(),
            mock_event_bus(),
            log_limiter(),
        );

        let handles = result.unwrap();
        // 2 good plugins loaded, 1 bad plugin skipped
        assert_eq!(handles.len(), 2);
        let ids: Vec<&str> = handles.iter().map(|h| h.manifest.plugin.id.as_str()).collect();
        assert!(ids.contains(&"good-plugin"));
        assert!(ids.contains(&"other-plugin"));
        assert!(!ids.contains(&"bad-plugin"));
    }
}
