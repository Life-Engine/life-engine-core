//! Trigger validation and resolution.
//!
//! Provides startup validation for trigger declarations (event names, endpoint
//! format, cron expressions) and resolution helpers that map incoming stimuli
//! to workflows via the `TriggerRegistry`.

use crate::error::WorkflowError;
use crate::loader::TriggerRegistry;
use crate::types::WorkflowDef;

/// Validate all trigger declarations across a set of workflow definitions.
///
/// Called at startup before building the `TriggerRegistry`. This catches:
/// - Malformed event names (must be dot-separated, e.g. `webhook.email.received`)
/// - Workflows with no trigger declared at all
///
/// Endpoint format and cron expression validation are handled by `TriggerRegistry::build`.
pub fn validate_triggers(workflows: &[WorkflowDef]) -> Result<(), WorkflowError> {
    for workflow in workflows {
        let trigger = &workflow.trigger;

        let has_trigger = trigger.endpoint.is_some()
            || trigger.event.is_some()
            || trigger.schedule.is_some();

        if !has_trigger {
            return Err(WorkflowError::InvalidDefinition {
                workflow_id: workflow.id.clone(),
                reason: "workflow must declare at least one trigger (endpoint, event, or schedule)"
                    .into(),
            });
        }

        if let Some(ref event_name) = trigger.event {
            validate_event_name(event_name, &workflow.id)?;
        }
    }

    Ok(())
}

/// Validate that an event name follows the dot-separated naming convention.
///
/// Valid: `webhook.email.received`, `system.startup`, `a.b`
/// Invalid: empty string, no dots, leading/trailing dots, consecutive dots
fn validate_event_name(event_name: &str, workflow_id: &str) -> Result<(), WorkflowError> {
    if event_name.is_empty() {
        return Err(WorkflowError::InvalidDefinition {
            workflow_id: workflow_id.to_string(),
            reason: "event trigger name must not be empty".into(),
        });
    }

    if !event_name.contains('.') {
        return Err(WorkflowError::InvalidDefinition {
            workflow_id: workflow_id.to_string(),
            reason: format!(
                "event trigger '{}' must be dot-separated (e.g., 'webhook.email.received')",
                event_name
            ),
        });
    }

    if event_name.starts_with('.') || event_name.ends_with('.') {
        return Err(WorkflowError::InvalidDefinition {
            workflow_id: workflow_id.to_string(),
            reason: format!(
                "event trigger '{}' must not start or end with a dot",
                event_name
            ),
        });
    }

    if event_name.contains("..") {
        return Err(WorkflowError::InvalidDefinition {
            workflow_id: workflow_id.to_string(),
            reason: format!(
                "event trigger '{}' must not contain consecutive dots",
                event_name
            ),
        });
    }

    // Each segment must be non-empty and alphanumeric with hyphens/underscores.
    for segment in event_name.split('.') {
        if segment.is_empty() {
            return Err(WorkflowError::InvalidDefinition {
                workflow_id: workflow_id.to_string(),
                reason: format!("event trigger '{}' contains an empty segment", event_name),
            });
        }
        if !segment
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(WorkflowError::InvalidDefinition {
                workflow_id: workflow_id.to_string(),
                reason: format!(
                    "event trigger '{}' segment '{}' contains invalid characters (only alphanumeric, hyphens, underscores allowed)",
                    event_name, segment
                ),
            });
        }
    }

    Ok(())
}

/// Resolve an endpoint trigger to a workflow definition.
///
/// Thin wrapper around `TriggerRegistry::find_endpoint` that returns
/// `Ok(workflow)` or a `WorkflowError` when no match is found.
pub fn resolve_endpoint<'a>(
    registry: &'a TriggerRegistry,
    method: &str,
    path: &str,
) -> Result<&'a WorkflowDef, WorkflowError> {
    registry
        .find_endpoint(method, path)
        .ok_or_else(|| WorkflowError::InvalidDefinition {
            workflow_id: String::new(),
            reason: format!("no workflow registered for endpoint {} {}", method, path),
        })
}

