//! Plugin manifest parser for `manifest.toml` files.
//!
//! Parses plugin identity, actions, capabilities, collections, events,
//! and config schema from a TOML manifest file.
//!
//! Provides two validation modes:
//! - `parse_manifest` / `parse_manifest_toml` — fail-fast on first error
//! - `validate_manifest` — collect all validation errors for reporting

use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::str::FromStr;

use life_engine_traits::Capability;
use serde::Deserialize;

use crate::error::PluginError;

/// Default action timeout in milliseconds (30 seconds).
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;

/// Reserved collection name prefixes and exact names that plugins cannot use.
const RESERVED_NAMES: &[&str] = &["audit_log"];
const RESERVED_PREFIXES: &[&str] = &["system."];

/// Trust level for a plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustLevel {
    /// First-party plugin: capabilities are auto-granted.
    FirstParty,
    /// Third-party plugin: capabilities require explicit approval.
    ThirdParty,
}

impl TrustLevel {
    fn from_str_value(s: &str) -> Result<Self, String> {
        match s {
            "first_party" => Ok(TrustLevel::FirstParty),
            "third_party" => Ok(TrustLevel::ThirdParty),
            other => Err(format!(
                "invalid trust level '{other}': must be 'first_party' or 'third_party'"
            )),
        }
    }
}

/// Access level for a collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionAccess {
    Read,
    Write,
    ReadWrite,
}

impl CollectionAccess {
    fn from_str_value(s: &str) -> Result<Self, String> {
        match s {
            "read" => Ok(CollectionAccess::Read),
            "write" => Ok(CollectionAccess::Write),
            "read-write" => Ok(CollectionAccess::ReadWrite),
            other => Err(format!(
                "invalid collection access '{other}': must be 'read', 'write', or 'read-write'"
            )),
        }
    }
}

/// Parsed plugin manifest containing identity, actions, capabilities,
/// collections, events, and config schema.
#[derive(Debug, Clone)]
pub struct PluginManifest {
    /// Plugin identity metadata.
    pub plugin: PluginMeta,
    /// Named actions the plugin exposes.
    pub actions: HashMap<String, ActionDef>,
    /// Capabilities the plugin requires.
    pub capabilities: CapabilitySet,
    /// Collections the plugin declares.
    pub collections: HashMap<String, CollectionDef>,
    /// Events the plugin emits and subscribes to.
    pub events: EventsDef,
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
    /// Optional license.
    pub license: Option<String>,
    /// Trust level (defaults to ThirdParty).
    pub trust: TrustLevel,
}

/// Definition of a single plugin action.
#[derive(Debug, Clone)]
pub struct ActionDef {
    /// Human-readable action description.
    pub description: String,
    /// Action timeout in milliseconds.
    pub timeout_ms: u64,
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

impl CapabilitySet {
    fn has(&self, cap: Capability) -> bool {
        self.required.contains(&cap)
    }
}

/// Definition of a declared collection.
#[derive(Debug, Clone)]
pub struct CollectionDef {
    /// Schema reference: `cdm:<name>` or a relative file path.
    pub schema: String,
    /// Access level for this collection.
    pub access: CollectionAccess,
    /// Whether strict mode is enabled (reject unknown fields).
    pub strict: bool,
    /// Index hints for the storage adapter.
    pub indexes: Vec<String>,
    /// Extension fields following `ext.<plugin-id>.<field>` naming.
    pub extensions: Vec<String>,
    /// Optional extension schema path.
    pub extension_schema: Option<String>,
    /// Optional extension index hints.
    pub extension_indexes: Vec<String>,
}

/// Events the plugin emits and subscribes to.
#[derive(Debug, Clone, Default)]
pub struct EventsDef {
    /// Event names this plugin is allowed to emit.
    pub emit: Vec<String>,
    /// Event names this plugin subscribes to.
    pub subscribe: Vec<String>,
}

/// Plugin configuration schema.
#[derive(Debug, Clone)]
pub struct ConfigSchema {
    /// Raw JSON Schema value.
    pub schema: serde_json::Value,
}

// --- Raw TOML deserialization types ---

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    plugin: Option<RawPluginMeta>,
    actions: Option<HashMap<String, RawActionDef>>,
    capabilities: Option<RawCapabilities>,
    collections: Option<HashMap<String, RawCollectionDef>>,
    events: Option<RawEvents>,
    config: Option<RawConfigSchema>,
}

#[derive(Deserialize)]
struct RawPluginMeta {
    id: Option<String>,
    name: Option<String>,
    version: Option<String>,
    description: Option<String>,
    author: Option<String>,
    license: Option<String>,
    trust: Option<String>,
}

#[derive(Deserialize)]
struct RawActionDef {
    description: Option<String>,
    timeout_ms: Option<u64>,
    input_schema: Option<String>,
    output_schema: Option<String>,
}

#[derive(Deserialize)]
struct RawCapabilities {
    required: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct RawCollectionDef {
    schema: Option<String>,
    access: Option<String>,
    strict: Option<bool>,
    indexes: Option<Vec<String>>,
    extensions: Option<Vec<String>>,
    extension_schema: Option<String>,
    extension_indexes: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct RawEvents {
    emit: Option<RawEventList>,
    subscribe: Option<RawEventList>,
}

#[derive(Deserialize)]
struct RawEventList {
    events: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct RawConfigSchema {
    schema: Option<serde_json::Value>,
}

/// Validates that a string is a valid plugin ID: lowercase letters, digits,
/// and hyphens, starting with a letter.
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

/// Checks whether a name is reserved.
fn is_reserved_name(name: &str) -> bool {
    if RESERVED_NAMES.contains(&name) {
        return true;
    }
    for prefix in RESERVED_PREFIXES {
        if name.starts_with(prefix) {
            return true;
        }
    }
    false
}

/// Parses a `manifest.toml` file into a `PluginManifest`.
pub fn parse_manifest(path: &Path) -> Result<PluginManifest, PluginError> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        PluginError::ManifestInvalid(format!("failed to read {}: {e}", path.display()))
    })?;

