<!--
project: life-engine-core
phase: 8
specs: plugin-system, capability-enforcement
updated: 2026-03-23
-->

# Phase 8 — Plugin System and Capability Enforcement

## Plan Overview

This phase implements the Core plugin system and capability enforcement: directory-based discovery, manifest parsing, WASM loading via Extism, the six host functions (storage, config, events, HTTP, logging), lifecycle management, the execution bridge between the workflow engine and plugin WASM modules, and the two-layer capability enforcement (injection gating + runtime checks). This is where plugins become real — WASM modules loaded at runtime, sandboxed, with deny-by-default capabilities.

This phase depends on Phase 3 (traits, capabilities), Phase 4 (plugin SDK), Phase 5 (storage), Phase 6 (auth), and Phase 7 (workflow engine). Phase 9 (Core startup) wires the plugin system into the startup sequence.

> spec: .odm/spec/plugin-system/brief.md, .odm/spec/capability-enforcement/brief.md

Progress: 3 / 22 work packages complete

---

## 8.1 — Plugin Directory Scanner
> spec: .odm/spec/plugin-system/brief.md

- [x] Implement directory scanner that discovers plugin subdirectories
  <!-- file: packages/plugin-system/src/discovery.rs -->
  <!-- purpose: Implement pub fn scan_plugins_directory(path: &Path) -> Result<Vec<DiscoveredPlugin>, PluginError>. Logic: (1) verify the plugins directory exists, (2) iterate subdirectories, (3) for each subdirectory, check if both plugin.wasm and manifest.toml exist, (4) if both present, create a DiscoveredPlugin { path, wasm_path, manifest_path }, (5) if one is missing, log a warning with the directory name and which file is missing, skip it, (6) if neither exists, silently skip (probably not a plugin directory). Define DiscoveredPlugin struct with path (PathBuf), wasm_path (PathBuf), manifest_path (PathBuf). Return the list sorted by directory name for deterministic loading order. Log info: "Discovered N plugins in {path}". -->
  <!-- requirements: from plugin-system spec 1.1, 1.2, 1.3 -->
  <!-- leverage: none -->

- [x] Add directory scanner tests
  <!-- file: packages/plugin-system/src/discovery.rs -->
  <!-- purpose: Create a temporary directory structure in tests. Test cases: (1) directory with plugin.wasm + manifest.toml is discovered, (2) directory with only manifest.toml is skipped with warning, (3) directory with only plugin.wasm is skipped with warning, (4) empty plugins directory returns empty Vec, (5) nested directories are not recursively scanned (only immediate children), (6) non-directory entries (files) in the plugins directory are ignored. -->
  <!-- requirements: from plugin-system spec 1.1, 1.2, 1.3 -->
  <!-- leverage: none -->

---

## 8.2 — Manifest Parser
> depends: 8.1
> spec: .odm/spec/plugin-system/brief.md

- [x] Implement manifest.toml parser with struct definitions
  <!-- file: packages/plugin-system/src/manifest.rs -->
  <!-- purpose: Define PluginManifest struct: plugin section (PluginMeta: id String, name String, version String, description Option<String>, author Option<String>), actions (HashMap<String, ActionDef>: name String, description String, input_schema Option<String>, output_schema Option<String>), capabilities (CapabilitySet: required Vec<Capability>), config (Option<ConfigSchema>: schema serde_json::Value — JSON Schema for plugin-specific config). Implement pub fn parse_manifest(path: &Path) -> Result<PluginManifest, PluginError>. Validation: [plugin] section must exist, id/name/version are required, id must match regex [a-z][a-z0-9-]* (lowercase with hyphens), version must be valid semver, actions section is optional (plugin with no actions is valid — it might only subscribe to events), capabilities section is optional (defaults to no capabilities). Use toml crate for parsing. -->
  <!-- requirements: from plugin-system spec 2.1, 2.2, 2.3, 2.4, 2.5 -->
  <!-- leverage: none -->

