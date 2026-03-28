//! Plugin-to-plugin communication integration tests (WP 8.19).
//!
//! Validates the three communication paths between plugins:
//! 1. Workflow chaining — output of one plugin becomes input to the next
//! 2. Shared canonical collections — plugins read/write the same CDM data
//! 3. No direct plugin calls — there is no host function for cross-plugin invocation

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use life_engine_plugin_system::capability::ApprovedCapabilities;
use life_engine_plugin_system::execute::PluginSystemExecutor;
use life_engine_plugin_system::injection::injected_function_names;
use life_engine_plugin_system::lifecycle::LifecycleManager;
use life_engine_plugin_system::loader::PluginHandle;
use life_engine_plugin_system::manifest::{ActionDef, CapabilitySet, EventsDef, PluginManifest, PluginMeta, TrustLevel, DEFAULT_TIMEOUT_MS};
use life_engine_plugin_system::runtime::load_plugin_from_bytes;
use life_engine_traits::{Capability, EngineError, StorageBackend};
use life_engine_types::{
    CdmType, Contact, ContactEmail, ContactInfoType, ContactName, MessageMetadata, PipelineMessage,
    StorageMutation, StorageQuery, TypedPayload,
};
use life_engine_workflow_engine::executor::PipelineExecutor;
use life_engine_workflow_engine::types::{
    ExecutionMode, StepDef, TriggerDef, ValidationLevel, WorkflowDef,
};
use life_engine_workflow_engine::PluginExecutor;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// WASM fixture
// ---------------------------------------------------------------------------

/// A minimal WASM module that echoes input back as output.
/// Used for both plugin-a and plugin-b since we test chaining at the
/// executor/workflow level — the WASM just passes data through.
fn echo_wasm_module() -> Vec<u8> {
    wat::parse_str(
        r#"
        (module
            (import "extism:host/env" "input_length" (func $input_length (result i64)))
            (import "extism:host/env" "input_load_u8" (func $input_load_u8 (param i64) (result i32)))
            (import "extism:host/env" "output_set" (func $output_set (param i64 i64)))
            (import "extism:host/env" "length" (func $length (param i64) (result i64)))
            (import "extism:host/env" "alloc" (func $alloc (param i64) (result i64)))
            (import "extism:host/env" "store_u8" (func $store_u8 (param i64 i32)))

            (memory (export "memory") 1)

            (func (export "write-contact") (result i32)
                (local $len i64)
                (local $offset i64)
                (local $i i64)
                (local $byte i32)
                (local.set $len (call $input_length))
                (local.set $offset (call $alloc (local.get $len)))
                (local.set $i (i64.const 0))
                (block $break
                    (loop $loop
                        (br_if $break (i64.ge_u (local.get $i) (local.get $len)))
                        (local.set $byte (call $input_load_u8 (local.get $i)))
                        (call $store_u8
                            (i64.add (local.get $offset) (local.get $i))
                            (local.get $byte)
                        )
                        (local.set $i (i64.add (local.get $i) (i64.const 1)))
                        (br $loop)
                    )
                )
                (call $output_set (local.get $offset) (local.get $len))
                (i32.const 0)
            )

            (func (export "read-and-note") (result i32)
                (local $len i64)
                (local $offset i64)
                (local $i i64)
                (local $byte i32)
                (local.set $len (call $input_length))
                (local.set $offset (call $alloc (local.get $len)))
                (local.set $i (i64.const 0))
                (block $break
                    (loop $loop
                        (br_if $break (i64.ge_u (local.get $i) (local.get $len)))
                        (local.set $byte (call $input_load_u8 (local.get $i)))
                        (call $store_u8
                            (i64.add (local.get $offset) (local.get $i))
                            (local.get $byte)
                        )
                        (local.set $i (i64.add (local.get $i) (i64.const 1)))
                        (br $loop)
                    )
                )
                (call $output_set (local.get $offset) (local.get $len))
                (i32.const 0)
            )
        )
        "#,
    )
    .expect("failed to compile echo WAT to WASM")
}

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

