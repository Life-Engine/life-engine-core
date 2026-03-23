//! Plugin manifest parser for `manifest.toml` files.
//!
//! Parses plugin identity, actions, capabilities, and config schema
//! from a TOML manifest file.

use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;

use life_engine_traits::Capability;
use serde::Deserialize;

use crate::error::PluginError;

/// Parsed plugin manifest containing identity, actions, capabilities, and config schema.
#[derive(Debug, Clone)]
pub struct PluginManifest {
    /// Plugin identity metadata.
    pub plugin: PluginMeta,
    /// Named actions the plugin exposes.
    pub actions: HashMap<String, ActionDef>,
    /// Capabilities the plugin requires.
    pub capabilities: CapabilitySet,
    /// Optional JSON Schema for plugin-specific configuration.
    pub config: Option<ConfigSchema>,
}

/// Plugin identity metadata from the `[plugin]` section.
#[derive(Debug, Clone)]
pub struct PluginMeta {
    /// Unique plugin identifier (lowercase with hyphens).
    pub id: String,
    /// Human-readable plugin name.
    pub name: String,
    /// Semver version string.
    pub version: String,
    /// Optional plugin description.
    pub description: Option<String>,
    /// Optional plugin author.
    pub author: Option<String>,
}

/// Definition of a single plugin action.
#[derive(Debug, Clone)]
pub struct ActionDef {
    /// Human-readable action description.
    pub description: String,
    /// Optional JSON Schema for action input.
    pub input_schema: Option<String>,
    /// Optional JSON Schema for action output.
    pub output_schema: Option<String>,
}

/// Set of capabilities a plugin requires.
#[derive(Debug, Clone, Default)]
pub struct CapabilitySet {
    /// Required capabilities.
    pub required: Vec<Capability>,
}

/// Plugin configuration schema.
#[derive(Debug, Clone)]
pub struct ConfigSchema {
    /// Raw JSON Schema value.
    pub schema: serde_json::Value,
}

// --- Raw TOML deserialization types ---

#[derive(Deserialize)]
struct RawManifest {
    plugin: Option<RawPluginMeta>,
    actions: Option<HashMap<String, RawActionDef>>,
    capabilities: Option<RawCapabilities>,
    config: Option<RawConfigSchema>,
}

#[derive(Deserialize)]
struct RawPluginMeta {
    id: Option<String>,
    name: Option<String>,
    version: Option<String>,
    description: Option<String>,
    author: Option<String>,
}

#[derive(Deserialize)]
struct RawActionDef {
    description: Option<String>,
    input_schema: Option<String>,
    output_schema: Option<String>,
}