/// Resolve an event trigger to all matching workflow definitions.
///
/// Returns an empty vec when no workflows match the event name.
pub fn resolve_event<'a>(registry: &'a TriggerRegistry, event_name: &str) -> Vec<&'a WorkflowDef> {
    registry.find_event(event_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorkflowConfig;
    use crate::loader::load_workflows;
    use std::path::Path;
    use tempfile::TempDir;

    fn write_yaml(dir: &Path, filename: &str, content: &str) {
        std::fs::write(dir.join(filename), content).unwrap();
    }

    fn load_and_validate(dir: &Path, files: &[(&str, &str)]) -> Result<(), WorkflowError> {
        for (name, content) in files {
            write_yaml(dir, name, content);
        }
        let config = WorkflowConfig {
            path: dir.to_str().unwrap().into(),
        };
        let workflows = load_workflows(&config)?;
        validate_triggers(&workflows)
    }

    // --- Event name validation tests ---

    #[test]
    fn valid_event_name_passes() {
        let dir = TempDir::new().unwrap();
        let result = load_and_validate(
            dir.path(),
            &[(
                "valid.yaml",
                r#"
workflows:
  handler:
    id: handler
    name: Handler
    trigger:
      event: "webhook.email.received"
    steps:
      - plugin: p1
        action: a1
"#,
            )],
        );
        assert!(result.is_ok());
    }

    #[test]
    fn event_name_without_dot_rejected() {
        let dir = TempDir::new().unwrap();
        let result = load_and_validate(
            dir.path(),
            &[(
                "bad.yaml",
                r#"
workflows:
  handler:
    id: handler
    name: Handler
    trigger:
      event: "nodots"
    steps:
      - plugin: p1
        action: a1
"#,
            )],
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("dot-separated"),
            "expected dot-separated error, got: {err}"
        );
    }

    #[test]
    fn event_name_with_leading_dot_rejected() {
        let dir = TempDir::new().unwrap();
        let result = load_and_validate(
            dir.path(),
            &[(
                "bad.yaml",
                r#"
workflows:
  handler:
    id: handler
    name: Handler
    trigger:
      event: ".leading.dot"
    steps:
      - plugin: p1
        action: a1
"#,
            )],
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("empty segment")
                || err.to_string().contains("must not start or end with a dot"),
            "expected event name validation error, got: {err}"
        );
    }

    #[test]
    fn event_name_with_consecutive_dots_rejected() {
        let dir = TempDir::new().unwrap();
        let result = load_and_validate(
            dir.path(),
            &[(
                "bad.yaml",
                r#"
workflows:
  handler:
    id: handler
    name: Handler
    trigger:
      event: "webhook..email"
    steps:
      - plugin: p1
        action: a1
"#,
            )],
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("empty segment")
                || err.to_string().contains("consecutive dots"),
            "expected event name validation error, got: {err}"
        );
    }

    #[test]
    fn workflow_with_no_trigger_rejected() {
        let dir = TempDir::new().unwrap();
        let result = load_and_validate(
            dir.path(),
            &[(
                "no-trigger.yaml",
                r#"
workflows:
  handler:
    id: handler
    name: Handler
    trigger: {}
    steps:
      - plugin: p1
        action: a1
"#,
            )],
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("at least one trigger"),
            "expected no-trigger error, got: {err}"
        );
    }

    // --- Resolution tests ---

    #[test]
    fn resolve_endpoint_returns_matching_workflow() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            dir.path(),
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
        );
        let config = WorkflowConfig {
            path: dir.path().to_str().unwrap().into(),
        };
        let workflows = load_workflows(&config).unwrap();
        let registry = TriggerRegistry::build(workflows).unwrap();

        let result = resolve_endpoint(&registry, "POST", "/email/sync");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, "email-sync");
    }

    #[test]
    fn resolve_endpoint_returns_error_for_no_match() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            dir.path(),
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
        );
        let config = WorkflowConfig {
            path: dir.path().to_str().unwrap().into(),
        };
        let workflows = load_workflows(&config).unwrap();
        let registry = TriggerRegistry::build(workflows).unwrap();

        let result = resolve_endpoint(&registry, "GET", "/nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn resolve_event_returns_all_matching_workflows() {
        let dir = TempDir::new().unwrap();
        write_yaml(
            dir.path(),
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
"#,
        );
        let config = WorkflowConfig {
            path: dir.path().to_str().unwrap().into(),
        };
        let workflows = load_workflows(&config).unwrap();
        let registry = TriggerRegistry::build(workflows).unwrap();

        let matches = resolve_event(&registry, "webhook.email.received");
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn multiple_trigger_types_on_single_workflow_pass_validation() {
        let dir = TempDir::new().unwrap();
        let result = load_and_validate(
            dir.path(),
            &[(
                "multi.yaml",
                r#"
workflows:
  multi:
    id: multi
    name: Multi Trigger
    trigger:
      endpoint: "POST /sync"
      event: "webhook.email.received"
      schedule: "0 0 * * * *"
    steps:
      - plugin: p1
        action: a1
"#,
            )],
        );
        assert!(result.is_ok());
    }
}
