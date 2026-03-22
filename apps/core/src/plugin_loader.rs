//! Plugin discovery, loading, lifecycle management, and error isolation.
//!
//! In Phase 1, plugins are native Rust types implementing `CorePlugin`.
//! They are registered programmatically via `register()` and discovered
//! from configured directory paths by scanning for `plugin.json` manifests.
//! WASM-based plugin loading is deferred to Phase 4.

use crate::credential_bridge::PluginCredentialBridge;
use crate::schema_registry::SchemaRegistry;
use life_engine_plugin_sdk::credential_store::CredentialStore;
use life_engine_plugin_sdk::types::PluginContext;
use life_engine_plugin_sdk::CorePlugin;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{error, info, warn};

/// Status of a loaded plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginStatus {
    /// Plugin is registered but not yet loaded.
    Registered,
    /// Plugin is loaded and running.
    Loaded,
    /// Plugin failed to load.
    Failed(String),
    /// Plugin has been unloaded.
    Unloaded,
}

/// Metadata about a loaded plugin, exposed to the health endpoint.
#[derive(Debug, Clone)]
pub struct PluginInfo {
    /// Unique plugin identifier.
    pub id: String,
    /// Human-readable display name.
    pub display_name: String,
    /// Plugin version string.
    pub version: String,
    /// Current lifecycle status.
    pub status: PluginStatus,
}

/// A plugin manifest parsed from a `plugin.json` file.
///
/// This struct represents the on-disk declaration of a plugin. It shares
/// some fields with `PluginInfo` (`id`, `display_name`, `version`) but
/// serves a different purpose: `PluginManifest` is the file-based declaration
/// read at discovery time, while `PluginInfo` is the runtime metadata tracked
/// after registration. They are intentionally kept separate.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginManifest {
    /// Unique plugin identifier in reverse-domain notation (e.g. `com.example.my-plugin`).
    pub id: String,
    /// Human-readable display name.
    pub display_name: String,
    /// Plugin version in semver format (`X.Y.Z`).
    pub version: String,
    /// A description of what this plugin does.
    #[serde(default)]
    pub description: String,
    /// Capabilities this plugin requests from Core.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Minimum Core version this plugin is compatible with.
    #[serde(default)]
    pub min_core_version: Option<String>,
    /// The entry point file for this plugin.
    #[serde(default)]
    pub entry_point: Option<String>,
    /// Private collection definitions with inline JSON Schema.
    #[serde(default)]
    pub collections: Vec<ManifestCollection>,
}

/// A private collection declared in a plugin manifest.
#[derive(Debug, Clone, Deserialize)]
pub struct ManifestCollection {
    /// Collection name (e.g. `recipes`). Namespaced as `{plugin_id}/{name}` on registration.
    pub name: String,
    /// Inline JSON Schema definition for validating records in this collection.
    pub schema: serde_json::Value,
}

/// Read and validate a `plugin.json` manifest from disk.
///
/// Returns a validated `PluginManifest` or an error with a clear
/// description of what is wrong with the manifest.
pub fn read_manifest(path: &Path) -> anyhow::Result<PluginManifest> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("failed to read manifest at {}: {e}", path.display()))?;

    let manifest: PluginManifest = serde_json::from_str(&contents)
        .map_err(|e| anyhow::anyhow!("failed to parse manifest at {}: {e}", path.display()))?;

    // Validate required fields are non-empty.
    if manifest.id.is_empty() {
        return Err(anyhow::anyhow!(
            "manifest at {} has empty 'id' field",
            path.display()
        ));
    }

    // Validate reverse-domain notation (must contain at least one dot).
    if !manifest.id.contains('.') {
        return Err(anyhow::anyhow!(
            "manifest at {} has invalid 'id' format '{}': must use reverse-domain notation (e.g. com.example.plugin)",
            path.display(),
            manifest.id
        ));
    }

    if manifest.display_name.is_empty() {
        return Err(anyhow::anyhow!(
            "manifest at {} has empty 'display_name' field",
            path.display()
        ));
    }

    // Validate semver format: X.Y.Z where X, Y, Z are numeric.
    if !is_valid_semver(&manifest.version) {
        return Err(anyhow::anyhow!(
            "manifest at {} has invalid version '{}': must be semver format X.Y.Z",
            path.display(),
            manifest.version
        ));
    }

    Ok(manifest)
}

