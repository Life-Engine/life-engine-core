//! Integration tests for `SqliteDocumentAdapter`.

use life_engine_storage_sqlite::document::SqliteDocumentAdapter;
use life_engine_traits::storage::{
    CollectionDescriptor, DocumentStorageAdapter, FieldDescriptor, FieldType, FilterNode,
    FilterOperator, HealthStatus, Pagination, QueryDescriptor, SortDirection, SortField,
    StorageError,
};
use rusqlite::Connection;
use serde_json::json;

fn setup_adapter() -> SqliteDocumentAdapter {
    let conn = Connection::open_in_memory().expect("open in-memory db");
    conn.execute_batch(life_engine_storage_sqlite::schema::PLUGIN_DATA_DDL)
        .expect("create plugin_data table");
    conn.execute_batch(life_engine_storage_sqlite::schema::SCHEMA_VERSIONS_DDL)
        .expect("create schema_versions table");
    SqliteDocumentAdapter::from_connection(conn, "test-plugin".into())
}

// ── CRUD tests ──────────────────────────────────────────────────────

#[tokio::test]
async fn create_and_get() {
    let adapter = setup_adapter();
    let doc = json!({"title": "Hello", "priority": 1});
    let created = adapter.create("events", doc).await.unwrap();

    assert!(created.get("id").is_some());
    assert!(created.get("created_at").is_some());
    assert!(created.get("updated_at").is_some());
    assert_eq!(created["title"], "Hello");

    let id = created["id"].as_str().unwrap();
    let fetched = adapter.get("events", id).await.unwrap();
    assert_eq!(fetched["title"], "Hello");
    assert_eq!(fetched["priority"], 1);
}

#[tokio::test]
async fn create_with_explicit_id() {
    let adapter = setup_adapter();
    let doc = json!({"id": "my-id-123", "name": "test"});
    let created = adapter.create("events", doc).await.unwrap();
    assert_eq!(created["id"], "my-id-123");

    let fetched = adapter.get("events", "my-id-123").await.unwrap();
    assert_eq!(fetched["name"], "test");
}

#[tokio::test]
async fn create_duplicate_id_fails() {
    let adapter = setup_adapter();
    let doc = json!({"id": "dup-1", "name": "first"});
    adapter.create("events", doc).await.unwrap();

    let doc2 = json!({"id": "dup-1", "name": "second"});
    let result = adapter.create("events", doc2).await;
    assert!(matches!(result, Err(StorageError::AlreadyExists { .. })));
}

#[tokio::test]
async fn get_not_found() {
    let adapter = setup_adapter();
    let result = adapter.get("events", "nonexistent").await;
    assert!(matches!(result, Err(StorageError::NotFound { .. })));
}

#[tokio::test]
async fn update_replaces_document() {
    let adapter = setup_adapter();
    let doc = json!({"id": "u1", "name": "original", "extra": true});
    adapter.create("events", doc).await.unwrap();

    let updated = adapter
        .update("events", "u1", json!({"name": "updated"}))
        .await
        .unwrap();
    assert_eq!(updated["name"], "updated");
    assert!(updated.get("extra").is_none());
    assert_eq!(updated["id"], "u1");
    assert!(updated.get("created_at").is_some());
}

#[tokio::test]
async fn update_not_found() {
    let adapter = setup_adapter();
    let result = adapter
        .update("events", "missing", json!({"name": "x"}))
        .await;
    assert!(matches!(result, Err(StorageError::NotFound { .. })));
}

#[tokio::test]
async fn partial_update_merges() {
    let adapter = setup_adapter();
    let doc = json!({"id": "p1", "name": "original", "count": 1, "nested": {"a": 1, "b": 2}});
    adapter.create("events", doc).await.unwrap();

    let patched = adapter
        .partial_update(
            "events",
            "p1",
            json!({"count": 42, "nested": {"b": null, "c": 3}}),
        )
        .await
        .unwrap();
    assert_eq!(patched["name"], "original");
    assert_eq!(patched["count"], 42);
    assert_eq!(patched["nested"]["a"], 1);
    assert!(patched["nested"].get("b").is_none());
    assert_eq!(patched["nested"]["c"], 3);
}

#[tokio::test]
async fn partial_update_not_found() {
    let adapter = setup_adapter();
    let result = adapter
        .partial_update("events", "missing", json!({"x": 1}))
        .await;
    assert!(matches!(result, Err(StorageError::NotFound { .. })));
}

