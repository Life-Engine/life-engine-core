//! Transport configuration types and validation.
//!
//! Defines the listener, handler, route, TLS, and auth config structures
//! required by requirements 1-4, plus default config generation (requirement 2)
//! and route namespace validation (requirement 4).

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

use crate::error::RestError;

/// Top-level listener configuration (Requirement 1).
///
/// Each listener binds to one address/port and serves one or more handlers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListenerConfig {
    /// Human-readable label for this listener.
    #[serde(default = "default_binding_label")]
    pub binding: String,
    /// Port number (1..=65535).
    pub port: u16,
    /// Bind address; defaults to `127.0.0.1`.
    #[serde(default = "default_address")]
    pub address: String,
    /// Optional TLS configuration.
    pub tls: Option<TlsConfig>,
    /// Optional auth configuration.
    pub auth: Option<AuthConfig>,
    /// Handlers served on this listener.
    pub handlers: Vec<HandlerConfig>,
}

/// A handler mounted on a listener.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandlerConfig {
    /// Handler type identifier, e.g. `"rest"` or `"graphql"`.
    #[serde(rename = "type")]
    pub handler_type: String,
    /// Routes served by this handler.
    pub routes: Vec<RouteConfig>,
}

/// A single route definition mapping a path+method to a workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConfig {
    /// HTTP method (`GET`, `POST`, `PUT`, `DELETE`).
    pub method: String,
    /// URL path pattern, e.g. `/api/v1/data/:collection`.
    pub path: String,
    /// Target workflow name, e.g. `collection.list`.
    pub workflow: String,
    /// If true, the route is accessible without authentication.
    #[serde(default)]
    pub public: bool,
}

/// TLS certificate/key pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Path to PEM certificate file.
    pub cert: String,
    /// Path to PEM private key file.
    pub key: String,
}

/// Auth configuration for a listener.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// OIDC issuer URL.
    pub issuer: String,
    /// OIDC audience.
    pub audience: String,
}

// ── Defaults ──────────────────────────────────────────────────────────

fn default_binding_label() -> String {
    "default".to_string()
}

fn default_address() -> String {
    "127.0.0.1".to_string()
}

// ── Default config generation (Requirement 2) ─────────────────────────

/// Generate the default listener config that ships on first run.
pub fn default_listener_config() -> ListenerConfig {
    ListenerConfig {
        binding: "default".to_string(),
        port: 3000,
        address: "127.0.0.1".to_string(),
        tls: None,
        auth: None,
        handlers: vec![
            HandlerConfig {
                handler_type: "rest".to_string(),
                routes: default_rest_routes(),
            },
            HandlerConfig {
                handler_type: "graphql".to_string(),
                routes: vec![RouteConfig {
                    method: "POST".to_string(),
                    path: "/graphql".to_string(),
                    workflow: "graphql.query".to_string(),
                    public: false,
                }],
            },
        ],
    }
}

/// The default CRUD REST routes plus the health-check route (Requirements 2 & 6).
fn default_rest_routes() -> Vec<RouteConfig> {
    vec![
        RouteConfig {
            method: "GET".to_string(),
            path: "/api/v1/health".to_string(),
            workflow: "health.check".to_string(),
            public: true,
        },
        RouteConfig {
            method: "GET".to_string(),
            path: "/api/v1/data/:collection".to_string(),
            workflow: "collection.list".to_string(),
            public: false,
        },
        RouteConfig {
            method: "GET".to_string(),
            path: "/api/v1/data/:collection/:id".to_string(),
            workflow: "collection.get".to_string(),
            public: false,
        },
        RouteConfig {
            method: "POST".to_string(),
            path: "/api/v1/data/:collection".to_string(),
            workflow: "collection.create".to_string(),
            public: false,
        },
        RouteConfig {
            method: "PUT".to_string(),
            path: "/api/v1/data/:collection/:id".to_string(),
            workflow: "collection.update".to_string(),
            public: false,
        },
        RouteConfig {
            method: "DELETE".to_string(),
            path: "/api/v1/data/:collection/:id".to_string(),
            workflow: "collection.delete".to_string(),
            public: false,
        },
    ]
}

