//! Parse and validate `[[migrations]]` entries from a plugin's `manifest.toml`.
//!
//! Each migration entry declares a version range transform: records matching the
//! `from` semver range are transformed to the `to` version using the named WASM
//! export function.

use std::path::Path;

use semver::Version;
use serde::Deserialize;

use crate::migration::MigrationError;

/// A validated migration entry from a plugin manifest.
#[derive(Debug, Clone)]
pub struct MigrationEntry {
    /// Source version range (simplified semver: `1.x`, `1.0.x`, or exact `1.0.0`).
    pub from: String,
    /// Target version (exact semver).
    pub to: Version,
    /// Name of the WASM export function that performs the transform.
    pub transform: String,
    /// Human-readable description of what the migration does.
    pub description: String,
    /// Which collection this migration applies to.
    pub collection: String,
}

/// Raw TOML structure for the migrations array.
#[derive(Deserialize)]
struct RawManifestMigrations {
    migrations: Option<Vec<RawMigrationEntry>>,
}

#[derive(Deserialize)]
struct RawMigrationEntry {
    from: Option<String>,
    to: Option<String>,
    transform: Option<String>,
    description: Option<String>,
    collection: Option<String>,
}

/// Parse and validate migration entries from a manifest.toml file.
///
/// Returns an empty vec if no `[[migrations]]` section exists.
pub fn parse_migration_entries(path: &Path) -> Result<Vec<MigrationEntry>, MigrationError> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        MigrationError::ManifestParse(format!("failed to read {}: {e}", path.display()))
    })?;

    parse_migration_entries_from_str(&content)
}

/// Parse and validate migration entries from a TOML string.
pub fn parse_migration_entries_from_str(
    content: &str,
) -> Result<Vec<MigrationEntry>, MigrationError> {
    let raw: RawManifestMigrations = toml::from_str(content).map_err(|e| {
        MigrationError::ManifestParse(format!("failed to parse manifest TOML: {e}"))
    })?;

    let raw_entries = match raw.migrations {
        Some(entries) => entries,
        None => return Ok(Vec::new()),
    };

    let mut entries = Vec::with_capacity(raw_entries.len());

    for (i, raw) in raw_entries.iter().enumerate() {
        let from = raw.from.as_deref().ok_or_else(|| {
            MigrationError::ManifestValidation(format!(
                "migration entry {i}: missing 'from' field"
            ))
        })?;

        let to_str = raw.to.as_deref().ok_or_else(|| {
            MigrationError::ManifestValidation(format!("migration entry {i}: missing 'to' field"))
        })?;

        let transform = raw.transform.as_deref().ok_or_else(|| {
            MigrationError::ManifestValidation(format!(
                "migration entry {i}: missing 'transform' field"
            ))
        })?;

        let description = raw.description.as_deref().ok_or_else(|| {
            MigrationError::ManifestValidation(format!(
                "migration entry {i}: missing 'description' field"
            ))
        })?;

        let collection = raw.collection.as_deref().ok_or_else(|| {
            MigrationError::ManifestValidation(format!(
                "migration entry {i}: missing 'collection' field"
            ))
        })?;

        validate_from_range(from, i)?;

        let to = Version::parse(to_str).map_err(|e| {
            MigrationError::ManifestValidation(format!(
                "migration entry {i}: invalid 'to' version '{to_str}': {e}"
            ))
        })?;

        validate_from_less_than_to(from, &to, i)?;
        validate_transform_name(transform, i)?;
        validate_collection_name(collection, i)?;

        entries.push(MigrationEntry {
            from: from.to_string(),
            to,
            transform: transform.to_string(),
            description: description.to_string(),
            collection: collection.to_string(),
        });
    }

    validate_no_overlapping_ranges(&entries)?;
    validate_chain_contiguity(&entries)?;

    Ok(entries)
}

