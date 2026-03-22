//! Plugin manifest reading and validation.
//!
//! Reads `plugin.json` manifest files from plugin directories, deserializes
//! them into `PluginManifest` structs, and validates their contents against
//! the Life Engine plugin manifest specification.
//!
//! Deserialization is lenient (unknown fields are tolerated via
//! `serde_json::Value` for complex nested types), but validation is strict:
//! IDs must be reverse-domain format, versions must be semver, custom
//! elements must contain a hyphen, and required fields must be non-empty.

use crate::error::CoreError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Sidebar configuration for a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidebarConfig {
    /// Label shown in the sidebar.
    pub label: String,
    /// Icon identifier for the sidebar entry.
    pub icon: String,
    /// Display order in the sidebar (lower values appear first).
    #[serde(default)]
    pub order: Option<u32>,
}

/// Command palette entry for a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandPaletteEntry {
    /// Unique command identifier within the plugin.
    pub id: String,
    /// Human-readable label shown in the command palette.
    pub label: String,
    /// Action identifier triggered when the command is selected.
    pub action: String,
}

/// Dashboard widget configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardConfig {
    /// Custom element tag name for the dashboard widget.
    pub element: String,
    /// Default widget size on the dashboard.
    pub default_size: String,
}

/// Plugin dependency configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DependenciesConfig {
    /// Shared module names the plugin consumes.
    #[serde(default)]
    pub shared_modules: Vec<String>,
}

/// A parsed plugin manifest from `plugin.json`.
///
/// All required fields from the JSON schema are non-optional. Optional
/// fields use `Option` or `Vec` with `#[serde(default)]`. Complex nested
/// types that may vary between schema versions use `serde_json::Value`
/// for forward compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    /// Unique plugin identifier in reverse-domain format.
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Semver version string.
    pub version: String,
    /// Path to the plugin's entry file, relative to package root.
    pub entry: String,
    /// Custom element tag name the plugin registers.
    pub element: String,
    /// Minimum compatible App shell version (semver).
    pub min_shell_version: String,
    /// Sidebar configuration for the plugin (schema-defined top-level field).
    #[serde(default)]
    pub sidebar: Option<SidebarConfig>,
    /// Slot definitions for the plugin (real-world manifests use this
    /// instead of or alongside `sidebar`).
    #[serde(default)]
    pub slots: Option<Vec<serde_json::Value>>,
    /// Capability strings the plugin requires.
    pub capabilities: Vec<String>,
    /// Short description for the plugin directory.
    #[serde(default)]
    pub description: Option<String>,
    /// Author name or structured author object.
    #[serde(default)]
    pub author: Option<serde_json::Value>,
    /// SPDX licence identifier.
    #[serde(default)]
    pub license: Option<String>,
    /// Command palette entries for the plugin.
    #[serde(default)]
    pub command_palette: Option<Vec<CommandPaletteEntry>>,
    /// Dashboard widget configuration.
    #[serde(default)]
    pub dashboard: Option<DashboardConfig>,
    /// Private collection definitions (kept as `Value` for schema flexibility).
    #[serde(default)]
    pub collections: Option<serde_json::Value>,
    /// Domain strings the plugin may make HTTP requests to.
    #[serde(default)]
    pub allowed_domains: Option<Vec<String>>,
    /// Settings schema for auto-rendered plugin settings UI.
    #[serde(default)]
    pub settings: Option<Vec<serde_json::Value>>,
    /// Plugin dependency configuration.
    #[serde(default)]
    pub dependencies: Option<DependenciesConfig>,
    /// Whether the plugin bundle exceeds 200 KB gzipped.
    #[serde(default)]
    pub large_bundle: Option<bool>,
    /// Route definitions (present in some manifests, not in the formal schema).
    #[serde(default)]
    pub routes: Option<Vec<serde_json::Value>>,
}

