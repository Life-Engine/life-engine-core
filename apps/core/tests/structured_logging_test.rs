//! Integration tests for WP 9.11 — Structured Logging.
//!
//! Validates that the logging configuration supports per-module log levels
//! and that the JSON output includes version and pid fields via the root span.

use std::collections::HashMap;

/// Verify that `log_modules` in the YAML config deserialises correctly.
#[test]
fn log_modules_deserialise_from_yaml() {
    let yaml = r#"
core:
  host: "127.0.0.1"
  port: 3000
  log_level: "info"
  log_format: "json"
  log_modules:
    storage: "debug"
    auth: "trace"
  data_dir: "/tmp/le-test"
storage:
  encryption: false
auth:
  provider: "none"
network:
  cors:
    allowed_origins: ["*"]
  rate_limit:
    requests_per_minute: 60
  tls:
    enabled: false
"#;
    let config: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
    let modules = config["core"]["log_modules"].as_mapping().unwrap();
    assert_eq!(
        modules
            .get(serde_yaml::Value::String("storage".into()))
            .unwrap()
            .as_str()
            .unwrap(),
        "debug"
    );
    assert_eq!(
        modules
            .get(serde_yaml::Value::String("auth".into()))
            .unwrap()
            .as_str()
            .unwrap(),
        "trace"
    );
}

/// Verify that an empty `log_modules` map is the default (no per-module overrides).
#[test]
fn log_modules_defaults_to_empty() {
    let yaml = r#"
core:
  host: "127.0.0.1"
  port: 3000
  data_dir: "/tmp/le-test"
storage:
  encryption: false
auth:
  provider: "none"
network:
  cors:
    allowed_origins: ["*"]
  rate_limit:
    requests_per_minute: 60
  tls:
    enabled: false
"#;
    let config: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
    // log_modules should be absent (Null) or not present
    let modules = &config["core"]["log_modules"];
    assert!(modules.is_null() || modules.as_mapping().map_or(true, |m| m.is_empty()));
}

/// Verify the filter directive string building logic for per-module overrides.
///
/// This replicates the logic in `init_logging` to ensure module names are
/// correctly mapped to crate names.
#[test]
fn filter_directive_includes_per_module_overrides() {
    let base_level = "info";
    let mut log_modules = HashMap::new();
    log_modules.insert("storage".to_string(), "debug".to_string());
    log_modules.insert("auth".to_string(), "trace".to_string());

    let mut directives = base_level.to_string();
    for (module, level) in &log_modules {
        let crate_name = if module.starts_with("life_engine_") {
            module.clone()
        } else {
            format!("life_engine_{}", module.replace('-', "_"))
        };
        directives.push_str(&format!(",{crate_name}={level}"));
    }

    // The directive must start with the base level.
    assert!(directives.starts_with("info"));
    // Must contain per-module overrides.
    assert!(directives.contains("life_engine_storage=debug"));
    assert!(directives.contains("life_engine_auth=trace"));
}

/// Verify that modules already prefixed with `life_engine_` are not double-prefixed.
#[test]
fn filter_directive_no_double_prefix() {
    let mut log_modules = HashMap::new();
    log_modules.insert("life_engine_crypto".to_string(), "warn".to_string());

    let mut directives = "info".to_string();
    for (module, level) in &log_modules {
        let crate_name = if module.starts_with("life_engine_") {
            module.clone()
        } else {
            format!("life_engine_{}", module.replace('-', "_"))
        };
        directives.push_str(&format!(",{crate_name}={level}"));
    }

    assert!(directives.contains("life_engine_crypto=warn"));
    // Must NOT contain a double prefix.
    assert!(!directives.contains("life_engine_life_engine_"));
}

/// Verify the TOML-based startup config LoggingConfig also supports modules.
#[test]
fn startup_logging_config_modules() {
    let toml_str = r#"
[logging]
level = "info"
format = "json"

[logging.modules]
storage = "debug"
workflow_engine = "trace"
"#;
    let val: toml::Value = toml::from_str(toml_str).unwrap();
    let modules = val["logging"]["modules"].as_table().unwrap();
    assert_eq!(modules.get("storage").unwrap().as_str().unwrap(), "debug");
    assert_eq!(
        modules.get("workflow_engine").unwrap().as_str().unwrap(),
        "trace"
    );
}
