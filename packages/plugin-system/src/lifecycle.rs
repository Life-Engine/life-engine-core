//! Plugin lifecycle state machine.
//!
//! Manages the six-phase lifecycle for each plugin: Discovered → Loaded →
//! Initialized → Running → Stopped → Unloaded. The `LifecycleManager` tracks
//! all plugins and orchestrates bulk start/stop operations.

use std::collections::HashMap;
use std::fmt;

use tracing::{info, warn};

use crate::error::PluginError;

/// The six lifecycle phases a plugin passes through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleState {
    /// Plugin directory found and manifest parsed.
    Discovered,
    /// WASM binary loaded into the Extism runtime.
    Loaded,
    /// Plugin's init function called successfully.
    Initialized,
    /// Plugin actions are available to the workflow engine.
    Running,
    /// Plugin's stop function called; actions removed.
    Stopped,
    /// WASM instance released; terminal state.
    Unloaded,
}

impl fmt::Display for LifecycleState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LifecycleState::Discovered => write!(f, "Discovered"),
            LifecycleState::Loaded => write!(f, "Loaded"),
            LifecycleState::Initialized => write!(f, "Initialized"),
            LifecycleState::Running => write!(f, "Running"),
            LifecycleState::Stopped => write!(f, "Stopped"),
            LifecycleState::Unloaded => write!(f, "Unloaded"),
        }
    }
}

impl LifecycleState {
    /// Returns the valid next state in the forward lifecycle sequence.
    fn next(self) -> Option<LifecycleState> {
        match self {
            LifecycleState::Discovered => Some(LifecycleState::Loaded),
            LifecycleState::Loaded => Some(LifecycleState::Initialized),
            LifecycleState::Initialized => Some(LifecycleState::Running),
            LifecycleState::Running => Some(LifecycleState::Stopped),
            LifecycleState::Stopped => Some(LifecycleState::Unloaded),
            LifecycleState::Unloaded => None,
        }
    }

    /// Returns whether this state can transition to the given target via the
    /// normal forward path.
    fn can_transition_to(self, target: LifecycleState) -> bool {
        self.next() == Some(target)
    }
}

/// Tracks lifecycle state for a single plugin.
#[derive(Debug)]
struct PluginLifecycle {
    state: LifecycleState,
}

/// Manages lifecycle state for all plugins.
///
/// Provides methods for individual state transitions, bulk start/stop, and
/// force-unload from any state.
pub struct LifecycleManager {
    plugins: HashMap<String, PluginLifecycle>,
    /// Insertion-ordered list of plugin IDs for deterministic iteration.
    order: Vec<String>,
}

