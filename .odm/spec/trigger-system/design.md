<!--
domain: trigger-system
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Trigger System — Design

## Purpose

This document describes the technical design for the trigger system: the component that connects external stimuli (HTTP requests, event bus broadcasts, cron schedules) to workflow execution. The trigger system scans workflow definitions at startup, builds an immutable trigger registry, validates all declarations, and resolves incoming signals to `TriggerContext` values for the pipeline executor.

## Crate Location

The trigger system lives in `packages/workflow-engine/src/triggers/`. It depends on `packages/types` for shared types (`TriggerContext`, `WorkflowRequest`, `WorkflowDefinition`) and on the event bus and scheduler crates for integration.

## Trigger Declaration

Workflows declare triggers in their YAML definition under the `trigger` key. All three fields are optional individually, but at least one must be present:

```yaml
name: email-sync
mode: sync
trigger:
  endpoint: "POST /email/sync"
  event: "webhook.email.received"
  schedule: "*/5 * * * *"
steps:
  - action: "connector-email/fetch"
  - action: "core/store"
```

The `trigger` section is parsed into a `TriggerDeclaration` struct:

```rust
// packages/types/src/workflow.rs

#[derive(Debug, Clone, Deserialize)]
pub struct TriggerDeclaration {
    pub endpoint: Option<String>,
    pub event: Option<String>,
    pub schedule: Option<String>,
}
```

## Trigger Registry

The `TriggerRegistry` is built once at startup and is immutable for the lifetime of the process. It holds three lookup structures:

```rust
// packages/workflow-engine/src/triggers/registry.rs

pub struct TriggerRegistry {
    /// Route pattern -> workflow ID (one-to-one)
    endpoints: HashMap<String, String>,
    /// Event name -> list of workflow IDs (one-to-many)
    events: HashMap<String, Vec<String>>,
    /// Schedule entries are forwarded to the scheduler at registration time.
    /// This field holds the list for reference/debugging only.
    schedules: Vec<ScheduleEntry>,
}

pub struct ScheduleEntry {
    pub cron_expr: String,
    pub workflow_id: String,
}
```

### Registration

The `TriggerRegistry` is constructed by the `TriggerRegistrar`:

```rust
// packages/workflow-engine/src/triggers/registrar.rs

pub struct TriggerRegistrar<'a> {
    router_config: &'a RouterConfig,
    scheduler: &'a Scheduler,
}

impl<'a> TriggerRegistrar<'a> {
    pub fn register_all(
        &self,
        workflows: &HashMap<String, WorkflowDefinition>,
    ) -> Result<TriggerRegistry, Vec<StartupError>> {
        let mut errors: Vec<StartupError> = Vec::new();
        let mut warnings: Vec<StartupWarning> = Vec::new();
        let mut endpoints: HashMap<String, String> = HashMap::new();
        let mut events: HashMap<String, Vec<String>> = HashMap::new();
        let mut schedules: Vec<ScheduleEntry> = Vec::new();

        for (id, def) in workflows {
            let trigger = &def.trigger;

            // Endpoint registration
            if let Some(ref ep) = trigger.endpoint {
                if let Some(existing) = endpoints.get(ep) {
                    errors.push(StartupError::DuplicateEndpoint {
                        route: ep.clone(),
                        workflow_a: existing.clone(),
                        workflow_b: id.clone(),
                    });
                } else {
                    if !self.router_config.has_route(ep) {
                        warnings.push(StartupWarning::UnmatchedEndpoint {
                            workflow_id: id.clone(),
                            route: ep.clone(),
                        });
                    }
                    endpoints.insert(ep.clone(), id.clone());
                }
            }

            // Event registration
            if let Some(ref ev) = trigger.event {
                if !is_valid_event_name(ev) {
                    errors.push(StartupError::InvalidEventName {
                        workflow_id: id.clone(),
                        event_name: ev.clone(),
                    });
                } else {
                    events.entry(ev.clone()).or_default().push(id.clone());
                }
            }

            // Schedule registration
            if let Some(ref cron) = trigger.schedule {
                if !is_valid_cron(cron) {
                    errors.push(StartupError::InvalidCronExpression {
                        workflow_id: id.clone(),
                        expression: cron.clone(),
                    });
                } else {
                    self.scheduler.register(cron, id)?;
                    schedules.push(ScheduleEntry {
                        cron_expr: cron.clone(),
                        workflow_id: id.clone(),
                    });
                }
            }
        }

        // Log warnings
        for w in &warnings {
            tracing::warn!("{}", w);
        }

        if errors.is_empty() {
            Ok(TriggerRegistry { endpoints, events, schedules })
        } else {
            Err(errors)
        }
    }
}
```

