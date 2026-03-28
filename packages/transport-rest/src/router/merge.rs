//! Route merging: combine config routes with plugin manifest routes.
//!
//! Plugin manifest routes are additive — they cannot override config routes.
//! Conflicts (same method + path from two sources) are rejected at startup.

use std::collections::HashSet;

use crate::config::{ListenerConfig, PluginRoute, RouteConfig};
use crate::error::RestError;

/// Merged route with its source and handler type preserved.
#[derive(Debug, Clone)]
pub struct MergedRoute {
    /// The underlying route configuration.
    pub route: RouteConfig,
    /// Handler type: `"rest"` or `"graphql"`.
    pub handler_type: String,
    /// Where this route came from.
    pub source: RouteSource,
}

/// Identifies where a route originated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteSource {
    /// Declared in the listener config file.
    Config,
    /// Declared in a plugin manifest.
    Plugin(String),
}

/// Merge config routes with plugin manifest routes into a single route list.
///
/// Validates namespace rules (REST under `/api/`, GraphQL under `/graphql`)
/// and rejects conflicts (duplicate method + path pairs).
///
/// Returns a flat list of `MergedRoute` items ready for router construction.
pub fn merge_routes(
    config: &ListenerConfig,
    plugin_routes: &[PluginRoute],
) -> Result<Vec<MergedRoute>, RestError> {
    let mut all: Vec<MergedRoute> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut errors: Vec<String> = Vec::new();

    // Config routes first — these take priority.
    for handler in &config.handlers {
        for route in &handler.routes {
            let key = route_key(&route.method, &route.path);

            if !seen.insert(key.clone()) {
                errors.push(format!("duplicate config route: {key}"));
                continue;
            }

            if let Err(e) = validate_route_namespace(&handler.handler_type, &route.path) {
                errors.push(e);
            }

            all.push(MergedRoute {
                route: route.clone(),
                handler_type: handler.handler_type.clone(),
                source: RouteSource::Config,
            });
        }
    }

    // Plugin routes — additive only, cannot override config routes.
    for pr in plugin_routes {
        let key = route_key(&pr.method, &pr.path);

        if seen.contains(&key) {
            errors.push(format!(
                "plugin '{}' route conflicts with existing route: {key}",
                pr.plugin_id
            ));
            continue;
        }

        if !seen.insert(key.clone()) {
            errors.push(format!(
                "duplicate plugin route from '{}': {key}",
                pr.plugin_id
            ));
            continue;
        }

        // Plugin routes are always REST (served under /api/).
        if let Err(e) = validate_route_namespace("rest", &pr.path) {
            errors.push(format!("plugin '{}': {e}", pr.plugin_id));
        }

        all.push(MergedRoute {
            route: RouteConfig {
                method: pr.method.clone(),
                path: pr.path.clone(),
                workflow: pr.workflow.clone(),
                public: pr.public,
            },
            handler_type: "rest".to_string(),
            source: RouteSource::Plugin(pr.plugin_id.clone()),
        });
    }

    if errors.is_empty() {
        Ok(all)
    } else {
        Err(RestError::InvalidConfig(errors.join("; ")))
    }
}

/// Build a deduplication key from method + path.
fn route_key(method: &str, path: &str) -> String {
    format!("{} {}", method.to_uppercase(), path)
}

/// Validate that a route path respects handler-type namespace rules.
fn validate_route_namespace(handler_type: &str, path: &str) -> Result<(), String> {
    match handler_type {
        "rest" => {
            if !path.starts_with("/api/") {
                return Err(format!("REST route '{path}' must start with /api/"));
            }
        }
        "graphql" => {
            if !path.starts_with("/graphql") {
                return Err(format!("GraphQL route '{path}' must start with /graphql"));
            }
        }
        _ => {}
    }
    Ok(())
}

/// Convenience: flatten a `ListenerConfig` into a plain `Vec<RouteConfig>`.
///
/// This is a compatibility bridge for callers that don't need handler-type
/// information (e.g. the existing `build_router` signature).
pub fn flatten_merged(merged: &[MergedRoute]) -> Vec<RouteConfig> {
    merged.iter().map(|m| m.route.clone()).collect()
}
