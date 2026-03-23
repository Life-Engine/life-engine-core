//! Capability approval policy for first-party and third-party plugins.
//!
//! First-party plugins (located within the monorepo `plugins/` directory)
//! are auto-granted all declared capabilities. Third-party plugins must
//! have each declared capability explicitly approved in the Core config.

use std::collections::HashSet;
use std::path::Path;

use life_engine_traits::Capability;

use crate::error::PluginError;
use crate::manifest::PluginManifest;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{CapabilitySet, PluginMeta};
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
            },
            actions: HashMap::new(),
            capabilities: CapabilitySet {
                required: capabilities,
            },
            config: None,
        }
    }

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
                Capability::HttpOutbound,
            ],
        );

        let result =
            check_capability_approval(&manifest, &plugin_dir, plugins_dir.path(), &[]);

        let approved = result.unwrap();
        assert!(approved.has(Capability::StorageRead));
        assert!(approved.has(Capability::StorageWrite));
        assert!(approved.has(Capability::HttpOutbound));
        assert_eq!(approved.len(), 3);
    }

    #[test]
    fn third_party_plugin_with_fully_approved_capabilities_passes() {
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
            &[Capability::StorageRead], // only read approved, not write
        );

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::CapabilityViolation(_)));
        assert!(err.to_string().contains("storage:write"));
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

        // No approved capabilities at all
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

    #[test]
    fn display_fromstr_round_trip_all_capabilities() {
        use std::str::FromStr;

        let all = [
            Capability::StorageRead,
            Capability::StorageWrite,
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