#[tokio::test]
async fn delete_removes_document() {
    let adapter = setup_adapter();
    let doc = json!({"id": "d1", "name": "doomed"});
    adapter.create("events", doc).await.unwrap();

    adapter.delete("events", "d1").await.unwrap();
    let result = adapter.get("events", "d1").await;
    assert!(matches!(result, Err(StorageError::NotFound { .. })));
}

#[tokio::test]
async fn delete_not_found() {
    let adapter = setup_adapter();
    let result = adapter.delete("events", "missing").await;
    assert!(matches!(result, Err(StorageError::NotFound { .. })));
}

// ── Query tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn query_all() {
    let adapter = setup_adapter();
    for i in 0..5 {
        adapter
            .create("tasks", json!({"id": format!("t{i}"), "priority": i}))
            .await
            .unwrap();
    }

    let result = adapter
        .query(QueryDescriptor {
            collection: "tasks".into(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(result.total_count, 5);
    assert_eq!(result.documents.len(), 5);
    assert!(result.next_cursor.is_none());
}

#[tokio::test]
async fn query_with_filter() {
    let adapter = setup_adapter();
    for i in 0..10 {
        adapter
            .create(
                "tasks",
                json!({"id": format!("t{i}"), "priority": i, "status": if i < 5 { "open" } else { "closed" }}),
            )
            .await
            .unwrap();
    }

    let result = adapter
        .query(QueryDescriptor {
            collection: "tasks".into(),
            filter: Some(FilterNode::Comparison {
                field: "status".into(),
                operator: FilterOperator::Eq,
                value: json!("open"),
            }),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(result.total_count, 5);
    assert_eq!(result.documents.len(), 5);
}

#[tokio::test]
async fn query_with_sort() {
    let adapter = setup_adapter();
    adapter
        .create("tasks", json!({"id": "a", "priority": 3}))
        .await
        .unwrap();
    adapter
        .create("tasks", json!({"id": "b", "priority": 1}))
        .await
        .unwrap();
    adapter
        .create("tasks", json!({"id": "c", "priority": 2}))
        .await
        .unwrap();

    let result = adapter
        .query(QueryDescriptor {
            collection: "tasks".into(),
            sort: vec![SortField {
                field: "priority".into(),
                direction: SortDirection::Asc,
            }],
            ..Default::default()
        })
        .await
        .unwrap();

    let ids: Vec<&str> = result
        .documents
        .iter()
        .map(|d| d["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["b", "c", "a"]);
}

#[tokio::test]
async fn query_with_pagination() {
    let adapter = setup_adapter();
    for i in 0..10 {
        adapter
            .create("tasks", json!({"id": format!("t{i:02}"), "n": i}))
            .await
            .unwrap();
    }

    let page1 = adapter
        .query(QueryDescriptor {
            collection: "tasks".into(),
            pagination: Pagination {
                limit: 3,
                cursor: None,
            },
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(page1.documents.len(), 3);
    assert_eq!(page1.total_count, 10);
    assert!(page1.next_cursor.is_some());

    let page2 = adapter
        .query(QueryDescriptor {
            collection: "tasks".into(),
            pagination: Pagination {
                limit: 3,
                cursor: page1.next_cursor,
            },
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(page2.documents.len(), 3);
    assert!(page2.next_cursor.is_some());
}

#[tokio::test]
async fn query_with_field_projection() {
    let adapter = setup_adapter();
    adapter
        .create(
            "tasks",
            json!({"id": "fp1", "name": "test", "secret": "hidden"}),
        )
        .await
        .unwrap();

    let result = adapter
        .query(QueryDescriptor {
            collection: "tasks".into(),
            fields: Some(vec!["id".into(), "name".into()]),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(result.documents.len(), 1);
    let doc = &result.documents[0];
    assert_eq!(doc["id"], "fp1");
    assert_eq!(doc["name"], "test");
    assert!(doc.get("secret").is_none());
}

#[tokio::test]
async fn query_and_filter() {
    let adapter = setup_adapter();
    adapter
        .create("tasks", json!({"id": "x1", "a": 1, "b": 2}))
        .await
        .unwrap();
    adapter
        .create("tasks", json!({"id": "x2", "a": 1, "b": 3}))
        .await
        .unwrap();
    adapter
        .create("tasks", json!({"id": "x3", "a": 2, "b": 2}))
        .await
        .unwrap();

    let result = adapter
        .query(QueryDescriptor {
            collection: "tasks".into(),
            filter: Some(FilterNode::And(vec![
                FilterNode::Comparison {
                    field: "a".into(),
                    operator: FilterOperator::Eq,
                    value: json!(1),
                },
                FilterNode::Comparison {
                    field: "b".into(),
                    operator: FilterOperator::Gt,
                    value: json!(2),
                },
            ])),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(result.total_count, 1);
    assert_eq!(result.documents[0]["id"], "x2");
}

#[tokio::test]
async fn query_or_filter() {
    let adapter = setup_adapter();
    adapter
        .create("tasks", json!({"id": "o1", "status": "open"}))
        .await
        .unwrap();
    adapter
        .create("tasks", json!({"id": "o2", "status": "closed"}))
        .await
        .unwrap();
    adapter
        .create("tasks", json!({"id": "o3", "status": "draft"}))
        .await
        .unwrap();

    let result = adapter
        .query(QueryDescriptor {
            collection: "tasks".into(),
            filter: Some(FilterNode::Or(vec![
                FilterNode::Comparison {
                    field: "status".into(),
                    operator: FilterOperator::Eq,
                    value: json!("open"),
                },
                FilterNode::Comparison {
                    field: "status".into(),
                    operator: FilterOperator::Eq,
                    value: json!("draft"),
                },
            ])),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(result.total_count, 2);
}

#[tokio::test]
async fn query_contains_filter() {
    let adapter = setup_adapter();
    adapter
        .create("tasks", json!({"id": "c1", "name": "hello world"}))
        .await
        .unwrap();
    adapter
        .create("tasks", json!({"id": "c2", "name": "goodbye"}))
        .await
        .unwrap();

    let result = adapter
        .query(QueryDescriptor {
            collection: "tasks".into(),
            filter: Some(FilterNode::Comparison {
                field: "name".into(),
                operator: FilterOperator::Contains,
                value: json!("hello"),
            }),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(result.total_count, 1);
    assert_eq!(result.documents[0]["id"], "c1");
}

#[tokio::test]
async fn query_in_filter() {
    let adapter = setup_adapter();
    adapter
        .create("tasks", json!({"id": "i1", "priority": 1}))
        .await
        .unwrap();
    adapter
        .create("tasks", json!({"id": "i2", "priority": 2}))
        .await
        .unwrap();
    adapter
        .create("tasks", json!({"id": "i3", "priority": 3}))
        .await
        .unwrap();

    let result = adapter
        .query(QueryDescriptor {
            collection: "tasks".into(),
            filter: Some(FilterNode::Comparison {
                field: "priority".into(),
                operator: FilterOperator::In,
                value: json!([1, 3]),
            }),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(result.total_count, 2);
}

// ── Count tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn count_all() {
    let adapter = setup_adapter();
    for i in 0..7 {
        adapter
            .create("notes", json!({"id": format!("n{i}")}))
            .await
            .unwrap();
    }

    let count = adapter.count("notes", None).await.unwrap();
    assert_eq!(count, 7);
}

#[tokio::test]
async fn count_with_filter() {
    let adapter = setup_adapter();
    for i in 0..10 {
        adapter
            .create(
                "notes",
                json!({"id": format!("n{i}"), "active": i % 2 == 0}),
            )
            .await
            .unwrap();
    }

    let count = adapter
        .count(
            "notes",
            Some(FilterNode::Comparison {
                field: "active".into(),
                operator: FilterOperator::Eq,
                value: json!(true),
            }),
        )
        .await
        .unwrap();
    assert_eq!(count, 5);
}

// ── Batch tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn batch_create_all_or_nothing() {
    let adapter = setup_adapter();
    let docs = vec![
        json!({"id": "b1", "name": "first"}),
        json!({"id": "b2", "name": "second"}),
        json!({"id": "b3", "name": "third"}),
    ];
    let results = adapter.batch_create("events", docs).await.unwrap();
    assert_eq!(results.len(), 3);

    let count = adapter.count("events", None).await.unwrap();
    assert_eq!(count, 3);
}

#[tokio::test]
async fn batch_create_rolls_back_on_duplicate() {
    let adapter = setup_adapter();
    adapter
        .create("events", json!({"id": "existing"}))
        .await
        .unwrap();

    let docs = vec![
        json!({"id": "new1", "name": "ok"}),
        json!({"id": "existing", "name": "dup"}),
    ];
    let result = adapter.batch_create("events", docs).await;
    assert!(result.is_err());

    // Only the original document should exist.
    let count = adapter.count("events", None).await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn batch_update() {
    let adapter = setup_adapter();
    adapter
        .create("events", json!({"id": "bu1", "val": 1}))
        .await
        .unwrap();
    adapter
        .create("events", json!({"id": "bu2", "val": 2}))
        .await
        .unwrap();

    let updates = vec![
        ("bu1".into(), json!({"val": 10})),
        ("bu2".into(), json!({"val": 20})),
    ];
    let results = adapter.batch_update("events", updates).await.unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["val"], 10);
    assert_eq!(results[1]["val"], 20);
}

#[tokio::test]
async fn batch_delete() {
    let adapter = setup_adapter();
    for i in 0..5 {
        adapter
            .create("events", json!({"id": format!("bd{i}")}))
            .await
            .unwrap();
    }

    adapter
        .batch_delete("events", vec!["bd0".into(), "bd2".into(), "bd4".into()])
        .await
        .unwrap();

    let count = adapter.count("events", None).await.unwrap();
    assert_eq!(count, 2);
}

// ── Migration tests ─────────────────────────────────────────────────

#[tokio::test]
async fn migrate_idempotent() {
    let adapter = setup_adapter();
    let desc = CollectionDescriptor {
        name: "tasks".into(),
        plugin_id: "test-plugin".into(),
        fields: vec![FieldDescriptor {
            name: "title".into(),
            field_type: FieldType::String,
            required: true,
        }],
        indexes: vec!["title".into()],
    };

    adapter.migrate(desc.clone()).await.unwrap();
    adapter.migrate(desc).await.unwrap(); // Should not fail.
}

// ── Health check tests ──────────────────────────────────────────────

#[tokio::test]
async fn health_reports_status() {
    let adapter = setup_adapter();
    let report = adapter.health().await.unwrap();
    // In-memory databases don't support WAL, so the best we get is Degraded.
    assert!(
        report.status == HealthStatus::Healthy || report.status == HealthStatus::Degraded,
        "expected Healthy or Degraded, got {:?}",
        report.status
    );
    assert!(!report.checks.is_empty());
    // Connection check should always be healthy.
    let conn_check = report.checks.iter().find(|c| c.name == "connection").unwrap();
    assert_eq!(conn_check.status, HealthStatus::Healthy);
}

// ── Capabilities tests ──────────────────────────────────────────────

#[tokio::test]
async fn capabilities_reports_correct_flags() {
    let adapter = setup_adapter();
    let caps = adapter.capabilities();
    assert!(caps.indexing);
    assert!(caps.transactions);
    assert!(caps.batch_operations);
    assert!(caps.encryption);
    assert!(!caps.full_text_search);
    assert!(!caps.watch);
}

// ── Watch test ──────────────────────────────────────────────────────

#[tokio::test]
async fn watch_returns_empty_channel() {
    let adapter = setup_adapter();
    let mut rx = adapter.watch("events").await.unwrap();
    // Channel should be empty (sender dropped immediately).
    assert!(rx.try_recv().is_err());
}

// ── Collection scoping tests ────────────────────────────────────────

#[tokio::test]
async fn different_collections_are_isolated() {
    let adapter = setup_adapter();
    adapter
        .create("events", json!({"id": "e1", "name": "event"}))
        .await
        .unwrap();
    adapter
        .create("tasks", json!({"id": "t1", "name": "task"}))
        .await
        .unwrap();

    let event_count = adapter.count("events", None).await.unwrap();
    let task_count = adapter.count("tasks", None).await.unwrap();
    assert_eq!(event_count, 1);
    assert_eq!(task_count, 1);

    // Getting an event id from tasks collection should fail.
    let result = adapter.get("tasks", "e1").await;
    assert!(matches!(result, Err(StorageError::NotFound { .. })));
}

// ── Validation tests ────────────────────────────────────────────────

#[tokio::test]
async fn invalid_collection_name_rejected() {
    let adapter = setup_adapter();
    let result = adapter
        .create("foo;DROP TABLE", json!({"name": "bad"}))
        .await;
    assert!(matches!(
        result,
        Err(StorageError::ValidationFailed { .. })
    ));
}
