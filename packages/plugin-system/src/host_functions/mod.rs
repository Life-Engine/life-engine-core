//! Host function implementations for WASM plugin sandbox.
//!
//! Each host function is capability-gated: the plugin must have the
//! corresponding capability approved before the function will execute.

pub mod storage;
