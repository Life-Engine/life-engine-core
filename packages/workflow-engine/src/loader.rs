//! Workflow definition loader and trigger registry.
//!
//! Scans a directory for YAML files, parses workflow definitions, and builds
//! an immutable trigger registry for endpoint, event, and schedule lookups.

use std::collections::HashMap;
use std::path::Path;

use cron::Schedule;
use tracing::info;

use crate::config::WorkflowConfig;
use crate::error::WorkflowError;
use crate::types::WorkflowDef;

/// Top-level YAML structure: a map of workflow ID to definition.
#[derive(Debug, serde::Deserialize)]
struct WorkflowFile {
    workflows: HashMap<String, WorkflowDef>,
}

/// Load all workflow definitions from the configured directory.
///
/// Scans for `*.yaml` and `*.yml` files, parses each into workflow definitions,
/// validates required fields, and checks for duplicate IDs and endpoint triggers.
pub fn load_workflows(config: &WorkflowConfig) -> Result<Vec<WorkflowDef>, WorkflowError> {
    let dir = Path::new(&config.path);

    if !dir.is_dir() {
        return Err(WorkflowError::LoadFailed {
            file: config.path.clone(),
            cause: "workflow directory does not exist or is not a directory".into(),
        });
    }

    let mut all_workflows: Vec<WorkflowDef> = Vec::new();
    // Track which file each workflow ID was first seen in for duplicate detection.
    let mut id_sources: HashMap<String, String> = HashMap::new();
    // Track which workflow each endpoint trigger was first seen in for duplicate detection.
    let mut endpoint_sources: HashMap<String, String> = HashMap::new();
    let mut file_count = 0;

    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| WorkflowError::LoadFailed {
            file: config.path.clone(),
            cause: e.to_string(),
        })?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let path = entry.path();
            matches!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("yaml" | "yml")
            )
        })
        .collect();

    // Sort for deterministic ordering.
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let contents = std::fs::read_to_string(&path).map_err(|e| WorkflowError::LoadFailed {
            file: filename.clone(),
            cause: e.to_string(),
        })?;

        let workflow_file: WorkflowFile =
            serde_yaml::from_str(&contents).map_err(|e| WorkflowError::LoadFailed {
                file: filename.clone(),
                cause: e.to_string(),
            })?;

        file_count += 1;

        for (key, mut workflow) in workflow_file.workflows {
            // If the YAML key differs from the workflow's id field, use the key.
            if workflow.id.is_empty() {
                workflow.id = key;
            }

            validate_workflow(&workflow, &filename)?;

            // Check for duplicate workflow IDs.
            if let Some(prev_file) = id_sources.get(&workflow.id) {
                return Err(WorkflowError::DuplicateWorkflowId {
                    id: workflow.id.clone(),
                    file1: prev_file.clone(),
                    file2: filename,
                });
            }
            id_sources.insert(workflow.id.clone(), filename.clone());

            // Check for duplicate endpoint triggers.
            if let Some(ref endpoint) = workflow.trigger.endpoint {
                if let Some(prev_workflow) = endpoint_sources.get(endpoint) {
                    return Err(WorkflowError::DuplicateEndpoint {
                        endpoint: endpoint.clone(),
                        workflow1: prev_workflow.clone(),
                        workflow2: workflow.id.clone(),
                    });
                }
                endpoint_sources.insert(endpoint.clone(), workflow.id.clone());
            }

            all_workflows.push(workflow);
        }
    }

    info!(
        "Loaded {} workflows from {} files",
        all_workflows.len(),
        file_count
    );

    Ok(all_workflows)
}

/// Validate that a workflow definition has all required fields.
fn validate_workflow(workflow: &WorkflowDef, filename: &str) -> Result<(), WorkflowError> {
    if workflow.id.is_empty() {
        return Err(WorkflowError::InvalidDefinition {
            workflow_id: format!("(unknown in {})", filename),
            reason: "workflow is missing an 'id' field".into(),
        });
    }

    if workflow.steps.is_empty() {
        return Err(WorkflowError::InvalidDefinition {
            workflow_id: workflow.id.clone(),
            reason: "workflow must have at least one step".into(),
        });
    }

    // Validate event name format if present.
    if let Some(ref event) = workflow.trigger.event {
        validate_event_name(event, &workflow.id)?;
    }

    for (i, step) in workflow.steps.iter().enumerate() {
        validate_step(step, &workflow.id, i, false)?;
    }

    Ok(())
}

