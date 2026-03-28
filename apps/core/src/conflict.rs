//! Conflict resolution engine for local-first sync.
//!
//! Detects and resolves conflicts when both local and remote versions of a
//! record have been modified since the last sync. Supports multiple resolution
//! strategies: last-write-wins, field-level merge, and manual resolution.
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
// Uses std::sync::Mutex intentionally: ConflictStore methods are synchronous
// (non-async) and lock durations are short (in-memory HashMap ops only), so
// std::sync::Mutex avoids the overhead of tokio::sync::Mutex.
use std::sync::Mutex;

use crate::storage::Record;

/// Resolution strategies for handling conflicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionStrategy {
    /// Newest `updated_at` wins (default).
    LastWriteWins,
    /// Merge individual fields (for contacts, events).
    FieldLevelMerge,
    /// Flag for manual user resolution (for notes).
    ManualResolution,
}

/// How a conflict was resolved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ConflictResolution {
    /// The local version was kept.
    KeepLocal,
    /// The remote version was kept.
    KeepRemote,
    /// A merged result was produced.
    Merged {
        /// The merged data.
        data: Value,
    },
    /// Auto-merge could not resolve conflicting field changes.
    /// Requires manual resolution from the user.
    RequiresManual,
}

/// A detected conflict between local and remote versions of a record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    /// Unique conflict identifier.
    pub id: String,
    /// The collection the conflicting record belongs to.
    pub collection: String,
    /// The ID of the record in conflict.
    pub record_id: String,
    /// The local version of the record.
    pub local_version: Record,
    /// The remote version of the record.
    pub remote_version: Record,
    /// The resolution strategy to apply.
    pub strategy: ResolutionStrategy,
    /// Whether this conflict has been resolved.
    pub resolved: bool,
    /// The resolution, if any.
    pub resolution: Option<ConflictResolution>,
    /// When the conflict was detected.
    pub detected_at: DateTime<Utc>,
}

/// Returns the resolution strategy for a given collection name.
pub fn strategy_for_collection(collection: &str) -> ResolutionStrategy {
    match collection {
        "events" | "contacts" => ResolutionStrategy::FieldLevelMerge,
        "notes" => ResolutionStrategy::ManualResolution,
        // emails, tasks, files, and any unknown collection
        _ => ResolutionStrategy::LastWriteWins,
    }
}

/// Detects whether two record versions are in conflict.
///
/// A conflict exists when both local and remote versions have diverged
/// from the `base_version` — i.e., both have been modified since the
/// last sync.
///
/// Returns `None` if there is no conflict (versions match, or only one
/// side changed).
pub fn detect_conflict(
    local: &Record,
    remote: &Record,
    base_version: i64,
) -> Option<Conflict> {
    // If they are the same version, no conflict.
    if local.version == remote.version {
        return None;
    }

    // If only one side changed from the base, no conflict.
    if local.version == base_version || remote.version == base_version {
        return None;
    }

    // Both sides diverged from the base — conflict detected.
    let collection = local.collection.clone();
    let strategy = strategy_for_collection(&collection);

    Some(Conflict {
        id: uuid::Uuid::new_v4().to_string(),
        collection,
        record_id: local.id.clone(),
        local_version: local.clone(),
        remote_version: remote.clone(),
        strategy,
        resolved: false,
        resolution: None,
        detected_at: Utc::now(),
    })
}

/// Resolves a conflict using the last-write-wins strategy.
///
/// The record with the more recent `updated_at` wins. If timestamps are
/// equal, the remote version wins (server authority).
pub fn resolve_last_write_wins(conflict: &Conflict) -> ConflictResolution {
    if conflict.local_version.updated_at > conflict.remote_version.updated_at {
        ConflictResolution::KeepLocal
    } else {
        // Remote wins on tie (server authority).
        ConflictResolution::KeepRemote
    }
}