    parse_manifest_toml(&content, &path.display().to_string())
}

/// Parses TOML content into a `PluginManifest`. Separated for testability.
pub fn parse_manifest_toml(
    content: &str,
    path_str: &str,
) -> Result<PluginManifest, PluginError> {
    let raw: RawManifest = toml::from_str(content).map_err(|e| {
        PluginError::ManifestInvalid(format!("failed to parse {path_str}: {e}"))
    })?;

    // [plugin] section is required
    let raw_plugin = raw.plugin.ok_or_else(|| {
        PluginError::ManifestInvalid(format!("missing [plugin] section in {path_str}"))
    })?;

    // Required fields
    let id = raw_plugin
        .id
        .filter(|s| !s.is_empty())
        .ok_or_else(|| PluginError::ManifestMissingField {
            field: "id".to_string(),
            path: path_str.to_string(),
        })?;

    let name = raw_plugin
        .name
        .filter(|s| !s.is_empty())
        .ok_or_else(|| PluginError::ManifestMissingField {
            field: "name".to_string(),
            path: path_str.to_string(),
        })?;

    let version = raw_plugin
        .version
        .filter(|s| !s.is_empty())
        .ok_or_else(|| PluginError::ManifestMissingField {
            field: "version".to_string(),
            path: path_str.to_string(),
        })?;

    // Validate plugin ID format
    if !is_valid_plugin_id(&id) {
        return Err(PluginError::ManifestInvalid(format!(
            "invalid plugin ID '{id}': must start with a lowercase letter and contain only lowercase letters, digits, and hyphens"
        )));
    }

    // Check reserved names for plugin ID
    if is_reserved_name(&id) {
        return Err(PluginError::ManifestInvalid(format!(
            "reserved name '{id}': plugin IDs cannot use reserved names"
        )));
    }

    // Validate semver
    if !is_valid_semver(&version) {
        return Err(PluginError::ManifestInvalid(format!(
            "invalid version '{version}': must be valid semver (e.g., 1.0.0)"
        )));
    }

    // Parse trust level
    let trust = match raw_plugin.trust {
        Some(ref t) => TrustLevel::from_str_value(t).map_err(PluginError::ManifestInvalid)?,
        None => TrustLevel::ThirdParty,
    };

    let plugin = PluginMeta {
        id,
        name,
        version,
        description: raw_plugin.description,
        author: raw_plugin.author,
        license: raw_plugin.license,
        trust,
    };

    // Parse actions — at least one action is required (Req 2.2)
    let actions = match raw.actions {
        Some(raw_actions) => {
            if raw_actions.is_empty() {
                return Err(PluginError::ManifestInvalid(format!(
                    "no actions declared in manifest for plugin '{}'",
                    plugin.id
                )));
            }
            let mut actions = HashMap::with_capacity(raw_actions.len());
            for (action_name, raw_action) in raw_actions {
                // Action description is required (Req 2.3)
                let description =
                    raw_action
                        .description
                        .filter(|d| !d.is_empty())
                        .ok_or_else(|| {
                            PluginError::ManifestInvalid(format!(
                            "action '{}' missing required 'description' field in manifest for plugin '{}'",
                            action_name, plugin.id
                        ))
                        })?;

                let action = ActionDef {
                    description,
                    timeout_ms: raw_action.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS),
                    input_schema: raw_action.input_schema,
                    output_schema: raw_action.output_schema,
                };
                actions.insert(action_name, action);
            }
            actions
        }
        None => {
            return Err(PluginError::ManifestInvalid(format!(
                "no actions declared in manifest for plugin '{}'",
                plugin.id
            )));
        }
    };

    // Parse capabilities
    let capabilities = match raw.capabilities {
        Some(raw_caps) => {
            let required = match raw_caps.required {
                Some(cap_strings) => {
                    let mut caps = Vec::with_capacity(cap_strings.len());
                    for s in &cap_strings {
                        let cap = Capability::from_str(s).map_err(|_| {
                            PluginError::ManifestInvalid(format!(
                                "unknown capability '{s}' in manifest for plugin '{}'. Valid capabilities: storage:doc:read, storage:doc:write, storage:doc:delete, storage:blob:read, storage:blob:write, storage:blob:delete, http:outbound, events:emit, events:subscribe, config:read",
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

    // Parse collections
    let collections = match raw.collections {
        Some(raw_collections) => {
            let mut collections = HashMap::with_capacity(raw_collections.len());
            for (coll_name, raw_coll) in raw_collections {
                // Check reserved names
                if is_reserved_name(&coll_name) {
                    return Err(PluginError::ManifestInvalid(format!(
                        "reserved name '{coll_name}': collection names cannot use reserved names"
                    )));
                }

                let schema = raw_coll.schema.ok_or_else(|| {
                    PluginError::ManifestInvalid(format!(
                        "collection '{}' missing required 'schema' field in manifest for plugin '{}'",
                        coll_name, plugin.id
                    ))
                })?;

                let access_str = raw_coll.access.ok_or_else(|| {
                    PluginError::ManifestInvalid(format!(
                        "collection '{}' missing required 'access' field in manifest for plugin '{}'",
                        coll_name, plugin.id
                    ))
                })?;

                let access =
                    CollectionAccess::from_str_value(&access_str).map_err(|e| {
                        PluginError::ManifestInvalid(format!(
                            "collection '{}' in plugin '{}': {e}",
                            coll_name, plugin.id
                        ))
                    })?;

                // Validate extension naming convention
                if let Some(ref extensions) = raw_coll.extensions {
                    for ext in extensions {
                        if !ext.starts_with("ext.") {
                            return Err(PluginError::ManifestInvalid(format!(
                                "collection '{}' extension '{}' must follow 'ext.<plugin-id>.<field>' naming convention",
                                coll_name, ext
                            )));
                        }
                    }
                }

                let coll = CollectionDef {
                    schema,
                    access,
                    strict: raw_coll.strict.unwrap_or(false),
                    indexes: raw_coll.indexes.unwrap_or_default(),
                    extensions: raw_coll.extensions.unwrap_or_default(),
                    extension_schema: raw_coll.extension_schema,
                    extension_indexes: raw_coll.extension_indexes.unwrap_or_default(),
                };
                collections.insert(coll_name, coll);
            }
            collections
        }
        None => HashMap::new(),
    };

    // Parse events
    let events = match raw.events {
        Some(raw_events) => {
            let emit = raw_events
                .emit
                .and_then(|e| e.events)
                .unwrap_or_default();
            let subscribe = raw_events
                .subscribe
                .and_then(|s| s.events)
                .unwrap_or_default();
            EventsDef { emit, subscribe }
        }
        None => EventsDef::default(),
    };

    // Parse config schema
    let config = raw
        .config
        .and_then(|c| c.schema.map(|schema| ConfigSchema { schema }));

    // --- Cross-section consistency checks ---

    // Req 5.5: events.emit requires events:emit capability
    if !events.emit.is_empty() && !capabilities.has(Capability::EventsEmit) {
        return Err(PluginError::ManifestInvalid(format!(
            "plugin '{}' declares events to emit but lacks 'events:emit' capability",
            plugin.id
        )));
    }

    // Req 5.6: events.subscribe requires events:subscribe capability
    if !events.subscribe.is_empty() && !capabilities.has(Capability::EventsSubscribe) {
        return Err(PluginError::ManifestInvalid(format!(
            "plugin '{}' declares event subscriptions but lacks 'events:subscribe' capability",
            plugin.id
        )));
    }

    // Req 6.5: config section requires config:read capability
    if config.is_some() && !capabilities.has(Capability::ConfigRead) {
        return Err(PluginError::ManifestInvalid(format!(
            "plugin '{}' declares [config] section but lacks 'config:read' capability",
            plugin.id
        )));
    }

    // Req 5.3: validate event naming convention (<plugin-id>.<action>.<outcome>)
    for event_name in &events.emit {
        let parts: Vec<&str> = event_name.split('.').collect();
        if parts.len() < 3 {
            return Err(PluginError::ManifestInvalid(format!(
                "event name '{}' does not follow '<plugin-id>.<action>.<outcome>' convention",
                event_name
            )));
        }
    }

    Ok(PluginManifest {
        plugin,
        actions,
        capabilities,
        collections,
        events,
        config,
    })
}

/// A single manifest validation error with field path and message.
#[derive(Debug, Clone)]
pub struct ManifestValidationError {
    /// Dot-separated path to the offending field (e.g., "plugin.id", "collections.items.schema").
    pub field: String,
    /// Human-readable error description.
    pub message: String,
}

impl fmt::Display for ManifestValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

/// Validates a parsed `PluginManifest` by collecting all validation errors
/// rather than failing on the first one. This is useful for reporting all
/// problems at once so plugin authors can fix them in a single pass.
///
/// The `plugin_dir` parameter is used to resolve relative schema paths
/// and verify they point to existing files on disk.
pub fn validate_manifest(
    manifest: &PluginManifest,
    plugin_dir: Option<&Path>,
) -> Vec<ManifestValidationError> {
    let mut errors = Vec::new();

    // Validate plugin ID format
    if !is_valid_plugin_id(&manifest.plugin.id) {
        errors.push(ManifestValidationError {
            field: "plugin.id".to_string(),
            message: format!(
                "invalid plugin ID '{}': must start with a lowercase letter and contain only lowercase letters, digits, and hyphens",
                manifest.plugin.id
            ),
        });
    }

    // Check reserved plugin ID
    if is_reserved_name(&manifest.plugin.id) {
        errors.push(ManifestValidationError {
            field: "plugin.id".to_string(),
            message: format!(
                "reserved name '{}': plugin IDs cannot use reserved names",
                manifest.plugin.id
            ),
        });
    }

    // Validate semver
    if !is_valid_semver(&manifest.plugin.version) {
        errors.push(ManifestValidationError {
            field: "plugin.version".to_string(),
            message: format!(
                "invalid version '{}': must be valid semver (e.g., 1.0.0)",
                manifest.plugin.version
            ),
        });
    }

    // Validate actions are not empty
    if manifest.actions.is_empty() {
        errors.push(ManifestValidationError {
            field: "actions".to_string(),
            message: "no actions declared".to_string(),
        });
    }

    // Validate each action has a description
    for (name, action) in &manifest.actions {
        if action.description.is_empty() {
            errors.push(ManifestValidationError {
                field: format!("actions.{name}.description"),
                message: "action description is required".to_string(),
            });
        }
    }

    // Validate collections
    for (name, coll) in &manifest.collections {
        // Check reserved names
        if is_reserved_name(name) {
            errors.push(ManifestValidationError {
                field: format!("collections.{name}"),
                message: format!("reserved name '{name}': collection names cannot use reserved names"),
            });
        }

        // Validate extension naming
        for ext in &coll.extensions {
            if !ext.starts_with("ext.") {
                errors.push(ManifestValidationError {
                    field: format!("collections.{name}.extensions"),
                    message: format!(
                        "extension '{ext}' must follow 'ext.<plugin-id>.<field>' naming convention"
                    ),
                });
            }
        }

        // Validate schema paths resolve to existing files
        if let Some(dir) = plugin_dir {
            if !coll.schema.starts_with("cdm:") {
                let schema_path = dir.join(&coll.schema);
                if !schema_path.exists() {
                    errors.push(ManifestValidationError {
                        field: format!("collections.{name}.schema"),
                        message: format!(
                            "schema path '{}' does not resolve to an existing file",
                            coll.schema
                        ),
                    });
                }
            }

            // Validate extension schema path
            if let Some(ref ext_schema) = coll.extension_schema {
                let ext_schema_path = dir.join(ext_schema);
                if !ext_schema_path.exists() {
                    errors.push(ManifestValidationError {
                        field: format!("collections.{name}.extension_schema"),
                        message: format!(
                            "extension schema path '{ext_schema}' does not resolve to an existing file"
                        ),
                    });
                }
            }
        }
    }

    // Cross-section: events.emit requires events:emit capability
    if !manifest.events.emit.is_empty() && !manifest.capabilities.has(Capability::EventsEmit) {
        errors.push(ManifestValidationError {
            field: "events.emit".to_string(),
            message: "declares events to emit but lacks 'events:emit' capability".to_string(),
        });
    }

    // Cross-section: events.subscribe requires events:subscribe capability
    if !manifest.events.subscribe.is_empty()
        && !manifest.capabilities.has(Capability::EventsSubscribe)
    {
        errors.push(ManifestValidationError {
            field: "events.subscribe".to_string(),
            message: "declares event subscriptions but lacks 'events:subscribe' capability"
                .to_string(),
        });
    }

    // Cross-section: config requires config:read capability
    if manifest.config.is_some() && !manifest.capabilities.has(Capability::ConfigRead) {
        errors.push(ManifestValidationError {
            field: "config".to_string(),
            message: "declares [config] section but lacks 'config:read' capability".to_string(),
        });
    }

    // Validate event naming convention
    for event_name in &manifest.events.emit {
        let parts: Vec<&str> = event_name.split('.').collect();
        if parts.len() < 3 {
            errors.push(ManifestValidationError {
                field: "events.emit".to_string(),
                message: format!(
                    "event name '{event_name}' does not follow '<plugin-id>.<action>.<outcome>' convention"
                ),
            });
        }
    }

    // Validate event names in capabilities match events section
    for event_name in &manifest.events.emit {
        let parts: Vec<&str> = event_name.split('.').collect();
        if parts.len() >= 3 {
            // Verify the action part corresponds to a declared action
            let action_part = parts[1];
            if !manifest.actions.contains_key(action_part) {
                errors.push(ManifestValidationError {
                    field: "events.emit".to_string(),
                    message: format!(
                        "event '{event_name}' references action '{action_part}' which is not declared in [actions]"
                    ),
                });
            }
        }
    }

    // Validate collection names in capabilities match collections section
    // (storage doc capabilities should correspond to declared collections)

    // Validate config schema path if plugin_dir is provided
    if let (Some(config), Some(dir)) = (&manifest.config, plugin_dir) {
        // If the config schema references a file path, verify it exists
        if let Some(path_str) = config.schema.as_str() {
            if !path_str.starts_with("{") && !path_str.starts_with("[") {
                let config_schema_path = dir.join(path_str);
                if !config_schema_path.exists() && !path_str.contains("type") {
                    errors.push(ManifestValidationError {
                        field: "config.schema".to_string(),
                        message: format!(
                            "config schema path '{path_str}' does not resolve to an existing file"
                        ),
                    });
                }
            }
        }
    }

    errors
}

/// Determines the trust level for a plugin based on its directory location.
///
/// A plugin is considered `FirstParty` if its directory is under the
/// `builtin_plugins_path`. All other plugins are `ThirdParty`.
///
/// First-party plugins have their capabilities auto-granted; third-party
/// plugins require explicit approval in Core's configuration.
pub fn determine_trust_level(plugin_dir: &Path, builtin_plugins_path: &Path) -> TrustLevel {
    // Canonicalize both paths for reliable comparison.
    // If canonicalization fails (e.g., path doesn't exist), fall back to
    // starts_with on the raw paths.
    let plugin_canonical = plugin_dir.canonicalize().unwrap_or_else(|_| plugin_dir.to_path_buf());
    let builtin_canonical = builtin_plugins_path
        .canonicalize()
        .unwrap_or_else(|_| builtin_plugins_path.to_path_buf());

    if plugin_canonical.starts_with(&builtin_canonical) {
        TrustLevel::FirstParty
    } else {
        TrustLevel::ThirdParty
    }
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

    fn parse_toml(content: &str) -> Result<PluginManifest, PluginError> {
        parse_manifest_toml(content, "<test>")
    }

    // ========================================================
    // 1. Valid manifest parses correctly
    // ========================================================
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
license = "MIT"
trust = "first_party"

[actions.fetch-emails]
description = "Fetches new emails"
timeout_ms = 60000
input_schema = '{"type": "object"}'
output_schema = '{"type": "array"}'

[actions.send-email]
description = "Sends an email"

[capabilities]
required = ["storage:doc:read", "storage:doc:write", "http:outbound", "events:emit", "config:read"]

[collections.emails]
schema = "cdm:email"
access = "read-write"
strict = true
indexes = ["from", "date"]

[events.emit]
events = ["connector-email.fetch-emails.completed", "connector-email.send-email.completed"]

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
        assert_eq!(manifest.plugin.license.as_deref(), Some("MIT"));
        assert_eq!(manifest.plugin.trust, TrustLevel::FirstParty);
        assert_eq!(manifest.actions.len(), 2);
        assert!(manifest.actions.contains_key("fetch-emails"));
        assert!(manifest.actions.contains_key("send-email"));
        assert_eq!(manifest.actions["fetch-emails"].timeout_ms, 60_000);
        assert_eq!(
            manifest.actions["fetch-emails"].input_schema.as_deref(),
            Some(r#"{"type": "object"}"#)
        );
        assert_eq!(manifest.capabilities.required.len(), 5);
        assert_eq!(manifest.collections.len(), 1);
        assert!(manifest.collections.contains_key("emails"));
        assert_eq!(
            manifest.collections["emails"].access,
            CollectionAccess::ReadWrite
        );
        assert!(manifest.collections["emails"].strict);
        assert_eq!(manifest.events.emit.len(), 2);
        assert!(manifest.config.is_some());
    }

    // ========================================================
    // 2. Invalid plugin ID rejected
    // ========================================================
    #[test]
    fn invalid_plugin_id_uppercase_returns_error() {
        let result = parse_toml(
            r#"
[plugin]
id = "MyPlugin"
name = "Test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"
"#,
        );

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid(_)));
        assert!(err.to_string().contains("invalid plugin ID"));
    }

    #[test]
    fn invalid_plugin_id_spaces_returns_error() {
        let result = parse_toml(
            r#"
[plugin]
id = "my plugin"
name = "Test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"
"#,
        );

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid(_)));
    }

    #[test]
    fn empty_plugin_id_returns_error() {
        let result = parse_toml(
            r#"
[plugin]
id = ""
name = "Test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"
"#,
        );

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ManifestMissingField { .. }));
    }

    // ========================================================
    // 3. Invalid semver rejected
    // ========================================================
    #[test]
    fn invalid_semver_returns_error() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "not-a-version"

[actions.do-thing]
description = "does a thing"
"#,
        );

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid(_)));
        assert!(err.to_string().contains("invalid version"));
    }

    // ========================================================
    // 4. Reserved name rejected
    // ========================================================
    #[test]
    fn reserved_collection_name_audit_log_rejected() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"