/// Validate that a `from` field is a valid simplified semver range.
///
/// Accepted formats: `major.x`, `major.minor.x`, or exact `major.minor.patch`.
fn validate_from_range(from: &str, entry_index: usize) -> Result<(), MigrationError> {
    let parts: Vec<&str> = from.split('.').collect();

    match parts.len() {
        2 => {
            // major.x
            parse_u64(parts[0], "major", from, entry_index)?;
            if parts[1] != "x" {
                return Err(MigrationError::ManifestValidation(format!(
                    "migration entry {entry_index}: invalid 'from' range '{from}': \
                     two-part range must be 'major.x'"
                )));
            }
        }
        3 => {
            // major.minor.x OR major.minor.patch
            parse_u64(parts[0], "major", from, entry_index)?;
            parse_u64(parts[1], "minor", from, entry_index)?;
            if parts[2] != "x" {
                // Must be a valid patch number (exact version)
                parse_u64(parts[2], "patch", from, entry_index)?;
            }
        }
        _ => {
            return Err(MigrationError::ManifestValidation(format!(
                "migration entry {entry_index}: invalid 'from' range '{from}': \
                 expected 'major.x', 'major.minor.x', or 'major.minor.patch'"
            )));
        }
    }

    Ok(())
}

fn parse_u64(s: &str, part_name: &str, from: &str, idx: usize) -> Result<u64, MigrationError> {
    s.parse::<u64>().map_err(|_| {
        MigrationError::ManifestValidation(format!(
            "migration entry {idx}: invalid 'from' range '{from}': \
             '{s}' is not a valid {part_name} version number"
        ))
    })
}

/// Validate that the `from` minimum version is strictly less than `to`.
fn validate_from_less_than_to(
    from: &str,
    to: &Version,
    entry_index: usize,
) -> Result<(), MigrationError> {
    let min_version = from_range_min_version(from);

    if min_version >= *to {
        return Err(MigrationError::ManifestValidation(format!(
            "migration entry {entry_index}: 'from' range '{from}' minimum version \
             ({min_version}) must be strictly less than 'to' version ({to})"
        )));
    }

    Ok(())
}

/// Compute the minimum concrete version that matches a `from` range.
fn from_range_min_version(from: &str) -> Version {
    let parts: Vec<&str> = from.split('.').collect();

    match parts.len() {
        2 => {
            // major.x -> major.0.0
            let major: u64 = parts[0].parse().unwrap_or(0);
            Version::new(major, 0, 0)
        }
        3 if parts[2] == "x" => {
            // major.minor.x -> major.minor.0
            let major: u64 = parts[0].parse().unwrap_or(0);
            let minor: u64 = parts[1].parse().unwrap_or(0);
            Version::new(major, minor, 0)
        }
        3 => {
            // exact version
            let major: u64 = parts[0].parse().unwrap_or(0);
            let minor: u64 = parts[1].parse().unwrap_or(0);
            let patch: u64 = parts[2].parse().unwrap_or(0);
            Version::new(major, minor, patch)
        }
        _ => Version::new(0, 0, 0),
    }
}

/// Validate that the transform name is a valid Rust identifier.
fn validate_transform_name(name: &str, entry_index: usize) -> Result<(), MigrationError> {
    if name.is_empty() {
        return Err(MigrationError::ManifestValidation(format!(
            "migration entry {entry_index}: transform name must not be empty"
        )));
    }

    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphabetic() && first != '_' {
        return Err(MigrationError::ManifestValidation(format!(
            "migration entry {entry_index}: transform name '{name}' is not a valid identifier: \
             must start with a letter or underscore"
        )));
    }

    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(MigrationError::ManifestValidation(format!(
            "migration entry {entry_index}: transform name '{name}' is not a valid identifier: \
             must contain only letters, digits, and underscores"
        )));
    }

    Ok(())
}

/// Validate that a collection name is non-empty and uses valid characters.
fn validate_collection_name(name: &str, entry_index: usize) -> Result<(), MigrationError> {
    if name.is_empty() {
        return Err(MigrationError::ManifestValidation(format!(
            "migration entry {entry_index}: collection name must not be empty"
        )));
    }

    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ':')
    {
        return Err(MigrationError::ManifestValidation(format!(
            "migration entry {entry_index}: collection name '{name}' contains invalid characters: \
             must contain only letters, digits, hyphens, underscores, and colons"
        )));
    }

    Ok(())
}

