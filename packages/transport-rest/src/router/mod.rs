//! Axum router construction with route merging and path-parameter extraction.
//!
//! Builds an immutable Axum `Router` once at startup from the merged route
//! table (Requirement 5). Each matched route resolves a workflow by name and
//! extracts path parameters as `HashMap<String, String>`.

pub mod build;
pub mod merge;

// Re-export key types for convenience.
pub use build::{build_merged_router, build_router};
pub use merge::{MergedRoute, RouteSource, flatten_merged, merge_routes};

/// A resolved route: the workflow name, path params, and public flag.
#[derive(Debug, Clone)]
pub struct ResolvedRoute {
    pub workflow: String,
    pub params: std::collections::HashMap<String, String>,
    pub public: bool,
}

#[cfg(test)]
mod tests;
