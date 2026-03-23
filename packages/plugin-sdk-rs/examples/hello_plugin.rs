//! Minimal example plugin demonstrating the Life Engine Plugin SDK.
//!
//! This plugin implements the `Plugin` trait with a single "greet" action
//! that echoes the input message back. It serves as both a reference for
//! plugin authors and as the target for the SDK smoke test.
//!
//! # Building for WASM
//!
//! ```bash
//! cargo build --example hello_plugin --target wasm32-wasip1
//! ```

use life_engine_plugin_sdk::prelude::*;
use std::fmt;

/// A minimal plugin that echoes pipeline messages.
#[derive(Default)]
pub struct HelloPlugin;

impl Plugin for HelloPlugin {
    fn id(&self) -> &str {
        "hello-plugin"
    }

    fn display_name(&self) -> &str {
        "Hello Plugin"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn actions(&self) -> Vec<Action> {
        vec![Action::new("greet", "Echoes the input pipeline message back")]
    }

    fn execute(
        &self,
        action: &str,
        input: PipelineMessage,
    ) -> Result<PipelineMessage, Box<dyn EngineError>> {
        match action {
            "greet" => Ok(input),
            other => Err(Box::new(HelloError::UnknownAction(other.to_string()))),
        }
    }
}

// Register the WASM entry point (only compiled on wasm32 targets).
life_engine_plugin_sdk::register_plugin!(HelloPlugin);

/// Error type for the hello plugin.
#[derive(Debug)]
enum HelloError {
    UnknownAction(String),
}

impl fmt::Display for HelloError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HelloError::UnknownAction(action) => write!(f, "unknown action: {}", action),
        }
    }
}

impl std::error::Error for HelloError {}

impl EngineError for HelloError {
    fn code(&self) -> &str {
        "HELLO_001"
    }

    fn severity(&self) -> Severity {
        Severity::Fatal
    }

    fn source_module(&self) -> &str {
        "hello-plugin"
    }
}

fn main() {
    // This main function exists only so `cargo build --example hello_plugin`
    // works on native targets. The real entry point for WASM is the
    // `register_plugin!` macro-generated `execute` export.
    println!("HelloPlugin v{}", HelloPlugin.version());
    println!("Actions: {:?}", HelloPlugin.actions().iter().map(|a| &a.name).collect::<Vec<_>>());
}
