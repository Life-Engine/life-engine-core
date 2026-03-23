//! Plugin discovery, loading, and capability enforcement for Life Engine.
//!
//! This crate implements the Core plugin system: directory-based discovery,
//! manifest parsing, WASM loading via Extism, host function injection,
//! lifecycle management, and two-layer capability enforcement.

pub mod capability;
pub mod discovery;
pub mod error;
pub mod host_functions;
pub mod manifest;
pub mod runtime;

pub use capability::{check_capability_approval, ApprovedCapabilities};
pub use discovery::{scan_plugins_directory, DiscoveredPlugin};
pub use error::PluginError;
pub use manifest::{parse_manifest, ActionDef, CapabilitySet, ConfigSchema, PluginManifest, PluginMeta};
pub use host_functions::storage::{host_storage_read, host_storage_write, StorageHostContext};
pub use runtime::{load_plugin, load_plugin_from_bytes, PluginInstance};
