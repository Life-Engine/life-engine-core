//! Host function implementations for WASM plugin sandbox.
//!
//! Each host function is capability-gated: the plugin must have the
//! corresponding capability approved before the function will execute.

pub mod blob;
pub mod config;
pub mod events;
pub mod http;
pub mod logging;
pub mod storage;