[collections.audit_log]
schema = "cdm:audit"
access = "read-write"
"#,
        );

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid(_)));
        assert!(err.to_string().contains("reserved name"));
    }

    #[test]
    fn reserved_collection_name_system_prefix_rejected() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"

[collections."system.internal"]
schema = "cdm:internal"
access = "read"
"#,
        );

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid(_)));
        assert!(err.to_string().contains("reserved name"));
    }

    // ========================================================
    // 5. Missing required sections error
    // ========================================================
    #[test]
    fn missing_plugin_section_returns_error() {
        let result = parse_toml(
            r#"
[actions.something]
description = "No plugin section"
"#,
        );

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid(_)));
        assert!(err.to_string().contains("[plugin]"));
    }

    #[test]
    fn missing_required_field_id_returns_error() {
        let result = parse_toml(
            r#"
[plugin]
name = "No ID"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"
"#,
        );

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ManifestMissingField { .. }));
        assert!(err.to_string().contains("id"));
    }

    #[test]
    fn missing_required_field_name_returns_error() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"
"#,
        );

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ManifestMissingField { .. }));
        assert!(err.to_string().contains("name"));
    }

    #[test]
    fn missing_required_field_version_returns_error() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"

[actions.do-thing]
description = "does a thing"
"#,
        );

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ManifestMissingField { .. }));
        assert!(err.to_string().contains("version"));
    }

    #[test]
    fn missing_actions_section_returns_error() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"
