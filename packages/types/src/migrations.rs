//! Canonical schema migration declarations.
//!
//! Defines the current schema version for each canonical collection and
//! the migration directory structure. During Core startup, the system
//! compares stored versions against these declared versions and runs
//! WASM migration transforms when the stored version is behind.

/// Plugin ID used for canonical (built-in) collections.
pub const CANONICAL_PLUGIN_ID: &str = "core";

/// A canonical collection with its current declared schema version.
#[derive(Debug, Clone)]
pub struct CanonicalCollection {
    /// Collection name (e.g., "events", "tasks").
    pub name: &'static str,
    /// Current schema version declared by the types crate.
    pub version: i64,
    /// Subdirectory under `migrations/` containing WASM transform binaries.
    pub migration_dir: &'static str,
}

/// All canonical collections and their current schema versions.
///
/// When a canonical schema evolves, bump the version here and add
/// a corresponding WASM transform binary in the migration directory.
pub const CANONICAL_COLLECTIONS: &[CanonicalCollection] = &[
    CanonicalCollection {
        name: "events",
        version: 1,
        migration_dir: "events",
    },
    CanonicalCollection {
        name: "tasks",
        version: 1,
        migration_dir: "tasks",
    },
    CanonicalCollection {
        name: "contacts",
        version: 1,
        migration_dir: "contacts",
    },
    CanonicalCollection {
        name: "notes",
        version: 1,
        migration_dir: "notes",
    },
    CanonicalCollection {
        name: "emails",
        version: 1,
        migration_dir: "emails",
    },
    CanonicalCollection {
        name: "files",
        version: 1,
        migration_dir: "files",
    },
    CanonicalCollection {
        name: "credentials",
        version: 1,
        migration_dir: "credentials",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_collections_have_unique_names() {
        let mut names: Vec<&str> = CANONICAL_COLLECTIONS.iter().map(|c| c.name).collect();
        let original_len = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), original_len, "duplicate canonical collection names");
    }

    #[test]
    fn canonical_collections_have_positive_versions() {
        for col in CANONICAL_COLLECTIONS {
            assert!(col.version >= 1, "collection {} has version < 1", col.name);
        }
    }

    #[test]
    fn canonical_plugin_id_is_core() {
        assert_eq!(CANONICAL_PLUGIN_ID, "core");
    }
}