/// Write the default listener config to a YAML file in the given directory.
///
/// Creates a `listeners.yaml` file at `config_dir/listeners.yaml`.
/// Returns the path to the written file.
pub fn write_default_config(config_dir: &Path) -> Result<std::path::PathBuf, RestError> {
    let config = default_listener_config();
    let yaml = serde_yaml::to_string(&config).map_err(|e| {
        RestError::InvalidConfig(format!("failed to serialize default config: {e}"))
    })?;
    let path = config_dir.join("listeners.yaml");
    std::fs::write(&path, &yaml).map_err(|e| {
        RestError::InvalidConfig(format!("failed to write {}: {e}", path.display()))
    })?;
    Ok(path)
}

// ── Validation (Requirements 1, 4, 15) ───────────────────────────────

/// Validate a listener config, collecting all errors found.
///
/// Returns all violations (not just the first) so the user can fix
/// everything in one pass.
pub fn validate_listener(config: &ListenerConfig) -> Result<(), RestError> {
    let mut errors: Vec<String> = Vec::new();

    // Port range check.
    if config.port == 0 {
        errors.push("port must be between 1 and 65535".to_string());
    }

    // TLS cert/key existence check (Requirement 15).
    if let Some(tls) = &config.tls {
        if tls.cert.is_empty() || tls.key.is_empty() {
            errors.push("TLS config requires both cert and key paths".to_string());
        }
    }

    // Collect all routes across handlers for duplicate detection and namespace validation.
    let mut seen = HashSet::new();
    for handler in &config.handlers {
        for route in &handler.routes {
            let key = format!("{} {}", route.method.to_uppercase(), route.path);
            if !seen.insert(key.clone()) {
                errors.push(format!("duplicate route: {key}"));
            }

            // Namespace validation (Requirement 4).
            if let Err(e) = validate_route_namespace(&handler.handler_type, route) {
                errors.push(e.to_string());
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(RestError::InvalidConfig(errors.join("; ")))
    }
}

/// Ensure REST routes start with `/api/` and GraphQL routes start with `/graphql`.
fn validate_route_namespace(handler_type: &str, route: &RouteConfig) -> Result<(), RestError> {
    match handler_type {
        "rest" => {
            if !route.path.starts_with("/api/") {
                return Err(RestError::InvalidConfig(format!(
                    "REST route '{}' must start with /api/",
                    route.path
                )));
            }
        }
        "graphql" => {
            if !route.path.starts_with("/graphql") {
                return Err(RestError::InvalidConfig(format!(
                    "GraphQL route '{}' must start with /graphql",
                    route.path
                )));
            }
        }
        _ => {}
    }
    Ok(())
}

// ── Route merging (Requirement 3) ────────────────────────────────────

/// A plugin-declared route that will be merged into the router.
#[derive(Debug, Clone)]
pub struct PluginRoute {
    pub method: String,
    pub path: String,
    pub workflow: String,
    pub public: bool,
}

/// Merge config routes with plugin manifest routes, detecting conflicts.
///
/// Returns a flat list of `RouteConfig` items ready for router construction.
pub fn merge_routes(
    config: &ListenerConfig,
    plugin_routes: &[PluginRoute],
) -> Result<Vec<RouteConfig>, RestError> {
    let mut all: Vec<RouteConfig> = Vec::new();
    let mut seen = HashSet::new();

    // Config routes first.
    for handler in &config.handlers {
        for route in &handler.routes {
            let key = format!("{} {}", route.method.to_uppercase(), route.path);
            seen.insert(key);
            all.push(route.clone());
        }
    }

    // Plugin routes.
    for pr in plugin_routes {
        let key = format!("{} {}", pr.method.to_uppercase(), pr.path);
        if seen.contains(&key) {
            return Err(RestError::InvalidConfig(format!(
                "plugin route conflicts with config route: {key}"
            )));
        }
        seen.insert(key);
        all.push(RouteConfig {
            method: pr.method.clone(),
            path: pr.path.clone(),
            workflow: pr.workflow.clone(),
            public: pr.public,
        });
    }

    Ok(all)
}
