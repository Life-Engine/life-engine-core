//! Validates the structure of `registry/plugin-registry.json`.
//!
//! Ensures the registry file is valid JSON and has the expected top-level
//! structure so that deserialization errors are caught early.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("failed to resolve repo root")
}

#[test]
fn plugin_registry_is_valid_json_with_expected_structure() {
    let path = repo_root().join("registry/plugin-registry.json");
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    let doc: serde_json::Value =
        serde_json::from_str(&content).expect("plugin-registry.json is not valid JSON");

    // Top-level must be an object with "version", "updated", and "plugins".
    let obj = doc.as_object().expect("registry must be a JSON object");

    assert!(
        obj.contains_key("version"),
        "registry must have a 'version' field"
    );
    assert!(
        obj.contains_key("updated"),
        "registry must have an 'updated' field"
    );

    let plugins = obj
        .get("plugins")
        .and_then(|v| v.as_array())
        .expect("registry must have a 'plugins' array");

    // Each plugin entry must have at minimum an "id", "name", and "version".
    for (i, plugin) in plugins.iter().enumerate() {
        let p = plugin
            .as_object()
            .unwrap_or_else(|| panic!("plugins[{i}] must be a JSON object"));
        for field in ["id", "name", "version"] {
            assert!(
                p.contains_key(field),
                "plugins[{i}] must have a '{field}' field"
            );
        }
    }
}