/// Validate a single step definition.
///
/// If `inside_branch` is true, this step is inside a condition's then/else
/// branch and must not contain another condition (nesting depth limit).
fn validate_step(
    step: &crate::types::StepDef,
    workflow_id: &str,
    index: usize,
    inside_branch: bool,
) -> Result<(), WorkflowError> {
    if let Some(ref condition) = step.condition {
        if inside_branch {
            return Err(WorkflowError::InvalidDefinition {
                workflow_id: workflow_id.to_string(),
                reason: format!(
                    "step {} contains a nested condition inside a branch; only flat plugin steps are allowed in then/else branches",
                    index
                ),
            });
        }
        // Validate branch steps are flat (no nested conditions).
        for (j, branch_step) in condition.then_steps.iter().enumerate() {
            validate_step(branch_step, workflow_id, j, true)?;
        }
        for (j, branch_step) in condition.else_steps.iter().enumerate() {
            validate_step(branch_step, workflow_id, j, true)?;
        }
    } else {
        // Plugin step: must have plugin and action.
        if step.plugin.is_empty() {
            return Err(WorkflowError::InvalidDefinition {
                workflow_id: workflow_id.to_string(),
                reason: format!("step {} is missing a 'plugin' field", index),
            });
        }
        if step.action.is_empty() {
            return Err(WorkflowError::InvalidDefinition {
                workflow_id: workflow_id.to_string(),
                reason: format!("step {} is missing an 'action' field", index),
            });
        }
    }
    Ok(())
}

/// Validate that an event name is non-empty and consists of dot-separated segments.
fn validate_event_name(event: &str, workflow_id: &str) -> Result<(), WorkflowError> {
    if event.is_empty() {
        return Err(WorkflowError::InvalidDefinition {
            workflow_id: workflow_id.to_string(),
            reason: "event name must not be empty".into(),
        });
    }
    for segment in event.split('.') {
        if segment.is_empty() {
            return Err(WorkflowError::InvalidDefinition {
                workflow_id: workflow_id.to_string(),
                reason: format!(
                    "event name '{}' contains an empty segment; use dot-separated names like 'domain.action'",
                    event
                ),
            });
        }
    }
    Ok(())
}

/// HTTP method extracted from an endpoint trigger string (e.g., "POST /email/sync").
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Propfind,
    Report,
    Mkcalendar,
    Mkcol,
}

impl HttpMethod {
    /// Parse an HTTP method string (case-insensitive).
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "GET" => Some(Self::Get),
            "POST" => Some(Self::Post),
            "PUT" => Some(Self::Put),
            "PATCH" => Some(Self::Patch),
            "DELETE" => Some(Self::Delete),
            "PROPFIND" => Some(Self::Propfind),
            "REPORT" => Some(Self::Report),
            "MKCALENDAR" => Some(Self::Mkcalendar),
            "MKCOL" => Some(Self::Mkcol),
            _ => None,
        }
    }
}

/// Immutable trigger registry built from loaded workflow definitions.
///
/// Maps endpoint triggers, event triggers, and schedule triggers to their
/// corresponding workflow definitions for fast lookup at runtime.
pub struct TriggerRegistry {
    /// Maps (HTTP method, path) to the workflow definition.
    endpoints: HashMap<(HttpMethod, String), WorkflowDef>,
    /// Maps event name to a list of workflows (multiple workflows can share an event).
    events: HashMap<String, Vec<WorkflowDef>>,
    /// Cron-triggered workflows with their parsed schedule.
    schedules: Vec<(Schedule, WorkflowDef)>,
}