- [x] Add manifest parser tests
  <!-- file: packages/plugin-system/src/manifest.rs -->
  <!-- purpose: Test cases: (1) valid complete manifest parses correctly with all fields, (2) minimal manifest with only [plugin] section parses (empty actions, no capabilities), (3) missing [plugin] section returns error, (4) missing required field (id, name, version) returns error with field name, (5) invalid plugin ID format (uppercase, spaces) returns error, (6) invalid semver version returns error, (7) actions are correctly extracted with input/output schemas, (8) capabilities are parsed as Capability enum values, (9) unknown capability string returns error, (10) config schema is preserved as raw JSON. -->
  <!-- requirements: from plugin-system spec 2.1, 2.2, 2.3, 2.4, 2.5 -->
  <!-- leverage: none -->

---

## 8.3 — Capability Approval Policy
> depends: 8.2
> spec: .odm/spec/capability-enforcement/brief.md

- [x] Implement first-party detection and third-party approval checking
  <!-- file: packages/plugin-system/src/capability.rs -->
  <!-- purpose: Define pub fn check_capability_approval(manifest: &PluginManifest, plugin_path: &Path, plugins_dir: &Path, config: &PluginConfig) -> Result<ApprovedCapabilities, CapabilityViolation>. Logic: (1) determine if plugin is first-party by checking if plugin_path is a child of the monorepo's plugins/ directory (canonical_path comparison), (2) if first-party, auto-grant all declared capabilities — return ApprovedCapabilities with the full declared set, (3) if third-party, read [plugins.<id>].approved_capabilities from the config, (4) compare manifest's declared capabilities against approved list, (5) if any declared capability is not in the approved list, return CapabilityViolation with code "CAP_001" listing the unapproved capabilities, (6) if all declared capabilities are approved, return ApprovedCapabilities with the intersection. Define ApprovedCapabilities struct wrapping a HashSet<Capability> with a has(cap: Capability) -> bool method. -->
  <!-- requirements: from capability-enforcement spec 1.2, 1.3, 1.4, 1.5, 5.1, 5.2, 5.3, 5.4 -->
  <!-- leverage: Capability types from Phase 3 -->

- [x] Add capability approval tests
  <!-- file: packages/plugin-system/src/capability.rs -->
  <!-- purpose: Test cases: (1) first-party plugin in plugins/ directory auto-granted all capabilities, (2) third-party plugin with fully approved capabilities passes, (3) third-party plugin declaring storage:write but only approved for storage:read returns CAP_001 error, (4) third-party plugin with no config entry refuses to load, (5) plugin with empty approved_capabilities loads but gets no host functions, (6) Display/FromStr round-trip for all 6 capability variants. -->
  <!-- requirements: from capability-enforcement spec 1.2, 1.3, 5.2, 5.4 -->
  <!-- leverage: none -->

---

## 8.4 — Extism Runtime Setup
> spec: .odm/spec/plugin-system/brief.md

- [ ] Add extism dependency and create runtime wrapper
  <!-- file: packages/plugin-system/Cargo.toml -->
  <!-- file: packages/plugin-system/src/runtime.rs -->
  <!-- purpose: Add extism = "1" dependency to Cargo.toml. Define ExtismRuntime struct. Implement pub fn load_plugin(wasm_path: &Path, host_functions: Vec<HostFunction>) -> Result<PluginInstance, PluginError> that: (1) reads the WASM binary from disk, (2) creates an Extism Manifest with the WASM data, (3) configures memory limits (default 256 MB per plugin), (4) configures execution timeout (default 30 seconds per call), (5) registers the provided host functions, (6) creates and returns a PluginInstance wrapping the Extism Plugin. Define PluginInstance struct with methods: call(function_name: &str, input: &[u8]) -> Result<Vec<u8>, PluginError> that invokes a WASM export, id() -> &str returning the manifest's plugin ID. Handle loading errors: corrupt WASM binary, missing exports, memory allocation failures. -->
  <!-- requirements: from plugin-system spec 3.1, 3.2, 3.5 -->
  <!-- leverage: none -->