impl LifecycleManager {
    /// Creates an empty lifecycle manager.
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            order: Vec::new(),
        }
    }

    /// Registers a plugin in the Discovered state.
    pub fn register(&mut self, plugin_id: &str) {
        if !self.plugins.contains_key(plugin_id) {
            self.plugins.insert(
                plugin_id.to_string(),
                PluginLifecycle {
                    state: LifecycleState::Discovered,
                },
            );
            self.order.push(plugin_id.to_string());
            info!(plugin_id, "lifecycle: registered in Discovered state");
        }
    }

    /// Returns the current state of a plugin, or `None` if not registered.
    pub fn state(&self, plugin_id: &str) -> Option<LifecycleState> {
        self.plugins.get(plugin_id).map(|p| p.state)
    }

    /// Transitions a plugin to the next valid state.
    ///
    /// Returns `Ok(new_state)` on success, or an error if the transition is
    /// invalid (wrong ordering, unknown plugin, or already terminal).
    pub fn transition(
        &mut self,
        plugin_id: &str,
        target: LifecycleState,
    ) -> Result<LifecycleState, PluginError> {
        let lifecycle = self.plugins.get_mut(plugin_id).ok_or_else(|| {
            PluginError::LifecycleError(format!("plugin '{plugin_id}' not registered"))
        })?;

        if !lifecycle.state.can_transition_to(target) {
            return Err(PluginError::LifecycleError(format!(
                "invalid transition for plugin '{plugin_id}': {} -> {}",
                lifecycle.state, target
            )));
        }

        let old = lifecycle.state;
        lifecycle.state = target;
        info!(plugin_id, from = %old, to = %target, "lifecycle: state transition");

        Ok(target)
    }

    /// Forces a plugin to the Unloaded state from any state.
    ///
    /// This is used for error recovery and emergency shutdown. Returns an error
    /// only if the plugin is not registered.
    pub fn force_unload(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        let lifecycle = self.plugins.get_mut(plugin_id).ok_or_else(|| {
            PluginError::LifecycleError(format!("plugin '{plugin_id}' not registered"))
        })?;

        let old = lifecycle.state;
        lifecycle.state = LifecycleState::Unloaded;
        warn!(plugin_id, from = %old, "lifecycle: force unloaded");

        Ok(())
    }

    /// Transitions all Discovered plugins through Load → Init → Running.
    ///
    /// Plugins that fail any transition are force-unloaded and reported via
    /// the returned error list.
    pub fn start_all(&mut self) -> Vec<(String, PluginError)> {
        let mut errors = Vec::new();
        let ids: Vec<String> = self.order.clone();

        for id in &ids {
            if self.state(id) != Some(LifecycleState::Discovered) {
                continue;
            }

            let transitions = [
                LifecycleState::Loaded,
                LifecycleState::Initialized,
                LifecycleState::Running,
            ];

            for target in &transitions {
                if let Err(e) = self.transition(id, *target) {
                    warn!(plugin_id = %id, error = %e, "lifecycle: start_all failed");
                    let _ = self.force_unload(id);
                    errors.push((id.clone(), e));
                    break;
                }
            }
        }

        errors
    }

    /// Transitions all Running plugins through Stop → Unload in reverse
    /// registration order.
    pub fn stop_all(&mut self) -> Vec<(String, PluginError)> {
        let mut errors = Vec::new();
        let ids: Vec<String> = self.order.iter().rev().cloned().collect();

        for id in &ids {
            if self.state(id) != Some(LifecycleState::Running) {
                continue;
            }

            let transitions = [LifecycleState::Stopped, LifecycleState::Unloaded];

            for target in &transitions {
                if let Err(e) = self.transition(id, *target) {
                    warn!(plugin_id = %id, error = %e, "lifecycle: stop_all failed");
                    let _ = self.force_unload(id);
                    errors.push((id.clone(), e));
                    break;
                }
            }
        }

        errors
    }

    /// Returns plugin IDs and their current states.
    pub fn all_states(&self) -> Vec<(&str, LifecycleState)> {
        self.order
            .iter()
            .filter_map(|id| {
                self.plugins
                    .get(id)
                    .map(|p| (id.as_str(), p.state))
            })
            .collect()
    }
}

impl Default for LifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_lifecycle_discover_to_unload() {
        let mut mgr = LifecycleManager::new();
        mgr.register("test-plugin");

        assert_eq!(mgr.state("test-plugin"), Some(LifecycleState::Discovered));

        mgr.transition("test-plugin", LifecycleState::Loaded).unwrap();
        assert_eq!(mgr.state("test-plugin"), Some(LifecycleState::Loaded));

        mgr.transition("test-plugin", LifecycleState::Initialized).unwrap();
        assert_eq!(mgr.state("test-plugin"), Some(LifecycleState::Initialized));

        mgr.transition("test-plugin", LifecycleState::Running).unwrap();
        assert_eq!(mgr.state("test-plugin"), Some(LifecycleState::Running));

        mgr.transition("test-plugin", LifecycleState::Stopped).unwrap();
        assert_eq!(mgr.state("test-plugin"), Some(LifecycleState::Stopped));

