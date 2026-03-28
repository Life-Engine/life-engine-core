//! Axum router construction from merged route tables.
//!
//! Builds an immutable `Router` once at startup (Requirement 5). Each matched
//! route extracts path parameters as `HashMap<String, String>` and dispatches
//! to the appropriate handler based on handler type (REST or GraphQL).

use axum::{
    Router,
    extract::Path,
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use serde_json::json;
use std::collections::HashMap;

use super::merge::MergedRoute;
use crate::config::RouteConfig;

/// Build an Axum `Router` from a list of `MergedRoute` entries.
///
/// Each route captures the workflow name, handler type, and public flag.
/// Path parameters (`:collection`, `:id`) are extracted into a `HashMap`.
/// The router is built once at startup and wrapped in `Arc` by the caller.
pub fn build_merged_router(routes: &[MergedRoute]) -> Router<()> {
    let mut router: Router<()> = Router::new();

    for merged in routes {
        let workflow = merged.route.workflow.clone();
        let public = merged.route.public;
        let handler_type = merged.handler_type.clone();
        let axum_path = to_axum_path(&merged.route.path);

        let handler = {
            let wf = workflow.clone();
            let ht = handler_type.clone();
            move |Path(params): Path<HashMap<String, String>>| {
                let wf = wf.clone();
                let ht = ht.clone();
                async move {
                    let resolved = json!({
                        "workflow": wf,
                        "params": params,
                        "public": public,
                        "handler_type": ht,
                    });
                    (StatusCode::OK, axum::Json(resolved)).into_response()
                }
            }
        };

        router = match merged.route.method.to_uppercase().as_str() {
            "GET" => router.route(&axum_path, get(handler)),
            "POST" => router.route(&axum_path, post(handler)),
            "PUT" => router.route(&axum_path, put(handler)),
            "DELETE" => router.route(&axum_path, delete(handler)),
            _ => router,
        };
    }

    router
}

/// Build an Axum `Router` from a flat list of `RouteConfig`.
///
/// Convenience function for callers that don't need handler-type routing.
/// Each route gets a handler that extracts path parameters and returns
/// a JSON response containing the resolved workflow name and extracted
/// parameters.
pub fn build_router(routes: &[RouteConfig]) -> Router<()> {
    let mut router: Router<()> = Router::new();

    for route in routes {
        let workflow = route.workflow.clone();
        let public = route.public;
        let axum_path = to_axum_path(&route.path);

        let handler = {
            let wf = workflow.clone();
            move |Path(params): Path<HashMap<String, String>>| {
                let wf = wf.clone();
                async move {
                    let resolved = json!({
                        "workflow": wf,
                        "params": params,
                        "public": public,
                    });
                    (StatusCode::OK, axum::Json(resolved)).into_response()
                }
            }
        };

        router = match route.method.to_uppercase().as_str() {
            "GET" => router.route(&axum_path, get(handler)),
            "POST" => router.route(&axum_path, post(handler)),
            "PUT" => router.route(&axum_path, put(handler)),
            "DELETE" => router.route(&axum_path, delete(handler)),
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
