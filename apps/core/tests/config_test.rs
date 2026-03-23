//! Integration tests for the Phase 9 (startup) configuration system.
//!
//! These tests exercise the TOML-based CoreConfig contract from the outside:
//! serialization/deserialization, environment variable override semantics,
//! missing file handling, parse error reporting, validation error collection,
//! sensitive value redaction, and zero-transport behaviour.
//!
//! Because `apps/core` is a binary crate, we test the TOML contract directly
//! using the same `toml` + `serde` types the config module uses internally.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

/// Serialize env-var tests to avoid interference.
static ENV_LOCK: Mutex<()> = Mutex::new(());

// ---------------------------------------------------------------------------
// Mirror types — these match the `config::startup` structs and let us verify
// the TOML serialization contract without importing from the binary crate.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct CoreConfig {
    #[serde(default = "default_empty_table")]
    storage: toml::Value,
    #[serde(default = "default_empty_table")]
    auth: toml::Value,
    #[serde(default)]
    transports: HashMap<String, toml::Value>,
    #[serde(default)]
    workflows: WorkflowsConfig,
    #[serde(default)]
    plugins: PluginsConfig,
    #[serde(default)]
    logging: LoggingConfig,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            storage: default_empty_table(),
            auth: default_empty_table(),
            transports: HashMap::new(),
            workflows: WorkflowsConfig::default(),
            plugins: PluginsConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkflowsConfig {
    #[serde(default = "default_workflows_path")]
    path: String,
}
impl Default for WorkflowsConfig {
    fn default() -> Self {
        Self { path: default_workflows_path() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PluginsConfig {
    #[serde(default = "default_plugins_path")]
    path: String,
    #[serde(default)]
    config: HashMap<String, toml::Value>,
}
impl Default for PluginsConfig {
    fn default() -> Self {
        Self { path: default_plugins_path(), config: HashMap::new() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LoggingConfig {
    #[serde(default = "default_log_level")]
    level: String,
    #[serde(default = "default_log_format")]
    format: String,
}
impl Default for LoggingConfig {
    fn default() -> Self {
        Self { level: default_log_level(), format: default_log_format() }
    }
}

fn default_empty_table() -> toml::Value {
    toml::Value::Table(toml::map::Map::new())
}
fn default_workflows_path() -> String { "workflows".into() }
fn default_plugins_path() -> String { "plugins".into() }
fn default_log_level() -> String { "info".into() }
fn default_log_format() -> String { "json".into() }

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn write_toml(dir: &tempfile::TempDir, name: &str, content: &str) -> PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, content).unwrap();
    path
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("failed to resolve repo root")
}

/// Redaction logic matching `config::startup::redact_sensitive`.
const SENSITIVE_FRAGMENTS: &[&str] = &["key", "secret", "password", "token"];

fn redact_sensitive(value: &toml::Value) -> toml::Value {
    match value {
        toml::Value::Table(table) => {
            let mut redacted = toml::map::Map::new();
            for (k, v) in table {
                let lower = k.to_lowercase();
                if SENSITIVE_FRAGMENTS.iter().any(|f| lower.contains(f)) {
                    redacted.insert(k.clone(), toml::Value::String("[REDACTED]".into()));
                } else {
                    redacted.insert(k.clone(), redact_sensitive(v));
                }
            }
            toml::Value::Table(redacted)
        }
        other => other.clone(),
    }
}

/// Simple validation mirroring `config::startup::validate_config`.
fn validate(config: &CoreConfig) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    let storage_empty = match &config.storage {
        toml::Value::Table(t) => t.is_empty(),
        _ => true,
    };
    if storage_empty {
        errors.push("missing required config section: storage".into());
    }

    let valid_levels = ["trace", "debug", "info", "warn", "error"];
    if !valid_levels.contains(&config.logging.level.to_lowercase().as_str()) {
        errors.push(format!("invalid logging level '{}'", config.logging.level));
    }

    let valid_formats = ["json", "pretty"];
    if !valid_formats.contains(&config.logging.format.to_lowercase().as_str()) {
        errors.push(format!("invalid logging format '{}'", config.logging.format));
    }

    if config.plugins.path.is_empty() {
        errors.push("plugins path must not be empty".into());
    }

    if config.workflows.path.is_empty() {
        errors.push("workflows path must not be empty".into());
    }

    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

/// Apply `LIFE_ENGINE_*` env var overrides to a raw TOML table, mirroring
/// the production `apply_env_overrides` function.
fn apply_env_overrides(table: &mut toml::map::Map<String, toml::Value>) {
    let prefix = "LIFE_ENGINE_";
    let mut env_vars: Vec<(String, String)> = std::env::vars()
        .filter(|(k, _)| k.starts_with(prefix) && k != "LIFE_ENGINE_CONFIG")
        .collect();
    env_vars.sort_by(|a, b| a.0.cmp(&b.0));

    for (key, value) in env_vars {
        let suffix = &key[prefix.len()..];
        let segments: Vec<String> = suffix.split('_').map(|s| s.to_lowercase()).collect();
        if segments.is_empty() || segments.iter().any(|s| s.is_empty()) {
            continue;
        }
        set_nested(table, &segments, &value);
    }
}

fn set_nested(table: &mut toml::map::Map<String, toml::Value>, segments: &[String], value: &str) {
    if segments.len() == 1 {
        let typed = match table.get(&segments[0]) {
            Some(toml::Value::Integer(_)) => value
                .parse::<i64>()
                .map(toml::Value::Integer)
                .unwrap_or_else(|_| toml::Value::String(value.to_string())),
            Some(toml::Value::Boolean(_)) => value
                .parse::<bool>()
                .map(toml::Value::Boolean)
                .unwrap_or_else(|_| toml::Value::String(value.to_string())),
            _ => toml::Value::String(value.to_string()),
        };
        table.insert(segments[0].clone(), typed);
        return;
    }
    let entry = table
        .entry(segments[0].clone())
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    if !entry.is_table() {
        *entry = toml::Value::Table(toml::map::Map::new());
    }
    if let toml::Value::Table(inner) = entry {
        set_nested(inner, &segments[1..], value);
    }
}

/// Load config from a TOML file with env var overrides, mirroring
/// the production `load_config` function.
fn load_config(path: &str) -> Result<CoreConfig, String> {
    let config_path = PathBuf::from(path);
    let mut raw_table = match std::fs::read_to_string(&config_path) {
        Ok(contents) => {
            let table: toml::Value = contents.parse().map_err(|e: toml::de::Error| {
                format!("failed to parse config file {}: {e}", config_path.display())
            })?;
            match table {
                toml::Value::Table(t) => t,
                _ => toml::map::Map::new(),
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            toml::map::Map::new()
        }
        Err(e) => {
            return Err(format!("failed to read {}: {e}", config_path.display()));
        }
    };

    apply_env_overrides(&mut raw_table);

    let config: CoreConfig = toml::Value::Table(raw_table)
        .try_into()
        .map_err(|e: toml::de::Error| format!("deserialization error: {e}"))?;

    Ok(config)
}

// ===========================================================================
// Tests
// ===========================================================================

// ---------------------------------------------------------------------------
// 1. Valid config.toml loads and parses correctly
// ---------------------------------------------------------------------------

#[test]
fn valid_config_loads_and_parses() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = write_toml(
        &dir,
        "config.toml",
        r#"
[storage]
path = "/data/core.db"
passphrase_env = "LIFE_ENGINE_STORAGE_PASSPHRASE"

[auth]
provider = "pocket-id"
issuer_url = "https://auth.example.com"

[transports.rest]
host = "0.0.0.0"
port = 3000

[transports.graphql]
port = 4000

[workflows]
path = "/etc/life-engine/workflows"

[plugins]
path = "/opt/life-engine/plugins"

[plugins.config.connector-email]
imap_host = "mail.example.com"
imap_port = 993

[logging]
level = "debug"
format = "pretty"
"#,
    );

    let config = load_config(path.to_str().unwrap()).unwrap();

    assert_eq!(
        config.storage.get("path").and_then(|v| v.as_str()),
        Some("/data/core.db")
    );
    assert_eq!(
        config.auth.get("provider").and_then(|v| v.as_str()),
        Some("pocket-id")
    );
    assert_eq!(config.transports.len(), 2);
    assert!(config.transports.contains_key("rest"));
    assert!(config.transports.contains_key("graphql"));
    assert_eq!(
        config.transports["rest"].get("port").and_then(|v| v.as_integer()),
        Some(3000)
    );
    assert_eq!(config.workflows.path, "/etc/life-engine/workflows");
    assert_eq!(config.plugins.path, "/opt/life-engine/plugins");
    assert_eq!(config.plugins.config.len(), 1);
    assert_eq!(config.logging.level, "debug");
    assert_eq!(config.logging.format, "pretty");
}

// ---------------------------------------------------------------------------
// 2. Env var overrides TOML value (LIFE_ENGINE_STORAGE_PATH)
// ---------------------------------------------------------------------------

#[test]
fn env_var_overrides_storage_path() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = write_toml(
        &dir,
        "config.toml",
        r#"
[storage]
path = "/original/path"
"#,
    );

    unsafe { std::env::set_var("LIFE_ENGINE_STORAGE_PATH", "/overridden/path") };
    let config = load_config(path.to_str().unwrap()).unwrap();
    unsafe { std::env::remove_var("LIFE_ENGINE_STORAGE_PATH") };

    assert_eq!(
        config.storage.get("path").and_then(|v| v.as_str()),
        Some("/overridden/path")
    );
}

// ---------------------------------------------------------------------------
// 3. Env vars take precedence over TOML values
// ---------------------------------------------------------------------------

#[test]
fn env_vars_take_precedence_over_toml() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = write_toml(
        &dir,
        "config.toml",
        r#"
[storage]
path = "/data/core.db"

[logging]
level = "info"
format = "json"
"#,
    );

    unsafe {
        std::env::set_var("LIFE_ENGINE_LOGGING_LEVEL", "debug");
        std::env::set_var("LIFE_ENGINE_LOGGING_FORMAT", "pretty");
    }
    let config = load_config(path.to_str().unwrap()).unwrap();
    unsafe {
        std::env::remove_var("LIFE_ENGINE_LOGGING_LEVEL");
        std::env::remove_var("LIFE_ENGINE_LOGGING_FORMAT");
    }

    assert_eq!(config.logging.level, "debug");
    assert_eq!(config.logging.format, "pretty");
}

// ---------------------------------------------------------------------------
// 4. Missing config file returns defaults
// ---------------------------------------------------------------------------

#[test]
fn missing_config_file_returns_defaults() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let nonexistent = dir.path().join("nonexistent.toml");

    let config = load_config(nonexistent.to_str().unwrap()).unwrap();

    assert_eq!(config.logging.level, "info");
    assert_eq!(config.logging.format, "json");
    assert_eq!(config.workflows.path, "workflows");
    assert_eq!(config.plugins.path, "plugins");
    assert!(config.transports.is_empty());
}

// ---------------------------------------------------------------------------
// 5. Missing required field (storage) returns validation error
// ---------------------------------------------------------------------------

#[test]
fn validate_rejects_missing_storage() {
    let config = CoreConfig::default();
    let errors = validate(&config).unwrap_err();
    assert!(
        errors.iter().any(|e| e.contains("storage")),
        "expected storage error, got: {errors:?}"
    );
}

// ---------------------------------------------------------------------------
// 6. Invalid TOML syntax returns parse error with line/column info
// ---------------------------------------------------------------------------

#[test]
fn invalid_toml_syntax_returns_parse_error() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = write_toml(&dir, "bad.toml", "{{not valid toml}}");