/// Parsed representation of a simplified semver range for overlap comparison.
#[derive(Debug)]
enum VersionRange {
    /// `major.x` — matches all versions with the given major.
    MajorWildcard(u64),
    /// `major.minor.x` — matches all versions with given major.minor.
    MinorWildcard(u64, u64),
    /// `major.minor.patch` — matches exactly one version.
    Exact(u64, u64, u64),
}

impl VersionRange {
    fn parse(from: &str) -> Self {
        let parts: Vec<&str> = from.split('.').collect();
        match parts.len() {
            2 => {
                let major = parts[0].parse().unwrap_or(0);
                VersionRange::MajorWildcard(major)
            }
            3 if parts[2] == "x" => {
                let major = parts[0].parse().unwrap_or(0);
                let minor = parts[1].parse().unwrap_or(0);
                VersionRange::MinorWildcard(major, minor)
            }
            3 => {
                let major = parts[0].parse().unwrap_or(0);
                let minor = parts[1].parse().unwrap_or(0);
                let patch = parts[2].parse().unwrap_or(0);
                VersionRange::Exact(major, minor, patch)
            }
            _ => VersionRange::Exact(0, 0, 0),
        }
    }

    /// Returns true if any concrete version could match both `self` and `other`.
    fn overlaps(&self, other: &VersionRange) -> bool {
        match (self, other) {
            (VersionRange::MajorWildcard(m1), VersionRange::MajorWildcard(m2)) => m1 == m2,
            (VersionRange::MajorWildcard(m1), VersionRange::MinorWildcard(m2, _))
            | (VersionRange::MinorWildcard(m2, _), VersionRange::MajorWildcard(m1)) => m1 == m2,
            (VersionRange::MajorWildcard(m1), VersionRange::Exact(m2, _, _))
            | (VersionRange::Exact(m2, _, _), VersionRange::MajorWildcard(m1)) => m1 == m2,
            (VersionRange::MinorWildcard(m1, n1), VersionRange::MinorWildcard(m2, n2)) => {
                m1 == m2 && n1 == n2
            }
            (VersionRange::MinorWildcard(m1, n1), VersionRange::Exact(m2, n2, _))
            | (VersionRange::Exact(m2, n2, _), VersionRange::MinorWildcard(m1, n1)) => {
                m1 == m2 && n1 == n2
            }
            (VersionRange::Exact(m1, n1, p1), VersionRange::Exact(m2, n2, p2)) => {
                m1 == m2 && n1 == n2 && p1 == p2
            }
        }
    }

    /// Returns an example concrete version that matches this range.
    fn example_version(&self) -> String {
        match self {
            VersionRange::MajorWildcard(m) => format!("{m}.0.0"),
            VersionRange::MinorWildcard(m, n) => format!("{m}.{n}.0"),
            VersionRange::Exact(m, n, p) => format!("{m}.{n}.{p}"),
        }
    }
}

/// Validate that no two migration entries within the same collection have overlapping `from` ranges.
fn validate_no_overlapping_ranges(entries: &[MigrationEntry]) -> Result<(), MigrationError> {
    let mut by_collection: std::collections::HashMap<&str, Vec<&MigrationEntry>> =
        std::collections::HashMap::new();

    for entry in entries {
        by_collection
            .entry(entry.collection.as_str())
            .or_default()
            .push(entry);
    }

    for (collection, chain) in &by_collection {
        for i in 0..chain.len() {
            for j in (i + 1)..chain.len() {
                let range_a = VersionRange::parse(&chain[i].from);
                let range_b = VersionRange::parse(&chain[j].from);

                if range_a.overlaps(&range_b) {
                    let example = find_overlap_example(&range_a, &range_b);
                    return Err(MigrationError::ManifestValidation(format!(
                        "overlapping 'from' ranges in collection '{collection}': \
                         '{}' and '{}' both match version {example}",
                        chain[i].from, chain[j].from
                    )));
                }
            }
        }
    }

    Ok(())
}