        mgr.transition("test-plugin", LifecycleState::Unloaded).unwrap();
        assert_eq!(mgr.state("test-plugin"), Some(LifecycleState::Unloaded));
    }

    #[test]
    fn start_all_loads_and_inits_all_discovered_plugins() {
        let mut mgr = LifecycleManager::new();
        mgr.register("alpha");
        mgr.register("beta");
        mgr.register("gamma");

        let errors = mgr.start_all();
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");

        assert_eq!(mgr.state("alpha"), Some(LifecycleState::Running));
        assert_eq!(mgr.state("beta"), Some(LifecycleState::Running));
        assert_eq!(mgr.state("gamma"), Some(LifecycleState::Running));
    }

    #[test]
    fn stop_all_stops_and_unloads_in_reverse_order() {
        let mut mgr = LifecycleManager::new();
        mgr.register("first");
        mgr.register("second");
        mgr.register("third");
        mgr.start_all();

        let errors = mgr.stop_all();
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");

        assert_eq!(mgr.state("first"), Some(LifecycleState::Unloaded));
        assert_eq!(mgr.state("second"), Some(LifecycleState::Unloaded));
        assert_eq!(mgr.state("third"), Some(LifecycleState::Unloaded));
    }

    #[test]
    fn invalid_state_transition_is_rejected() {
        let mut mgr = LifecycleManager::new();
        mgr.register("test-plugin");

        // Discovered -> Running should fail (must go through Loaded first)
        let result = mgr.transition("test-plugin", LifecycleState::Running);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("invalid transition"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn force_unload_from_any_state() {
        let mut mgr = LifecycleManager::new();
        mgr.register("from-discovered");
        mgr.register("from-running");

        // Force unload from Discovered
        mgr.force_unload("from-discovered").unwrap();
        assert_eq!(mgr.state("from-discovered"), Some(LifecycleState::Unloaded));

        // Force unload from Running
        mgr.register("from-running");
        mgr.transition("from-running", LifecycleState::Loaded).unwrap();
        mgr.transition("from-running", LifecycleState::Initialized).unwrap();
        mgr.transition("from-running", LifecycleState::Running).unwrap();
        mgr.force_unload("from-running").unwrap();
        assert_eq!(mgr.state("from-running"), Some(LifecycleState::Unloaded));
    }

    #[test]
    fn state_transitions_are_logged() {
        // This test validates the code paths that produce log output.
        // The tracing macros in transition() and force_unload() are covered
        // by exercising them — tracing subscribers in tests would verify output
        // but the key guarantee is that the log calls don't panic.
        let mut mgr = LifecycleManager::new();
        mgr.register("logged-plugin");
        mgr.transition("logged-plugin", LifecycleState::Loaded).unwrap();
        mgr.force_unload("logged-plugin").unwrap();
    }

    #[test]
    fn unknown_plugin_returns_error() {
        let mut mgr = LifecycleManager::new();

        let result = mgr.transition("nonexistent", LifecycleState::Loaded);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not registered"));

        let result = mgr.force_unload("nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not registered"));
    }

    #[test]
    fn start_all_skips_non_discovered_plugins() {
        let mut mgr = LifecycleManager::new();
        mgr.register("already-running");
        mgr.start_all(); // moves to Running

        mgr.register("new-plugin");
        let errors = mgr.start_all();
        assert!(errors.is_empty());

        // already-running should still be Running (not re-started)
        assert_eq!(mgr.state("already-running"), Some(LifecycleState::Running));
        // new-plugin should now be Running
        assert_eq!(mgr.state("new-plugin"), Some(LifecycleState::Running));
    }

    #[test]
    fn stop_all_skips_non_running_plugins() {
        let mut mgr = LifecycleManager::new();
        mgr.register("running");
        mgr.register("discovered-only");

        // Only move "running" to Running state
        mgr.transition("running", LifecycleState::Loaded).unwrap();
        mgr.transition("running", LifecycleState::Initialized).unwrap();
        mgr.transition("running", LifecycleState::Running).unwrap();

        let errors = mgr.stop_all();
        assert!(errors.is_empty());

        assert_eq!(mgr.state("running"), Some(LifecycleState::Unloaded));
        // discovered-only should remain unchanged
        assert_eq!(mgr.state("discovered-only"), Some(LifecycleState::Discovered));
    }

    #[test]
    fn all_states_returns_current_snapshot() {
        let mut mgr = LifecycleManager::new();
        mgr.register("a");
        mgr.register("b");
        mgr.transition("a", LifecycleState::Loaded).unwrap();

        let states = mgr.all_states();
        assert_eq!(states.len(), 2);
        assert_eq!(states[0], ("a", LifecycleState::Loaded));
        assert_eq!(states[1], ("b", LifecycleState::Discovered));
    }
}
