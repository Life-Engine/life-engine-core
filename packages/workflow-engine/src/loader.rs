//! Workflow definition loader — scans a directory for YAML files and parses workflow definitions.

use std::collections::HashMap;
use std::path::Path;

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

    for (i, step) in workflow.steps.iter().enumerate() {
        if step.plugin.is_empty() {
            return Err(WorkflowError::InvalidDefinition {
                workflow_id: workflow.id.clone(),
                reason: format!("step {} is missing a 'plugin' field", i),
            });
        }
        if step.action.is_empty() {
            return Err(WorkflowError::InvalidDefinition {
                workflow_id: workflow.id.clone(),
                reason: format!("step {} is missing an 'action' field", i),
            });
        }
    }

    Ok(())
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
}