impl TriggerRegistry {
    /// Build a trigger registry from a list of workflow definitions.
    ///
    /// Parses endpoint trigger strings into (method, path) pairs, groups event
    /// triggers, and parses cron expressions for schedule triggers.
    pub fn build(workflows: Vec<WorkflowDef>) -> Result<Self, WorkflowError> {
        let mut endpoints: HashMap<(HttpMethod, String), WorkflowDef> = HashMap::new();
        let mut events: HashMap<String, Vec<WorkflowDef>> = HashMap::new();
        let mut schedules: Vec<(Schedule, WorkflowDef)> = Vec::new();

        for workflow in workflows {
            if let Some(ref endpoint_str) = workflow.trigger.endpoint {
                let (method, path) = Self::parse_endpoint(endpoint_str, &workflow.id)?;
                endpoints.insert((method, path), workflow.clone());
            }

            if let Some(ref event_name) = workflow.trigger.event {
                events
                    .entry(event_name.clone())
                    .or_default()
                    .push(workflow.clone());
            }

            if let Some(ref cron_expr) = workflow.trigger.schedule {
                let schedule: Schedule = cron_expr.parse().map_err(|e: cron::error::Error| {
                    WorkflowError::InvalidDefinition {
                        workflow_id: workflow.id.clone(),
                        reason: format!("invalid cron expression '{}': {}", cron_expr, e),
                    }
                })?;
                schedules.push((schedule, workflow.clone()));
            }
        }

        Ok(Self {
            endpoints,
            events,
            schedules,
        })
    }

    /// Look up a workflow by HTTP method and path.
    pub fn find_endpoint(&self, method: &str, path: &str) -> Option<&WorkflowDef> {
        let http_method = HttpMethod::parse(method)?;
        self.endpoints.get(&(http_method, path.to_string()))
    }

    /// Look up all workflows triggered by an event name.
    pub fn find_event(&self, event_name: &str) -> Vec<&WorkflowDef> {
        self.events
            .get(event_name)
            .map(|wfs| wfs.iter().collect())
            .unwrap_or_default()
    }

    /// Get all schedule-triggered workflows with their parsed cron expressions.
    pub fn get_schedules(&self) -> &[(Schedule, WorkflowDef)] {
        &self.schedules
    }

    /// Parse an endpoint string like "POST /email/sync" into (HttpMethod, path).
    fn parse_endpoint(
        endpoint: &str,
        workflow_id: &str,
    ) -> Result<(HttpMethod, String), WorkflowError> {
        let parts: Vec<&str> = endpoint.splitn(2, ' ').collect();
        if parts.len() != 2 {
            return Err(WorkflowError::InvalidDefinition {
                workflow_id: workflow_id.to_string(),
                reason: format!(
                    "endpoint trigger '{}' must be in 'METHOD /path' format",
                    endpoint
                ),
            });
        }
        let method =
            HttpMethod::parse(parts[0]).ok_or_else(|| WorkflowError::InvalidDefinition {
                workflow_id: workflow_id.to_string(),
                reason: format!("unsupported HTTP method '{}' in endpoint trigger", parts[0]),
            })?;
        Ok((method, parts[1].to_string()))
    }
}

