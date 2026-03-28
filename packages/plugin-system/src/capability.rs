//! Capability approval policy for first-party and third-party plugins.
//!
//! First-party plugins (located within the monorepo `plugins/` directory)
//! are auto-granted all declared capabilities. Third-party plugins must
//! have each declared capability explicitly approved in the Core config.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use life_engine_traits::Capability;

use crate::error::PluginError;
use crate::manifest::{CollectionDef, PluginManifest};

/// A set of approved capabilities for a loaded plugin.
#[derive(Debug, Clone)]
pub struct ApprovedCapabilities {
    capabilities: HashSet<Capability>,
}

impl ApprovedCapabilities {
    /// Creates an `ApprovedCapabilities` from a set of capabilities.
    pub fn new(capabilities: HashSet<Capability>) -> Self {
        Self { capabilities }
    }

    /// Creates an empty `ApprovedCapabilities` with no capabilities.
    pub fn empty() -> Self {
        Self {
            capabilities: HashSet::new(),
        }
    }

    /// Returns `true` if the given capability is approved.
    pub fn has(&self, cap: Capability) -> bool {
        self.capabilities.contains(&cap)
    }

    /// Returns an iterator over the approved capabilities.
    pub fn iter(&self) -> impl Iterator<Item = &Capability> {
        self.capabilities.iter()
    }

    /// Returns the number of approved capabilities.
    pub fn len(&self) -> usize {
        self.capabilities.len()
    }

    /// Returns `true` if no capabilities are approved.
    pub fn is_empty(&self) -> bool {
        self.capabilities.is_empty()
    }
}

/// Determines whether a plugin is first-party by checking if its path
/// is a child of the monorepo `plugins/` directory.
fn is_first_party(plugin_path: &Path, plugins_dir: &Path) -> bool {
    match (plugin_path.canonicalize(), plugins_dir.canonicalize()) {
        (Ok(canonical_plugin), Ok(canonical_plugins_dir)) => {
            canonical_plugin.starts_with(&canonical_plugins_dir)
        }
        _ => false,
    }
}

/// Checks whether a plugin's declared capabilities are approved.
///
/// - First-party plugins (inside `plugins_dir`) are auto-granted all declared capabilities.
/// - Third-party plugins must have each declared capability listed in `approved_capabilities`.
///
/// Returns `ApprovedCapabilities` on success, or `PluginError::CapabilityViolation`
/// if any declared capability is not approved.
pub fn check_capability_approval(
    manifest: &PluginManifest,
    plugin_path: &Path,
    plugins_dir: &Path,
    approved_capabilities: &[Capability],
) -> Result<ApprovedCapabilities, PluginError> {
    let declared: HashSet<Capability> = manifest.capabilities.required.iter().copied().collect();

    if is_first_party(plugin_path, plugins_dir) {
        return Ok(ApprovedCapabilities::new(declared));
    }

    // Third-party: check each declared capability against the approved list
    let approved_set: HashSet<Capability> = approved_capabilities.iter().copied().collect();

    let unapproved: Vec<Capability> = declared
        .iter()
        .filter(|cap| !approved_set.contains(cap))
        .copied()
        .collect();

    if !unapproved.is_empty() {
        let unapproved_names: Vec<String> = unapproved.iter().map(|c| c.to_string()).collect();
        return Err(PluginError::CapabilityViolation(format!(
            "plugin '{}' declares unapproved capabilities: {}",
            manifest.plugin.id,
            unapproved_names.join(", ")
        )));
    }

    // Return the intersection of declared and approved
    let intersection: HashSet<Capability> = declared
        .intersection(&approved_set)
        .copied()
        .collect();

    Ok(ApprovedCapabilities::new(intersection))
}

/// Checks a single capability at runtime, returning a `RuntimeCapabilityViolation`
/// error with a clear message if the plugin lacks the required capability.
pub fn check_capability(
    approved: &ApprovedCapabilities,
    plugin_id: &str,
    required: Capability,
) -> Result<(), PluginError> {
    if approved.has(required) {
        Ok(())
    } else {
        Err(PluginError::RuntimeCapabilityViolation(format!(
            "plugin '{}' lacks capability '{}'",
            plugin_id, required
        )))
    }
}