- [ ] Add runtime loading tests
  <!-- file: packages/plugin-system/src/runtime.rs -->
  <!-- purpose: Test cases: (1) valid WASM binary loads into isolated instance (use a minimal test WASM module), (2) corrupt binary data returns PluginError, (3) WASM module can call registered host functions, (4) execution timeout is enforced — a WASM module that loops forever is terminated after the configured timeout. Note: test WASM modules should be small precompiled binaries stored in test fixtures, or compiled from Rust during test setup. -->
  <!-- requirements: from plugin-system spec 3.1, 3.2, 3.4 -->
  <!-- leverage: none -->

---

## 8.5 — Host Function — Storage
> depends: 8.4, 8.3
> spec: .odm/spec/plugin-system/brief.md

- [ ] Implement storage read and write host functions
  <!-- file: packages/plugin-system/src/host_functions/storage.rs -->
  <!-- purpose: Implement host_storage_read host function: (1) deserialize the plugin's request from WASM memory (contains collection name, query filters), (2) check that the calling plugin has storage:read capability in its ApprovedCapabilities, (3) if not approved, return serialized CapabilityViolation error, (4) construct a StorageQuery with the plugin's ID and delegate to StorageBackend::execute(), (5) serialize the Vec<PipelineMessage> result back to WASM memory. Implement host_storage_write host function: (1) deserialize the mutation request (insert/update/delete), (2) check storage:write capability, (3) construct a StorageMutation with the plugin's ID and delegate to StorageBackend::mutate(), (4) serialize the result. Both functions are registered as Extism host functions with the plugin's context (plugin_id, capabilities, storage backend reference). -->
  <!-- requirements: from plugin-system spec 5.4, 6.2 -->
  <!-- leverage: StorageBackend from Phase 5 -->

- [ ] Add storage host function tests
  <!-- file: packages/plugin-system/src/host_functions/storage.rs -->
  <!-- purpose: Test cases: (1) read succeeds with storage:read capability, (2) write succeeds with storage:write capability, (3) read without storage:read returns CAP_002 error, (4) write without storage:write returns CAP_002 error, (5) plugin_id is correctly scoped in the StorageQuery, (6) query results are correctly serialized back to WASM format. Use mock StorageBackend for tests. -->
  <!-- requirements: from plugin-system spec 5.4, 5.9, 6.2 -->
  <!-- leverage: mock storage from Phase 4 -->

---

## 8.6 — Host Function — Config
> depends: 8.4, 8.3
> spec: .odm/spec/plugin-system/brief.md

- [ ] Implement config read host function
  <!-- file: packages/plugin-system/src/host_functions/config.rs -->
  <!-- purpose: Implement host_config_read host function: (1) check that the calling plugin has config:read capability, (2) if approved, look up the plugin's config section from the loaded config.toml [plugins.<id>] section, (3) serialize the config section as JSON and return to the WASM module, (4) if the plugin has no config section, return an empty JSON object {}. The host function only ever returns the calling plugin's own config — never another plugin's config or global config. -->
  <!-- requirements: from plugin-system spec 5.8, 6.3 -->
  <!-- leverage: none -->

- [ ] Add config host function tests
  <!-- file: packages/plugin-system/src/host_functions/config.rs -->
  <!-- purpose: Test cases: (1) read returns plugin-specific config section only, (2) without config:read capability returns CAP_002 error, (3) nonexistent config section returns empty {}, (4) config data preserves types (numbers, booleans, nested objects). -->
  <!-- requirements: from plugin-system spec 5.8, 5.9, 6.3 -->
  <!-- leverage: none -->

---

## 8.7 — Host Function — Events
> depends: 8.4, 8.3
> spec: .odm/spec/plugin-system/brief.md