/// Resolves a conflict using field-level merge.
///
/// For each top-level field in the JSON data:
/// - If only one side changed it from base, take that side's value.
/// - If both sides changed it, the merge cannot be completed automatically
///   and the conflict is flagged for manual resolution.
///
/// `base_data` is the last known common ancestor data. If `None`, we compare
/// the two versions directly — fields present in both with different values
/// are treated as both-modified.
pub fn resolve_field_merge(
    conflict: &Conflict,
    base_data: Option<&Value>,
) -> ConflictResolution {
    let local_obj = conflict.local_version.data.as_object();
    let remote_obj = conflict.remote_version.data.as_object();

    let (Some(local_fields), Some(remote_fields)) = (local_obj, remote_obj) else {
        // Non-object data cannot be field-merged; fall back to LWW.
        return resolve_last_write_wins(conflict);
    };

    let base_fields = base_data.and_then(|v| v.as_object());
    let mut merged = serde_json::Map::new();
    let mut has_overlap_conflict = false;

    // Collect all unique keys from both sides using HashSet for O(1) lookups.
    let all_keys: HashSet<&String> = local_fields.keys().chain(remote_fields.keys()).collect();

    for key in &all_keys {
        let local_val = local_fields.get(*key);
        let remote_val = remote_fields.get(*key);
        let base_val = base_fields.and_then(|b| b.get(*key));

        let local_changed = local_val != base_val;
        let remote_changed = remote_val != base_val;

        match (local_changed, remote_changed) {
            (true, true) => {
                // Both sides modified the same field.
                if local_val == remote_val {
                    // Same change on both sides — no conflict for this field.
                    if let Some(v) = local_val {
                        merged.insert((*key).clone(), v.clone());
                    }
                } else {
                    has_overlap_conflict = true;
                    break;
                }
            }
            (true, false) => {
                // Only local changed.
                if let Some(v) = local_val {
                    merged.insert((*key).clone(), v.clone());
                }
            }
            (false, true) => {
                // Only remote changed.
                if let Some(v) = remote_val {
                    merged.insert((*key).clone(), v.clone());
                }
            }
            (false, false) => {
                // Neither changed — keep base value.
                if let Some(v) = base_val.or(local_val).or(remote_val) {
                    merged.insert((*key).clone(), v.clone());
                }
            }
        }
    }

    if has_overlap_conflict {
        // Cannot auto-merge; flag for manual resolution.
        ConflictResolution::RequiresManual
    } else {
        ConflictResolution::Merged {
            data: Value::Object(merged),
        }
    }
}

/// Returns whether a field-level merge has overlapping changes that require
/// manual resolution.
pub fn field_merge_needs_manual(
    conflict: &Conflict,
    base_data: Option<&Value>,
) -> bool {
    let local_obj = conflict.local_version.data.as_object();
    let remote_obj = conflict.remote_version.data.as_object();

    let (Some(local_fields), Some(remote_fields)) = (local_obj, remote_obj) else {
        return false;
    };

    let base_fields = base_data.and_then(|v| v.as_object());

    let all_keys: HashSet<&String> = local_fields.keys().chain(remote_fields.keys()).collect();

    for key in &all_keys {
        let local_val = local_fields.get(*key);
        let remote_val = remote_fields.get(*key);
        let base_val = base_fields.and_then(|b| b.get(*key));

        let local_changed = local_val != base_val;
        let remote_changed = remote_val != base_val;

        if local_changed && remote_changed && local_val != remote_val {
            return true;
        }
    }

    false
}

/// Marks a conflict for manual resolution — always returns `None` as the
/// resolution, leaving it for the user to decide.
pub fn resolve_manual(_conflict: &Conflict) -> Option<ConflictResolution> {
    // Manual resolution means no automatic resolution is produced.
    None
}

/// In-memory store for unresolved conflicts.
#[derive(Debug)]
pub struct ConflictStore {
    conflicts: Mutex<HashMap<String, Conflict>>,
}