### Resolution

The registry exposes read-only lookup methods used by the transport layer and event bus:

```rust
// packages/workflow-engine/src/triggers/registry.rs

impl TriggerRegistry {
    /// Resolve an endpoint route to a workflow ID.
    /// Returns None if no workflow is registered for this route.
    pub fn resolve_endpoint(&self, route: &str) -> Option<&str> {
        self.endpoints.get(route).map(|s| s.as_str())
    }

    /// Resolve an event name to all matching workflow IDs.
    /// Returns an empty slice if no workflows subscribe to this event.
    pub fn resolve_event(&self, event_name: &str) -> &[String] {
        self.events.get(event_name).map(|v| v.as_slice()).unwrap_or(&[])
    }
}
```

## TriggerContext Construction

Each trigger type produces a `TriggerContext` variant before calling the pipeline executor:

```rust
// packages/types/src/trigger.rs

pub enum TriggerContext {
    Endpoint(WorkflowRequest),
    Event {
        name: String,
        payload: Option<Value>,
        source: String,
    },
    Schedule {
        workflow_id: String,
    },
}
```

Construction by trigger type:

- **Endpoint** — The transport handler builds `WorkflowRequest` from the HTTP request (body, route params, query params, identity) and wraps it in `TriggerContext::Endpoint`.
- **Event** — The event bus subscriber builds `TriggerContext::Event` from the `Event` struct fields (name, payload, source).
- **Schedule** — The scheduler builds `TriggerContext::Schedule` with only the workflow ID.

## Validation Functions

### Event Name Validation

Event names must be dot-separated segments where each segment is a non-empty lowercase alphanumeric string with optional hyphens:

```rust
// packages/workflow-engine/src/triggers/validation.rs

pub fn is_valid_event_name(name: &str) -> bool {
    let segments: Vec<&str> = name.split('.').collect();
    if segments.len() < 2 {
        return false;
    }
    segments.iter().all(|s| {
        !s.is_empty()
            && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            && !s.starts_with('-')
            && !s.ends_with('-')
    })
}
```

### Cron Expression Validation

Cron expressions are validated using the `cron` crate's parser. The system supports standard five-field cron expressions:

```rust
// packages/workflow-engine/src/triggers/validation.rs

pub fn is_valid_cron(expr: &str) -> bool {
    cron::Schedule::from_str(expr).is_ok()
}
```

## Startup Error Types

```rust
// packages/workflow-engine/src/triggers/error.rs

pub enum StartupError {
    DuplicateEndpoint {
        route: String,
        workflow_a: String,
        workflow_b: String,
    },
    InvalidEventName {
        workflow_id: String,
        event_name: String,
    },
    InvalidCronExpression {
        workflow_id: String,
        expression: String,
    },
}

pub enum StartupWarning {
    UnmatchedEndpoint {
        workflow_id: String,
        route: String,
    },
}
```

Each error variant produces a human-readable message identifying the affected workflow(s) and the specific problem.

## Integration Points

- **Transport layer** — Calls `registry.resolve_endpoint()` when an HTTP request arrives, then builds `TriggerContext::Endpoint` and calls the pipeline executor.
- **Event bus** — Calls `registry.resolve_event()` when an event is broadcast, then builds `TriggerContext::Event` for each matching workflow and spawns them concurrently.
- **Scheduler** — Receives cron registrations during startup, fires callbacks that build `TriggerContext::Schedule` and call the pipeline executor.
- **Pipeline executor** — Receives `TriggerContext` and runs the workflow steps. The executor does not know or care which trigger type initiated the execution.

## Conventions

- The `TriggerRegistry` is always behind a shared reference (`Arc<TriggerRegistry>`) since it is immutable after construction.
- Endpoint route strings use the format `METHOD /path` (e.g., `POST /email/sync`) to match the YAML declaration format.
- Event names use dot-separated lowercase segments (e.g., `webhook.email.received`). The `system.*` prefix is reserved for Core-emitted events.
- Cron expressions use the standard five-field format (minute, hour, day-of-month, month, day-of-week).
- All startup errors are collected and reported together rather than failing on the first error, so that operators can fix all issues in one pass.