impl std::fmt::Debug for TriggerRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TriggerRegistry")
            .field("endpoints", &self.endpoints.len())
            .field("events", &self.events.len())
            .field("schedules", &self.schedules.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_yaml(dir: &Path, filename: &str, content: &str) {
        fs::write(dir.join(filename), content).unwrap();
    }

    fn valid_workflow_yaml() -> &'static str {
        r#"
workflows:
  email-sync:
    id: email-sync
    name: Email Sync
    trigger:
      endpoint: "POST /email/sync"
    steps:
      - plugin: email-fetcher
        action: fetch
      - plugin: spam-filter
        action: classify
"#
    }

    #[test]
    fn loads_single_workflow_file() {
        let dir = TempDir::new().unwrap();
        write_yaml(dir.path(), "email.yaml", valid_workflow_yaml());

        let config = WorkflowConfig {
            path: dir.path().to_str().unwrap().into(),
        };
        let workflows = load_workflows(&config).unwrap();

        assert_eq!(workflows.len(), 1);
        assert_eq!(workflows[0].id, "email-sync");
        assert_eq!(workflows[0].name, "Email Sync");
        assert_eq!(workflows[0].steps.len(), 2);
    }

    #[test]
    fn loads_multiple_files() {
        let dir = TempDir::new().unwrap();
        write_yaml(dir.path(), "email.yaml", valid_workflow_yaml());
        write_yaml(
            dir.path(),
            "backup.yml",
            r#"
workflows:
  daily-backup:
    id: daily-backup
    name: Daily Backup
    trigger:
      schedule: "0 2 * * *"
    steps:
      - plugin: backup
        action: run
"#,
        );

        let config = WorkflowConfig {
            path: dir.path().to_str().unwrap().into(),
        };
        let workflows = load_workflows(&config).unwrap();
        assert_eq!(workflows.len(), 2);
    }

    #[test]
    fn rejects_duplicate_workflow_id() {
        let dir = TempDir::new().unwrap();
        write_yaml(dir.path(), "a.yaml", valid_workflow_yaml());
        write_yaml(
            dir.path(),
            "b.yaml",
            r#"
workflows:
  email-sync:
    id: email-sync
    name: Duplicate
    trigger:
      event: "some.event"
    steps:
      - plugin: foo
        action: bar
"#,
        );

        let config = WorkflowConfig {
            path: dir.path().to_str().unwrap().into(),
        };
        let err = load_workflows(&config).unwrap_err();
        assert!(
            matches!(err, WorkflowError::DuplicateWorkflowId { .. }),
            "expected DuplicateWorkflowId, got: {err}"
        );
    }

    #[test]
    fn rejects_duplicate_endpoint() {
        let dir = TempDir::new().unwrap();
        write_yaml(dir.path(), "a.yaml", valid_workflow_yaml());
        write_yaml(
            dir.path(),
            "b.yaml",
            r#"
workflows:
  other-sync:
    id: other-sync
    name: Other Sync
    trigger:
      endpoint: "POST /email/sync"
    steps:
      - plugin: foo
        action: bar
"#,
        );

        let config = WorkflowConfig {
            path: dir.path().to_str().unwrap().into(),
        };
        let err = load_workflows(&config).unwrap_err();
        assert!(
            matches!(err, WorkflowError::DuplicateEndpoint { .. }),
            "expected DuplicateEndpoint, got: {err}"
        );
    }

    #[test]
    fn rejects_workflow_with_no_steps() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            dir.path(),
            "empty.yaml",
            r#"
workflows:
  empty:
    id: empty
    name: Empty Workflow
    trigger:
      event: "some.event"
    steps: []
"#,
        );

        let config = WorkflowConfig {
            path: dir.path().to_str().unwrap().into(),
        };
        let err = load_workflows(&config).unwrap_err();
        assert!(
            matches!(err, WorkflowError::InvalidDefinition { .. }),
            "expected InvalidDefinition, got: {err}"
        );
    }

    #[test]
    fn rejects_step_missing_plugin() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            dir.path(),
            "bad.yaml",
            r#"
workflows:
  bad:
    id: bad
    name: Bad Workflow
    trigger:
      event: "some.event"
    steps:
      - plugin: ""
        action: fetch
"#,
        );

        let config = WorkflowConfig {
            path: dir.path().to_str().unwrap().into(),
        };
        let err = load_workflows(&config).unwrap_err();
        assert!(
            matches!(err, WorkflowError::InvalidDefinition { .. }),
            "expected InvalidDefinition, got: {err}"
        );
    }

    #[test]
    fn rejects_invalid_yaml() {
        let dir = TempDir::new().unwrap();
        write_yaml(dir.path(), "bad.yaml", "this is not valid yaml: [[[");

        let config = WorkflowConfig {
            path: dir.path().to_str().unwrap().into(),
        };
        let err = load_workflows(&config).unwrap_err();
        assert!(
            matches!(err, WorkflowError::LoadFailed { .. }),
            "expected LoadFailed, got: {err}"
        );
    }

    #[test]
    fn rejects_nonexistent_directory() {
        let config = WorkflowConfig {
            path: "/nonexistent/path".into(),
        };
        let err = load_workflows(&config).unwrap_err();
        assert!(
            matches!(err, WorkflowError::LoadFailed { .. }),
            "expected LoadFailed, got: {err}"
        );
    }

    #[test]
    fn ignores_non_yaml_files() {
        let dir = TempDir::new().unwrap();
        write_yaml(dir.path(), "email.yaml", valid_workflow_yaml());
        write_yaml(dir.path(), "readme.md", "# Not a workflow");
        write_yaml(dir.path(), "data.json", "{}");

        let config = WorkflowConfig {
            path: dir.path().to_str().unwrap().into(),
        };
        let workflows = load_workflows(&config).unwrap();
        assert_eq!(workflows.len(), 1);
    }

    #[test]
    fn multiple_workflows_in_single_file() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            dir.path(),
            "multi.yaml",
            r#"
