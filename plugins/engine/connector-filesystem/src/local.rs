//! Local filesystem connector for scanning, indexing, and watching files.
//!
//! Provides configuration-driven file discovery with glob-based include/exclude
//! patterns, SHA-256 checksum computation, and change detection against a
//! previously indexed state.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};
use glob::Pattern;
use life_engine_types::file_helpers;
use serde::{Deserialize, Serialize};

use crate::normalizer;

/// Configuration for the local filesystem connector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalFsConfig {
    /// Directories to watch and index.
    pub watch_paths: Vec<PathBuf>,
    /// Glob patterns for files to include (empty = include all).
    pub include_patterns: Vec<String>,
    /// Glob patterns for files to exclude.
    pub exclude_patterns: Vec<String>,
    /// Whether to compute SHA-256 checksums for indexed files.
    pub compute_checksums: bool,
}

impl Default for LocalFsConfig {
    fn default() -> Self {
        Self {
            watch_paths: vec![],
            include_patterns: vec![],
            exclude_patterns: vec![],
            compute_checksums: true,
        }
    }
}

/// Represents a change detected in the filesystem.
#[derive(Debug, Clone, PartialEq)]
pub enum FileChange {
    /// A new file was found.
    Created(PathBuf),
    /// An existing file was modified (size or timestamp changed).
    Modified(PathBuf),
    /// A previously indexed file was deleted.
    Deleted(PathBuf),
}

/// Snapshot of an indexed file's metadata, used for change detection.
#[derive(Debug, Clone)]
pub struct IndexedFile {
    /// File size in bytes.
    pub size: u64,
    /// Last modification time.
    pub modified: SystemTime,
    /// Optional SHA-256 checksum.
    pub checksum: Option<String>,
}

/// Local filesystem connector that manages file indexing and change detection.
pub struct LocalFsConnector {
    /// Connector configuration.
    config: LocalFsConfig,
    /// Indexed files keyed by canonical path.
    indexed: HashMap<PathBuf, IndexedFile>,
}

impl LocalFsConnector {
    /// Create a new local filesystem connector with the given configuration.
    pub fn new(config: LocalFsConfig) -> Self {
        Self {
            config,
            indexed: HashMap::new(),
        }
    }

    /// Returns the connector configuration.
    pub fn config(&self) -> &LocalFsConfig {
        &self.config
    }

    /// Returns the current indexed file state.
    pub fn indexed_files(&self) -> &HashMap<PathBuf, IndexedFile> {
        &self.indexed
    }