"#,
        );

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid(_)));
        assert!(err.to_string().contains("no actions declared"));
    }

    #[test]
    fn action_missing_description_returns_error() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[actions.do-thing]
timeout_ms = 5000
"#,
        );

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid(_)));
        assert!(err.to_string().contains("description"));
    }

    #[test]
    fn collection_missing_schema_returns_error() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"

[collections.items]
access = "read"
"#,
        );

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid(_)));
        assert!(err.to_string().contains("schema"));
    }

    #[test]
    fn collection_missing_access_returns_error() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"

[collections.items]
schema = "cdm:item"
"#,
        );

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid(_)));
        assert!(err.to_string().contains("access"));
    }

    // ========================================================
    // 6. Trust model distinction
    // ========================================================
    #[test]
    fn trust_first_party_parsed() {
        let result = parse_toml(
            r#"
[plugin]
id = "core-plugin"
name = "Core Plugin"
version = "1.0.0"
trust = "first_party"

[actions.do-thing]
description = "does a thing"
"#,
        );

        let manifest = result.unwrap();
        assert_eq!(manifest.plugin.trust, TrustLevel::FirstParty);
    }

    #[test]
    fn trust_third_party_parsed() {
        let result = parse_toml(
            r#"
[plugin]
id = "external-plugin"
name = "External Plugin"
version = "1.0.0"
trust = "third_party"

[actions.do-thing]
description = "does a thing"
"#,
        );

        let manifest = result.unwrap();
        assert_eq!(manifest.plugin.trust, TrustLevel::ThirdParty);
    }

    #[test]
    fn trust_defaults_to_third_party() {
        let result = parse_toml(
            r#"
[plugin]
id = "no-trust-field"
name = "No Trust"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"
"#,
        );

        let manifest = result.unwrap();
        assert_eq!(manifest.plugin.trust, TrustLevel::ThirdParty);
    }

    #[test]
    fn invalid_trust_level_returns_error() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"