/// Find a concrete version that matches both ranges (for error messages).
fn find_overlap_example(a: &VersionRange, b: &VersionRange) -> String {
    // Pick the more specific range's example — it will match both since they overlap.
    match (a, b) {
        (VersionRange::Exact(_, _, _), _) => a.example_version(),
        (_, VersionRange::Exact(_, _, _)) => b.example_version(),
        (VersionRange::MinorWildcard(_, _), _) => a.example_version(),
        (_, VersionRange::MinorWildcard(_, _)) => b.example_version(),
        _ => a.example_version(),
    }
}

/// Validate that migration entries within the same collection form a contiguous chain.
///
/// For each collection, entries sorted by `to` version must have the `to` of one entry
/// within the `from` range of the next.
fn validate_chain_contiguity(entries: &[MigrationEntry]) -> Result<(), MigrationError> {
    // Group entries by collection
    let mut by_collection: std::collections::HashMap<&str, Vec<&MigrationEntry>> =
        std::collections::HashMap::new();

    for entry in entries {
        by_collection
            .entry(entry.collection.as_str())
            .or_default()
            .push(entry);
    }

    for (collection, mut chain) in by_collection {
        if chain.len() <= 1 {
            continue;
        }

        // Sort by the `to` version
        chain.sort_by(|a, b| a.to.cmp(&b.to));

        for pair in chain.windows(2) {
            let prev = pair[0];
            let next = pair[1];

            // The `to` of the previous entry must be matchable by the `from` of the next
            if !version_matches_range(&prev.to, &next.from) {
                return Err(MigrationError::ManifestValidation(format!(
                    "migration chain gap in collection '{collection}': \
                     entry targeting {} is not followed by an entry whose 'from' range \
                     matches {}. Next entry has 'from' range '{}'",
                    prev.to, prev.to, next.from
                )));
            }
        }
    }

    Ok(())
}