#[derive(Deserialize)]
struct RawCapabilities {
    required: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct RawConfigSchema {
    schema: Option<serde_json::Value>,
}

/// Regex pattern for valid plugin IDs: lowercase letters, digits, and hyphens,
/// starting with a letter.
fn is_valid_plugin_id(id: &str) -> bool {
    let mut chars = id.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Validates that a string is a valid semver version.
fn is_valid_semver(version: &str) -> bool {
    semver::Version::parse(version).is_ok()
}

/// Parses a `manifest.toml` file into a `PluginManifest`.
pub fn parse_manifest(path: &Path) -> Result<PluginManifest, PluginError> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        PluginError::ManifestInvalid(format!("failed to read {}: {e}", path.display()))
    })?;

    let raw: RawManifest = toml::from_str(&content).map_err(|e| {
        PluginError::ManifestInvalid(format!("failed to parse {}: {e}", path.display()))
    })?;

    let path_str = path.display().to_string();

    // [plugin] section is required
    let raw_plugin = raw.plugin.ok_or_else(|| {
        PluginError::ManifestInvalid(format!(
            "missing [plugin] section in {path_str}"
        ))
    })?;

    // Required fields
    let id = raw_plugin.id.ok_or_else(|| PluginError::ManifestMissingField {
        field: "id".to_string(),
        path: path_str.clone(),
    })?;

    let name = raw_plugin
        .name
        .ok_or_else(|| PluginError::ManifestMissingField {
            field: "name".to_string(),
            path: path_str.clone(),
        })?;

    let version = raw_plugin
        .version
        .ok_or_else(|| PluginError::ManifestMissingField {
            field: "version".to_string(),
            path: path_str.clone(),
        })?;

    // Validate plugin ID format
    if !is_valid_plugin_id(&id) {
        return Err(PluginError::ManifestInvalid(format!(
            "invalid plugin ID '{id}': must start with a lowercase letter and contain only lowercase letters, digits, and hyphens"
        )));
    }

    // Validate semver
    if !is_valid_semver(&version) {
        return Err(PluginError::ManifestInvalid(format!(
            "invalid version '{version}': must be valid semver (e.g., 1.0.0)"
        )));
    }

    let plugin = PluginMeta {
        id,
        name,
        version,
        description: raw_plugin.description,
        author: raw_plugin.author,
    };

    // Parse actions (optional)
    let actions = match raw.actions {
        Some(raw_actions) => raw_actions
            .into_iter()
            .map(|(name, raw_action)| {
                let action = ActionDef {
                    description: raw_action.description.unwrap_or_default(),
                    input_schema: raw_action.input_schema,
                    output_schema: raw_action.output_schema,
                };
                (name, action)
            })
            .collect(),
        None => HashMap::new(),
    };

    // Parse capabilities (optional)
    let capabilities = match raw.capabilities {
        Some(raw_caps) => {
            let required = match raw_caps.required {
                Some(cap_strings) => {
                    let mut caps = Vec::with_capacity(cap_strings.len());
                    for s in &cap_strings {
                        let cap = Capability::from_str(s).map_err(|_| {
                            PluginError::ManifestInvalid(format!(
                                "unknown capability '{s}' in manifest for plugin '{}'. Valid capabilities: storage:read, storage:write, http:outbound, events:emit, events:subscribe, config:read",
                                plugin.id
                            ))
                        })?;
                        caps.push(cap);
                    }
                    caps
                }
                None => Vec::new(),
            };
            CapabilitySet { required }
        }
        None => CapabilitySet::default(),
    };

    // Parse config schema (optional)
    let config = raw.config.and_then(|c| {
        c.schema.map(|schema| ConfigSchema { schema })
    });

    Ok(PluginManifest {
        plugin,
        actions,
        capabilities,
        config,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_manifest(dir: &Path, content: &str) -> std::path::PathBuf {
        let path = dir.join("manifest.toml");
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn parses_complete_manifest() {
        let tmp = TempDir::new().unwrap();
        let path = write_manifest(
            tmp.path(),
            r#"
[plugin]
id = "connector-email"
name = "Email Connector"
version = "1.2.3"
description = "Connects to email providers"
author = "Life Engine Team"

[actions.fetch-emails]
description = "Fetches new emails"
input_schema = '{"type": "object"}'
output_schema = '{"type": "array"}'

[actions.send-email]
description = "Sends an email"

[capabilities]
required = ["storage:read", "storage:write", "http:outbound"]

[config.schema]
type = "object"

[config.schema.properties.poll_interval]
type = "string"
"#,
        );

        let manifest = parse_manifest(&path).unwrap();

        assert_eq!(manifest.plugin.id, "connector-email");
        assert_eq!(manifest.plugin.name, "Email Connector");
        assert_eq!(manifest.plugin.version, "1.2.3");
        assert_eq!(
            manifest.plugin.description.as_deref(),
            Some("Connects to email providers")
        );
        assert_eq!(
            manifest.plugin.author.as_deref(),
            Some("Life Engine Team")
        );
        assert_eq!(manifest.actions.len(), 2);
        assert!(manifest.actions.contains_key("fetch-emails"));
        assert!(manifest.actions.contains_key("send-email"));
        assert_eq!(
            manifest.actions["fetch-emails"].input_schema.as_deref(),
            Some(r#"{"type": "object"}"#)
        );
        assert_eq!(manifest.capabilities.required.len(), 3);
        assert!(manifest.config.is_some());
    }

    #[test]
    fn parses_minimal_manifest() {
        let tmp = TempDir::new().unwrap();
        let path = write_manifest(
            tmp.path(),
            r#"
[plugin]
id = "minimal"
name = "Minimal Plugin"
version = "0.1.0"
"#,
        );

        let manifest = parse_manifest(&path).unwrap();

        assert_eq!(manifest.plugin.id, "minimal");
        assert!(manifest.actions.is_empty());
        assert!(manifest.capabilities.required.is_empty());
        assert!(manifest.config.is_none());
    }

    #[test]
    fn missing_plugin_section_returns_error() {
        let tmp = TempDir::new().unwrap();
        let path = write_manifest(
            tmp.path(),
            r#"
[actions.something]
description = "No plugin section"
"#,
        );

        let err = parse_manifest(&path).unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid(_)));
        assert!(err.to_string().contains("[plugin]"));
    }

    #[test]
    fn missing_required_field_id_returns_error() {
        let tmp = TempDir::new().unwrap();
        let path = write_manifest(
            tmp.path(),
            r#"
[plugin]
name = "No ID"
version = "1.0.0"
"#,
        );

        let err = parse_manifest(&path).unwrap_err();
        assert!(matches!(err, PluginError::ManifestMissingField { .. }));
        assert!(err.to_string().contains("id"));
    }

    #[test]
    fn missing_required_field_name_returns_error() {
        let tmp = TempDir::new().unwrap();
        let path = write_manifest(
            tmp.path(),
            r#"
[plugin]
id = "test"
version = "1.0.0"
"#,
        );

        let err = parse_manifest(&path).unwrap_err();
        assert!(matches!(err, PluginError::ManifestMissingField { .. }));
        assert!(err.to_string().contains("name"));
    }

    #[test]
    fn missing_required_field_version_returns_error() {
        let tmp = TempDir::new().unwrap();
        let path = write_manifest(
            tmp.path(),
            r#"
[plugin]
id = "test"
name = "Test"
"#,
        );

        let err = parse_manifest(&path).unwrap_err();
        assert!(matches!(err, PluginError::ManifestMissingField { .. }));
        assert!(err.to_string().contains("version"));
    }

    #[test]
    fn invalid_plugin_id_uppercase_returns_error() {
        let tmp = TempDir::new().unwrap();
        let path = write_manifest(
            tmp.path(),
            r#"
[plugin]
id = "MyPlugin"
name = "Test"
version = "1.0.0"
"#,
        );

        let err = parse_manifest(&path).unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid(_)));
        assert!(err.to_string().contains("invalid plugin ID"));
    }

    #[test]
    fn invalid_plugin_id_spaces_returns_error() {
        let tmp = TempDir::new().unwrap();
        let path = write_manifest(
            tmp.path(),
            r#"
[plugin]
id = "my plugin"
name = "Test"
version = "1.0.0"
"#,
        );

        let err = parse_manifest(&path).unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid(_)));
    }

    #[test]
    fn invalid_semver_returns_error() {
        let tmp = TempDir::new().unwrap();
        let path = write_manifest(
            tmp.path(),
            r#"
[plugin]
id = "test"
name = "Test"
version = "not-a-version"
"#,
        );

        let err = parse_manifest(&path).unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid(_)));
        assert!(err.to_string().contains("invalid version"));
    }

    #[test]
    fn actions_extracted_with_schemas() {
        let tmp = TempDir::new().unwrap();
        let path = write_manifest(
            tmp.path(),
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[actions.process]
description = "Processes data"
input_schema = '{"type": "object"}'
output_schema = '{"type": "string"}'
"#,
        );

        let manifest = parse_manifest(&path).unwrap();
        let action = &manifest.actions["process"];

        assert_eq!(action.description, "Processes data");
        assert_eq!(action.input_schema.as_deref(), Some(r#"{"type": "object"}"#));
        assert_eq!(action.output_schema.as_deref(), Some(r#"{"type": "string"}"#));
    }

    #[test]
    fn capabilities_parsed_as_capability_enum() {
        let tmp = TempDir::new().unwrap();
        let path = write_manifest(
            tmp.path(),
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[capabilities]
required = ["storage:read", "config:read"]
"#,
        );

        let manifest = parse_manifest(&path).unwrap();

        assert_eq!(manifest.capabilities.required.len(), 2);
        assert!(manifest.capabilities.required.contains(&Capability::StorageRead));
        assert!(manifest.capabilities.required.contains(&Capability::ConfigRead));
    }

    #[test]
    fn unknown_capability_returns_error() {
        let tmp = TempDir::new().unwrap();
        let path = write_manifest(
            tmp.path(),
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[capabilities]
required = ["storage:read", "magic:powers"]
"#,
        );

        let err = parse_manifest(&path).unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid(_)));
        assert!(err.to_string().contains("magic:powers"));
        assert!(err.to_string().contains("Valid capabilities"));
    }

    #[test]
    fn config_schema_preserved_as_json() {
        let tmp = TempDir::new().unwrap();
        let path = write_manifest(
            tmp.path(),
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[config.schema]
type = "object"

[config.schema.properties.interval]
type = "number"
"#,
        );

        let manifest = parse_manifest(&path).unwrap();
        let config = manifest.config.unwrap();

        assert_eq!(config.schema["type"], "object");
        assert!(config.schema["properties"]["interval"].is_object());
    }
}