/// Checks whether a plugin is allowed to access a given collection.
///
/// A plugin can access a collection if:
/// - The collection is declared in the plugin's `[collections]` section, OR
/// - The collection is plugin-scoped (prefixed with `<plugin_id>.`), which is auto-allowed
///
/// Returns `Ok(())` on success, or `RuntimeCapabilityViolation` if denied.
pub fn check_collection_access(
    plugin_id: &str,
    collection: &str,
    declared_collections: &HashMap<String, CollectionDef>,
) -> Result<(), PluginError> {
    // Plugin-scoped collections are auto-allowed
    let plugin_prefix = format!("{}.", plugin_id);
    if collection.starts_with(&plugin_prefix) {
        return Ok(());
    }

    if declared_collections.contains_key(collection) {
        return Ok(());
    }

    Err(PluginError::RuntimeCapabilityViolation(format!(
        "plugin '{}' is not allowed to access collection '{}': \
         collection not declared in manifest [collections] section",
        plugin_id, collection
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{CapabilitySet, CollectionAccess, EventsDef, PluginMeta, TrustLevel};
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn make_manifest(id: &str, capabilities: Vec<Capability>) -> PluginManifest {
        PluginManifest {
            plugin: PluginMeta {
                id: id.to_string(),
                name: "Test Plugin".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                license: None,
                trust: TrustLevel::ThirdParty,
            },
            actions: HashMap::new(),
            capabilities: CapabilitySet {
                required: capabilities,
            },
            collections: HashMap::new(),
            events: EventsDef::default(),
            config: None,
        }
    }

    fn make_approved(caps: &[Capability]) -> ApprovedCapabilities {
        let set: HashSet<Capability> = caps.iter().copied().collect();
        ApprovedCapabilities::new(set)
    }

    fn make_collection_def(access: CollectionAccess) -> CollectionDef {
        CollectionDef {
            schema: "cdm:task".to_string(),
            access,
            strict: false,
            indexes: vec![],
            extensions: vec![],
            extension_schema: None,
            extension_indexes: vec![],
        }
    }

    // ---------------------------------------------------------------
    // Load-time approval tests
    // ---------------------------------------------------------------

    #[test]
    fn first_party_plugin_auto_granted_all_capabilities() {
        let plugins_dir = TempDir::new().unwrap();
        let plugin_dir = plugins_dir.path().join("my-plugin");
        std::fs::create_dir(&plugin_dir).unwrap();

        let manifest = make_manifest(
            "my-plugin",
            vec![
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
            ],
        );

        let result =
            check_capability_approval(&manifest, &plugin_dir, plugins_dir.path(), &[]);

        let approved = result.unwrap();
        assert!(approved.has(Capability::StorageRead));
        assert!(approved.has(Capability::StorageWrite));
        assert!(approved.has(Capability::StorageDelete));
        assert!(approved.has(Capability::StorageBlobRead));
        assert!(approved.has(Capability::StorageBlobWrite));
        assert!(approved.has(Capability::StorageBlobDelete));
        assert!(approved.has(Capability::HttpOutbound));
        assert!(approved.has(Capability::EventsEmit));
        assert!(approved.has(Capability::EventsSubscribe));
        assert!(approved.has(Capability::ConfigRead));
        assert_eq!(approved.len(), 10);
    }

    #[test]
    fn third_party_plugin_requires_explicit_grants() {
        let plugins_dir = TempDir::new().unwrap();
        let third_party_dir = TempDir::new().unwrap();
        let plugin_dir = third_party_dir.path().join("ext-plugin");
        std::fs::create_dir(&plugin_dir).unwrap();

        let manifest = make_manifest(
            "ext-plugin",
            vec![Capability::StorageRead, Capability::ConfigRead],
        );

        let result = check_capability_approval(
            &manifest,
            &plugin_dir,
            plugins_dir.path(),
            &[Capability::StorageRead, Capability::ConfigRead],
        );

        let approved = result.unwrap();
        assert!(approved.has(Capability::StorageRead));
        assert!(approved.has(Capability::ConfigRead));
        assert_eq!(approved.len(), 2);
    }

    #[test]
    fn third_party_plugin_with_unapproved_capability_returns_error() {
        let plugins_dir = TempDir::new().unwrap();
        let third_party_dir = TempDir::new().unwrap();
        let plugin_dir = third_party_dir.path().join("ext-plugin");
        std::fs::create_dir(&plugin_dir).unwrap();

        let manifest = make_manifest(
            "ext-plugin",
            vec![Capability::StorageRead, Capability::StorageWrite],
        );

        let result = check_capability_approval(
            &manifest,
            &plugin_dir,
            plugins_dir.path(),
            &[Capability::StorageRead],
        );

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::CapabilityViolation(_)));
        assert!(err.to_string().contains("storage:doc:write"));
        assert!(err.to_string().contains("ext-plugin"));
    }

    #[test]
    fn third_party_plugin_with_no_config_entry_refuses_to_load() {
        let plugins_dir = TempDir::new().unwrap();
        let third_party_dir = TempDir::new().unwrap();
        let plugin_dir = third_party_dir.path().join("ext-plugin");
        std::fs::create_dir(&plugin_dir).unwrap();

        let manifest = make_manifest(
            "ext-plugin",
            vec![Capability::StorageRead],
        );

        let result = check_capability_approval(
            &manifest,
            &plugin_dir,
            plugins_dir.path(),
            &[],
        );

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::CapabilityViolation(_)));
    }

    #[test]
    fn third_party_plugin_with_empty_capabilities_loads_with_none() {
        let plugins_dir = TempDir::new().unwrap();
        let third_party_dir = TempDir::new().unwrap();
        let plugin_dir = third_party_dir.path().join("ext-plugin");
        std::fs::create_dir(&plugin_dir).unwrap();

        let manifest = make_manifest("ext-plugin", vec![]);

        let result = check_capability_approval(
            &manifest,
            &plugin_dir,
            plugins_dir.path(),
            &[],
        );

        let approved = result.unwrap();
        assert!(approved.is_empty());
    }

    // ---------------------------------------------------------------
    // Runtime capability check tests (storage doc)
    // ---------------------------------------------------------------

    #[test]
    fn storage_doc_read_allowed_with_capability() {
        let approved = make_approved(&[Capability::StorageRead]);
        let result = check_capability(&approved, "test-plugin", Capability::StorageRead);
        assert!(result.is_ok());
    }

    #[test]
    fn storage_doc_write_denied_without_capability() {
        let approved = make_approved(&[Capability::StorageRead]);
        let result = check_capability(&approved, "test-plugin", Capability::StorageWrite);
        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::RuntimeCapabilityViolation(_)));
        assert!(err.to_string().contains("storage:doc:write"));
        assert!(err.to_string().contains("test-plugin"));
    }

    #[test]
    fn storage_doc_delete_denied_without_capability() {
        let approved = make_approved(&[Capability::StorageRead, Capability::StorageWrite]);
        let result = check_capability(&approved, "test-plugin", Capability::StorageDelete);
        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::RuntimeCapabilityViolation(_)));
        assert!(err.to_string().contains("storage:doc:delete"));
    }

    // ---------------------------------------------------------------
    // Runtime capability check tests (blob)
    // ---------------------------------------------------------------

    #[test]
    fn blob_read_allowed_with_capability() {
        let approved = make_approved(&[Capability::StorageBlobRead]);
        let result = check_capability(&approved, "test-plugin", Capability::StorageBlobRead);
        assert!(result.is_ok());
    }

    #[test]
    fn blob_write_denied_without_capability() {
        let approved = make_approved(&[Capability::StorageBlobRead]);
        let result = check_capability(&approved, "test-plugin", Capability::StorageBlobWrite);
        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::RuntimeCapabilityViolation(_)));
        assert!(err.to_string().contains("storage:blob:write"));
    }

    #[test]
    fn blob_delete_denied_without_capability() {
        let approved = make_approved(&[Capability::StorageBlobRead, Capability::StorageBlobWrite]);
        let result = check_capability(&approved, "test-plugin", Capability::StorageBlobDelete);
        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::RuntimeCapabilityViolation(_)));
        assert!(err.to_string().contains("storage:blob:delete"));
    }

    // ---------------------------------------------------------------
    // Collection access enforcement tests
    // ---------------------------------------------------------------

    #[test]
    fn collection_access_limited_to_declared_collections() {
        let mut collections = HashMap::new();
        collections.insert("tasks".to_string(), make_collection_def(CollectionAccess::ReadWrite));
        collections.insert("contacts".to_string(), make_collection_def(CollectionAccess::Read));

        // Declared collection access succeeds
        assert!(check_collection_access("my-plugin", "tasks", &collections).is_ok());
        assert!(check_collection_access("my-plugin", "contacts", &collections).is_ok());

        // Undeclared collection access fails
        let err = check_collection_access("my-plugin", "calendar_events", &collections).unwrap_err();
        assert!(matches!(err, PluginError::RuntimeCapabilityViolation(_)));
        assert!(err.to_string().contains("my-plugin"));
        assert!(err.to_string().contains("calendar_events"));
        assert!(err.to_string().contains("not declared"));
    }

    #[test]
    fn plugin_scoped_collection_auto_allowed() {
        let collections = HashMap::new(); // no declared collections at all

        // Plugin-scoped collections (prefixed with plugin_id.) are auto-allowed
        assert!(check_collection_access("my-plugin", "my-plugin.cache", &collections).is_ok());
        assert!(check_collection_access("my-plugin", "my-plugin.internal_state", &collections).is_ok());

        // But another plugin's scoped collection is not
        let err = check_collection_access("my-plugin", "other-plugin.cache", &collections).unwrap_err();
        assert!(matches!(err, PluginError::RuntimeCapabilityViolation(_)));
        assert!(err.to_string().contains("other-plugin.cache"));
    }

    // ---------------------------------------------------------------
    // Clear error message tests
    // ---------------------------------------------------------------

    #[test]
    fn denial_error_includes_plugin_id_and_capability() {
        let approved = make_approved(&[]);
        let err = check_capability(&approved, "email-connector", Capability::HttpOutbound).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("email-connector"), "error should name the plugin");
        assert!(msg.contains("http:outbound"), "error should name the capability");
    }

    #[test]
    fn collection_denial_error_includes_collection_and_plugin() {
        let collections = HashMap::new();
        let err = check_collection_access("email-connector", "private_data", &collections).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("email-connector"), "error should name the plugin");
        assert!(msg.contains("private_data"), "error should name the collection");
        assert!(msg.contains("not declared"), "error should explain why denied");
    }

    #[test]
    fn third_party_unapproved_error_lists_all_unapproved_capabilities() {
        let plugins_dir = TempDir::new().unwrap();
        let third_party_dir = TempDir::new().unwrap();
        let plugin_dir = third_party_dir.path().join("ext-plugin");
        std::fs::create_dir(&plugin_dir).unwrap();

        let manifest = make_manifest(
            "ext-plugin",
            vec![
                Capability::StorageRead,
                Capability::StorageWrite,
                Capability::HttpOutbound,
            ],
        );

        // Only approve StorageRead
        let result = check_capability_approval(
            &manifest,
            &plugin_dir,
            plugins_dir.path(),
            &[Capability::StorageRead],
        );

        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("ext-plugin"), "error should name the plugin");
        // Both unapproved capabilities should be listed
        assert!(
            msg.contains("storage:doc:write") || msg.contains("http:outbound"),
            "error should list at least one unapproved capability: {msg}"
        );
    }

    // ---------------------------------------------------------------
    // Display/FromStr round trip
    // ---------------------------------------------------------------

    #[test]
    fn display_fromstr_round_trip_all_capabilities() {
        use std::str::FromStr;

        let all = [
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
        ];

        for cap in &all {
            let s = cap.to_string();
            let parsed = Capability::from_str(&s).unwrap();
            assert_eq!(*cap, parsed);
        }
    }
}