workflows:
  wf-a:
    id: wf-a
    name: Workflow A
    trigger:
      endpoint: "GET /a"
    steps:
      - plugin: p1
        action: a1
  wf-b:
    id: wf-b
    name: Workflow B
    trigger:
      endpoint: "GET /b"
    steps:
      - plugin: p2
        action: a2
"#,
        );

        let config = WorkflowConfig {
            path: dir.path().to_str().unwrap().into(),
        };
        let workflows = load_workflows(&config).unwrap();
        assert_eq!(workflows.len(), 2);
    }

    // --- TriggerRegistry tests ---

    fn build_registry_from_yaml(dir: &Path, files: &[(&str, &str)]) -> TriggerRegistry {
        for (name, content) in files {
            write_yaml(dir, name, content);
        }
        let config = WorkflowConfig {
            path: dir.to_str().unwrap().into(),
        };
        let workflows = load_workflows(&config).unwrap();
        TriggerRegistry::build(workflows).unwrap()
    }

    #[test]
    fn registry_endpoint_lookup_by_method_and_path() {
        let dir = TempDir::new().unwrap();
        let registry = build_registry_from_yaml(
            dir.path(),
            &[(
                "api.yaml",
                r#"
workflows:
  email-sync:
    id: email-sync
    name: Email Sync
    trigger:
      endpoint: "POST /email/sync"
    steps:
      - plugin: email-fetcher
        action: fetch
  get-status:
    id: get-status
    name: Get Status
    trigger:
      endpoint: "GET /status"
    steps:
      - plugin: status
        action: check
"#,
            )],
        );

        let found = registry.find_endpoint("POST", "/email/sync");
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "email-sync");

        let found = registry.find_endpoint("GET", "/status");
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "get-status");
    }

    #[test]
    fn registry_endpoint_missing_returns_none() {
        let dir = TempDir::new().unwrap();
        let registry = build_registry_from_yaml(
            dir.path(),
            &[(
                "api.yaml",
                r#"
workflows:
  email-sync:
    id: email-sync
    name: Email Sync
    trigger:
      endpoint: "POST /email/sync"
    steps:
      - plugin: email-fetcher
        action: fetch
"#,
            )],
        );

        assert!(registry.find_endpoint("GET", "/email/sync").is_none());
        assert!(registry.find_endpoint("POST", "/nonexistent").is_none());
    }

    #[test]
    fn registry_event_lookup_returns_multiple_workflows() {
        let dir = TempDir::new().unwrap();
        let registry = build_registry_from_yaml(
            dir.path(),
            &[(
                "events.yaml",
                r#"
workflows:
  handler-a:
    id: handler-a
    name: Handler A
    trigger:
      event: "webhook.email.received"
    steps:
      - plugin: p1
        action: a1
  handler-b:
    id: handler-b
    name: Handler B
    trigger:
      event: "webhook.email.received"
    steps:
      - plugin: p2
        action: a2
  handler-c:
    id: handler-c
    name: Handler C
    trigger:
      event: "other.event"
    steps:
      - plugin: p3
        action: a3
"#,
            )],
        );

        let matches = registry.find_event("webhook.email.received");
        assert_eq!(matches.len(), 2);
        let ids: Vec<&str> = matches.iter().map(|w| w.id.as_str()).collect();
        assert!(ids.contains(&"handler-a"));
        assert!(ids.contains(&"handler-b"));

        let other = registry.find_event("other.event");
        assert_eq!(other.len(), 1);
        assert_eq!(other[0].id, "handler-c");
    }

    #[test]
    fn registry_event_missing_returns_empty() {
        let dir = TempDir::new().unwrap();
        let registry = build_registry_from_yaml(
            dir.path(),
            &[(
                "events.yaml",
                r#"
workflows:
  handler:
    id: handler
    name: Handler
    trigger:
      event: "some.event"
    steps:
      - plugin: p1
        action: a1
"#,
            )],
        );

        let matches = registry.find_event("nonexistent.event");
        assert!(matches.is_empty());
    }

    #[test]
    fn registry_schedule_enumeration() {
        let dir = TempDir::new().unwrap();
        let registry = build_registry_from_yaml(
            dir.path(),
            &[(
                "schedules.yaml",
                r#"
workflows:
  daily-backup:
    id: daily-backup
    name: Daily Backup
    trigger:
      schedule: "0 0 2 * * *"
    steps:
      - plugin: backup
        action: run
  hourly-sync:
    id: hourly-sync
    name: Hourly Sync
    trigger:
      schedule: "0 0 * * * *"
    steps:
      - plugin: sync
        action: run
"#,
            )],
        );

        let schedules = registry.get_schedules();
        assert_eq!(schedules.len(), 2);
        let ids: Vec<&str> = schedules.iter().map(|(_, w)| w.id.as_str()).collect();
        assert!(ids.contains(&"daily-backup"));
        assert!(ids.contains(&"hourly-sync"));
    }

    #[test]
    fn registry_rejects_invalid_cron_expression() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            dir.path(),
            "bad-cron.yaml",
            r#"