trust = "super_trusted"

[actions.do-thing]
description = "does a thing"
"#,
        );

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid(_)));
        assert!(err.to_string().contains("invalid trust level"));
    }

    // ========================================================
    // 7. Action timeout defaults applied
    // ========================================================
    #[test]
    fn action_timeout_defaults_to_30s() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"
"#,
        );

        let manifest = result.unwrap();
        assert_eq!(manifest.actions["do-thing"].timeout_ms, 30_000);
    }

    #[test]
    fn action_timeout_custom_value() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[actions.slow-thing]
description = "does a slow thing"
timeout_ms = 120000
"#,
        );

        let manifest = result.unwrap();
        assert_eq!(manifest.actions["slow-thing"].timeout_ms, 120_000);
    }

    // ========================================================
    // 8. Collection schema reference validation
    // ========================================================
    #[test]
    fn collection_with_cdm_schema_accepted() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"

[collections.items]
schema = "cdm:item"
access = "read"
"#,
        );

        let manifest = result.unwrap();
        assert_eq!(manifest.collections["items"].schema, "cdm:item");
        assert_eq!(
            manifest.collections["items"].access,
            CollectionAccess::Read
        );
    }

    #[test]
    fn collection_with_relative_schema_accepted() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"

[collections.custom]
schema = "schemas/custom.json"
access = "write"
"#,
        );

        let manifest = result.unwrap();
        assert_eq!(
            manifest.collections["custom"].schema,
            "schemas/custom.json"
        );
        assert_eq!(
            manifest.collections["custom"].access,
            CollectionAccess::Write
        );
    }

    #[test]
    fn collection_access_levels_parsed() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"

