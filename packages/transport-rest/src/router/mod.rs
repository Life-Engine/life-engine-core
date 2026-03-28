//! Axum router construction with path-parameter extraction.
//!
//! Builds an immutable Axum `Router` once at startup from the merged route
//! table (Requirement 5). Each matched route resolves a workflow by name and
//! extracts path parameters as `HashMap<String, String>`.

use axum::{
    Router,
    extract::Path,
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use serde_json::json;
use std::collections::HashMap;

use crate::config::RouteConfig;

/// A resolved route: the workflow name, path params, and public flag.
#[derive(Debug, Clone)]
pub struct ResolvedRoute {
    pub workflow: String,
    pub params: HashMap<String, String>,
    pub public: bool,
}

/// Build an Axum `Router` from a flat list of `RouteConfig`.
///
/// Each route gets a handler that extracts path parameters and returns
/// a JSON response containing the resolved workflow name and extracted
/// parameters. In a full deployment the handler would dispatch to the
/// workflow engine; here we return the resolution so callers can inspect it.
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
fn to_axum_path(path: &str) -> String {
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

#[cfg(test)]
mod path_tests {
    use super::to_axum_path;

    #[test]
    fn converts_colon_params_to_braces() {
        assert_eq!(
            to_axum_path("/api/v1/data/:collection/:id"),
            "/api/v1/data/{collection}/{id}"
        );
    }

    #[test]
    fn leaves_static_paths_unchanged() {
        assert_eq!(to_axum_path("/api/v1/health"), "/api/v1/health");
    }
}