workflows:
  bad-schedule:
    id: bad-schedule
    name: Bad Schedule
    trigger:
      schedule: "not a cron"
    steps:
      - plugin: p1
        action: a1
"#,
        );

        let config = WorkflowConfig {
            path: dir.path().to_str().unwrap().into(),
        };
        let workflows = load_workflows(&config).unwrap();
        let err = TriggerRegistry::build(workflows).unwrap_err();
        assert!(
            matches!(err, WorkflowError::InvalidDefinition { .. }),
            "expected InvalidDefinition, got: {err}"
        );
    }

    #[test]
    fn registry_rejects_invalid_endpoint_format() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            dir.path(),
            "bad-endpoint.yaml",
            r#"
workflows:
  bad-ep:
    id: bad-ep
    name: Bad Endpoint
    trigger:
      endpoint: "/no-method"
    steps:
      - plugin: p1
        action: a1
"#,
        );

        let config = WorkflowConfig {
            path: dir.path().to_str().unwrap().into(),
        };
        let workflows = load_workflows(&config).unwrap();
        let err = TriggerRegistry::build(workflows).unwrap_err();
        assert!(
            matches!(err, WorkflowError::InvalidDefinition { .. }),
            "expected InvalidDefinition, got: {err}"
        );
    }

    // --- Definition types verification tests ---

    #[test]
    fn definition_types_are_complete() {
        use crate::types::{ErrorStrategyType, ExecutionMode};

        // ExecutionMode defaults to Sync.
        assert_eq!(ExecutionMode::default(), ExecutionMode::Sync);

        // ErrorStrategyType defaults to Halt.
        assert_eq!(ErrorStrategyType::default(), ErrorStrategyType::Halt);

        // WorkflowDef includes description field.
        let dir = TempDir::new().unwrap();
        write_yaml(
            dir.path(),
            "typed.yaml",
            r#"
workflows:
  typed-wf:
    id: typed-wf
    name: Typed Workflow
    description: "A workflow with all definition fields"
    mode: async
    trigger:
      endpoint: "POST /typed"
    steps:
      - plugin: validator
        action: check
        on_error:
          strategy: retry
          max_retries: 5
"#,
        );

        let config = WorkflowConfig {
            path: dir.path().to_str().unwrap().into(),
        };
        let workflows = load_workflows(&config).unwrap();
        assert_eq!(workflows.len(), 1);

        let wf = &workflows[0];
        assert_eq!(wf.id, "typed-wf");
        assert_eq!(
            wf.description.as_deref(),
            Some("A workflow with all definition fields")
        );
        assert_eq!(wf.mode, ExecutionMode::Async);
        assert!(wf.steps[0].on_error.is_some());

        let err_strategy = wf.steps[0].on_error.as_ref().unwrap();
        assert_eq!(err_strategy.strategy, ErrorStrategyType::Retry);
        assert_eq!(err_strategy.max_retries, Some(5));
    }

    #[test]
    fn description_field_is_optional() {
        let dir = TempDir::new().unwrap();
        write_yaml(dir.path(), "no-desc.yaml", valid_workflow_yaml());

        let config = WorkflowConfig {
            path: dir.path().to_str().unwrap().into(),
        };
        let workflows = load_workflows(&config).unwrap();
        assert!(workflows[0].description.is_none());
    }

    // --- Condition step validation tests ---

    #[test]
    fn loader_accepts_condition_steps() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            dir.path(),
            "cond.yaml",
            r#"