/// Mock storage that records all read/write operations and can return
/// pre-configured results. Used to verify shared collection access.
struct RecordingStorage {
    /// Records of execute calls: (plugin_id, collection).
    read_calls: Mutex<Vec<(String, String)>>,
    /// Records of mutate calls: (plugin_id, collection, mutation_type).
    write_calls: Mutex<Vec<(String, String, String)>>,
    /// What to return from execute (storage read).
    read_results: Mutex<Vec<PipelineMessage>>,
}

impl RecordingStorage {
    fn new() -> Self {
        Self {
            read_calls: Mutex::new(vec![]),
            write_calls: Mutex::new(vec![]),
            read_results: Mutex::new(vec![]),
        }
    }

    fn with_read_results(results: Vec<PipelineMessage>) -> Self {
        Self {
            read_calls: Mutex::new(vec![]),
            write_calls: Mutex::new(vec![]),
            read_results: Mutex::new(results),
        }
    }
}

#[async_trait]
impl StorageBackend for RecordingStorage {
    async fn execute(
        &self,
        query: StorageQuery,
    ) -> Result<Vec<PipelineMessage>, Box<dyn EngineError>> {
        self.read_calls
            .lock()
            .unwrap()
            .push((query.plugin_id.clone(), query.collection.clone()));
        Ok(self.read_results.lock().unwrap().clone())
    }

    async fn mutate(&self, op: StorageMutation) -> Result<(), Box<dyn EngineError>> {
        let (plugin_id, collection, op_type) = match &op {
            StorageMutation::Insert {
                plugin_id,
                collection,
                ..
            } => (plugin_id.clone(), collection.clone(), "insert"),
            StorageMutation::Update {
                plugin_id,
                collection,
                ..
            } => (plugin_id.clone(), collection.clone(), "update"),
            StorageMutation::Delete {
                plugin_id,
                collection,
                ..
            } => (plugin_id.clone(), collection.clone(), "delete"),
        };
        self.write_calls
            .lock()
            .unwrap()
            .push((plugin_id, collection, op_type.to_string()));
        Ok(())
    }