/// Check if a concrete version matches a simplified semver range.
fn version_matches_range(version: &Version, range: &str) -> bool {
    let parts: Vec<&str> = range.split('.').collect();

    match parts.len() {
        2 if parts[1] == "x" => {
            // major.x
            let major: u64 = parts[0].parse().unwrap_or(u64::MAX);
            version.major == major
        }
        3 if parts[2] == "x" => {
            // major.minor.x
            let major: u64 = parts[0].parse().unwrap_or(u64::MAX);
            let minor: u64 = parts[1].parse().unwrap_or(u64::MAX);
            version.major == major && version.minor == minor
        }
        3 => {
            // exact version
            match Version::parse(range) {
                Ok(v) => version == &v,
                Err(_) => false,
            }
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_migration_entries() {
        let toml = r#"
[[migrations]]
from = "1.x"
to = "2.0.0"
transform = "migrate_v1_to_v2"
description = "Rename title to name"
collection = "events"

[[migrations]]
from = "2.x"
to = "3.0.0"
transform = "migrate_v2_to_v3"
description = "Add priority field"
collection = "events"
"#;

        let entries = parse_migration_entries_from_str(toml).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].from, "1.x");
        assert_eq!(entries[0].to, Version::new(2, 0, 0));
        assert_eq!(entries[0].transform, "migrate_v1_to_v2");
        assert_eq!(entries[0].collection, "events");
        assert_eq!(entries[1].from, "2.x");
        assert_eq!(entries[1].to, Version::new(3, 0, 0));
    }

    #[test]
    fn no_migrations_section_returns_empty() {
        let toml = r#"
[plugin]
id = "test"
name = "Test"
version = "1.0.0"
"#;
        let entries = parse_migration_entries_from_str(toml).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn missing_from_field_returns_error() {
        let toml = r#"
[[migrations]]
to = "2.0.0"
transform = "migrate"
description = "desc"
collection = "events"
"#;
        let err = parse_migration_entries_from_str(toml).unwrap_err();
        assert!(err.to_string().contains("missing 'from'"));
    }

    #[test]
    fn missing_to_field_returns_error() {
        let toml = r#"
[[migrations]]
from = "1.x"
transform = "migrate"
description = "desc"
collection = "events"
"#;
        let err = parse_migration_entries_from_str(toml).unwrap_err();
        assert!(err.to_string().contains("missing 'to'"));
    }

    #[test]
    fn missing_transform_field_returns_error() {
        let toml = r#"
[[migrations]]
from = "1.x"
to = "2.0.0"
description = "desc"
collection = "events"
"#;
        let err = parse_migration_entries_from_str(toml).unwrap_err();
        assert!(err.to_string().contains("missing 'transform'"));
    }

    #[test]
    fn missing_description_field_returns_error() {
        let toml = r#"
[[migrations]]
from = "1.x"
to = "2.0.0"
transform = "migrate"
collection = "events"
"#;
        let err = parse_migration_entries_from_str(toml).unwrap_err();
        assert!(err.to_string().contains("missing 'description'"));
    }

    #[test]
    fn missing_collection_field_returns_error() {
        let toml = r#"
[[migrations]]
from = "1.x"
to = "2.0.0"
transform = "migrate"
description = "desc"
"#;
        let err = parse_migration_entries_from_str(toml).unwrap_err();
        assert!(err.to_string().contains("missing 'collection'"));
    }

    #[test]
    fn invalid_from_range_returns_error() {
        let toml = r#"
[[migrations]]
from = "abc"
to = "2.0.0"
transform = "migrate"
description = "desc"
collection = "events"
"#;
        let err = parse_migration_entries_from_str(toml).unwrap_err();
        assert!(err.to_string().contains("invalid 'from' range"));
    }

    #[test]
    fn invalid_from_two_part_non_x_returns_error() {
        let toml = r#"
[[migrations]]
from = "1.2"
to = "2.0.0"
transform = "migrate"
description = "desc"
collection = "events"
"#;
        let err = parse_migration_entries_from_str(toml).unwrap_err();
        assert!(err.to_string().contains("two-part range must be 'major.x'"));
    }

    #[test]
    fn invalid_to_version_returns_error() {
        let toml = r#"
[[migrations]]
from = "1.x"
to = "not-semver"
transform = "migrate"
description = "desc"
collection = "events"
"#;
        let err = parse_migration_entries_from_str(toml).unwrap_err();
        assert!(err.to_string().contains("invalid 'to' version"));
    }

    #[test]
    fn from_not_less_than_to_returns_error() {
        let toml = r#"
[[migrations]]
from = "3.x"
to = "2.0.0"
transform = "migrate"
description = "desc"
collection = "events"
"#;
        let err = parse_migration_entries_from_str(toml).unwrap_err();
        assert!(err.to_string().contains("must be strictly less than"));
    }

    #[test]
    fn from_equal_to_returns_error() {
        let toml = r#"
[[migrations]]
from = "2.0.0"
to = "2.0.0"
transform = "migrate"
description = "desc"
collection = "events"
"#;
        let err = parse_migration_entries_from_str(toml).unwrap_err();
        assert!(err.to_string().contains("must be strictly less than"));
    }

    #[test]
    fn invalid_transform_name_returns_error() {
        let toml = r#"
[[migrations]]
from = "1.x"
to = "2.0.0"
transform = "123invalid"
description = "desc"
collection = "events"
"#;
        let err = parse_migration_entries_from_str(toml).unwrap_err();
        assert!(err.to_string().contains("not a valid identifier"));
    }

    #[test]
    fn transform_with_special_chars_returns_error() {
        let toml = r#"
[[migrations]]
from = "1.x"
to = "2.0.0"
transform = "my-transform"
description = "desc"
collection = "events"
"#;
        let err = parse_migration_entries_from_str(toml).unwrap_err();
        assert!(err.to_string().contains("not a valid identifier"));
    }

    #[test]
    fn underscore_transform_name_is_valid() {
        let toml = r#"
[[migrations]]
from = "1.x"
to = "2.0.0"
transform = "_migrate_v1"
description = "desc"
collection = "events"
"#;
        let entries = parse_migration_entries_from_str(toml).unwrap();
        assert_eq!(entries[0].transform, "_migrate_v1");
    }

    #[test]
    fn empty_collection_returns_error() {
        let toml = r#"
[[migrations]]
from = "1.x"
to = "2.0.0"
transform = "migrate"
description = "desc"
collection = ""
"#;
        let err = parse_migration_entries_from_str(toml).unwrap_err();
        assert!(err.to_string().contains("collection name must not be empty"));
    }

    #[test]
    fn collection_with_invalid_chars_returns_error() {
        let toml = r#"
[[migrations]]
from = "1.x"
to = "2.0.0"
transform = "migrate"
description = "desc"
collection = "my collection!"
"#;
        let err = parse_migration_entries_from_str(toml).unwrap_err();
        assert!(err.to_string().contains("invalid characters"));
    }

    #[test]
    fn contiguous_chain_passes() {
        let toml = r#"
[[migrations]]
from = "1.x"
to = "2.0.0"
transform = "migrate_v1_v2"
description = "v1 to v2"
collection = "events"

[[migrations]]
from = "2.x"
to = "3.0.0"
transform = "migrate_v2_v3"
description = "v2 to v3"
collection = "events"
"#;
        let entries = parse_migration_entries_from_str(toml).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn non_contiguous_chain_returns_error() {
        let toml = r#"
[[migrations]]
from = "1.x"
to = "2.0.0"
transform = "migrate_v1_v2"
description = "v1 to v2"
collection = "events"

[[migrations]]
from = "4.x"
to = "5.0.0"
transform = "migrate_v4_v5"
description = "v4 to v5"
collection = "events"
"#;
        let err = parse_migration_entries_from_str(toml).unwrap_err();
        assert!(err.to_string().contains("chain gap"));
    }

    #[test]
    fn different_collections_are_independent() {
        let toml = r#"
[[migrations]]
from = "1.x"
to = "2.0.0"
transform = "migrate_events"
description = "events v1 to v2"
collection = "events"

[[migrations]]
from = "5.x"
to = "6.0.0"
transform = "migrate_tasks"
description = "tasks v5 to v6"
collection = "tasks"
"#;
        // No chain gap error because these are different collections
        let entries = parse_migration_entries_from_str(toml).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn exact_from_version_validates() {
        let toml = r#"
[[migrations]]
from = "1.0.0"
to = "1.1.0"
transform = "migrate_patch"
description = "exact version migration"
collection = "events"
"#;
        let entries = parse_migration_entries_from_str(toml).unwrap();
        assert_eq!(entries[0].from, "1.0.0");
    }

    #[test]
    fn minor_wildcard_from_validates() {
        let toml = r#"
[[migrations]]
from = "1.0.x"
to = "1.1.0"
transform = "migrate_minor"
description = "minor wildcard migration"
collection = "events"
"#;
        let entries = parse_migration_entries_from_str(toml).unwrap();
        assert_eq!(entries[0].from, "1.0.x");
    }

    #[test]
    fn version_matches_major_wildcard() {
        assert!(version_matches_range(&Version::new(1, 0, 0), "1.x"));
        assert!(version_matches_range(&Version::new(1, 5, 3), "1.x"));
        assert!(!version_matches_range(&Version::new(2, 0, 0), "1.x"));
    }

    #[test]
    fn version_matches_minor_wildcard() {
        assert!(version_matches_range(&Version::new(1, 0, 0), "1.0.x"));
        assert!(version_matches_range(&Version::new(1, 0, 5), "1.0.x"));
        assert!(!version_matches_range(&Version::new(1, 1, 0), "1.0.x"));
    }

    #[test]
    fn version_matches_exact() {
        assert!(version_matches_range(&Version::new(1, 0, 0), "1.0.0"));
        assert!(!version_matches_range(&Version::new(1, 0, 1), "1.0.0"));
    }

    #[test]
    fn contiguous_chain_with_minor_wildcards() {
        let toml = r#"
[[migrations]]
from = "1.0.x"
to = "1.1.0"
transform = "migrate_a"
description = "a"
collection = "events"

[[migrations]]
from = "1.1.x"
to = "1.2.0"
transform = "migrate_b"
description = "b"
collection = "events"
"#;
        let entries = parse_migration_entries_from_str(toml).unwrap();
        assert_eq!(entries.len(), 2);
    }

    // --- Overlap detection tests ---

    #[test]
    fn non_overlapping_ranges_pass() {
        let toml = r#"
[[migrations]]
from = "1.x"
to = "2.0.0"
transform = "migrate_v1_v2"
description = "v1 to v2"
collection = "events"

[[migrations]]
from = "2.x"
to = "3.0.0"
transform = "migrate_v2_v3"
description = "v2 to v3"
collection = "events"
"#;
        let entries = parse_migration_entries_from_str(toml).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn overlapping_major_and_minor_wildcard_fails() {
        let toml = r#"
[[migrations]]
from = "1.x"
to = "2.0.0"
transform = "migrate_a"
description = "a"
collection = "events"

[[migrations]]
from = "1.0.x"
to = "1.1.0"
transform = "migrate_b"
description = "b"
collection = "events"
"#;
        let err = parse_migration_entries_from_str(toml).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("overlapping"), "expected overlap error, got: {msg}");
        assert!(msg.contains("1.x"));
        assert!(msg.contains("1.0.x"));
    }

    #[test]
    fn same_from_range_different_collections_allowed() {
        let toml = r#"
[[migrations]]
from = "1.x"
to = "2.0.0"
transform = "migrate_events"
description = "events migration"
collection = "events"

[[migrations]]
from = "1.x"
to = "2.0.0"
transform = "migrate_tasks"
description = "tasks migration"
collection = "tasks"
"#;
        let entries = parse_migration_entries_from_str(toml).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn exact_version_ranges_do_not_overlap() {
        let toml = r#"
[[migrations]]
from = "1.0.0"
to = "1.0.1"
transform = "migrate_a"
description = "a"
collection = "events"

[[migrations]]
from = "1.0.1"
to = "1.0.2"
transform = "migrate_b"
description = "b"
collection = "events"
"#;
        let entries = parse_migration_entries_from_str(toml).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn identical_exact_versions_overlap() {
        let toml = r#"
[[migrations]]
from = "1.0.0"
to = "1.1.0"
transform = "migrate_a"
description = "a"
collection = "events"

[[migrations]]
from = "1.0.0"
to = "2.0.0"
transform = "migrate_b"
description = "b"
collection = "events"
"#;
        let err = parse_migration_entries_from_str(toml).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("overlapping"), "expected overlap error, got: {msg}");
    }

    #[test]
    fn non_overlapping_minor_wildcards_pass() {
        let toml = r#"
[[migrations]]
from = "1.0.x"
to = "1.1.0"
transform = "migrate_a"
description = "a"
collection = "events"

[[migrations]]
from = "1.1.x"
to = "1.2.0"
transform = "migrate_b"
description = "b"
collection = "events"
"#;
        let entries = parse_migration_entries_from_str(toml).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn overlap_error_includes_example_version() {
        let toml = r#"
[[migrations]]
from = "1.x"
to = "2.0.0"
transform = "migrate_a"
description = "a"
collection = "events"

[[migrations]]
from = "1.0.x"
to = "1.1.0"
transform = "migrate_b"
description = "b"
collection = "events"
"#;
        let err = parse_migration_entries_from_str(toml).unwrap_err();
        let msg = err.to_string();
        // The error should include an example version that matches both ranges
        assert!(
            msg.contains("1.0.0"),
            "expected example version in error, got: {msg}"
        );
    }
}