workflows:
  cond-wf:
    id: cond-wf
    name: Conditional Workflow
    trigger:
      endpoint: "POST /conditional"
    steps:
      - plugin: ""
        action: ""
        condition:
          field: "payload.category"
          operator: equals
          value: "spam"
          then_steps:
            - plugin: spam-handler
              action: quarantine
          else_steps:
            - plugin: inbox
              action: deliver
"#,
        );

        let config = WorkflowConfig {
            path: dir.path().to_str().unwrap().into(),
        };
        let workflows = load_workflows(&config).unwrap();
        assert_eq!(workflows.len(), 1);
        assert!(workflows[0].steps[0].condition.is_some());
    }

    #[test]
    fn loader_rejects_nested_conditions() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            dir.path(),
            "nested.yaml",
            r#"
workflows:
  nested-wf:
    id: nested-wf
    name: Nested Conditions
    trigger:
      event: "some.event"
    steps:
      - plugin: ""
        action: ""
        condition:
          field: "payload.type"
          operator: equals
          value: "a"
          then_steps:
            - plugin: ""
              action: ""
              condition:
                field: "payload.sub"
                operator: equals
                value: "b"
                then_steps:
                  - plugin: handler
                    action: run
                else_steps:
                  - plugin: fallback
                    action: run
          else_steps:
            - plugin: default
              action: run
"#,
        );

        let config = WorkflowConfig {
            path: dir.path().to_str().unwrap().into(),
        };
        let err = load_workflows(&config).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("nested condition"),
            "expected nested condition error, got: {msg}"
        );
    }

    // --- Event name validation tests ---

    #[test]
    fn loader_rejects_empty_event_name() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            dir.path(),
            "empty-event.yaml",
            r#"
workflows:
  empty-ev:
    id: empty-ev
    name: Empty Event
    trigger:
      event: ""
    steps:
      - plugin: p1
        action: a1
"#,
        );

        let config = WorkflowConfig {
            path: dir.path().to_str().unwrap().into(),
        };
        let err = load_workflows(&config).unwrap_err();
        assert!(
            matches!(err, WorkflowError::InvalidDefinition { .. }),
            "expected InvalidDefinition, got: {err}"
        );
    }

    #[test]
    fn loader_rejects_malformed_event_name() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            dir.path(),
            "bad-event.yaml",
            r#"
workflows:
  bad-ev:
    id: bad-ev
    name: Bad Event
    trigger:
      event: "webhook..received"
    steps:
      - plugin: p1
        action: a1
"#,
        );

        let config = WorkflowConfig {
            path: dir.path().to_str().unwrap().into(),
        };
        let err = load_workflows(&config).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("empty segment"),
            "expected empty segment error, got: {msg}"
        );
    }

    #[test]
    fn loader_accepts_valid_event_name() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            dir.path(),
            "good-event.yaml",
            r#"
workflows:
  good-ev:
    id: good-ev
    name: Good Event
    trigger:
      event: "webhook.email.received"
    steps:
      - plugin: p1
        action: a1
"#,
        );

        let config = WorkflowConfig {
            path: dir.path().to_str().unwrap().into(),
        };
        let workflows = load_workflows(&config).unwrap();
        assert_eq!(workflows.len(), 1);
    }
}