    async fn init(
        _config: toml::Value,
        _key: [u8; 32],
    ) -> Result<Self, Box<dyn EngineError>> {
        Ok(RecordingStorage::new())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn test_manifest(id: &str, actions: Vec<&str>) -> PluginManifest {
    let mut action_map = HashMap::new();
    for action in actions {
        action_map.insert(
            action.to_string(),
            ActionDef {
                description: format!("{action} action"),
                timeout_ms: DEFAULT_TIMEOUT_MS,
                input_schema: None,
                output_schema: None,
            },
        );
    }
    PluginManifest {
        plugin: PluginMeta {
            id: id.to_string(),
            name: format!("Test Plugin {id}"),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            license: None,
            trust: TrustLevel::ThirdParty,
        },
        actions: action_map,
        capabilities: CapabilitySet::default(),
        collections: HashMap::new(),
        events: EventsDef::default(),
        config: None,
    }
}

fn test_handle(id: &str, actions: Vec<&str>) -> PluginHandle {
    let wasm = echo_wasm_module();
    let instance = load_plugin_from_bytes(&wasm, id, vec![]).unwrap();
    PluginHandle {
        instance,
        manifest: test_manifest(id, actions),
        capabilities: ApprovedCapabilities::new(HashSet::new()),
    }
}

fn running_lifecycle(plugin_ids: &[&str]) -> LifecycleManager {
    let mut mgr = LifecycleManager::new();
    for id in plugin_ids {
        mgr.register(id);
    }
    mgr.start_all();
    mgr
}

fn make_contact_message(source: &str) -> PipelineMessage {
    PipelineMessage {
        metadata: MessageMetadata {
            correlation_id: Uuid::new_v4(),
            source: source.to_string(),
            timestamp: Utc::now(),
            auth_context: None,
            warnings: vec![],
        },
        payload: TypedPayload::Cdm(Box::new(CdmType::Contact(Contact {
            id: Uuid::new_v4(),
            name: ContactName {
                given: "Alice".to_string(),
                family: "Smith".to_string(),
                prefix: None,
                suffix: None,
                middle: None,
            },
            emails: vec![ContactEmail {
                address: "alice@example.com".to_string(),
                email_type: Some(ContactInfoType::Work),
                primary: Some(true),
            }],
            phones: vec![],
            addresses: vec![],
            organization: None,
            title: None,
            birthday: None,
            photo_url: None,
            notes: None,
            groups: vec![],
            source: source.to_string(),
            source_id: "c-1".to_string(),
            extensions: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }))),
    }
}

// ===========================================================================
// Test 1: Workflow chaining — plugin-a output becomes plugin-b input
// ===========================================================================

#[tokio::test]
async fn workflow_chaining_passes_output_as_next_input() {
    // Set up two echo plugins: plugin-a with "write-contact", plugin-b with "read-and-note"
    let handle_a = test_handle("plugin-a", vec!["write-contact"]);
    let handle_b = test_handle("plugin-b", vec!["read-and-note"]);
    let lifecycle = running_lifecycle(&["plugin-a", "plugin-b"]);

    let executor = PluginSystemExecutor::new(vec![handle_a, handle_b], lifecycle);
    let pipeline = PipelineExecutor::new(Arc::new(executor));

    // Define a two-step workflow: plugin-a:write-contact → plugin-b:read-and-note
    let workflow = WorkflowDef {
        id: "contact-pipeline".to_string(),
        name: "Contact Pipeline".to_string(),
        description: None,
        mode: ExecutionMode::Sync,
        validate: ValidationLevel::None,
        trigger: TriggerDef {
            endpoint: Some("POST /contacts".to_string()),
            event: None,
            schedule: None,
        },
        steps: vec![
            StepDef {
                plugin: "plugin-a".to_string(),
                action: "write-contact".to_string(),
                on_error: None,
                condition: None,
            },
            StepDef {
                plugin: "plugin-b".to_string(),
                action: "read-and-note".to_string(),
                on_error: None,
                condition: None,
            },
        ],
    };

    let input = make_contact_message("test:workflow-chaining");
    let correlation_id = input.metadata.correlation_id;

    let result = pipeline.execute_workflow(&workflow, input.clone()).await;
    assert!(result.is_ok(), "workflow should succeed: {result:?}");

    let output = result.unwrap();

    // The echo WASM returns input unchanged, so after two echo steps the
    // final output should still carry the same correlation_id and source
    // (proving data flowed from step 1's output into step 2's input).
    assert_eq!(
        output.metadata.correlation_id, correlation_id,
        "correlation_id must be preserved through the chain"
    );
    assert_eq!(
        output.metadata.source, "test:workflow-chaining",
        "source should be preserved (echo plugins pass through)"
    );

    // Verify the payload survived the chain
    match &output.payload {
        TypedPayload::Cdm(cdm) => match cdm.as_ref() {
            CdmType::Contact(contact) => {
                assert_eq!(contact.name.given, "Alice");
                assert_eq!(contact.name.family, "Smith");
                assert_eq!(contact.emails[0].address, "alice@example.com");
            }
            other => panic!("expected Contact, got {other:?}"),
        },
        TypedPayload::Custom(_) => panic!("expected Cdm payload, got Custom"),
    }
}

// ===========================================================================
// Test 2: Shared canonical collection — both plugins access same data
// ===========================================================================

#[tokio::test]
async fn shared_canonical_collection_visible_to_both_plugins() {
    // This test validates the storage layer concept: when plugin-a writes
    // a contact and plugin-b reads contacts, they share the same canonical
    // collection through the shared StorageBackend.

    use life_engine_plugin_system::host_functions::storage::{
        host_storage_read, host_storage_write, StorageHostContext,
    };

    let contact_msg = make_contact_message("plugin-a:write");

    // Create a storage backend that will return the contact after plugin-a writes
    let storage = Arc::new(RecordingStorage::with_read_results(vec![
        contact_msg.clone(),
    ]));

    // --- Plugin A writes a contact ---
    let ctx_a = StorageHostContext {
        plugin_id: "plugin-a".to_string(),
        capabilities: ApprovedCapabilities::new(
            [Capability::StorageRead, Capability::StorageWrite]
                .iter()
                .copied()
                .collect(),
        ),
        storage: storage.clone(),
    };

    let write_mutation = StorageMutation::Insert {
        plugin_id: "plugin-a".to_string(),
        collection: "contacts".to_string(),
        data: contact_msg.clone(),
    };
    let write_input = serde_json::to_vec(&write_mutation).unwrap();
    let write_result = host_storage_write(&ctx_a, &write_input).await;
    assert!(write_result.is_ok(), "plugin-a write should succeed");

    // Verify the write was scoped to plugin-a
    let write_calls = storage.write_calls.lock().unwrap();
    assert_eq!(write_calls.len(), 1);
    assert_eq!(write_calls[0].0, "plugin-a", "write scoped to plugin-a");
    assert_eq!(write_calls[0].1, "contacts");
    drop(write_calls);

    // --- Plugin B reads the contacts collection ---
    let ctx_b = StorageHostContext {
        plugin_id: "plugin-b".to_string(),
        capabilities: ApprovedCapabilities::new(
            [Capability::StorageRead].iter().copied().collect(),
        ),
        storage: storage.clone(),
    };

    let read_query = StorageQuery {
        collection: "contacts".to_string(),
        plugin_id: "plugin-b".to_string(),
        filters: vec![],
        sort: vec![],
        limit: None,
        offset: None,
    };
    let read_input = serde_json::to_vec(&read_query).unwrap();
    let read_result = host_storage_read(&ctx_b, &read_input).await;
    assert!(read_result.is_ok(), "plugin-b read should succeed");

    // Verify plugin-b can see the data (from shared canonical collection)
    let read_output: Vec<PipelineMessage> =
        serde_json::from_slice(&read_result.unwrap()).unwrap();
    assert_eq!(read_output.len(), 1, "plugin-b should see the contact");

    match &read_output[0].payload {
        TypedPayload::Cdm(cdm) => match cdm.as_ref() {
            CdmType::Contact(contact) => {
                assert_eq!(contact.name.given, "Alice");
                assert_eq!(contact.name.family, "Smith");
            }
            other => panic!("expected Contact, got {other:?}"),
        },
        _ => panic!("expected Cdm payload"),
    }

    // Verify the read was scoped to plugin-b
    let read_calls = storage.read_calls.lock().unwrap();
    assert_eq!(read_calls.len(), 1);
    assert_eq!(read_calls[0].0, "plugin-b", "read scoped to plugin-b");
    assert_eq!(read_calls[0].1, "contacts");
}

// ===========================================================================
// Test 3: No direct plugin-to-plugin calls
// ===========================================================================

#[test]
fn no_host_function_exists_for_direct_plugin_calls() {
    // Verify that there is no host function named anything like
    // "call_plugin", "invoke_plugin", or "plugin_call" in the injection
    // system. Communication between plugins is ONLY through workflow
    // chaining or shared canonical collections.

    // Test with all capabilities — even a fully-privileged plugin has no
    // way to directly call another plugin.
    let all_caps = ApprovedCapabilities::new(
        [
            Capability::StorageRead,
            Capability::StorageWrite,
            Capability::HttpOutbound,
            Capability::EventsEmit,
            Capability::EventsSubscribe,
            Capability::ConfigRead,
        ]
        .iter()
        .copied()
        .collect(),
    );

    let fn_names = injected_function_names(&all_caps);

    // The complete set of host functions is:
    // host_log, host_storage_read, host_storage_write, host_http_request,
    // host_events_emit, host_events_subscribe, host_config_read
    assert_eq!(
        fn_names.len(),
        7,
        "exactly 7 host functions exist (6 capability + host_log)"
    );

    // Verify none of them allow direct plugin invocation
    for name in &fn_names {
        assert!(
            !name.contains("call_plugin"),
            "no call_plugin host function should exist"
        );
        assert!(
            !name.contains("invoke_plugin"),
            "no invoke_plugin host function should exist"
        );
        assert!(
            !name.contains("plugin_call"),
            "no plugin_call host function should exist"
        );
        assert!(
            !name.contains("plugin_invoke"),
            "no plugin_invoke host function should exist"
        );
    }

    // Verify the exhaustive list matches expectations
    let expected = vec![
        "host_log",
        "host_storage_read",
        "host_storage_write",
        "host_http_request",
        "host_events_emit",
        "host_events_subscribe",
        "host_config_read",
    ];
    for expected_fn in &expected {
        assert!(
            fn_names.contains(expected_fn),
            "expected function {expected_fn} not found"
        );
    }
}

#[tokio::test]
async fn plugin_cannot_execute_another_plugin_through_executor() {
    // Even if a plugin knew another plugin's ID, the PluginSystemExecutor
    // requires the caller to go through the workflow engine. A plugin
    // cannot call executor.execute() for a different plugin from within
    // its own WASM sandbox — it can only be called by the workflow engine.
    //
    // This test verifies that the executor properly isolates plugin
    // execution: calling plugin-a's action only runs plugin-a, not plugin-b.

    let handle_a = test_handle("plugin-a", vec!["write-contact"]);
    let handle_b = test_handle("plugin-b", vec!["read-and-note"]);
    let lifecycle = running_lifecycle(&["plugin-a", "plugin-b"]);

    let executor = PluginSystemExecutor::new(vec![handle_a, handle_b], lifecycle);
    let input = make_contact_message("test:isolation");

    // Execute plugin-a's action
    let result = executor
        .execute("plugin-a", "write-contact", input.clone())
        .await;
    assert!(result.is_ok());

    // plugin-a cannot call plugin-b's action — that would require a
    // separate executor.execute() call, which only the workflow engine makes
    let cross_call = executor
        .execute("plugin-a", "read-and-note", input.clone())
        .await;
    assert!(
        cross_call.is_err(),
        "plugin-a should not have access to plugin-b's actions"
    );
    let err = cross_call.unwrap_err();
    assert!(
        err.to_string().contains("unknown action"),
        "should report unknown action: {err}"
    );
}

// ===========================================================================
// Test 4: Storage plugin_id scoping prevents cross-plugin data spoofing
// ===========================================================================

#[tokio::test]
async fn storage_scoping_prevents_plugin_impersonation() {
    // Even if a plugin tries to set another plugin's ID in a storage
    // operation, the host function overwrites it with the actual caller's ID.

    use life_engine_plugin_system::host_functions::storage::{
        host_storage_read, host_storage_write, StorageHostContext,
    };

    let storage = Arc::new(RecordingStorage::new());

    let ctx = StorageHostContext {
        plugin_id: "honest-plugin".to_string(),
        capabilities: ApprovedCapabilities::new(
            [Capability::StorageRead, Capability::StorageWrite]
                .iter()
                .copied()
                .collect(),
        ),
        storage: storage.clone(),
    };

    // Try to write as "other-plugin" — should be overwritten
    let mutation = StorageMutation::Insert {
        plugin_id: "other-plugin".to_string(),
        collection: "contacts".to_string(),
        data: make_contact_message("spoofed"),
    };
    let input = serde_json::to_vec(&mutation).unwrap();
    let _ = host_storage_write(&ctx, &input).await;

    let write_calls = storage.write_calls.lock().unwrap();
    assert_eq!(
        write_calls[0].0, "honest-plugin",
        "plugin_id must be scoped to the actual caller, not the spoofed ID"
    );
    drop(write_calls);

    // Try to read as "other-plugin" — should be overwritten
    let query = StorageQuery {
        collection: "contacts".to_string(),
        plugin_id: "other-plugin".to_string(),
        filters: vec![],
        sort: vec![],
        limit: None,
        offset: None,
    };
    let input = serde_json::to_vec(&query).unwrap();
    let _ = host_storage_read(&ctx, &input).await;

    let read_calls = storage.read_calls.lock().unwrap();
    assert_eq!(
        read_calls[0].0, "honest-plugin",
        "plugin_id must be scoped to the actual caller for reads too"
    );
}