impl PluginManifest {
    /// Load a plugin manifest from a directory containing `plugin.json`.
    ///
    /// Returns a `CoreError::Manifest` if the file cannot be read or parsed.
    pub fn load_from_path(dir: &Path) -> anyhow::Result<Self> {
        let manifest_path = dir.join("plugin.json");

        if !dir.exists() {
            return Err(CoreError::Manifest(format!(
                "plugin directory does not exist: {}",
                dir.display()
            ))
            .into());
        }

        if !manifest_path.exists() {
            return Err(CoreError::Manifest(format!(
                "plugin.json not found in {}",
                dir.display()
            ))
            .into());
        }

        let contents = std::fs::read_to_string(&manifest_path).map_err(|e| {
            CoreError::Manifest(format!(
                "failed to read {}: {e}",
                manifest_path.display()
            ))
        })?;

        let manifest: PluginManifest = serde_json::from_str(&contents).map_err(|e| {
            CoreError::Manifest(format!(
                "failed to parse {}: {e}",
                manifest_path.display()
            ))
        })?;

        debug!(
            plugin_id = %manifest.id,
            path = %manifest_path.display(),
            "loaded plugin manifest"
        );

        Ok(manifest)
    }

    /// Validate the manifest against the plugin specification.
    ///
    /// Checks that:
    /// - `id` is in reverse-domain format (contains at least one dot,
    ///   segments are non-empty and start with a lowercase letter)
    /// - `version` is a valid semver string (major.minor.patch)
    /// - `element` contains a hyphen (Web Components spec requirement)
    /// - `name` is non-empty
    /// - `entry` is non-empty
    /// - `capabilities` is non-empty
    pub fn validate(&self) -> anyhow::Result<()> {
        // Name must be non-empty.
        if self.name.trim().is_empty() {
            return Err(CoreError::Manifest(
                "manifest 'name' must not be empty".into(),
            )
            .into());
        }

        // Entry must be non-empty.
        if self.entry.trim().is_empty() {
            return Err(CoreError::Manifest(
                "manifest 'entry' must not be empty".into(),
            )
            .into());
        }

        // Capabilities must be non-empty.
        if self.capabilities.is_empty() {
            return Err(CoreError::Manifest(
                "manifest 'capabilities' must not be empty".into(),
            )
            .into());
        }

        // ID must be reverse-domain format.
        validate_reverse_domain_id(&self.id)?;

        // Version must be semver.
        validate_semver(&self.version, "version")?;

        // min_shell_version must be semver.
        validate_semver(&self.min_shell_version, "minShellVersion")?;

        // Element must contain a hyphen (Web Components spec).
        if !self.element.contains('-') {
            return Err(CoreError::Manifest(format!(
                "manifest 'element' must contain a hyphen (Web Components spec), got '{}'",
                self.element
            ))
            .into());
        }

        Ok(())
    }
}

/// Validate that a string is in reverse-domain format.
///
/// Requirements:
/// - Contains at least one dot (at least two segments)
/// - Each segment is non-empty
/// - Each segment starts with a lowercase ASCII letter or digit
fn validate_reverse_domain_id(id: &str) -> anyhow::Result<()> {
    if id.trim().is_empty() {
        return Err(CoreError::Manifest(
            "manifest 'id' must not be empty".into(),
        )
        .into());
    }

    let segments: Vec<&str> = id.split('.').collect();

    if segments.len() < 2 {
        return Err(CoreError::Manifest(format!(
            "manifest 'id' must be in reverse-domain format (e.g. 'com.example.plugin'), got '{id}'"
        ))
        .into());
    }

    for segment in &segments {
        if segment.is_empty() {
            return Err(CoreError::Manifest(format!(
                "manifest 'id' contains an empty segment: '{id}'"
            ))
            .into());
        }

        let first_char = segment.chars().next().unwrap_or(' ');
        if !first_char.is_ascii_lowercase() {
            return Err(CoreError::Manifest(format!(
                "manifest 'id' segment must start with a lowercase letter, got '{segment}' in '{id}'"
            ))
            .into());
        }
    }

    Ok(())
}