/// Check whether a version string is valid semver (`X.Y.Z` where X, Y, Z are numeric).
fn is_valid_semver(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// Manages the lifecycle of `CorePlugin` instances.
pub struct PluginLoader {
    plugins: HashMap<String, Arc<dyn CorePlugin>>,
    statuses: HashMap<String, PluginInfo>,
    /// Optional schema registry for registering plugin-declared collections.
    schema_registry: Option<Arc<SchemaRegistry>>,
    /// Optional credential store for providing scoped credential access to plugins.
    credential_store: Option<Arc<dyn CredentialStore>>,
}

impl PluginLoader {
    /// Create a new, empty plugin loader.
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            statuses: HashMap::new(),
            schema_registry: None,
            credential_store: None,
        }
    }

    /// Create a new plugin loader wired to a schema registry.
    ///
    /// When a plugin is loaded, its `collections()` are registered
    /// in the schema registry under the `{plugin_id}/{collection_name}` namespace.
    pub fn with_schema_registry(schema_registry: Arc<SchemaRegistry>) -> Self {
        Self {
            plugins: HashMap::new(),
            statuses: HashMap::new(),
            schema_registry: Some(schema_registry),
            credential_store: None,
        }
    }

    /// Set the credential store for providing scoped credential access to plugins.
    ///
    /// When set, each plugin receives a `PluginContext` with a
    /// `CredentialAccess` bridge scoped to its own plugin ID.
    pub fn set_credential_store(&mut self, store: Arc<dyn CredentialStore>) {
        self.credential_store = Some(store);
    }

    /// Discover plugins by scanning configured directories for `plugin.json` files.
    ///
    /// Each path in the list is expected to be a directory containing plugin
    /// subdirectories. Each subdirectory that contains a `plugin.json` file
    /// is considered a discovered plugin.
    ///
    /// Non-existent paths and permission errors are logged as warnings and
    /// skipped without crashing.
    pub fn discover_plugins(paths: &[String]) -> Vec<PathBuf> {
        let mut manifests = Vec::new();

        for dir_path in paths {
            let dir = Path::new(dir_path);

            let entries = match std::fs::read_dir(dir) {
                Ok(entries) => entries,
                Err(e) => {
                    warn!(
                        path = %dir.display(),
                        error = %e,
                        "skipping plugin directory: cannot read"
                    );
                    continue;
                }
            };

            for entry in entries {
                let entry = match entry {
                    Ok(e) => e,
                    Err(e) => {
                        warn!(error = %e, "skipping directory entry: cannot read");
                        continue;
                    }
                };

                let manifest_path = entry.path().join("plugin.json");
                if manifest_path.is_file() {
                    info!(manifest = %manifest_path.display(), "discovered plugin manifest");
                    manifests.push(manifest_path);
                }
            }
        }

        manifests
    }

    /// Register a plugin instance. Does not call `on_load` yet.
    ///
    /// Returns an error if a plugin with the same ID is already registered.
    pub fn register(&mut self, plugin: Box<dyn CorePlugin>) -> anyhow::Result<()> {
        let id = plugin.id().to_string();
        if self.plugins.contains_key(&id) {
            return Err(anyhow::anyhow!("plugin '{id}' is already registered"));
        }

        let info = PluginInfo {
            id: id.clone(),
            display_name: plugin.display_name().to_string(),
            version: plugin.version().to_string(),
            status: PluginStatus::Registered,
        };

        info!(plugin_id = %id, "plugin registered");
        self.statuses.insert(id.clone(), info);
        self.plugins.insert(id, Arc::from(plugin));
        Ok(())
    }

    /// Load all registered plugins by calling `on_load`.
    ///
    /// One failing plugin does not prevent others from loading.
    pub async fn load_all(&mut self) -> Vec<anyhow::Error> {
        let mut errors = Vec::new();
        let ids: Vec<String> = self.plugins.keys().cloned().collect();

        for id in ids {
            if let Err(e) = self.load_plugin(&id).await {
                errors.push(e);
            }
        }
        errors
    }

    /// Load a single plugin by ID.
    ///
    /// After calling `on_load`, queries the plugin for private collection
    /// schemas and registers each one in the schema registry (if present).
    async fn load_plugin(&mut self, id: &str) -> anyhow::Result<()> {
        let ctx = match &self.credential_store {
            Some(store) => {
                let bridge = PluginCredentialBridge::new(Arc::clone(store), id.to_string());
                PluginContext::with_credentials(id, Arc::new(bridge))
            }
            None => PluginContext::new(id),
        };

        // Obtain exclusive access to the plugin for on_load (requires &mut self).
        let plugin_arc = self
            .plugins
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("plugin '{id}' not found"))?;

        let plugin_mut = Arc::get_mut(plugin_arc)
            .ok_or_else(|| anyhow::anyhow!("plugin '{id}' has outstanding references, cannot load"))?;

        let load_result = plugin_mut.on_load(&ctx).await;

        match load_result {
            Ok(()) => {
                info!(plugin_id = %id, "plugin loaded");
                if let Some(info) = self.statuses.get_mut(id) {
                    info.status = PluginStatus::Loaded;
                }

                // Register plugin-declared collection schemas.
                if let Some(ref registry) = self.schema_registry {
                    let plugin = self.plugins.get(id).expect("plugin just loaded");
                    let collections = plugin.collections();
                    for col in &collections {
                        if let Err(e) =
                            registry.register_plugin_schema(id, &col.name, &col.schema)
                        {
                            warn!(
                                plugin_id = %id,
                                collection = %col.name,
                                error = %e,
                                "failed to register plugin collection schema"
                            );
                        }
                    }
                }

                Ok(())
            }
            Err(e) => {
                let msg = format!("{e}");
                error!(plugin_id = %id, error = %msg, "plugin failed to load");
                if let Some(info) = self.statuses.get_mut(id) {
                    info.status = PluginStatus::Failed(msg.clone());
                }
                Err(anyhow::anyhow!("plugin '{id}' failed to load: {msg}"))
            }
        }
    }

    /// Unload all loaded plugins by calling `on_unload`.
    ///
    /// Errors during unload are logged but do not prevent other plugins
    /// from being unloaded.
    pub async fn unload_all(&mut self) {
        let ids: Vec<String> = self.plugins.keys().cloned().collect();
        for id in ids {
            self.unload_plugin(&id).await;
        }
    }

    /// Unload a single plugin by ID.
    async fn unload_plugin(&mut self, id: &str) {
        if let Some(plugin_arc) = self.plugins.get_mut(id) {
            let unload_result = match Arc::get_mut(plugin_arc) {
                Some(plugin_mut) => plugin_mut.on_unload().await,
                None => {
                    warn!(plugin_id = %id, "plugin has outstanding references, skipping unload");
                    return;
                }
            };
            match unload_result {
                Ok(()) => {
                    info!(plugin_id = %id, "plugin unloaded");
                    if let Some(info) = self.statuses.get_mut(id) {
                        info.status = PluginStatus::Unloaded;
                    }
                }
                Err(e) => {
                    warn!(plugin_id = %id, error = %e, "plugin unload error (non-fatal)");
                    if let Some(info) = self.statuses.get_mut(id) {
                        info.status = PluginStatus::Unloaded;
                    }
                }
            }
        }
    }

    /// Returns the number of loaded plugins.
    pub fn loaded_count(&self) -> usize {
        self.statuses
            .values()
            .filter(|info| info.status == PluginStatus::Loaded)
            .count()
    }

    /// Returns info about all registered plugins.
    pub fn plugin_info(&self) -> Vec<PluginInfo> {
        self.statuses.values().cloned().collect()
    }

    /// Returns info about a single plugin by ID.
    pub fn get_plugin_info(&self, id: &str) -> Option<&PluginInfo> {
        self.statuses.get(id)
    }

    /// Returns the total number of registered plugins.
    pub fn registered_count(&self) -> usize {
        self.plugins.len()
    }

    /// Register private collection schemas declared in a plugin manifest.
    ///
    /// Reads the `collections` field from the manifest and registers each
    /// collection's inline JSON Schema in the schema registry under the
    /// `{plugin_id}/{collection_name}` namespace.
    pub fn register_manifest_schemas(&self, manifest: &PluginManifest) -> Vec<anyhow::Error> {
        let mut errors = Vec::new();

        let registry = match &self.schema_registry {
            Some(r) => r,
            None => return errors,
        };

        for col in &manifest.collections {
            if let Err(e) = registry.register_plugin_schema(&manifest.id, &col.name, &col.schema) {
                warn!(
                    plugin_id = %manifest.id,
                    collection = %col.name,
                    error = %e,
                    "failed to register manifest collection schema"
                );
                errors.push(e);
            }
        }

        errors
    }

    /// Returns a reference to a loaded plugin by ID.
    ///
    /// Returns `None` if the plugin is not registered or not loaded.
    pub fn get_plugin(&self, id: &str) -> Option<&dyn CorePlugin> {
        let info = self.statuses.get(id)?;
        if info.status != PluginStatus::Loaded {
            return None;
        }
        self.plugins.get(id).map(|p| p.as_ref())
    }

    /// Returns an `Arc` handle to a loaded plugin by ID.
    ///
    /// The caller can clone this `Arc` and use the plugin after the
    /// `PluginLoader` lock is dropped, which is important for avoiding
    /// holding the mutex during IO-heavy operations like `handle_route`.
    pub fn get_plugin_arc(&self, id: &str) -> Option<Arc<dyn CorePlugin>> {
        let info = self.statuses.get(id)?;
        if info.status != PluginStatus::Loaded {
            return None;
        }
        self.plugins.get(id).cloned()
    }
}