impl ConflictStore {
    /// Create a new, empty conflict store.
    pub fn new() -> Self {
        Self {
            conflicts: Mutex::new(HashMap::new()),
        }
    }

    /// Add a conflict to the store.
    pub fn add(&self, conflict: Conflict) {
        let mut conflicts = self.conflicts.lock().expect("conflict store lock poisoned");
        conflicts.insert(conflict.id.clone(), conflict);
    }

    /// Get a conflict by its ID.
    pub fn get(&self, id: &str) -> Option<Conflict> {
        let conflicts = self.conflicts.lock().expect("conflict store lock poisoned");
        conflicts.get(id).cloned()
    }

    /// List all unresolved conflicts, with pagination.
    pub fn list_unresolved(&self, limit: usize, offset: usize) -> (Vec<Conflict>, usize) {
        let conflicts = self.conflicts.lock().expect("conflict store lock poisoned");
        let unresolved: Vec<Conflict> = conflicts
            .values()
            .filter(|c| !c.resolved)
            .cloned()
            .collect();
        let total = unresolved.len();
        let page = unresolved
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect();
        (page, total)
    }

    /// Resolve a conflict with the given resolution.
    ///
    /// Returns `true` if the conflict was found and resolved.
    pub fn resolve(&self, id: &str, resolution: ConflictResolution) -> bool {
        let mut conflicts = self.conflicts.lock().expect("conflict store lock poisoned");
        if let Some(conflict) = conflicts.get_mut(id) {
            conflict.resolved = true;
            conflict.resolution = Some(resolution);
            true
        } else {
            false
        }
    }

    /// Remove a conflict from the store entirely.
    ///
    /// Returns `true` if the conflict was found and removed.
    pub fn remove(&self, id: &str) -> bool {
        let mut conflicts = self.conflicts.lock().expect("conflict store lock poisoned");
        conflicts.remove(id).is_some()
    }
}