- [ ] Implement events emit and subscribe host functions
  <!-- file: packages/plugin-system/src/host_functions/events.rs -->
  <!-- purpose: Implement host_events_emit: (1) check events:emit capability, (2) deserialize event name and payload from WASM memory, (3) emit the event via the workflow engine's EventBus, (4) return success. Implement host_events_subscribe: (1) check events:subscribe capability, (2) register the plugin as a listener for the specified event name, (3) when the event fires, the workflow engine will route it through any matching workflow that includes this plugin. Note: subscribe is declarative — it registers interest, but actual delivery happens through workflow triggers, not direct callbacks into WASM. -->
  <!-- requirements: from plugin-system spec 5.6, 5.7, 6.6 -->
  <!-- leverage: EventBus from Phase 7 -->

- [ ] Add events host function tests
  <!-- file: packages/plugin-system/src/host_functions/events.rs -->
  <!-- purpose: Test cases: (1) emit succeeds with events:emit capability, (2) subscribe succeeds with events:subscribe capability, (3) emit without events:emit returns CAP_002 error, (4) subscribe without events:subscribe returns CAP_002 error, (5) emitted event is received by EventBus. -->
  <!-- requirements: from plugin-system spec 5.6, 5.7, 5.9, 6.6 -->
  <!-- leverage: mock EventBus -->

---

## 8.8 — Host Function — HTTP
> depends: 8.4, 8.3
> spec: .odm/spec/plugin-system/brief.md

