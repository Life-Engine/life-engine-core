//! Storage adapter trait for pluggable persistence backends.
//!
//! Re-exports from `life_engine_traits::legacy_storage` — the canonical
//! location after the architecture migration (WP 10.4). This module is
//! kept for backward compatibility; new code should import from
//! `life_engine_traits::legacy_storage` directly.

pub use life_engine_traits::legacy_storage::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::MockStorage;

    #[tokio::test]
    async fn create_and_get() {
        let storage = MockStorage::new();
        let data = serde_json::json!({"title": "Test"});
        let created = storage.create("plug1", "tasks", data.clone()).await.unwrap();
        assert_eq!(created.version, 1);
        assert_eq!(created.plugin_id, "plug1");
        assert_eq!(created.collection, "tasks");

        let fetched = storage
            .get("plug1", "tasks", &created.id)
            .await
            .unwrap()
            .expect("should find record");
        assert_eq!(fetched.data, data);
    }

    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let storage = MockStorage::new();
        let result = storage.get("plug1", "tasks", "nope").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn update_increments_version() {
        let storage = MockStorage::new();
        let created = storage
            .create("plug1", "tasks", serde_json::json!({"v": 1}))
            .await
            .unwrap();
        let updated = storage
            .update(
                "plug1",
                "tasks",
                &created.id,
                serde_json::json!({"v": 2}),
                1,
            )
            .await
            .unwrap();
        assert_eq!(updated.version, 2);
        assert_eq!(updated.data, serde_json::json!({"v": 2}));
    }

    #[tokio::test]
    async fn update_with_wrong_version_fails() {
        let storage = MockStorage::new();
        let created = storage
            .create("plug1", "tasks", serde_json::json!({"v": 1}))
            .await
            .unwrap();
        let result = storage
            .update(
                "plug1",
                "tasks",
                &created.id,
                serde_json::json!({"v": 2}),
                999,
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn delete_existing_record() {
        let storage = MockStorage::new();
        let created = storage
            .create("plug1", "tasks", serde_json::json!({}))
            .await
            .unwrap();
        let deleted = storage
            .delete("plug1", "tasks", &created.id)
            .await
            .unwrap();
        assert!(deleted);

        let after = storage.get("plug1", "tasks", &created.id).await.unwrap();
        assert!(after.is_none());
    }

    #[tokio::test]
    async fn delete_nonexistent_returns_false() {
        let storage = MockStorage::new();
        let deleted = storage.delete("plug1", "tasks", "nope").await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn list_returns_paginated() {
        let storage = MockStorage::new();
        for i in 0..5 {
            storage
                .create("plug1", "tasks", serde_json::json!({"i": i}))
                .await
                .unwrap();
        }
        let result = storage
            .list("plug1", "tasks", None, Pagination { limit: 2, offset: 0 })
            .await
            .unwrap();
        assert_eq!(result.total, 5);
        assert_eq!(result.records.len(), 2);
        assert_eq!(result.limit, 2);
    }

    #[tokio::test]
    async fn query_scoped_to_plugin_and_collection() {
        let storage = MockStorage::new();
        storage
            .create("plug1", "tasks", serde_json::json!({}))
            .await
            .unwrap();
        storage
            .create("plug2", "tasks", serde_json::json!({}))
            .await
            .unwrap();

        let result = storage
            .query(
                "plug1",
                "tasks",
                QueryFilters::default(),
                None,
                Pagination::default(),
            )
            .await
            .unwrap();
        assert_eq!(result.total, 1);
    }

    #[test]
    fn pagination_clamp() {
        let p = Pagination {
            limit: 5000,
            offset: 0,
        };
        let clamped = p.clamped();
        assert_eq!(clamped.limit, 1000);
    }

    #[test]
    fn pagination_default() {
        let p = Pagination::default();
        assert_eq!(p.limit, 50);
        assert_eq!(p.offset, 0);
    }

    #[test]
    fn record_serialization_roundtrip() {
        let now = chrono::Utc::now();
        let record = Record {
            id: "r1".into(),
            plugin_id: "plug1".into(),
            collection: "tasks".into(),
            data: serde_json::json!({"title": "Hello"}),
            version: 3,
            user_id: None,
            household_id: None,
            created_at: now,
            updated_at: now,
        };
        let json = serde_json::to_string(&record).unwrap();
        let restored: Record = serde_json::from_str(&json).unwrap();
        assert_eq!(record, restored);
    }

    #[test]
    fn sort_direction_values() {
        assert_ne!(SortDirection::Asc, SortDirection::Desc);
    }

    #[test]
    fn comparison_op_values() {
        assert_ne!(ComparisonOp::Gte, ComparisonOp::Lte);
        assert_ne!(ComparisonOp::Gt, ComparisonOp::Lt);
    }
}