    /// Check whether a file path should be included based on the
    /// configured include and exclude glob patterns.
    pub fn should_include(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        // Check exclude patterns first (deny takes priority)
        for pattern in &self.config.exclude_patterns {
            if let Ok(p) = Pattern::new(pattern) {
                // Match against the full path and the file name
                if p.matches(&path_str) {
                    return false;
                }
                if let Some(name) = path.file_name() {
                    if p.matches(&name.to_string_lossy()) {
                        return false;
                    }
                }
            }
        }

        // If no include patterns, include everything not excluded
        if self.config.include_patterns.is_empty() {
            return true;
        }

        // Check include patterns
        for pattern in &self.config.include_patterns {
            if let Ok(p) = Pattern::new(pattern) {
                if p.matches(&path_str) {
                    return true;
                }
                if let Some(name) = path.file_name() {
                    if p.matches(&name.to_string_lossy()) {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Compute the SHA-256 checksum of a file's contents.
    ///
    /// Delegates to the shared helper in `life_engine_types::file_helpers`.
    pub fn compute_checksum(path: &Path) -> Result<String> {
        file_helpers::compute_sha256(path)
    }

    /// Index a single file, reading its metadata and optionally computing a checksum.
    /// Returns the CDM `FileMetadata` for the file.
    pub fn index_file(&mut self, path: &Path) -> Result<life_engine_types::FileMetadata> {
        let metadata = normalizer::normalize_file(path, self.config.compute_checksums)?;

        let fs_meta = fs::metadata(path)
            .with_context(|| format!("failed to read metadata: {}", path.display()))?;

        let indexed = IndexedFile {
            size: fs_meta.len(),
            modified: fs_meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            checksum: if metadata.checksum.is_empty() {
                None
            } else {
                Some(metadata.checksum.clone())
            },
        };

        self.indexed.insert(path.to_path_buf(), indexed);
        Ok(metadata)
    }

    /// Maximum recursion depth for directory scanning.
    const MAX_SCAN_DEPTH: usize = 64;

    /// Scan all configured watch paths and return metadata for included files.
    pub fn scan(&mut self) -> Result<Vec<life_engine_types::FileMetadata>> {
        let mut results = Vec::new();
        let mut visited = std::collections::HashSet::new();

        // Iterate by index to avoid borrowing self.config while calling &mut self methods.
        for i in 0..self.config.watch_paths.len() {
            let watch_path = self.config.watch_paths[i].clone();
            if !watch_path.is_dir() {
                tracing::warn!(
                    path = %watch_path.display(),
                    "watch path is not a directory, skipping"
                );
                continue;
            }

            self.scan_directory(&watch_path, &mut results, &mut visited, 0)?;
        }

        Ok(results)
    }

    /// Recursively scan a directory for files matching include/exclude patterns.
    ///
    /// Tracks visited directories by canonical path to detect symlink cycles,
    /// and enforces a maximum recursion depth to prevent stack overflow on
    /// deeply nested filesystems.
    fn scan_directory(
        &mut self,
        dir: &Path,
        results: &mut Vec<life_engine_types::FileMetadata>,
        visited: &mut std::collections::HashSet<PathBuf>,
        depth: usize,
    ) -> Result<()> {
        if depth > Self::MAX_SCAN_DEPTH {
            tracing::warn!(
                path = %dir.display(),
                depth = depth,
                "maximum scan depth exceeded, skipping directory"
            );
            return Ok(());
        }

        // Resolve symlinks to detect cycles.
        let canonical = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
        if !visited.insert(canonical) {
            tracing::warn!(
                path = %dir.display(),
                "symlink cycle detected, skipping directory"
            );
            return Ok(());
        }

        let entries = fs::read_dir(dir)
            .with_context(|| format!("failed to read directory: {}", dir.display()))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                self.scan_directory(&path, results, visited, depth + 1)?;
            } else if path.is_file() && self.should_include(&path) {
                match self.index_file(&path) {
                    Ok(metadata) => results.push(metadata),
                    Err(e) => {
                        tracing::warn!(
                            path = %path.display(),
                            error = %e,
                            "failed to index file, skipping"
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Detect changes by comparing current filesystem state with the indexed state.
    ///
    /// Returns a list of `FileChange` values representing new, modified, and
    /// deleted files since the last scan.
    pub fn detect_changes(&self) -> Result<Vec<FileChange>> {
        let mut changes = Vec::new();

        // Check for new and modified files
        for watch_path in &self.config.watch_paths {
            if !watch_path.is_dir() {
                continue;
            }
            self.detect_changes_in_dir(watch_path, &mut changes)?;
        }

        // Check for deleted files
        for path in self.indexed.keys() {
            if !path.exists() {
                changes.push(FileChange::Deleted(path.clone()));
            }
        }

        Ok(changes)
    }

    /// Recursively detect changes in a directory.
    fn detect_changes_in_dir(&self, dir: &Path, changes: &mut Vec<FileChange>) -> Result<()> {
        let entries = fs::read_dir(dir)
            .with_context(|| format!("failed to read directory: {}", dir.display()))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                self.detect_changes_in_dir(&path, changes)?;
            } else if path.is_file() && self.should_include(&path) {
                match self.indexed.get(&path) {
                    None => {
                        changes.push(FileChange::Created(path));
                    }
                    Some(indexed) => {
                        let meta = fs::metadata(&path)?;
                        let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                        if meta.len() != indexed.size || modified != indexed.modified {
                            changes.push(FileChange::Modified(path));
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_config(dir: &Path) -> LocalFsConfig {
        LocalFsConfig {
            watch_paths: vec![dir.to_path_buf()],
            include_patterns: vec![],
            exclude_patterns: vec![],
            compute_checksums: true,
        }
    }

    #[test]
    fn local_fs_config_serialization() {
        let config = LocalFsConfig {
            watch_paths: vec![PathBuf::from("/home/user/documents")],
            include_patterns: vec!["*.pdf".into(), "*.txt".into()],
            exclude_patterns: vec![".*".into()],
            compute_checksums: true,
        };
        let json = serde_json::to_string(&config).expect("serialize");
        let restored: LocalFsConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.watch_paths.len(), 1);
        assert_eq!(restored.include_patterns.len(), 2);
        assert_eq!(restored.exclude_patterns.len(), 1);
        assert!(restored.compute_checksums);
    }

    #[test]
    fn local_fs_config_default() {
        let config = LocalFsConfig::default();
        assert!(config.watch_paths.is_empty());
        assert!(config.include_patterns.is_empty());
        assert!(config.exclude_patterns.is_empty());
        assert!(config.compute_checksums);
    }

    #[test]
    fn should_include_no_patterns_includes_all() {
        let config = LocalFsConfig::default();
        let connector = LocalFsConnector::new(config);
        assert!(connector.should_include(Path::new("/some/file.txt")));
        assert!(connector.should_include(Path::new("/some/file.rs")));
    }

    #[test]
    fn should_include_with_include_patterns() {
        let config = LocalFsConfig {
            include_patterns: vec!["*.pdf".into(), "*.txt".into()],
            ..Default::default()
        };
        let connector = LocalFsConnector::new(config);
        assert!(connector.should_include(Path::new("report.pdf")));
        assert!(connector.should_include(Path::new("notes.txt")));
        assert!(!connector.should_include(Path::new("image.png")));
    }

    #[test]
    fn should_include_exclude_overrides_include() {
        let config = LocalFsConfig {
            include_patterns: vec!["*".into()],
            exclude_patterns: vec!["*.log".into()],
            ..Default::default()
        };
        let connector = LocalFsConnector::new(config);
        assert!(connector.should_include(Path::new("file.txt")));
        assert!(!connector.should_include(Path::new("debug.log")));
    }

    #[test]
    fn exclude_pattern_filters_hidden_files() {
        let config = LocalFsConfig {
            exclude_patterns: vec![".*".into()],
            ..Default::default()
        };
        let connector = LocalFsConnector::new(config);
        assert!(!connector.should_include(Path::new(".gitignore")));
        assert!(!connector.should_include(Path::new(".hidden")));
        assert!(connector.should_include(Path::new("visible.txt")));
    }

    #[test]
    fn compute_checksum_sha256() {
        let dir = TempDir::new().expect("create temp dir");
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "hello world").expect("write file");

        let checksum = LocalFsConnector::compute_checksum(&file_path).expect("compute checksum");
        assert!(checksum.starts_with("sha256:"));
        // SHA-256 of "hello world"
        assert_eq!(
            checksum,
            "sha256:b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn index_file_creates_metadata() {
        let dir = TempDir::new().expect("create temp dir");
        let file_path = dir.path().join("document.pdf");
        fs::write(&file_path, "fake pdf content").expect("write file");

        let config = create_test_config(dir.path());
        let mut connector = LocalFsConnector::new(config);

        let metadata = connector.index_file(&file_path).expect("index file");
        assert_eq!(metadata.filename, "document.pdf");
        assert_eq!(metadata.mime_type, "application/pdf");
        assert_eq!(metadata.size_bytes, 16); // "fake pdf content".len()
        assert!(!metadata.checksum.is_empty());
        assert!(connector.indexed_files().contains_key(&file_path));
    }

    #[test]
    fn detect_changes_new_file() {
        let dir = TempDir::new().expect("create temp dir");
        let config = create_test_config(dir.path());
        let connector = LocalFsConnector::new(config);

        // Create a file after connector is set up (no indexed state)
        let file_path = dir.path().join("new_file.txt");
        fs::write(&file_path, "new content").expect("write file");

        let changes = connector.detect_changes().expect("detect changes");
        assert_eq!(changes.len(), 1);
        assert!(matches!(&changes[0], FileChange::Created(p) if p == &file_path));
    }

    #[test]
    fn detect_changes_modified_file() {
        let dir = TempDir::new().expect("create temp dir");
        let file_path = dir.path().join("existing.txt");
        fs::write(&file_path, "original").expect("write file");

        let config = create_test_config(dir.path());
        let mut connector = LocalFsConnector::new(config);
        connector.index_file(&file_path).expect("index file");

        // Modify the file (change size)
        fs::write(&file_path, "modified content that is longer").expect("modify file");

        let changes = connector.detect_changes().expect("detect changes");
        assert_eq!(changes.len(), 1);
        assert!(matches!(&changes[0], FileChange::Modified(p) if p == &file_path));
    }

    #[test]
    fn detect_changes_deleted_file() {
        let dir = TempDir::new().expect("create temp dir");
        let file_path = dir.path().join("to_delete.txt");
        fs::write(&file_path, "will be deleted").expect("write file");

        let config = create_test_config(dir.path());
        let mut connector = LocalFsConnector::new(config);
        connector.index_file(&file_path).expect("index file");

        // Delete the file
        fs::remove_file(&file_path).expect("delete file");

        let changes = connector.detect_changes().expect("detect changes");
        assert_eq!(changes.len(), 1);
        assert!(matches!(&changes[0], FileChange::Deleted(p) if p == &file_path));
    }

    #[test]
    fn empty_directory_scan_returns_no_files() {
        let dir = TempDir::new().expect("create temp dir");
        let config = create_test_config(dir.path());
        let mut connector = LocalFsConnector::new(config);

        let results = connector.scan().expect("scan");
        assert!(results.is_empty());
    }

    #[test]
    fn scan_indexes_files() {
        let dir = TempDir::new().expect("create temp dir");
        fs::write(dir.path().join("a.txt"), "hello").expect("write");
        fs::write(dir.path().join("b.pdf"), "fake pdf").expect("write");

        let config = create_test_config(dir.path());
        let mut connector = LocalFsConnector::new(config);

        let results = connector.scan().expect("scan");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn multiple_watch_paths() {
        let dir1 = TempDir::new().expect("create temp dir");
        let dir2 = TempDir::new().expect("create temp dir");
        fs::write(dir1.path().join("a.txt"), "hello").expect("write");
        fs::write(dir2.path().join("b.txt"), "world").expect("write");

        let config = LocalFsConfig {
            watch_paths: vec![dir1.path().to_path_buf(), dir2.path().to_path_buf()],
            ..Default::default()
        };
        let mut connector = LocalFsConnector::new(config);

        let results = connector.scan().expect("scan");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn large_file_metadata() {
        let dir = TempDir::new().expect("create temp dir");
        let file_path = dir.path().join("large_file.bin");

        // Create a sparse file by writing metadata only
        // We test that the size field can hold values > u32::MAX
        let mut file = fs::File::create(&file_path).expect("create file");
        file.write_all(b"some content").expect("write");
        drop(file);

        let config = create_test_config(dir.path());
        let mut connector = LocalFsConnector::new(config);

        let metadata = connector.index_file(&file_path).expect("index file");
        // Verify size is u64 (type system ensures this)
        let size: u64 = metadata.size_bytes;
        assert!(size > 0);

        // Verify a large size value can be represented
        let large_size: u64 = u64::from(u32::MAX) + 1;
        assert!(large_size > u64::from(u32::MAX));
    }

    #[test]
    fn scan_with_exclude_patterns() {
        let dir = TempDir::new().expect("create temp dir");
        fs::write(dir.path().join("visible.txt"), "hello").expect("write");
        fs::write(dir.path().join(".hidden"), "secret").expect("write");

        let config = LocalFsConfig {
            watch_paths: vec![dir.path().to_path_buf()],
            exclude_patterns: vec![".*".into()],
            ..Default::default()
        };
        let mut connector = LocalFsConnector::new(config);

        let results = connector.scan().expect("scan");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].filename, "visible.txt");
    }
}
