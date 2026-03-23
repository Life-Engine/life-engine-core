//! Plugin directory scanner for discovering plugin subdirectories.
//!
//! Scans a plugins directory for subdirectories containing both
//! `plugin.wasm` and `manifest.toml` files. Directories missing
//! either file are skipped with a warning log.

use std::path::{Path, PathBuf};

use tracing::{info, warn};

use crate::error::PluginError;

/// A discovered plugin with paths to its WASM binary and manifest.
#[derive(Debug, Clone)]
pub struct DiscoveredPlugin {
    /// Root directory of the plugin.
    pub path: PathBuf,
    /// Path to the plugin's WASM binary.
    pub wasm_path: PathBuf,
    /// Path to the plugin's manifest file.
    pub manifest_path: PathBuf,
}

/// Scans a directory for plugin subdirectories containing both
/// `plugin.wasm` and `manifest.toml`.
///
/// Only immediate children of the given directory are scanned (no recursion).
/// Non-directory entries are silently ignored. Subdirectories missing one of
/// the two required files are skipped with a warning. The returned list is
/// sorted by directory name for deterministic loading order.
pub fn scan_plugins_directory(path: &Path) -> Result<Vec<DiscoveredPlugin>, PluginError> {
    if !path.exists() {
        return Err(PluginError::DirectoryNotFound(path.display().to_string()));
    }

    if !path.is_dir() {
        return Err(PluginError::DirectoryNotFound(format!(
            "{} is not a directory",
            path.display()
        )));
    }

    let mut entries: Vec<_> = std::fs::read_dir(path)
        .map_err(|e| PluginError::DirectoryScanFailed(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| PluginError::DirectoryScanFailed(e.to_string()))?;

    // Sort by filename for deterministic ordering
    entries.sort_by_key(|e| e.file_name());

    let mut plugins = Vec::new();

    for entry in entries {
        let entry_path = entry.path();

        if !entry_path.is_dir() {
            continue;
        }

        let wasm_path = entry_path.join("plugin.wasm");
        let manifest_path = entry_path.join("manifest.toml");
        let dir_name = entry.file_name();
        let dir_name = dir_name.to_string_lossy();

        let has_wasm = wasm_path.exists();
        let has_manifest = manifest_path.exists();

        match (has_wasm, has_manifest) {
            (true, true) => {
                plugins.push(DiscoveredPlugin {
                    path: entry_path,
                    wasm_path,
                    manifest_path,
                });
            }
            (true, false) => {
                warn!(
                    directory = %dir_name,
                    "Skipping plugin directory: found plugin.wasm but missing manifest.toml"
                );
            }
            (false, true) => {
                warn!(
                    directory = %dir_name,
                    "Skipping plugin directory: found manifest.toml but missing plugin.wasm"
                );
            }
            (false, false) => {
                // Neither file present — not a plugin directory, skip silently
            }
        }
    }

    info!("Discovered {} plugins in {}", plugins.len(), path.display());

    Ok(plugins)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_plugin_dir(parent: &Path, name: &str, wasm: bool, manifest: bool) -> PathBuf {
        let dir = parent.join(name);
        fs::create_dir_all(&dir).unwrap();
        if wasm {
            fs::write(dir.join("plugin.wasm"), b"fake wasm").unwrap();
        }
        if manifest {
            fs::write(dir.join("manifest.toml"), b"[plugin]\nid = \"test\"").unwrap();
        }
        dir
    }

    #[test]
    fn discovers_valid_plugin_directory() {
        let tmp = TempDir::new().unwrap();
        create_plugin_dir(tmp.path(), "my-plugin", true, true);

        let result = scan_plugins_directory(tmp.path()).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path.file_name().unwrap(), "my-plugin");
        assert!(result[0].wasm_path.ends_with("plugin.wasm"));
        assert!(result[0].manifest_path.ends_with("manifest.toml"));
    }

    #[test]
    fn skips_directory_with_only_manifest() {
        let tmp = TempDir::new().unwrap();
        create_plugin_dir(tmp.path(), "incomplete", false, true);

        let result = scan_plugins_directory(tmp.path()).unwrap();

        assert!(result.is_empty());
    }

    #[test]
    fn skips_directory_with_only_wasm() {
        let tmp = TempDir::new().unwrap();
        create_plugin_dir(tmp.path(), "incomplete", true, false);

        let result = scan_plugins_directory(tmp.path()).unwrap();

        assert!(result.is_empty());
    }

    #[test]
    fn empty_directory_returns_empty_vec() {
        let tmp = TempDir::new().unwrap();

        let result = scan_plugins_directory(tmp.path()).unwrap();

        assert!(result.is_empty());
    }

    #[test]
    fn does_not_recurse_into_nested_directories() {
        let tmp = TempDir::new().unwrap();
        let nested = tmp.path().join("outer").join("inner");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("plugin.wasm"), b"fake wasm").unwrap();
        fs::write(nested.join("manifest.toml"), b"[plugin]").unwrap();

        // outer has neither file, inner is nested — neither should be discovered
        let result = scan_plugins_directory(tmp.path()).unwrap();

        assert!(result.is_empty());
    }

    #[test]
    fn ignores_non_directory_entries() {
        let tmp = TempDir::new().unwrap();
        // Create a regular file in the plugins directory
        fs::write(tmp.path().join("not-a-dir.txt"), b"hello").unwrap();
        create_plugin_dir(tmp.path(), "real-plugin", true, true);

        let result = scan_plugins_directory(tmp.path()).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path.file_name().unwrap(), "real-plugin");
    }

    #[test]
    fn returns_sorted_by_directory_name() {
        let tmp = TempDir::new().unwrap();
        create_plugin_dir(tmp.path(), "zzz-plugin", true, true);
        create_plugin_dir(tmp.path(), "aaa-plugin", true, true);
        create_plugin_dir(tmp.path(), "mmm-plugin", true, true);

        let result = scan_plugins_directory(tmp.path()).unwrap();

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].path.file_name().unwrap(), "aaa-plugin");
        assert_eq!(result[1].path.file_name().unwrap(), "mmm-plugin");
        assert_eq!(result[2].path.file_name().unwrap(), "zzz-plugin");
    }

    #[test]
    fn nonexistent_directory_returns_error() {
        let result = scan_plugins_directory(Path::new("/nonexistent/path"));

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::DirectoryNotFound(_)));
    }

    #[test]
    fn file_path_instead_of_directory_returns_error() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("not-a-dir");
        fs::write(&file, b"hello").unwrap();

        let result = scan_plugins_directory(&file);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PluginError::DirectoryNotFound(_)));
    }
}