[collections.readonly]
schema = "cdm:item"
access = "read"

[collections.writeonly]
schema = "cdm:item"
access = "write"

[collections.readwrite]
schema = "cdm:item"
access = "read-write"
"#,
        );

        let manifest = result.unwrap();
        assert_eq!(
            manifest.collections["readonly"].access,
            CollectionAccess::Read
        );
        assert_eq!(
            manifest.collections["writeonly"].access,
            CollectionAccess::Write
        );
        assert_eq!(
            manifest.collections["readwrite"].access,
            CollectionAccess::ReadWrite
        );
    }

    #[test]
    fn invalid_collection_access_returns_error() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"

[collections.items]
schema = "cdm:item"
access = "admin"
"#,
        );

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid(_)));
        assert!(err.to_string().contains("invalid collection access"));
    }

    // ========================================================
    // Events parsing and validation
    // ========================================================
    #[test]
    fn events_emit_parsed() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"

[capabilities]
required = ["events:emit"]

[events.emit]
events = ["test.do-thing.completed", "test.do-thing.failed"]
"#,
        );

        let manifest = result.unwrap();
        assert_eq!(manifest.events.emit.len(), 2);
    }

    #[test]
    fn events_subscribe_parsed() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"

[capabilities]
required = ["events:subscribe"]

[events.subscribe]
events = ["other-plugin.action.completed"]
"#,
        );

        let manifest = result.unwrap();
        assert_eq!(manifest.events.subscribe.len(), 1);
    }

    #[test]
    fn events_emit_without_capability_returns_error() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"

[events.emit]
events = ["test.do-thing.completed"]
"#,
        );

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid(_)));
        assert!(err.to_string().contains("events:emit"));
    }

    #[test]
    fn events_subscribe_without_capability_returns_error() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"

[events.subscribe]
events = ["other.action.completed"]
"#,
        );

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid(_)));
        assert!(err.to_string().contains("events:subscribe"));
    }

    #[test]
    fn event_name_bad_convention_returns_error() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"

[capabilities]
required = ["events:emit"]

[events.emit]
events = ["bad-event-name"]
"#,
        );

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid(_)));
        assert!(err.to_string().contains("convention"));
    }

    // ========================================================
    // Config / capability consistency
    // ========================================================
    #[test]
    fn config_without_capability_returns_error() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"

[config.schema]
type = "object"
"#,
        );

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid(_)));
        assert!(err.to_string().contains("config:read"));
    }

    #[test]
    fn config_with_capability_accepted() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"

[capabilities]
required = ["config:read"]

[config.schema]
type = "object"
"#,
        );

        let manifest = result.unwrap();
        assert!(manifest.config.is_some());
    }

    // ========================================================
    // Unknown sections rejected (deny_unknown_fields)
    // ========================================================
    #[test]
    fn unknown_top_level_section_returns_error() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"

[wizardry]
spell = "fireball"
"#,
        );

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid(_)));
    }

    // ========================================================
    // Capabilities parsed as Capability enum
    // ========================================================
    #[test]
    fn capabilities_parsed_as_capability_enum() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"

[capabilities]
required = ["storage:doc:read", "config:read"]
"#,
        );

        let manifest = result.unwrap();
        assert_eq!(manifest.capabilities.required.len(), 2);
        assert!(manifest
            .capabilities
            .required
            .contains(&Capability::StorageRead));
        assert!(manifest
            .capabilities
            .required
            .contains(&Capability::ConfigRead));
    }

    #[test]
    fn unknown_capability_returns_error() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"