/// Validate that a string is a valid semver version (major.minor.patch).
///
/// Uses string-based validation (no regex crate). Accepts optional
/// pre-release and build metadata suffixes.
fn validate_semver(version: &str, field_name: &str) -> anyhow::Result<()> {
    if version.trim().is_empty() {
        return Err(CoreError::Manifest(format!(
            "manifest '{field_name}' must not be empty"
        ))
        .into());
    }

    // Strip pre-release and build metadata for core version check.
    let core_version = version
        .split('+')
        .next()
        .unwrap_or(version)
        .split('-')
        .next()
        .unwrap_or(version);

    let parts: Vec<&str> = core_version.split('.').collect();

    if parts.len() != 3 {
        return Err(CoreError::Manifest(format!(
            "manifest '{field_name}' must be semver (major.minor.patch), got '{version}'"
        ))
        .into());
    }

    for (i, part) in parts.iter().enumerate() {
        let label = match i {
            0 => "major",
            1 => "minor",
            _ => "patch",
        };
        if part.parse::<u64>().is_err() {
            return Err(CoreError::Manifest(format!(
                "manifest '{field_name}' has non-numeric {label} version in '{version}'"
            ))
            .into());
        }
    }

    Ok(())
}

/// Discover plugin manifests from a list of parent directories.
///
/// Each path is treated as a parent directory containing plugin
/// subdirectories. Each subdirectory is checked for a `plugin.json` file.
///
/// Returns a list of `(path, result)` tuples where `path` is the plugin
/// directory and `result` is either the parsed manifest or an error.
pub fn discover_manifests(dirs: &[PathBuf]) -> Vec<(PathBuf, anyhow::Result<PluginManifest>)> {
    let mut results = Vec::new();

    for dir in dirs {
        if !dir.exists() {
            warn!(path = %dir.display(), "manifest discovery: directory does not exist, skipping");
            continue;
        }

        if !dir.is_dir() {
            warn!(path = %dir.display(), "manifest discovery: path is not a directory, skipping");
            continue;
        }

        debug!(path = %dir.display(), "scanning directory for plugin manifests");

        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) => {
                warn!(
                    path = %dir.display(),
                    error = %e,
                    "manifest discovery: failed to read directory"
                );
                continue;
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    debug!(error = %e, "manifest discovery: failed to read directory entry");
                    continue;
                }
            };

            let sub_path = entry.path();
            if !sub_path.is_dir() {
                continue;
            }

            let manifest_path = sub_path.join("plugin.json");
            if !manifest_path.exists() {
                debug!(
                    path = %sub_path.display(),
                    "no plugin.json found, skipping"
                );
                continue;
            }

            let result = PluginManifest::load_from_path(&sub_path);
            match &result {
                Ok(manifest) => {
                    info!(
                        plugin_id = %manifest.id,
                        path = %sub_path.display(),
                        "discovered plugin manifest"
                    );
                }
                Err(e) => {
                    warn!(
                        path = %sub_path.display(),
                        error = %e,
                        "failed to load plugin manifest"
                    );
                }
            }

            results.push((sub_path, result));
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Helper to create a valid manifest JSON string for testing.
    fn valid_manifest_json() -> String {
        r#"{
            "id": "com.life-engine.test-plugin",
            "name": "Test Plugin",
            "version": "1.0.0",
            "entry": "index.js",
            "element": "test-plugin",
            "minShellVersion": "0.1.0",
            "sidebar": {
                "label": "Test",
                "icon": "test-icon",
                "order": 1
            },
            "capabilities": ["data:read:tasks"]
        }"#
        .to_string()
    }

    /// Helper to create a minimal valid manifest JSON.
    fn minimal_manifest_json() -> String {
        r#"{
            "id": "com.example.minimal",
            "name": "Minimal",
            "version": "0.1.0",
            "entry": "main.js",
            "element": "minimal-plugin",
            "minShellVersion": "0.1.0",
            "sidebar": { "label": "Min", "icon": "star" },
            "capabilities": ["ui:navigate"]
        }"#
        .to_string()
    }

    // --- Deserialization tests ---

    #[test]
    fn deserialize_valid_manifest() {
        let json = valid_manifest_json();
        let manifest: PluginManifest = serde_json::from_str(&json).expect("should deserialize");

        assert_eq!(manifest.id, "com.life-engine.test-plugin");
        assert_eq!(manifest.name, "Test Plugin");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.entry, "index.js");
        assert_eq!(manifest.element, "test-plugin");
        assert_eq!(manifest.min_shell_version, "0.1.0");
        let sidebar = manifest.sidebar.as_ref().expect("sidebar should be present");
        assert_eq!(sidebar.label, "Test");
        assert_eq!(sidebar.icon, "test-icon");
        assert_eq!(sidebar.order, Some(1));
        assert_eq!(manifest.capabilities, vec!["data:read:tasks"]);
    }

    #[test]
    fn deserialize_minimal_manifest() {
        let json = minimal_manifest_json();
        let manifest: PluginManifest = serde_json::from_str(&json).expect("should deserialize");

        assert_eq!(manifest.id, "com.example.minimal");
        assert!(manifest.description.is_none());
        assert!(manifest.author.is_none());
        assert!(manifest.license.is_none());
        assert!(manifest.command_palette.is_none());
        assert!(manifest.dashboard.is_none());
        assert!(manifest.collections.is_none());
        assert!(manifest.allowed_domains.is_none());
        assert!(manifest.settings.is_none());
        assert!(manifest.dependencies.is_none());
        assert!(manifest.large_bundle.is_none());
        assert!(manifest.routes.is_none());
        assert!(manifest.sidebar.as_ref().unwrap().order.is_none());
    }

    #[test]
    fn deserialize_manifest_with_all_optional_fields() {
        let json = r#"{
            "id": "com.example.full",
            "name": "Full Plugin",
            "version": "2.0.0",
            "entry": "dist/index.js",
            "element": "full-plugin",
            "minShellVersion": "1.0.0",
            "sidebar": { "label": "Full", "icon": "star", "order": 5 },
            "capabilities": ["data:read:tasks", "network:fetch"],
            "description": "A full-featured plugin",
            "author": "Test Author",
            "license": "MIT",
            "commandPalette": [
                { "id": "cmd1", "label": "Command 1", "action": "doThing" }
            ],
            "dashboard": { "element": "full-widget", "defaultSize": "medium" },
            "collections": [{ "name": "items", "schema": "items.schema.json" }],
            "allowedDomains": ["api.example.com"],
            "settings": [{ "key": "theme", "label": "Theme", "type": "select", "options": ["dark", "light"] }],
            "dependencies": { "sharedModules": ["lit"] },
            "largeBundle": true,
            "routes": [{ "path": "/view", "label": "View" }]
        }"#;

        let manifest: PluginManifest = serde_json::from_str(json).expect("should deserialize");

        assert_eq!(manifest.description.as_deref(), Some("A full-featured plugin"));
        assert_eq!(manifest.author.as_ref().and_then(|a| a.as_str()), Some("Test Author"));
        assert_eq!(manifest.license.as_deref(), Some("MIT"));
        assert!(manifest.command_palette.is_some());
        assert_eq!(manifest.command_palette.as_ref().unwrap().len(), 1);
        assert!(manifest.dashboard.is_some());
        assert_eq!(manifest.dashboard.as_ref().unwrap().default_size, "medium");
        assert!(manifest.collections.is_some());
        assert!(manifest.allowed_domains.is_some());
        assert_eq!(manifest.allowed_domains.as_ref().unwrap(), &["api.example.com"]);
        assert!(manifest.settings.is_some());
        assert!(manifest.dependencies.is_some());
        assert_eq!(manifest.dependencies.as_ref().unwrap().shared_modules, vec!["lit"]);
        assert_eq!(manifest.large_bundle, Some(true));
        assert!(manifest.routes.is_some());
        assert_eq!(manifest.routes.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn deserialize_manifest_with_author_object() {
        let json = r#"{
            "id": "com.example.authored",
            "name": "Authored",
            "version": "1.0.0",
            "entry": "index.js",
            "element": "authored-plugin",
            "minShellVersion": "0.1.0",
            "sidebar": { "label": "Auth", "icon": "user" },
            "capabilities": ["ui:navigate"],
            "author": { "name": "Jane Doe", "email": "jane@example.com" }
        }"#;

        let manifest: PluginManifest = serde_json::from_str(json).expect("should deserialize");
        let author = manifest.author.as_ref().unwrap();
        assert_eq!(author["name"], "Jane Doe");
        assert_eq!(author["email"], "jane@example.com");
    }

    #[test]
    fn deserialize_collections_as_string_array() {
        // Real-world calendar plugin uses collections as string array.
        let json = r#"{
            "id": "com.example.cal",
            "name": "Cal",
            "version": "0.1.0",
            "entry": "index.js",
            "element": "cal-plugin",
            "minShellVersion": "0.1.0",
            "sidebar": { "label": "Cal", "icon": "cal" },
            "capabilities": ["data:read:events"],
            "collections": ["events"]
        }"#;

        let manifest: PluginManifest = serde_json::from_str(json).expect("should deserialize");
        assert!(manifest.collections.is_some());
        let collections = manifest.collections.unwrap();
        assert!(collections.is_array());
        assert_eq!(collections[0], "events");
    }

    // --- Validation tests ---

    #[test]
    fn validate_valid_manifest() {
        let json = valid_manifest_json();
        let manifest: PluginManifest = serde_json::from_str(&json).unwrap();
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn validate_rejects_empty_name() {
        let json = r#"{
            "id": "com.example.test",
            "name": "",
            "version": "1.0.0",
            "entry": "index.js",
            "element": "test-plugin",
            "minShellVersion": "0.1.0",
            "sidebar": { "label": "Test", "icon": "test" },
            "capabilities": ["data:read:tasks"]
        }"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        let err = manifest.validate().unwrap_err();
        assert!(err.to_string().contains("name"));
    }

    #[test]
    fn validate_rejects_whitespace_only_name() {
        let json = r#"{
            "id": "com.example.test",
            "name": "   ",
            "version": "1.0.0",
            "entry": "index.js",
            "element": "test-plugin",
            "minShellVersion": "0.1.0",
            "sidebar": { "label": "Test", "icon": "test" },
            "capabilities": ["data:read:tasks"]
        }"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        let err = manifest.validate().unwrap_err();
        assert!(err.to_string().contains("name"));
    }

    #[test]
    fn validate_rejects_empty_entry() {
        let json = r#"{
            "id": "com.example.test",
            "name": "Test",
            "version": "1.0.0",
            "entry": "",
            "element": "test-plugin",
            "minShellVersion": "0.1.0",
            "sidebar": { "label": "Test", "icon": "test" },
            "capabilities": ["data:read:tasks"]
        }"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        let err = manifest.validate().unwrap_err();
        assert!(err.to_string().contains("entry"));
    }

    #[test]
    fn validate_rejects_empty_capabilities() {
        let json = r#"{
            "id": "com.example.test",
            "name": "Test",
            "version": "1.0.0",
            "entry": "index.js",
            "element": "test-plugin",
            "minShellVersion": "0.1.0",
            "sidebar": { "label": "Test", "icon": "test" },
            "capabilities": []
        }"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        let err = manifest.validate().unwrap_err();
        assert!(err.to_string().contains("capabilities"));
    }

    #[test]
    fn validate_rejects_id_without_dot() {
        let json = r#"{
            "id": "nodotshere",
            "name": "Test",
            "version": "1.0.0",
            "entry": "index.js",
            "element": "test-plugin",
            "minShellVersion": "0.1.0",
            "sidebar": { "label": "Test", "icon": "test" },
            "capabilities": ["data:read:tasks"]
        }"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        let err = manifest.validate().unwrap_err();
        assert!(err.to_string().contains("reverse-domain"));
    }

    #[test]
    fn validate_rejects_id_with_empty_segment() {
        let json = r#"{
            "id": "com..test",
            "name": "Test",
            "version": "1.0.0",
            "entry": "index.js",
            "element": "test-plugin",
            "minShellVersion": "0.1.0",
            "sidebar": { "label": "Test", "icon": "test" },
            "capabilities": ["data:read:tasks"]
        }"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        let err = manifest.validate().unwrap_err();
        assert!(err.to_string().contains("empty segment"));
    }

    #[test]
    fn validate_rejects_id_segment_starting_with_uppercase() {
        let json = r#"{
            "id": "com.Example.test",
            "name": "Test",
            "version": "1.0.0",
            "entry": "index.js",
            "element": "test-plugin",
            "minShellVersion": "0.1.0",
            "sidebar": { "label": "Test", "icon": "test" },
            "capabilities": ["data:read:tasks"]
        }"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        let err = manifest.validate().unwrap_err();
        assert!(err.to_string().contains("lowercase"));
    }

    #[test]
    fn validate_rejects_id_segment_starting_with_digit() {
        let json = r#"{
            "id": "com.1bad.test",
            "name": "Test",
            "version": "1.0.0",
            "entry": "index.js",
            "element": "test-plugin",
            "minShellVersion": "0.1.0",
            "sidebar": { "label": "Test", "icon": "test" },
            "capabilities": ["data:read:tasks"]
        }"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        let err = manifest.validate().unwrap_err();
        assert!(err.to_string().contains("lowercase"));
    }

    #[test]
    fn validate_rejects_empty_id() {
        let json = r#"{
            "id": "",
            "name": "Test",
            "version": "1.0.0",
            "entry": "index.js",
            "element": "test-plugin",
            "minShellVersion": "0.1.0",
            "sidebar": { "label": "Test", "icon": "test" },
            "capabilities": ["data:read:tasks"]
        }"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        let err = manifest.validate().unwrap_err();
        assert!(err.to_string().contains("id"));
    }

    #[test]
    fn validate_rejects_non_semver_version() {
        let json = r#"{
            "id": "com.example.test",
            "name": "Test",
            "version": "1.0",
            "entry": "index.js",
            "element": "test-plugin",
            "minShellVersion": "0.1.0",
            "sidebar": { "label": "Test", "icon": "test" },
            "capabilities": ["data:read:tasks"]
        }"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        let err = manifest.validate().unwrap_err();
        assert!(err.to_string().contains("semver"));
    }

    #[test]
    fn validate_rejects_non_numeric_version_part() {
        let json = r#"{
            "id": "com.example.test",
            "name": "Test",
            "version": "1.abc.0",
            "entry": "index.js",
            "element": "test-plugin",
            "minShellVersion": "0.1.0",
            "sidebar": { "label": "Test", "icon": "test" },
            "capabilities": ["data:read:tasks"]
        }"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        let err = manifest.validate().unwrap_err();
        assert!(err.to_string().contains("non-numeric"));
    }

    #[test]
    fn validate_accepts_semver_with_prerelease() {
        let json = r#"{
            "id": "com.example.test",
            "name": "Test",
            "version": "1.0.0-beta.1",
            "entry": "index.js",
            "element": "test-plugin",
            "minShellVersion": "0.1.0",
            "sidebar": { "label": "Test", "icon": "test" },
            "capabilities": ["data:read:tasks"]
        }"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn validate_accepts_semver_with_build_metadata() {
        let json = r#"{
            "id": "com.example.test",
            "name": "Test",
            "version": "1.0.0+build.123",
            "entry": "index.js",
            "element": "test-plugin",
            "minShellVersion": "0.1.0",
            "sidebar": { "label": "Test", "icon": "test" },
            "capabilities": ["data:read:tasks"]
        }"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn validate_rejects_element_without_hyphen() {
        let json = r#"{
            "id": "com.example.test",
            "name": "Test",
            "version": "1.0.0",
            "entry": "index.js",
            "element": "nohyphen",
            "minShellVersion": "0.1.0",
            "sidebar": { "label": "Test", "icon": "test" },
            "capabilities": ["data:read:tasks"]
        }"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        let err = manifest.validate().unwrap_err();
        assert!(err.to_string().contains("hyphen"));
    }

    #[test]
    fn validate_rejects_invalid_min_shell_version() {
        let json = r#"{
            "id": "com.example.test",
            "name": "Test",
            "version": "1.0.0",
            "entry": "index.js",
            "element": "test-plugin",
            "minShellVersion": "not-a-version",
            "sidebar": { "label": "Test", "icon": "test" },
            "capabilities": ["data:read:tasks"]
        }"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        let err = manifest.validate().unwrap_err();
        assert!(err.to_string().contains("minShellVersion"));
    }

    // --- load_from_path tests ---

    #[test]
    fn load_from_path_with_missing_dir() {
        let result = PluginManifest::load_from_path(Path::new("/nonexistent/path/to/plugin"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("does not exist"));
    }

    #[test]
    fn load_from_path_with_missing_plugin_json() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let result = PluginManifest::load_from_path(dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("plugin.json not found"));
    }

    #[test]
    fn load_from_path_with_invalid_json() {
        let dir = tempfile::tempdir().expect("create tempdir");
        fs::write(dir.path().join("plugin.json"), "{ not valid json }").expect("write file");
        let result = PluginManifest::load_from_path(dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("failed to parse"));
    }

    #[test]
    fn load_from_path_with_valid_json() {
        let dir = tempfile::tempdir().expect("create tempdir");
        fs::write(dir.path().join("plugin.json"), valid_manifest_json()).expect("write file");
        let manifest = PluginManifest::load_from_path(dir.path()).expect("should load");
        assert_eq!(manifest.id, "com.life-engine.test-plugin");
        assert_eq!(manifest.name, "Test Plugin");
    }

    #[test]
    fn load_from_path_with_missing_required_field() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let json = r#"{ "id": "com.example.test", "name": "Test" }"#;
        fs::write(dir.path().join("plugin.json"), json).expect("write file");
        let result = PluginManifest::load_from_path(dir.path());
        assert!(result.is_err());
    }

    // --- discover_manifests tests ---

    #[test]
    fn discover_manifests_with_empty_dirs() {
        let results = discover_manifests(&[]);
        assert!(results.is_empty());
    }

    #[test]
    fn discover_manifests_skips_missing_dirs() {
        let results = discover_manifests(&[PathBuf::from("/nonexistent/path")]);
        assert!(results.is_empty());
    }

    #[test]
    fn discover_manifests_finds_valid_plugin() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let plugin_dir = dir.path().join("my-plugin");
        fs::create_dir(&plugin_dir).expect("create dir");
        fs::write(plugin_dir.join("plugin.json"), valid_manifest_json()).expect("write file");

        let results = discover_manifests(&[dir.path().to_path_buf()]);
        assert_eq!(results.len(), 1);
        let (path, result) = &results[0];
        assert!(path.ends_with("my-plugin"));
        assert!(result.is_ok());
        assert_eq!(result.as_ref().unwrap().id, "com.life-engine.test-plugin");
    }

    #[test]
    fn discover_manifests_records_invalid_manifests() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let plugin_dir = dir.path().join("bad-plugin");
        fs::create_dir(&plugin_dir).expect("create dir");
        fs::write(plugin_dir.join("plugin.json"), "{ bad json }").expect("write file");

        let results = discover_manifests(&[dir.path().to_path_buf()]);
        assert_eq!(results.len(), 1);
        let (_, result) = &results[0];
        assert!(result.is_err());
    }

    #[test]
    fn discover_manifests_skips_dirs_without_plugin_json() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let empty_dir = dir.path().join("empty-plugin");
        fs::create_dir(&empty_dir).expect("create dir");

        let results = discover_manifests(&[dir.path().to_path_buf()]);
        assert!(results.is_empty());
    }

    #[test]
    fn discover_manifests_finds_multiple_plugins() {
        let dir = tempfile::tempdir().expect("create tempdir");

        for (i, name) in ["plugin-a", "plugin-b"].iter().enumerate() {
            let plugin_dir = dir.path().join(name);
            fs::create_dir(&plugin_dir).expect("create dir");
            let json = format!(
                r#"{{
                    "id": "com.example.{name}",
                    "name": "Plugin {i}",
                    "version": "0.1.0",
                    "entry": "index.js",
                    "element": "{name}",
                    "minShellVersion": "0.1.0",
                    "sidebar": {{ "label": "P", "icon": "x" }},
                    "capabilities": ["ui:navigate"]
                }}"#
            );
            fs::write(plugin_dir.join("plugin.json"), json).expect("write file");
        }

        let results = discover_manifests(&[dir.path().to_path_buf()]);
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|(_, r)| r.is_ok()));
    }

    #[test]
    fn discover_manifests_scans_multiple_parent_dirs() {
        let dir_a = tempfile::tempdir().expect("create tempdir a");
        let dir_b = tempfile::tempdir().expect("create tempdir b");

        let plugin_a = dir_a.path().join("plug-a");
        fs::create_dir(&plugin_a).expect("create dir");
        fs::write(plugin_a.join("plugin.json"), valid_manifest_json()).expect("write file");

        let plugin_b = dir_b.path().join("plug-b");
        fs::create_dir(&plugin_b).expect("create dir");
        fs::write(
            plugin_b.join("plugin.json"),
            minimal_manifest_json(),
        )
        .expect("write file");

        let results = discover_manifests(&[
            dir_a.path().to_path_buf(),
            dir_b.path().to_path_buf(),
        ]);
        assert_eq!(results.len(), 2);
    }

    // --- Real manifest tests ---

    #[test]
    fn load_real_email_viewer_manifest() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../plugins/life/email-viewer");
        let manifest = PluginManifest::load_from_path(&manifest_dir)
            .expect("should load email-viewer manifest");

        assert_eq!(manifest.id, "com.life-engine.email-viewer");
        assert_eq!(manifest.name, "Email Viewer");
        assert_eq!(manifest.version, "0.1.0");
        assert_eq!(manifest.entry, "index.js");
        assert_eq!(manifest.element, "email-viewer-plugin");
        assert_eq!(manifest.min_shell_version, "0.1.0");
        assert_eq!(manifest.capabilities.len(), 5);
        assert!(manifest.settings.is_some());
        // Real manifest uses slots instead of sidebar.
        assert!(manifest.sidebar.is_none());
        assert!(manifest.slots.is_some());
        let slots = manifest.slots.as_ref().unwrap();
        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0]["type"], "sidebar.item");
        assert_eq!(slots[0]["label"], "Email");
        assert_eq!(slots[0]["icon"], "mail");

        // Validate passes.
        manifest.validate().expect("email-viewer manifest should be valid");
    }

    #[test]
    fn load_real_calendar_manifest() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../plugins/life/calendar");
        let manifest = PluginManifest::load_from_path(&manifest_dir)
            .expect("should load calendar manifest");

        assert_eq!(manifest.id, "com.life-engine.calendar");
        assert_eq!(manifest.name, "Calendar");
        assert_eq!(manifest.version, "0.1.0");
        assert_eq!(manifest.entry, "index.js");
        assert_eq!(manifest.element, "calendar-plugin");
        assert_eq!(manifest.min_shell_version, "0.1.0");
        assert_eq!(manifest.capabilities.len(), 6);
        assert!(manifest.collections.is_some());
        assert!(manifest.routes.is_some());
        // Real manifest uses slots instead of sidebar.
        assert!(manifest.sidebar.is_none());
        assert!(manifest.slots.is_some());
        let slots = manifest.slots.as_ref().unwrap();
        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0]["type"], "sidebar.item");
        assert_eq!(slots[0]["label"], "Calendar");
        assert_eq!(slots[0]["icon"], "calendar");

        // Validate passes.
        manifest.validate().expect("calendar manifest should be valid");
    }

    // --- validate_reverse_domain_id tests ---

    #[test]
    fn reverse_domain_valid_ids() {
        for id in [
            "com.example.plugin",
            "com.life-engine.test",
            "org.test.my-plugin",
            "io.github.user.repo",
        ] {
            assert!(
                validate_reverse_domain_id(id).is_ok(),
                "expected '{id}' to be valid"
            );
        }
    }

    #[test]
    fn reverse_domain_invalid_ids() {
        for id in ["single", "", "  ", "com.", ".com.test"] {
            assert!(
                validate_reverse_domain_id(id).is_err(),
                "expected '{id}' to be invalid"
            );
        }
    }

    // --- validate_semver tests ---

    #[test]
    fn semver_valid_versions() {
        for v in [
            "0.0.0",
            "1.0.0",
            "99.99.99",
            "1.0.0-alpha",
            "1.0.0-beta.1",
            "1.0.0+build.42",
            "1.0.0-rc.1+meta",
        ] {
            assert!(
                validate_semver(v, "version").is_ok(),
                "expected '{v}' to be valid semver"
            );
        }
    }

    #[test]
    fn semver_invalid_versions() {
        for v in ["1.0", "1", "abc", "", "1.0.0.0", "1.x.0"] {
            assert!(
                validate_semver(v, "version").is_err(),
                "expected '{v}' to be invalid semver"
            );
        }
    }

    // --- PluginManifest integration with PluginLoader ---

    #[test]
    fn manifest_serialization_roundtrip() {
        let json = valid_manifest_json();
        let manifest: PluginManifest = serde_json::from_str(&json).unwrap();
        let serialized = serde_json::to_string(&manifest).expect("serialize");
        let restored: PluginManifest =
            serde_json::from_str(&serialized).expect("deserialize roundtrip");
        assert_eq!(manifest.id, restored.id);
        assert_eq!(manifest.name, restored.name);
        assert_eq!(manifest.version, restored.version);
    }
}