impl Default for PluginLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use life_engine_plugin_sdk::types::{Capability, CollectionSchema, CoreEvent, PluginRoute};
    use life_engine_plugin_sdk::Result;

    /// A well-behaved test plugin that always succeeds.
    struct GoodPlugin {
        id: String,
        loaded: bool,
    }

    impl GoodPlugin {
        fn new(id: &str) -> Self {
            Self {
                id: id.into(),
                loaded: false,
            }
        }
    }

    #[async_trait]
    impl CorePlugin for GoodPlugin {
        fn id(&self) -> &str {
            &self.id
        }
        fn display_name(&self) -> &str {
            "Good Plugin"
        }
        fn version(&self) -> &str {
            "1.0.0"
        }
        fn capabilities(&self) -> Vec<Capability> {
            vec![Capability::StorageRead]
        }
        async fn on_load(&mut self, _ctx: &PluginContext) -> Result<()> {
            self.loaded = true;
            Ok(())
        }
        async fn on_unload(&mut self) -> Result<()> {
            self.loaded = false;
            Ok(())
        }
        fn routes(&self) -> Vec<PluginRoute> {
            vec![]
        }
        async fn handle_event(&self, _event: &CoreEvent) -> Result<()> {
            Ok(())
        }
    }

    /// A plugin that always fails on_load.
    struct BadPlugin;

    #[async_trait]
    impl CorePlugin for BadPlugin {
        fn id(&self) -> &str {
            "com.test.bad"
        }
        fn display_name(&self) -> &str {
            "Bad Plugin"
        }
        fn version(&self) -> &str {
            "0.0.1"
        }
        fn capabilities(&self) -> Vec<Capability> {
            vec![]
        }
        async fn on_load(&mut self, _ctx: &PluginContext) -> Result<()> {
            Err(anyhow::anyhow!("intentional load failure"))
        }
        async fn on_unload(&mut self) -> Result<()> {
            Ok(())
        }
        fn routes(&self) -> Vec<PluginRoute> {
            vec![]
        }
        async fn handle_event(&self, _event: &CoreEvent) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn new_loader_is_empty() {
        let loader = PluginLoader::new();
        assert_eq!(loader.loaded_count(), 0);
        assert_eq!(loader.registered_count(), 0);
        assert!(loader.plugin_info().is_empty());
    }

    #[test]
    fn register_plugin() {
        let mut loader = PluginLoader::new();
        loader
            .register(Box::new(GoodPlugin::new("com.test.good")))
            .unwrap();
        assert_eq!(loader.registered_count(), 1);
        assert_eq!(loader.loaded_count(), 0);

        let info = loader.get_plugin_info("com.test.good").unwrap();
        assert_eq!(info.status, PluginStatus::Registered);
        assert_eq!(info.display_name, "Good Plugin");
        assert_eq!(info.version, "1.0.0");
    }

    #[test]
    fn duplicate_registration_fails() {
        let mut loader = PluginLoader::new();
        loader
            .register(Box::new(GoodPlugin::new("com.test.dupe")))
            .unwrap();
        let result = loader.register(Box::new(GoodPlugin::new("com.test.dupe")));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("already registered"));
    }

    #[tokio::test]
    async fn load_all_succeeds() {
        let mut loader = PluginLoader::new();
        loader
            .register(Box::new(GoodPlugin::new("com.test.a")))
            .unwrap();
        loader
            .register(Box::new(GoodPlugin::new("com.test.b")))
            .unwrap();

        let errors = loader.load_all().await;
        assert!(errors.is_empty());
        assert_eq!(loader.loaded_count(), 2);
    }

    #[tokio::test]
    async fn bad_plugin_does_not_crash_others() {
        let mut loader = PluginLoader::new();
        loader
            .register(Box::new(GoodPlugin::new("com.test.good")))
            .unwrap();
        loader.register(Box::new(BadPlugin)).unwrap();

        let errors = loader.load_all().await;
        assert_eq!(errors.len(), 1);
        // Good plugin should still be loaded.
        assert_eq!(loader.loaded_count(), 1);

        let bad_info = loader.get_plugin_info("com.test.bad").unwrap();
        assert!(matches!(bad_info.status, PluginStatus::Failed(_)));

        let good_info = loader.get_plugin_info("com.test.good").unwrap();
        assert_eq!(good_info.status, PluginStatus::Loaded);
    }

    #[tokio::test]
    async fn unload_all_calls_on_unload() {
        let mut loader = PluginLoader::new();
        loader
            .register(Box::new(GoodPlugin::new("com.test.unload")))
            .unwrap();
        loader.load_all().await;
        assert_eq!(loader.loaded_count(), 1);

        loader.unload_all().await;
        assert_eq!(loader.loaded_count(), 0);

        let info = loader.get_plugin_info("com.test.unload").unwrap();
        assert_eq!(info.status, PluginStatus::Unloaded);
    }

    #[tokio::test]
    async fn plugin_info_returns_all() {
        let mut loader = PluginLoader::new();
        loader
            .register(Box::new(GoodPlugin::new("com.test.x")))
            .unwrap();
        loader
            .register(Box::new(GoodPlugin::new("com.test.y")))
            .unwrap();

        let infos = loader.plugin_info();
        assert_eq!(infos.len(), 2);
    }

    #[test]
    fn default_impl_works() {
        let loader = PluginLoader::default();
        assert_eq!(loader.registered_count(), 0);
    }

    // ── Schema registry integration tests ────────────────────

    /// A plugin that declares custom collections.
    struct PluginWithCollections {
        id: String,
    }

    impl PluginWithCollections {
        fn new(id: &str) -> Self {
            Self { id: id.into() }
        }
    }

    #[async_trait]
    impl CorePlugin for PluginWithCollections {
        fn id(&self) -> &str {
            &self.id
        }
        fn display_name(&self) -> &str {
            "Plugin With Collections"
        }
        fn version(&self) -> &str {
            "1.0.0"
        }
        fn capabilities(&self) -> Vec<Capability> {
            vec![]
        }
        async fn on_load(&mut self, _ctx: &PluginContext) -> Result<()> {
            Ok(())
        }
        async fn on_unload(&mut self) -> Result<()> {
            Ok(())
        }
        fn routes(&self) -> Vec<PluginRoute> {
            vec![]
        }
        fn collections(&self) -> Vec<CollectionSchema> {
            vec![CollectionSchema {
                name: "recipes".to_string(),
                schema: serde_json::json!({
                    "$schema": "http://json-schema.org/draft-07/schema#",
                    "title": "Recipe",
                    "type": "object",
                    "required": ["id", "name"],
                    "properties": {
                        "id": { "type": "string" },
                        "name": { "type": "string" }
                    },
                    "additionalProperties": false
                }),
            }]
        }
        async fn handle_event(&self, _event: &CoreEvent) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn plugin_with_collections_registers_schemas() {
        let registry = Arc::new(SchemaRegistry::new());
        let mut loader = PluginLoader::with_schema_registry(Arc::clone(&registry));
        loader
            .register(Box::new(PluginWithCollections::new("com.test.cook")))
            .unwrap();

        let errors = loader.load_all().await;
        assert!(errors.is_empty());

        // The collection should be registered under the namespaced key.
        assert!(registry.has_schema("com.test.cook/recipes"));
        assert!(!registry.has_schema("recipes"));

        // Validate data against the plugin's schema.
        let valid = serde_json::json!({ "id": "r1", "name": "Pancakes" });
        let result = registry.validate("com.test.cook/recipes", &valid).unwrap();
        assert!(result.valid);
    }

    #[tokio::test]
    async fn good_plugin_without_collections_no_schemas_registered() {
        let registry = Arc::new(SchemaRegistry::new());
        let mut loader = PluginLoader::with_schema_registry(Arc::clone(&registry));
        loader
            .register(Box::new(GoodPlugin::new("com.test.plain")))
            .unwrap();

        let errors = loader.load_all().await;
        assert!(errors.is_empty());

        // No plugin schemas should be registered.
        assert!(registry.collections().is_empty());
    }

    // ── Plugin discovery and manifest tests ────────────────────

    /// Helper: write a valid plugin.json manifest to the given directory.
    fn write_manifest(dir: &Path, id: &str, version: &str) {
        let manifest = serde_json::json!({
            "id": id,
            "display_name": "Test Plugin",
            "version": version,
            "description": "A test plugin"
        });
        let manifest_path = dir.join("plugin.json");
        std::fs::write(manifest_path, serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
    }

    #[test]
    fn discover_plugins_finds_valid_manifests() {
        let root = tempfile::tempdir().unwrap();

        // Create two plugin subdirectories with manifests.
        let plugin_a = root.path().join("plugin-a");
        let plugin_b = root.path().join("plugin-b");
        std::fs::create_dir_all(&plugin_a).unwrap();
        std::fs::create_dir_all(&plugin_b).unwrap();
        write_manifest(&plugin_a, "com.test.a", "1.0.0");
        write_manifest(&plugin_b, "com.test.b", "2.0.0");

        let paths = vec![root.path().to_string_lossy().to_string()];
        let discovered = PluginLoader::discover_plugins(&paths);

        assert_eq!(discovered.len(), 2);
        assert!(discovered.iter().all(|p| p.file_name().unwrap() == "plugin.json"));
    }

    #[test]
    fn discover_skips_nonexistent_directory() {
        let paths = vec!["/tmp/life-engine-nonexistent-path-12345".to_string()];
        let discovered = PluginLoader::discover_plugins(&paths);
        assert!(discovered.is_empty());
    }

    #[test]
    fn discover_skips_directories_without_manifest() {
        let root = tempfile::tempdir().unwrap();

        // Create a subdirectory without a plugin.json.
        let no_manifest = root.path().join("not-a-plugin");
        std::fs::create_dir_all(&no_manifest).unwrap();
        // Create a file that is not plugin.json.
        std::fs::write(no_manifest.join("README.md"), "not a manifest").unwrap();

        // Create a subdirectory with a plugin.json.
        let with_manifest = root.path().join("real-plugin");
        std::fs::create_dir_all(&with_manifest).unwrap();
        write_manifest(&with_manifest, "com.test.real", "1.0.0");

        let paths = vec![root.path().to_string_lossy().to_string()];
        let discovered = PluginLoader::discover_plugins(&paths);

        assert_eq!(discovered.len(), 1);
        assert!(discovered[0].to_string_lossy().contains("real-plugin"));
    }

    #[test]
    fn read_valid_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let manifest_json = serde_json::json!({
            "id": "com.example.test",
            "display_name": "Example Plugin",
            "version": "1.2.3",
            "description": "An example plugin",
            "capabilities": ["StorageRead", "HttpOutbound"],
            "min_core_version": "0.1.0",
            "entry_point": "main.wasm"
        });
        let path = dir.path().join("plugin.json");
        std::fs::write(&path, serde_json::to_string_pretty(&manifest_json).unwrap()).unwrap();

        let manifest = read_manifest(&path).unwrap();
        assert_eq!(manifest.id, "com.example.test");
        assert_eq!(manifest.display_name, "Example Plugin");
        assert_eq!(manifest.version, "1.2.3");
        assert_eq!(manifest.description, "An example plugin");
        assert_eq!(manifest.capabilities, vec!["StorageRead", "HttpOutbound"]);
        assert_eq!(manifest.min_core_version, Some("0.1.0".to_string()));
        assert_eq!(manifest.entry_point, Some("main.wasm".to_string()));
    }

    #[test]
    fn reject_manifest_missing_required_fields() {
        let dir = tempfile::tempdir().unwrap();
        // Missing "id" entirely.
        let manifest_json = serde_json::json!({
            "display_name": "No ID Plugin",
            "version": "1.0.0"
        });
        let path = dir.path().join("plugin.json");
        std::fs::write(&path, serde_json::to_string_pretty(&manifest_json).unwrap()).unwrap();

        let result = read_manifest(&path);
        assert!(result.is_err());

        // Empty "id".
        let manifest_json = serde_json::json!({
            "id": "",
            "display_name": "Empty ID Plugin",
            "version": "1.0.0"
        });
        std::fs::write(&path, serde_json::to_string_pretty(&manifest_json).unwrap()).unwrap();

        let result = read_manifest(&path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty 'id'"));
    }

    #[test]
    fn reject_manifest_invalid_semver() {
        let dir = tempfile::tempdir().unwrap();
        let manifest_json = serde_json::json!({
            "id": "com.test.bad-version",
            "display_name": "Bad Version",
            "version": "not-a-version"
        });
        let path = dir.path().join("plugin.json");
        std::fs::write(&path, serde_json::to_string_pretty(&manifest_json).unwrap()).unwrap();

        let result = read_manifest(&path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid version"));
    }

    #[test]
    fn reject_manifest_invalid_id_format() {
        let dir = tempfile::tempdir().unwrap();
        let manifest_json = serde_json::json!({
            "id": "myplugin",
            "display_name": "No Dots Plugin",
            "version": "1.0.0"
        });
        let path = dir.path().join("plugin.json");
        std::fs::write(&path, serde_json::to_string_pretty(&manifest_json).unwrap()).unwrap();

        let result = read_manifest(&path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("reverse-domain notation"));
    }

    // ── Manifest-driven collection schema registration tests ────────────────────

    #[test]
    fn manifest_with_collections_deserializes() {
        let dir = tempfile::tempdir().unwrap();
        let manifest_json = serde_json::json!({
            "id": "com.example.recipes",
            "display_name": "Recipe Plugin",
            "version": "1.0.0",
            "collections": [
                {
                    "name": "recipes",
                    "schema": {
                        "$schema": "http://json-schema.org/draft-07/schema#",
                        "type": "object",
                        "required": ["id", "name"],
                        "properties": {
                            "id": { "type": "string" },
                            "name": { "type": "string" }
                        },
                        "additionalProperties": false
                    }
                }
            ]
        });
        let path = dir.path().join("plugin.json");
        std::fs::write(&path, serde_json::to_string_pretty(&manifest_json).unwrap()).unwrap();

        let manifest = read_manifest(&path).unwrap();
        assert_eq!(manifest.collections.len(), 1);
        assert_eq!(manifest.collections[0].name, "recipes");
    }

    #[test]
    fn manifest_without_collections_defaults_to_empty() {
        let dir = tempfile::tempdir().unwrap();
        let manifest_json = serde_json::json!({
            "id": "com.example.plain",
            "display_name": "Plain Plugin",
            "version": "1.0.0"
        });
        let path = dir.path().join("plugin.json");
        std::fs::write(&path, serde_json::to_string_pretty(&manifest_json).unwrap()).unwrap();

        let manifest = read_manifest(&path).unwrap();
        assert!(manifest.collections.is_empty());
    }

    #[test]
    fn register_manifest_schemas_registers_collections() {
        let registry = Arc::new(SchemaRegistry::new());
        let loader = PluginLoader::with_schema_registry(Arc::clone(&registry));

        let manifest = PluginManifest {
            id: "com.example.recipes".into(),
            display_name: "Recipe Plugin".into(),
            version: "1.0.0".into(),
            description: String::new(),
            capabilities: vec![],
            min_core_version: None,
            entry_point: None,
            collections: vec![ManifestCollection {
                name: "recipes".into(),
                schema: serde_json::json!({
                    "$schema": "http://json-schema.org/draft-07/schema#",
                    "type": "object",
                    "required": ["id", "name"],
                    "properties": {
                        "id": { "type": "string" },
                        "name": { "type": "string" }
                    },
                    "additionalProperties": false
                }),
            }],
        };

        let errors = loader.register_manifest_schemas(&manifest);
        assert!(errors.is_empty());
        assert!(registry.has_schema("com.example.recipes/recipes"));

        // Validate data against the registered schema.
        let valid = serde_json::json!({ "id": "r1", "name": "Pancakes" });
        let result = registry.validate("com.example.recipes/recipes", &valid).unwrap();
        assert!(result.valid);

        let invalid = serde_json::json!({ "id": "r2" }); // missing required "name"
        let result = registry.validate("com.example.recipes/recipes", &invalid).unwrap();
        assert!(!result.valid);
    }

    #[test]
    fn register_manifest_schemas_rejects_core_cdm_names() {
        let registry = Arc::new(SchemaRegistry::new());
        let loader = PluginLoader::with_schema_registry(Arc::clone(&registry));

        let manifest = PluginManifest {
            id: "com.evil.plugin".into(),
            display_name: "Evil Plugin".into(),
            version: "1.0.0".into(),
            description: String::new(),
            capabilities: vec![],
            min_core_version: None,
            entry_point: None,
            collections: vec![ManifestCollection {
                name: "tasks".into(), // reserved CDM name
                schema: serde_json::json!({ "type": "object" }),
            }],
        };

        let errors = loader.register_manifest_schemas(&manifest);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].to_string().contains("reserved by Core CDM"));
        assert!(!registry.has_schema("com.evil.plugin/tasks"));
    }

    #[test]
    fn register_manifest_schemas_no_op_without_registry() {
        let loader = PluginLoader::new(); // no schema registry

        let manifest = PluginManifest {
            id: "com.example.noop".into(),
            display_name: "NoOp Plugin".into(),
            version: "1.0.0".into(),
            description: String::new(),
            capabilities: vec![],
            min_core_version: None,
            entry_point: None,
            collections: vec![ManifestCollection {
                name: "things".into(),
                schema: serde_json::json!({ "type": "object" }),
            }],
        };

        let errors = loader.register_manifest_schemas(&manifest);
        assert!(errors.is_empty()); // no errors, just no-op
    }
}