[capabilities]
required = ["storage:doc:read", "magic:powers"]
"#,
        );

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid(_)));
        assert!(err.to_string().contains("magic:powers"));
        assert!(err.to_string().contains("Valid capabilities"));
    }

    // ========================================================
    // Config schema preserved as JSON
    // ========================================================
    #[test]
    fn config_schema_preserved_as_json() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"

[capabilities]
required = ["config:read"]

[config.schema]
type = "object"

[config.schema.properties.interval]
type = "number"
"#,
        );

        let manifest = result.unwrap();
        let config = manifest.config.unwrap();
        assert_eq!(config.schema["type"], "object");
        assert!(config.schema["properties"]["interval"].is_object());
    }

    // ========================================================
    // Actions extracted with schemas and timeouts
    // ========================================================
    #[test]
    fn actions_extracted_with_schemas() {
        let result = parse_toml(
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

        let manifest = result.unwrap();
        let action = &manifest.actions["process"];

        assert_eq!(action.description, "Processes data");
        assert_eq!(
            action.input_schema.as_deref(),
            Some(r#"{"type": "object"}"#)
        );
        assert_eq!(
            action.output_schema.as_deref(),
            Some(r#"{"type": "string"}"#)
        );
        assert_eq!(action.timeout_ms, 30_000);
    }

    // ========================================================
    // Extension naming validation
    // ========================================================
    #[test]
    fn collection_extension_bad_naming_returns_error() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"

[collections.items]
schema = "cdm:item"
access = "read-write"
extensions = ["bad_field_name"]
"#,
        );

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid(_)));
        assert!(err.to_string().contains("ext."));
    }

    #[test]
    fn collection_extension_valid_naming_accepted() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"

