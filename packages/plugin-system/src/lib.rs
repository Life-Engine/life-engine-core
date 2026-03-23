//! Plugin discovery, loading, and capability enforcement for Life Engine.
//!
//! This crate implements the Core plugin system: directory-based discovery,
//! manifest parsing, WASM loading via Extism, host function injection,
//! lifecycle management, and two-layer capability enforcement.

pub mod discovery;
pub mod error;

pub use discovery::{scan_plugins_directory, DiscoveredPlugin};
pub use error::PluginError;