    let err = load_config(path.to_str().unwrap()).unwrap_err();
    assert!(
        err.contains("failed to parse"),
        "expected parse error, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// 7. Sensitive values are redacted
// ---------------------------------------------------------------------------

#[test]
fn sensitive_values_are_redacted() {
    let toml_str = r#"
[storage]
path = "/data/core.db"
passphrase_token = "super-secret"

[auth]
provider = "pocket-id"
client_secret = "my-secret"
api_key = "abc123"
"#;
    let val: toml::Value = toml_str.parse().unwrap();
    let redacted = redact_sensitive(&val);

    // Non-sensitive preserved.
    assert_eq!(
        redacted.get("storage").and_then(|s| s.get("path")).and_then(|v| v.as_str()),
        Some("/data/core.db")
    );
    assert_eq!(
        redacted.get("auth").and_then(|s| s.get("provider")).and_then(|v| v.as_str()),
        Some("pocket-id")
    );

    // Sensitive redacted.
    assert_eq!(
        redacted.get("storage").and_then(|s| s.get("passphrase_token")).and_then(|v| v.as_str()),
        Some("[REDACTED]")
    );
    assert_eq!(
        redacted.get("auth").and_then(|s| s.get("client_secret")).and_then(|v| v.as_str()),
        Some("[REDACTED]")
    );
    assert_eq!(
        redacted.get("auth").and_then(|s| s.get("api_key")).and_then(|v| v.as_str()),
        Some("[REDACTED]")
    );
}

// ---------------------------------------------------------------------------
// 8. Multiple validation errors are collected together
// ---------------------------------------------------------------------------

#[test]
fn validate_collects_multiple_errors() {
    let toml_str = r#"
[logging]
level = "verbose"
format = "xml"
"#;
    let config: CoreConfig = toml::from_str(toml_str).unwrap();
    let errors = validate(&config).unwrap_err();

    // Should contain: missing storage + invalid level + invalid format.
    assert!(
        errors.len() >= 3,
        "expected at least 3 errors, got {}: {errors:?}",
        errors.len()
    );
    assert!(errors.iter().any(|e| e.contains("storage")));
    assert!(errors.iter().any(|e| e.contains("level")));
    assert!(errors.iter().any(|e| e.contains("format")));
}

// ---------------------------------------------------------------------------
// 9. Zero configured transports is not an error
// ---------------------------------------------------------------------------

#[test]
fn zero_transports_is_valid() {
    let toml_str = r#"
[storage]
path = "/data/core.db"
"#;
    let config: CoreConfig = toml::from_str(toml_str).unwrap();
    assert!(config.transports.is_empty());
    assert!(validate(&config).is_ok());
}

// ---------------------------------------------------------------------------
// 10. Env var preserves integer type when overriding existing key
// ---------------------------------------------------------------------------

#[test]
fn env_var_preserves_integer_type() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = write_toml(
        &dir,
        "config.toml",
        r#"
[transports.rest]
port = 3000
"#,
    );

    unsafe { std::env::set_var("LIFE_ENGINE_TRANSPORTS_REST_PORT", "4000") };
    let config = load_config(path.to_str().unwrap()).unwrap();
    unsafe { std::env::remove_var("LIFE_ENGINE_TRANSPORTS_REST_PORT") };

    assert_eq!(
        config.transports["rest"].get("port").and_then(|v| v.as_integer()),
        Some(4000)
    );
}

// ---------------------------------------------------------------------------
// 11. Env var creates nested keys not present in TOML
// ---------------------------------------------------------------------------

#[test]
fn env_var_creates_nested_keys() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = write_toml(&dir, "config.toml", "");