- [ ] Implement HTTP outbound request host function
  <!-- file: packages/plugin-system/src/host_functions/http.rs -->
  <!-- purpose: Implement host_http_request: (1) check http:outbound capability, (2) deserialize HTTP request from WASM memory (method, URL, headers, body), (3) execute the outbound HTTP request using reqwest, (4) serialize the HTTP response (status code, headers, body) back to WASM memory. Apply safety constraints: request timeout of 30 seconds, maximum response body size of 10 MB, only HTTP/HTTPS schemes allowed (no file://, ftp://, etc.). Log outbound requests at debug level with URL and status code (never log request/response bodies). -->
  <!-- requirements: from plugin-system spec 5.5, 6.5 -->
  <!-- leverage: reqwest -->

- [ ] Add HTTP host function tests
  <!-- file: packages/plugin-system/src/host_functions/http.rs -->
  <!-- purpose: Test cases: (1) request succeeds with http:outbound capability (mock HTTP server), (2) without http:outbound returns CAP_002 error, (3) response is correctly serialized back with status, headers, body, (4) request timeout is enforced, (5) non-HTTP schemes are rejected. -->
  <!-- requirements: from plugin-system spec 5.5, 5.9, 6.5 -->
  <!-- leverage: none -->

---

## 8.9 — Host Function — Logging
> depends: 8.4
> spec: .odm/spec/plugin-system/brief.md

- [ ] Implement logging host function (no capability required)
  <!-- file: packages/plugin-system/src/host_functions/logging.rs -->
  <!-- purpose: Implement host_log: (1) deserialize log entry from WASM memory (level: "trace"/"debug"/"info"/"warn"/"error", message: String), (2) tag the log entry with the calling plugin's ID using tracing structured fields, (3) forward to the tracing subscriber at the appropriate level. This host function does NOT require any capability — all plugins can log. Log format: [plugin:connector-email] INFO: Fetched 42 new emails. Rate limit plugin logging: max 100 log entries per second per plugin to prevent log flooding from buggy plugins. Drop excess entries with a single warning. -->
  <!-- requirements: from plugin-system spec 6.4 -->
  <!-- leverage: tracing -->

---

## 8.10 — Plugin Loader
> depends: 8.1, 8.2, 8.3, 8.4
> spec: .odm/spec/plugin-system/brief.md

- [ ] Implement plugin loader orchestrating the full loading flow
  <!-- file: packages/plugin-system/src/loader.rs -->
  <!-- purpose: Implement pub async fn load_plugins(config: &PluginConfig, plugins_dir: &Path, storage: Arc<dyn StorageBackend>, event_bus: Arc<EventBus>) -> Result<Vec<PluginHandle>, PluginError>. Orchestration: (1) call scan_plugins_directory to discover plugins, (2) for each discovered plugin, parse manifest.toml, (3) check capability approval (first-party auto-grant, third-party config check), (4) build the host function set matching approved capabilities — only inject storage host functions if storage:read or storage:write approved, only inject http if http:outbound approved, etc., (5) load the WASM binary into Extism with the constructed host functions, (6) create a PluginHandle wrapping the PluginInstance, manifest, and approved capabilities, (7) log success: "Loaded plugin {id} v{version} with capabilities: [...]". Skip plugins that fail any step with a warning log — one bad plugin must not prevent others from loading. Return the list of successfully loaded PluginHandle objects. Define PluginHandle struct with fields: instance (PluginInstance), manifest (PluginManifest), capabilities (ApprovedCapabilities). -->
  <!-- requirements: from plugin-system spec 1.1-1.4, 2.1, 2.5, 3.1, 5.1-5.3 -->
  <!-- leverage: all previous WPs in this phase -->

- [ ] Add plugin loader integration tests
  <!-- file: packages/plugin-system/src/loader.rs -->
  <!-- purpose: Test cases: (1) valid plugin directory loads successfully with correct capabilities, (2) missing manifest skips with warning, (3) unapproved third-party capability rejects the plugin with CAP_001, (4) corrupt WASM binary skips with warning, (5) multiple plugins load independently — failure of one doesn't affect others, (6) first-party plugins get all capabilities auto-granted. -->
  <!-- requirements: from plugin-system spec 1.3, 1.4, 3.1, 5.3 -->
  <!-- leverage: test WASM fixtures -->

---

## 8.11 — Lifecycle Manager
> depends: 8.10
> spec: .odm/spec/plugin-system/brief.md

- [ ] Implement six-phase lifecycle state machine
  <!-- file: packages/plugin-system/src/lifecycle.rs -->
  <!-- purpose: Define PluginState enum: Discovered, Loaded, Initialized, Running, Stopped, Unloaded. Define PluginManager struct holding a HashMap<String, (PluginHandle, PluginState)>. Implement state transition methods with validation: transition(plugin_id, target_state) -> Result. Valid transitions: Discovered -> Loaded, Loaded -> Initialized (calls plugin init if it has an "init" action), Initialized -> Running, Running -> Stopped (calls plugin "shutdown" action if declared), Stopped -> Unloaded (drops the PluginInstance), any state -> Unloaded (force unload). Invalid transitions return PluginError with the current and target state. Implement pub async fn start_all(&mut self) that transitions all plugins through Discover -> Load -> Init -> Running in order. Implement pub async fn stop_all(&mut self) that transitions all running plugins through Running -> Stop -> Unload in reverse loading order. Log each state transition with plugin ID and timing. -->
  <!-- requirements: from plugin-system spec 4.1, 4.2, 4.3, 4.4, 4.5, 4.6 -->
  <!-- leverage: none -->

- [ ] Add lifecycle manager tests
  <!-- file: packages/plugin-system/src/lifecycle.rs -->
  <!-- purpose: Test cases: (1) full lifecycle Discover->Load->Init->Running->Stop->Unload succeeds, (2) start_all loads and inits all discovered plugins, (3) stop_all stops and unloads in reverse order, (4) invalid state transition (e.g., Discovered -> Running) is rejected, (5) force unload from any state works, (6) state transitions are logged. -->
  <!-- requirements: from plugin-system spec 4.1-4.6 -->
  <!-- leverage: none -->

---

## 8.12 — Plugin Execution Bridge
> depends: 8.11
> spec: .odm/spec/plugin-system/brief.md

- [ ] Implement execution bridge between workflow engine and plugin WASM
  <!-- file: packages/plugin-system/src/execute.rs -->
  <!-- purpose: Implement the PluginExecutor trait (defined in Phase 7) for the plugin system. Define PluginSystemExecutor struct holding the PluginManager. Implement async fn execute(&self, plugin_id: &str, action: &str, input: PipelineMessage) -> Result<PipelineMessage, Box<dyn EngineError>>. Logic: (1) look up the PluginHandle by plugin_id, return error if not found or not in Running state, (2) verify the action exists in the plugin's manifest actions list, return error if unknown action, (3) serialize the input PipelineMessage to JSON bytes, (4) call the PluginInstance's WASM "execute" export with the serialized input, (5) deserialize the WASM output bytes back into a PipelineMessage, (6) return the result. Handle WASM execution errors: timeout, trap (panic), invalid output format. Wrap WASM-level errors in PluginError implementing EngineError with appropriate severity. -->
  <!-- requirements: from plugin-system spec 7.1, 7.2, 7.3 -->
  <!-- leverage: PluginExecutor trait from Phase 7 -->

- [ ] Add plugin execution tests
  <!-- file: packages/plugin-system/src/execute.rs -->
  <!-- purpose: Test cases: (1) execute with valid action returns PipelineMessage, (2) unknown action returns error with action name, (3) plugin not in Running state returns error, (4) plugin not found returns error, (5) WASM execution error (trap/panic) propagates correctly with plugin ID context, (6) WASM timeout returns error with Severity::Retryable. -->
  <!-- requirements: from plugin-system spec 7.1, 7.2, 7.3 -->
  <!-- leverage: test WASM fixtures -->

---

## 8.13 — Plugin Config Section Parser
> depends: 8.2
> spec: .odm/spec/plugin-system/brief.md

- [ ] Extend Core config to parse plugin-specific sections
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: Define PluginConfig struct with fields: path (String — plugins directory path, default "./plugins"), plugins (HashMap<String, PluginInstanceConfig>). Define PluginInstanceConfig struct: approved_capabilities (Vec<String> — list of capability strings for third-party plugins), config (Option<toml::Value> — plugin-specific config values). Parse from the [plugins] section of config.toml. Example: [plugins] path = "./plugins", [plugins.connector-email] approved_capabilities = ["storage:read", "storage:write", "http:outbound"], poll_interval = "5m". The approved_capabilities list is used by the capability approval checker. Plugin-specific config values (like poll_interval) are passed to the plugin via the config:read host function. -->
  <!-- requirements: from plugin-system spec 5.2, 6.3 -->
  <!-- leverage: existing apps/core/src/config.rs -->

- [ ] Add plugin config parsing tests
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: Test cases: (1) plugins path is parsed correctly, (2) per-plugin approved_capabilities are extracted as Vec<String>, (3) plugin-specific config values are accessible as toml::Value, (4) missing [plugins] section uses defaults (path = "./plugins", empty plugin map), (5) plugin with no approved_capabilities gets empty Vec. -->
  <!-- requirements: from plugin-system spec 5.2, 6.3 -->
  <!-- leverage: none -->

---

## 8.14 — Manifest Capability Parsing Extension
> spec: .odm/spec/capability-enforcement/brief.md

- [ ] Parse and validate capabilities from manifest.toml [capabilities] section
  <!-- file: packages/plugin-system/src/manifest.rs -->
  <!-- purpose: Extend the manifest parser to handle the [capabilities] section more robustly. The required array contains capability strings like ["storage:read", "storage:write", "http:outbound"]. Parse each string using Capability::from_str. If any string doesn't match a known Capability variant, reject the entire manifest with a clear error: "Unknown capability 'xxx' in manifest for plugin 'yyy'. Valid capabilities: storage:read, storage:write, http:outbound, events:emit, events:subscribe, config:read". If the [capabilities] section is missing entirely, treat as empty (plugin declares no capabilities). Add tests: valid capabilities parse, unknown capability string causes rejection, missing section gives empty set. -->
  <!-- requirements: from capability-enforcement spec 1.1, 6.1, 6.2 -->
  <!-- leverage: manifest parser from WP 8.2 -->

---

## 8.15 — Host Function Injection Gating
> depends: 8.5, 8.6, 8.7, 8.8, 8.9
> spec: .odm/spec/capability-enforcement/brief.md

- [ ] Inject host functions per-plugin based on approved capabilities
  <!-- file: packages/plugin-system/src/loader.rs -->
  <!-- purpose: During the WASM loading step in the plugin loader, construct the host function set based on the plugin's ApprovedCapabilities. Mapping: storage:read → register host_storage_read, storage:write → register host_storage_write, http:outbound → register host_http_request, events:emit → register host_events_emit, events:subscribe → register host_events_subscribe, config:read → register host_config_read. host_log is ALWAYS registered regardless of capabilities. If a plugin has no approved capabilities, it gets only host_log. This is the first layer of enforcement: structural prevention — unapproved host functions simply don't exist in the WASM sandbox, so the plugin cannot call them at all. -->
  <!-- requirements: from capability-enforcement spec 2.1, 2.2, 2.3, 2.4, 2.5, 2.6 -->
  <!-- leverage: host functions from WPs 8.5-8.9, loader from WP 8.10 -->

- [ ] Add host function injection tests
  <!-- file: packages/plugin-system/tests/injection_test.rs -->
  <!-- purpose: Test cases: (1) plugin with storage:read only gets storage read host function but not write, (2) plugin with storage:write without storage:read does not get read function, (3) plugin with no capabilities gets only logging host function, (4) plugin with all capabilities gets all host functions, (5) host_log is always present. -->
  <!-- requirements: from capability-enforcement spec 2.1, 2.2, 2.6 -->
  <!-- leverage: test WASM fixtures -->

---

## 8.16 — Runtime Capability Checks
> spec: .odm/spec/capability-enforcement/brief.md

- [ ] Add synchronous capability check inside each host function
  <!-- file: packages/plugin-system/src/host_functions/storage.rs -->
  <!-- file: packages/plugin-system/src/host_functions/config.rs -->
  <!-- file: packages/plugin-system/src/host_functions/events.rs -->
  <!-- file: packages/plugin-system/src/host_functions/http.rs -->
  <!-- purpose: At the start of every host function, before executing any logic, check the calling plugin's ApprovedCapabilities for the required capability. This is the second layer of enforcement — a defence-in-depth check in case the injection gating is somehow bypassed. If the capability is not approved, return a CapabilityViolation error with code "CAP_002" (runtime violation, as opposed to CAP_001 for load-time) and Severity::Fatal. The check is synchronous and adds negligible overhead. This ensures that even if a host function is somehow registered incorrectly, it still enforces the capability requirement. -->
  <!-- requirements: from capability-enforcement spec 3.1, 3.2, 3.3, 3.4 -->
  <!-- leverage: host functions from WPs 8.5-8.8 -->

- [ ] Add runtime capability enforcement tests
  <!-- file: packages/plugin-system/tests/runtime_capability_test.rs -->
  <!-- purpose: Test cases: (1) approved capability allows host function execution and returns data, (2) unapproved capability returns Fatal EngineError with CAP_002 code, (3) the check is synchronous — no async waiting, (4) error message includes the capability name and plugin ID. Use direct host function calls with mock contexts rather than full WASM execution for these unit tests. -->
  <!-- requirements: from capability-enforcement spec 3.1, 3.2, 3.3, 3.4, 3.5, 4.5 -->
  <!-- leverage: none -->

---

## 8.17 — Crash Isolation Verification
> depends: 8.12
> spec: .odm/spec/plugin-system/brief.md

- [ ] Add crash isolation integration tests
  <!-- file: packages/plugin-system/tests/crash_isolation.rs -->
  <!-- purpose: Create a test WASM plugin that deliberately panics during execute. Load it alongside a well-behaved test plugin. Test cases: (1) the panicking plugin's execution returns an error — it does not crash Core, (2) the error is logged with the panicking plugin's ID, (3) the well-behaved plugin continues to execute normally after the crash, (4) the PluginManager can unload and optionally reload the crashed plugin, (5) WASM memory isolation ensures the crash doesn't corrupt other plugins' state. These tests validate Extism's sandboxing guarantees. -->
  <!-- requirements: from plugin-system spec 3.3, 3.4, 3.5 -->
  <!-- leverage: test WASM fixtures -->

---

## 8.18 — Capability Enforcement Integration Tests
> depends: 8.12
> spec: .odm/spec/capability-enforcement/brief.md

- [ ] Add end-to-end capability enforcement integration tests
  <!-- file: packages/plugin-system/tests/capability_enforcement.rs -->
  <!-- purpose: Load a first-party test plugin (in the plugins/ directory) and verify all declared capabilities are auto-granted. Load a third-party test plugin (outside plugins/ directory) with partial approval (e.g., only storage:read approved). Test cases: (1) first-party plugin can call all host functions, (2) third-party approved operations succeed, (3) third-party unapproved operations return Fatal EngineError, (4) third-party plugin with unapproved manifest capability refuses to load entirely with CAP_001, (5) modifying config to approve the capability allows the plugin to load on next restart. -->
  <!-- requirements: from capability-enforcement spec 1.2, 1.4, 2.1, 3.3, 4.1, 4.3 -->
  <!-- leverage: test WASM fixtures, config fixtures -->

---

## 8.19 — Plugin-to-Plugin Communication Tests
> depends: 8.12
> spec: .odm/spec/plugin-system/brief.md

- [ ] Add workflow chaining and shared collection tests
  <!-- file: packages/plugin-system/tests/communication.rs -->
  <!-- purpose: Set up two test plugins: plugin-a (writes to contacts collection) and plugin-b (reads from contacts collection and writes notes). Test cases: (1) workflow chaining: create a workflow with step 1 = plugin-a:write-contact, step 2 = plugin-b:read-and-note. Execute the workflow. Verify plugin-b receives plugin-a's output PipelineMessage as input. (2) Shared canonical collection: plugin-a writes a contact via storage:write, plugin-b queries contacts via storage:read and sees plugin-a's data (canonical collections are shared). (3) Direct plugin-to-plugin calls are not possible: there is no host function for "call another plugin" — communication is only through workflow chaining or shared collections. Verify that a plugin cannot address another plugin directly. -->
  <!-- requirements: from plugin-system spec 8.1, 8.2, 8.3 -->
  <!-- leverage: test WASM fixtures -->

---

## 8.20 — Community Plugin Loading Test
> depends: 8.10
> spec: .odm/spec/plugin-system/brief.md

- [ ] Add community plugin discovery, approval, and loading test
  <!-- file: packages/plugin-system/tests/community_plugin.rs -->
  <!-- purpose: Create a test directory outside the monorepo plugins/ path containing a minimal third-party plugin (plugin.wasm + manifest.toml declaring storage:read). Test cases: (1) the plugin is discovered during directory scanning, (2) without config approval, it is rejected with CAP_001 error and a log warning, (3) add [plugins.test-community-plugin] approved_capabilities = ["storage:read"] to the test config, (4) reload — the plugin now loads successfully, (5) the plugin can call storage:read but not storage:write. This validates the entire third-party plugin lifecycle from discovery to capability-gated execution. -->
  <!-- requirements: from plugin-system spec 9.1, 9.2, 9.3 -->
  <!-- leverage: test WASM fixtures, test config -->