[collections.items]
schema = "cdm:item"
access = "read-write"
extensions = ["ext.test.priority", "ext.test.tags"]
"#,
        );

        assert!(result.is_ok());
    }

    // ========================================================
    // Collection strict and indexes
    // ========================================================
    #[test]
    fn collection_strict_and_indexes_parsed() {
        let result = parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"

[collections.items]
schema = "cdm:item"
access = "read"
strict = true
indexes = ["name", "created_at"]
"#,
        );

        let manifest = result.unwrap();
        let coll = &manifest.collections["items"];
        assert!(coll.strict);
        assert_eq!(coll.indexes, vec!["name", "created_at"]);
    }

    // ========================================================
    // TOML syntax errors
    // ========================================================
    #[test]
    fn invalid_toml_syntax_returns_parse_error() {
        let result = parse_toml("this is not valid toml {{{{");
        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid(_)));
        assert!(err.to_string().contains("failed to parse"));
    }

    // ========================================================
    // File-based test via parse_manifest
    // ========================================================
    #[test]
    fn parse_manifest_from_file() {
        let tmp = TempDir::new().unwrap();
        let path = write_manifest(
            tmp.path(),
            r#"
[plugin]
id = "file-test"
name = "File Test"
version = "0.1.0"

[actions.run]
description = "Runs"
"#,
        );

        let manifest = parse_manifest(&path).unwrap();
        assert_eq!(manifest.plugin.id, "file-test");
    }

    #[test]
    fn parse_manifest_missing_file_returns_error() {
        let path = Path::new("/tmp/nonexistent-dir-12345/manifest.toml");
        let err = parse_manifest(path).unwrap_err();
        assert!(matches!(err, PluginError::ManifestInvalid(_)));
        assert!(err.to_string().contains("failed to read"));
    }

    // ========================================================
    // validate_manifest: multi-error collection
    // ========================================================

    fn make_valid_manifest() -> PluginManifest {
        parse_toml(
            r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"

[actions.do-thing]
description = "does a thing"
"#,
        )
        .unwrap()
    }

    #[test]
    fn validate_manifest_valid_returns_no_errors() {
        let manifest = make_valid_manifest();
        let errors = validate_manifest(&manifest, None);
        assert!(errors.is_empty(), "expected no errors, got: {:?}", errors);
    }

    #[test]
    fn validate_manifest_collects_multiple_errors() {
        // Build a manifest with multiple problems manually
        let manifest = PluginManifest {
            plugin: PluginMeta {
                id: "INVALID_ID".to_string(),
                name: "Test".to_string(),
                version: "not-semver".to_string(),
                description: None,
                author: None,
                license: None,
                trust: TrustLevel::ThirdParty,
            },
            actions: HashMap::new(), // no actions
            capabilities: CapabilitySet::default(),
            collections: {
                let mut map = HashMap::new();
                map.insert(
                    "audit_log".to_string(),
                    CollectionDef {
                        schema: "cdm:audit".to_string(),
                        access: CollectionAccess::ReadWrite,
                        strict: false,
                        indexes: vec![],
                        extensions: vec!["no_ext_prefix".to_string()],
                        extension_schema: None,
                        extension_indexes: vec![],
                    },
                );
                map
            },
            events: EventsDef {
                emit: vec!["bad-name".to_string()],
                subscribe: vec![],
            },
            config: None,
        };

        let errors = validate_manifest(&manifest, None);

        // Should have at least: invalid ID, invalid version, no actions,
        // reserved collection name, bad extension naming, missing capability,
        // bad event naming
        assert!(
            errors.len() >= 5,
            "expected at least 5 errors, got {}: {:?}",
            errors.len(),
            errors
        );

        let error_text: String = errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n");
        assert!(error_text.contains("plugin.id"), "should flag invalid ID");
        assert!(error_text.contains("plugin.version"), "should flag invalid version");
        assert!(error_text.contains("actions"), "should flag no actions");
        assert!(error_text.contains("reserved name"), "should flag reserved collection");
        assert!(error_text.contains("ext."), "should flag bad extension naming");
    }

    #[test]
    fn validate_manifest_checks_schema_path_exists() {
        let tmp = TempDir::new().unwrap();
        let manifest = PluginManifest {
            plugin: PluginMeta {
                id: "test".to_string(),
                name: "Test".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                license: None,
                trust: TrustLevel::ThirdParty,
            },
            actions: {
                let mut map = HashMap::new();
                map.insert(
                    "run".to_string(),
                    ActionDef {
                        description: "runs".to_string(),
                        timeout_ms: DEFAULT_TIMEOUT_MS,
                        input_schema: None,
                        output_schema: None,
                    },
                );
                map
            },
            capabilities: CapabilitySet::default(),
            collections: {
                let mut map = HashMap::new();
                map.insert(
                    "items".to_string(),
                    CollectionDef {
                        schema: "schemas/nonexistent.json".to_string(),
                        access: CollectionAccess::Read,
                        strict: false,
                        indexes: vec![],
                        extensions: vec![],
                        extension_schema: None,
                        extension_indexes: vec![],
                    },
                );
                map
            },
            events: EventsDef::default(),
            config: None,
        };

        let errors = validate_manifest(&manifest, Some(tmp.path()));
        assert!(
            errors.iter().any(|e| e.message.contains("does not resolve")),
            "expected schema path error, got: {:?}",
            errors
        );
    }

    #[test]
    fn validate_manifest_accepts_existing_schema_path() {
        let tmp = TempDir::new().unwrap();
        let schema_dir = tmp.path().join("schemas");
        fs::create_dir_all(&schema_dir).unwrap();
        fs::write(schema_dir.join("item.json"), r#"{"type":"object"}"#).unwrap();

        let manifest = PluginManifest {
            plugin: PluginMeta {
                id: "test".to_string(),
                name: "Test".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                license: None,
                trust: TrustLevel::ThirdParty,
            },
            actions: {
                let mut map = HashMap::new();
                map.insert(
                    "run".to_string(),
                    ActionDef {
                        description: "runs".to_string(),
                        timeout_ms: DEFAULT_TIMEOUT_MS,
                        input_schema: None,
                        output_schema: None,
                    },
                );
                map
            },
            capabilities: CapabilitySet::default(),
            collections: {
                let mut map = HashMap::new();
                map.insert(
                    "items".to_string(),
                    CollectionDef {
                        schema: "schemas/item.json".to_string(),
                        access: CollectionAccess::Read,
                        strict: false,
                        indexes: vec![],
                        extensions: vec![],
                        extension_schema: None,
                        extension_indexes: vec![],
                    },
                );
                map
            },
            events: EventsDef::default(),
            config: None,
        };

        let errors = validate_manifest(&manifest, Some(tmp.path()));
        assert!(
            errors.is_empty(),
            "expected no errors for valid schema path, got: {:?}",
            errors
        );
    }

    #[test]
    fn validate_manifest_cdm_schema_skips_path_check() {
        let manifest = make_valid_manifest();
        // CDM schemas are resolved by the SDK, not by file path
        let errors = validate_manifest(&manifest, Some(Path::new("/nonexistent")));
        assert!(errors.is_empty());
    }

    // ========================================================
    // determine_trust_level: directory-based trust
    // ========================================================

    #[test]
    fn trust_level_first_party_when_under_builtin_path() {
        let tmp = TempDir::new().unwrap();
        let builtin = tmp.path().join("plugins");
        let plugin_dir = builtin.join("my-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();

        let trust = determine_trust_level(&plugin_dir, &builtin);
        assert_eq!(trust, TrustLevel::FirstParty);
    }

    #[test]
    fn trust_level_third_party_when_outside_builtin_path() {
        let tmp = TempDir::new().unwrap();
        let builtin = tmp.path().join("builtin-plugins");
        let external = tmp.path().join("external-plugins").join("my-plugin");
        fs::create_dir_all(&builtin).unwrap();
        fs::create_dir_all(&external).unwrap();

        let trust = determine_trust_level(&external, &builtin);
        assert_eq!(trust, TrustLevel::ThirdParty);
    }

    #[test]
    fn trust_level_builtin_path_itself_is_first_party() {
        let tmp = TempDir::new().unwrap();
        let builtin = tmp.path().join("plugins");
        fs::create_dir_all(&builtin).unwrap();

        let trust = determine_trust_level(&builtin, &builtin);
        assert_eq!(trust, TrustLevel::FirstParty);
    }

    // ========================================================
    // validate_manifest: event name / action consistency
    // ========================================================

    #[test]
    fn validate_manifest_flags_event_referencing_undeclared_action() {
        let manifest = PluginManifest {
            plugin: PluginMeta {
                id: "test".to_string(),
                name: "Test".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                license: None,
                trust: TrustLevel::ThirdParty,
            },
            actions: {
                let mut map = HashMap::new();
                map.insert(
                    "do-thing".to_string(),
                    ActionDef {
                        description: "does a thing".to_string(),
                        timeout_ms: DEFAULT_TIMEOUT_MS,
                        input_schema: None,
                        output_schema: None,
                    },
                );
                map
            },
            capabilities: CapabilitySet {
                required: vec![Capability::EventsEmit],
            },
            collections: HashMap::new(),
            events: EventsDef {
                emit: vec!["test.nonexistent-action.completed".to_string()],
                subscribe: vec![],
            },
            config: None,
        };

        let errors = validate_manifest(&manifest, None);
        assert!(
            errors.iter().any(|e| e.message.contains("nonexistent-action")),
            "expected undeclared action error, got: {:?}",
            errors
        );
    }
}