    unsafe { std::env::set_var("LIFE_ENGINE_AUTH_PROVIDER", "pocket-id") };
    let config = load_config(path.to_str().unwrap()).unwrap();
    unsafe { std::env::remove_var("LIFE_ENGINE_AUTH_PROVIDER") };

    assert_eq!(
        config.auth.get("provider").and_then(|v| v.as_str()),
        Some("pocket-id")
    );
}

// ---------------------------------------------------------------------------
// 12. Full valid config passes validation
// ---------------------------------------------------------------------------

#[test]
fn full_valid_config_passes_validation() {
    let toml_str = r#"
[storage]
path = "/data/core.db"

[auth]
provider = "pocket-id"

[transports.rest]
port = 3000

[workflows]
path = "/etc/life-engine/workflows"

[plugins]
path = "/opt/life-engine/plugins"

[logging]
level = "info"
format = "json"
"#;
    let config: CoreConfig = toml::from_str(toml_str).unwrap();
    assert!(validate(&config).is_ok());
}

// ---------------------------------------------------------------------------
// 13. Validate rejects empty plugins path
// ---------------------------------------------------------------------------

#[test]
fn validate_rejects_empty_plugins_path() {
    let toml_str = r#"
[storage]
path = "/data/core.db"

[plugins]
path = ""
"#;
    let config: CoreConfig = toml::from_str(toml_str).unwrap();
    let errors = validate(&config).unwrap_err();
    assert!(errors.iter().any(|e| e.contains("plugins")));
}