impl Default for ConflictStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use serde_json::json;

    /// Helper to create a test record with the given parameters.
    fn make_record(
        id: &str,
        collection: &str,
        data: Value,
        version: i64,
        updated_at: DateTime<Utc>,
    ) -> Record {
        Record {
            id: id.into(),
            plugin_id: "core".into(),
            collection: collection.into(),
            data,
            version,
            user_id: None,
            household_id: None,
            created_at: updated_at - Duration::hours(1),
            updated_at,
        }
    }

    // ── Strategy mapping tests ──

    #[test]
    fn strategy_emails_uses_last_write_wins() {
        assert_eq!(
            strategy_for_collection("emails"),
            ResolutionStrategy::LastWriteWins
        );
    }

    #[test]
    fn strategy_tasks_uses_last_write_wins() {
        assert_eq!(
            strategy_for_collection("tasks"),
            ResolutionStrategy::LastWriteWins
        );
    }

    #[test]
    fn strategy_files_uses_last_write_wins() {
        assert_eq!(
            strategy_for_collection("files"),
            ResolutionStrategy::LastWriteWins
        );
    }

    #[test]
    fn strategy_events_uses_field_level_merge() {
        assert_eq!(
            strategy_for_collection("events"),
            ResolutionStrategy::FieldLevelMerge
        );
    }

    #[test]
    fn strategy_contacts_uses_field_level_merge() {
        assert_eq!(
            strategy_for_collection("contacts"),
            ResolutionStrategy::FieldLevelMerge
        );
    }

    #[test]
    fn strategy_notes_uses_manual_resolution() {
        assert_eq!(
            strategy_for_collection("notes"),
            ResolutionStrategy::ManualResolution
        );
    }

    #[test]
    fn strategy_unknown_collection_defaults_to_last_write_wins() {
        assert_eq!(
            strategy_for_collection("widgets"),
            ResolutionStrategy::LastWriteWins
        );
        assert_eq!(
            strategy_for_collection("custom_stuff"),
            ResolutionStrategy::LastWriteWins
        );
    }

    // ── Conflict detection tests ──

    #[test]
    fn detect_conflict_both_versions_changed() {
        let now = Utc::now();
        let local = make_record("r1", "tasks", json!({"a": 1}), 3, now);
        let remote = make_record("r1", "tasks", json!({"a": 2}), 4, now);
        let base_version = 2;

        let conflict = detect_conflict(&local, &remote, base_version);
        assert!(conflict.is_some());

        let c = conflict.unwrap();
        assert_eq!(c.record_id, "r1");
        assert_eq!(c.collection, "tasks");
        assert!(!c.resolved);
        assert!(c.resolution.is_none());
    }

    #[test]
    fn detect_conflict_only_one_side_changed_no_conflict() {
        let now = Utc::now();
        let local = make_record("r1", "tasks", json!({"a": 1}), 2, now);
        let remote = make_record("r1", "tasks", json!({"a": 2}), 3, now);
        let base_version = 2; // local is still at base

        assert!(detect_conflict(&local, &remote, base_version).is_none());
    }

    #[test]
    fn detect_conflict_versions_match_no_conflict() {
        let now = Utc::now();
        let local = make_record("r1", "tasks", json!({"a": 1}), 3, now);
        let remote = make_record("r1", "tasks", json!({"a": 1}), 3, now);
        let base_version = 2;

        assert!(detect_conflict(&local, &remote, base_version).is_none());
    }

    // ── LastWriteWins tests ──

    #[test]
    fn lww_newer_record_wins() {
        let now = Utc::now();
        let earlier = now - Duration::seconds(10);
        let local = make_record("r1", "tasks", json!({"v": "local"}), 3, now);
        let remote = make_record("r1", "tasks", json!({"v": "remote"}), 4, earlier);

        let conflict = Conflict {
            id: "c1".into(),
            collection: "tasks".into(),
            record_id: "r1".into(),
            local_version: local,
            remote_version: remote,
            strategy: ResolutionStrategy::LastWriteWins,
            resolved: false,
            resolution: None,
            detected_at: now,
        };

        let resolution = resolve_last_write_wins(&conflict);
        assert_eq!(resolution, ConflictResolution::KeepLocal);
    }

    #[test]
    fn lww_tie_keeps_remote() {
        let now = Utc::now();
        let local = make_record("r1", "tasks", json!({"v": "local"}), 3, now);
        let remote = make_record("r1", "tasks", json!({"v": "remote"}), 4, now);

        let conflict = Conflict {
            id: "c1".into(),
            collection: "tasks".into(),
            record_id: "r1".into(),
            local_version: local,
            remote_version: remote,
            strategy: ResolutionStrategy::LastWriteWins,
            resolved: false,
            resolution: None,
            detected_at: now,
        };

        let resolution = resolve_last_write_wins(&conflict);
        assert_eq!(resolution, ConflictResolution::KeepRemote);
    }

    // ── FieldLevelMerge tests ──

    #[test]
    fn field_merge_non_overlapping_changes_merge_cleanly() {
        let now = Utc::now();
        let base = json!({"name": "Alice", "phone": "111", "email": "a@b.com"});
        let local_data = json!({"name": "Alice B.", "phone": "111", "email": "a@b.com"});
        let remote_data = json!({"name": "Alice", "phone": "222", "email": "a@b.com"});

        let local = make_record("r1", "contacts", local_data, 3, now);
        let remote = make_record("r1", "contacts", remote_data, 4, now);

        let conflict = Conflict {
            id: "c1".into(),
            collection: "contacts".into(),
            record_id: "r1".into(),
            local_version: local,
            remote_version: remote,
            strategy: ResolutionStrategy::FieldLevelMerge,
            resolved: false,
            resolution: None,
            detected_at: now,
        };

        let resolution = resolve_field_merge(&conflict, Some(&base));
        match &resolution {
            ConflictResolution::Merged { data } => {
                assert_eq!(data["name"], "Alice B.");
                assert_eq!(data["phone"], "222");
                assert_eq!(data["email"], "a@b.com");
            }
            _ => panic!("expected Merged, got {resolution:?}"),
        }
    }

    #[test]
    fn field_merge_overlapping_changes_flag_manual() {
        let now = Utc::now();
        let base = json!({"name": "Alice", "phone": "111"});
        let local_data = json!({"name": "Alice Local", "phone": "111"});
        let remote_data = json!({"name": "Alice Remote", "phone": "111"});

        let local = make_record("r1", "contacts", local_data, 3, now);
        let remote = make_record("r1", "contacts", remote_data, 4, now);

        let conflict = Conflict {
            id: "c1".into(),
            collection: "contacts".into(),
            record_id: "r1".into(),
            local_version: local,
            remote_version: remote,
            strategy: ResolutionStrategy::FieldLevelMerge,
            resolved: false,
            resolution: None,
            detected_at: now,
        };

        assert!(field_merge_needs_manual(&conflict, Some(&base)));
    }

    #[test]
    fn field_merge_produces_valid_json() {
        let now = Utc::now();
        let base = json!({"title": "Meeting", "location": "Room A"});
        let local_data = json!({"title": "Meeting", "location": "Room B"});
        let remote_data = json!({"title": "Standup", "location": "Room A"});

        let local = make_record("r1", "events", local_data, 3, now);
        let remote = make_record("r1", "events", remote_data, 4, now);

        let conflict = Conflict {
            id: "c1".into(),
            collection: "events".into(),
            record_id: "r1".into(),
            local_version: local,
            remote_version: remote,
            strategy: ResolutionStrategy::FieldLevelMerge,
            resolved: false,
            resolution: None,
            detected_at: now,
        };

        // Each side changed a different field — no overlapping conflict.
        assert!(!field_merge_needs_manual(&conflict, Some(&base)));

        // Without overlapping changes: only location changed locally,
        // only title changed remotely.
        let base2 = json!({"title": "Meeting", "location": "Room A", "notes": "hi"});
        let local2 = make_record(
            "r1",
            "events",
            json!({"title": "Meeting", "location": "Room B", "notes": "hi"}),
            3,
            now,
        );
        let remote2 = make_record(
            "r1",
            "events",
            json!({"title": "Standup", "location": "Room A", "notes": "hi"}),
            4,
            now,
        );

        let conflict2 = Conflict {
            id: "c2".into(),
            collection: "events".into(),
            record_id: "r1".into(),
            local_version: local2,
            remote_version: remote2,
            strategy: ResolutionStrategy::FieldLevelMerge,
            resolved: false,
            resolution: None,
            detected_at: now,
        };

        let resolution = resolve_field_merge(&conflict2, Some(&base2));
        match &resolution {
            ConflictResolution::Merged { data } => {
                // Verify it roundtrips as valid JSON.
                let json_str = serde_json::to_string(data).unwrap();
                let parsed: Value = serde_json::from_str(&json_str).unwrap();
                assert_eq!(parsed["title"], "Standup");
                assert_eq!(parsed["location"], "Room B");
                assert_eq!(parsed["notes"], "hi");
            }
            _ => panic!("expected Merged, got {resolution:?}"),
        }
    }

    // ── ManualResolution tests ──

    #[test]
    fn manual_resolution_always_returns_none() {
        let now = Utc::now();
        let local = make_record("r1", "notes", json!({"body": "local text"}), 3, now);
        let remote = make_record("r1", "notes", json!({"body": "remote text"}), 4, now);

        let conflict = Conflict {
            id: "c1".into(),
            collection: "notes".into(),
            record_id: "r1".into(),
            local_version: local,
            remote_version: remote,
            strategy: ResolutionStrategy::ManualResolution,
            resolved: false,
            resolution: None,
            detected_at: now,
        };

        assert!(resolve_manual(&conflict).is_none());
    }

    // ── ConflictStore tests ──

    #[test]
    fn store_add_and_get() {
        let store = ConflictStore::new();
        let now = Utc::now();
        let conflict = Conflict {
            id: "c1".into(),
            collection: "tasks".into(),
            record_id: "r1".into(),
            local_version: make_record("r1", "tasks", json!({}), 2, now),
            remote_version: make_record("r1", "tasks", json!({}), 3, now),
            strategy: ResolutionStrategy::LastWriteWins,
            resolved: false,
            resolution: None,
            detected_at: now,
        };

        store.add(conflict.clone());

        let fetched = store.get("c1").expect("should find conflict");
        assert_eq!(fetched.id, "c1");
        assert_eq!(fetched.record_id, "r1");
    }

    #[test]
    fn store_get_nonexistent_returns_none() {
        let store = ConflictStore::new();
        assert!(store.get("nonexistent").is_none());
    }

    #[test]
    fn store_list_unresolved() {
        let store = ConflictStore::new();
        let now = Utc::now();

        for i in 0..5 {
            let conflict = Conflict {
                id: format!("c{i}"),
                collection: "tasks".into(),
                record_id: format!("r{i}"),
                local_version: make_record(&format!("r{i}"), "tasks", json!({}), 2, now),
                remote_version: make_record(&format!("r{i}"), "tasks", json!({}), 3, now),
                strategy: ResolutionStrategy::LastWriteWins,
                resolved: false,
                resolution: None,
                detected_at: now,
            };
            store.add(conflict);
        }

        let (conflicts, total) = store.list_unresolved(3, 0);
        assert_eq!(total, 5);
        assert_eq!(conflicts.len(), 3);

        let (conflicts, total) = store.list_unresolved(10, 3);
        assert_eq!(total, 5);
        assert_eq!(conflicts.len(), 2);
    }

    #[test]
    fn store_resolve_removes_from_unresolved() {
        let store = ConflictStore::new();
        let now = Utc::now();

        let conflict = Conflict {
            id: "c1".into(),
            collection: "tasks".into(),
            record_id: "r1".into(),
            local_version: make_record("r1", "tasks", json!({}), 2, now),
            remote_version: make_record("r1", "tasks", json!({}), 3, now),
            strategy: ResolutionStrategy::LastWriteWins,
            resolved: false,
            resolution: None,
            detected_at: now,
        };
        store.add(conflict);

        let resolved = store.resolve("c1", ConflictResolution::KeepRemote);
        assert!(resolved);

        // Should no longer appear in unresolved list.
        let (unresolved, total) = store.list_unresolved(10, 0);
        assert_eq!(total, 0);
        assert!(unresolved.is_empty());

        // But should still be retrievable.
        let fetched = store.get("c1").unwrap();
        assert!(fetched.resolved);
        assert_eq!(fetched.resolution, Some(ConflictResolution::KeepRemote));
    }

    #[test]
    fn store_resolve_nonexistent_returns_false() {
        let store = ConflictStore::new();
        assert!(!store.resolve("nonexistent", ConflictResolution::KeepLocal));
    }

    #[test]
    fn store_remove() {
        let store = ConflictStore::new();
        let now = Utc::now();

        let conflict = Conflict {
            id: "c1".into(),
            collection: "tasks".into(),
            record_id: "r1".into(),
            local_version: make_record("r1", "tasks", json!({}), 2, now),
            remote_version: make_record("r1", "tasks", json!({}), 3, now),
            strategy: ResolutionStrategy::LastWriteWins,
            resolved: false,
            resolution: None,
            detected_at: now,
        };
        store.add(conflict);

        assert!(store.remove("c1"));
        assert!(store.get("c1").is_none());
        assert!(!store.remove("c1")); // Already removed.
    }

    // ── Serialization tests ──

    #[test]
    fn conflict_serialization_roundtrip() {
        let now = Utc::now();
        let conflict = Conflict {
            id: "c1".into(),
            collection: "tasks".into(),
            record_id: "r1".into(),
            local_version: make_record("r1", "tasks", json!({"title": "Local"}), 2, now),
            remote_version: make_record("r1", "tasks", json!({"title": "Remote"}), 3, now),
            strategy: ResolutionStrategy::LastWriteWins,
            resolved: true,
            resolution: Some(ConflictResolution::KeepLocal),
            detected_at: now,
        };

        let json_str = serde_json::to_string(&conflict).unwrap();
        let restored: Conflict = serde_json::from_str(&json_str).unwrap();

        assert_eq!(restored.id, conflict.id);
        assert_eq!(restored.collection, conflict.collection);
        assert_eq!(restored.record_id, conflict.record_id);
        assert_eq!(restored.strategy, conflict.strategy);
        assert_eq!(restored.resolved, conflict.resolved);
        assert_eq!(restored.resolution, conflict.resolution);
    }

    #[test]
    fn resolution_strategy_serialization() {
        let lww = serde_json::to_string(&ResolutionStrategy::LastWriteWins).unwrap();
        assert_eq!(lww, "\"last_write_wins\"");

        let flm = serde_json::to_string(&ResolutionStrategy::FieldLevelMerge).unwrap();
        assert_eq!(flm, "\"field_level_merge\"");

        let mr = serde_json::to_string(&ResolutionStrategy::ManualResolution).unwrap();
        assert_eq!(mr, "\"manual_resolution\"");
    }

    // ── Edge case tests ──

    #[test]
    fn rapid_successive_edits_handled() {
        let now = Utc::now();
        let _t1 = now - Duration::milliseconds(100);
        let t2 = now - Duration::milliseconds(50);
        let t3 = now;

        // Simulate three rapid edits: base at t1, local at t2, remote at t3.
        let local = make_record("r1", "tasks", json!({"v": 2}), 3, t2);
        let remote = make_record("r1", "tasks", json!({"v": 3}), 4, t3);
        let base_version = 2;

        let conflict = detect_conflict(&local, &remote, base_version);
        assert!(conflict.is_some());

        let c = conflict.unwrap();
        // Remote is newer, so LWW should pick remote.
        let resolution = resolve_last_write_wins(&c);
        assert_eq!(resolution, ConflictResolution::KeepRemote);
    }

    #[test]
    fn concurrent_edit_simulation() {
        let now = Utc::now();
        // Both edits happen at nearly the same time, diverging from base v1.
        // When versions match (both at 2), detect_conflict returns None.
        // This is correct — version equality means the last write already landed.
        // Test the case where versions differ:
        let local2 = make_record(
            "r1",
            "contacts",
            json!({"name": "Alice", "phone": "local-phone"}),
            2,
            now,
        );
        let remote2 = make_record(
            "r1",
            "contacts",
            json!({"name": "Alice", "phone": "old-phone", "email": "new@b.com"}),
            3,
            now + Duration::milliseconds(1),
        );

        let conflict = detect_conflict(&local2, &remote2, 1);
        assert!(conflict.is_some());

        let c = conflict.unwrap();
        assert_eq!(c.strategy, ResolutionStrategy::FieldLevelMerge);

        // Field-level merge: local changed phone, remote added email.
        let base_data = json!({"name": "Alice", "phone": "old-phone"});
        let resolution = resolve_field_merge(&c, Some(&base_data));
        match &resolution {
            ConflictResolution::Merged { data } => {
                assert_eq!(data["name"], "Alice");
                assert_eq!(data["phone"], "local-phone");
                assert_eq!(data["email"], "new@b.com");
            }
            _ => panic!("expected Merged, got {resolution:?}"),
        }
    }

    #[test]
    fn conflict_store_default_impl() {
        let store = ConflictStore::default();
        let (list, total) = store.list_unresolved(10, 0);
        assert!(list.is_empty());
        assert_eq!(total, 0);
    }
}
