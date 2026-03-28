//! Axum router construction from merged route tables.
//!
//! Builds an immutable `Router` once at startup (Requirement 5). Each matched
//! route extracts path parameters as `HashMap<String, String>` and dispatches
//! to the appropriate handler based on handler type (REST or GraphQL).

use axum::{
    Extension, Router,
    routing::{delete, get, post, put},
};

use super::merge::MergedRoute;
use crate::config::RouteConfig;
use crate::handlers;
use crate::handlers::RouteConfig as HandlerRouteConfig;

/// Build an Axum `Router` from a list of `MergedRoute` entries.
///
/// REST routes dispatch to the real REST handlers (`handle_with_body` for
/// POST/PUT, `handle_without_body` for GET/DELETE). Each route injects a
/// `HandlerRouteConfig` extension carrying the target workflow name.
///
/// The `Identity` extension must be provided by auth middleware at the
/// application level.
///
/// The router is built once at startup and wrapped in `Arc` by the caller.
pub fn build_merged_router(routes: &[MergedRoute]) -> Router<()> {
    let mut router: Router<()> = Router::new();

    for merged in routes {
        let axum_path = to_axum_path(&merged.route.path);
        let route_ext = HandlerRouteConfig {
            workflow: merged.route.workflow.clone(),
        };

        router = match merged.route.method.to_uppercase().as_str() {
            "GET" => router.route(
                &axum_path,
                get(handlers::handle_without_body).layer(Extension(route_ext)),
            ),
            "DELETE" => router.route(
                &axum_path,
                delete(handlers::handle_without_body).layer(Extension(route_ext)),
            ),
            "POST" => router.route(
                &axum_path,
                post(handlers::handle_with_body).layer(Extension(route_ext)),
            ),
            "PUT" => router.route(
                &axum_path,
                put(handlers::handle_with_body).layer(Extension(route_ext)),
            ),
            _ => router,
        };
    }

    router
}

/// Build an Axum `Router` from a flat list of `RouteConfig`.
///
/// Convenience function for callers that don't need handler-type routing.
/// Each route dispatches to the real REST handlers with the workflow name
/// injected as a `HandlerRouteConfig` extension.
///
/// The `Identity` extension must be provided by auth middleware at the
/// application level.
pub fn build_router(routes: &[RouteConfig]) -> Router<()> {
    let mut router: Router<()> = Router::new();

    for route in routes {
        let axum_path = to_axum_path(&route.path);
        let route_ext = HandlerRouteConfig {
            workflow: route.workflow.clone(),
        };

        router = match route.method.to_uppercase().as_str() {
            "GET" => router.route(
                &axum_path,
                get(handlers::handle_without_body).layer(Extension(route_ext)),
            ),
            "DELETE" => router.route(
                &axum_path,
                delete(handlers::handle_without_body).layer(Extension(route_ext)),
            ),
            "POST" => router.route(
                &axum_path,
                post(handlers::handle_with_body).layer(Extension(route_ext)),
            ),
            "PUT" => router.route(
                &axum_path,
                put(handlers::handle_with_body).layer(Extension(route_ext)),
            ),
            _ => router,
        };
    }

    router
}

/// Convert `:param` syntax to Axum's `{param}` syntax.
///
/// E.g. `/api/v1/data/:collection/:id` -> `/api/v1/data/{collection}/{id}`
pub(crate) fn to_axum_path(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            if let Some(name) = segment.strip_prefix(':') {
                format!("{{{name}}}")
            } else {
                segment.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}