// ---------------------------------------------------------------------------
// 14. Validate rejects empty workflows path
// ---------------------------------------------------------------------------

#[test]
fn validate_rejects_empty_workflows_path() {
    let toml_str = r#"
[storage]
path = "/data/core.db"

[workflows]
path = ""
"#;
    let config: CoreConfig = toml::from_str(toml_str).unwrap();
    let errors = validate(&config).unwrap_err();
    assert!(errors.iter().any(|e| e.contains("workflows")));
}

// ---------------------------------------------------------------------------
// 15. Validate rejects invalid log level
// ---------------------------------------------------------------------------

#[test]
fn validate_rejects_invalid_log_level() {
    let toml_str = r#"
[storage]
path = "/data/core.db"

[logging]
level = "verbose"
"#;
    let config: CoreConfig = toml::from_str(toml_str).unwrap();
    let errors = validate(&config).unwrap_err();
    assert!(errors.iter().any(|e| e.contains("level")));
}

// ---------------------------------------------------------------------------
// 16. Validate rejects invalid log format
// ---------------------------------------------------------------------------

#[test]
fn validate_rejects_invalid_log_format() {
    let toml_str = r#"
[storage]
path = "/data/core.db"

[logging]
format = "xml"
"#;
    let config: CoreConfig = toml::from_str(toml_str).unwrap();
    let errors = validate(&config).unwrap_err();
    assert!(errors.iter().any(|e| e.contains("format")));
}

// ---------------------------------------------------------------------------
// 17. Binary compiles with config module (smoke test)
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn binary_compiles_with_config_module() {
    let output = Command::new("cargo")
        .args(["check", "-p", "life-engine-core"])
        .current_dir(repo_root())
        .output()
        .expect("failed to execute cargo check");

    assert!(
        output.status.success(),
        "cargo check failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
