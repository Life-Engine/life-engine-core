pub mod config;
pub mod error;
pub mod steps;
pub mod transform;
pub mod types;

#[cfg(test)]
mod tests;

use anyhow::Result;
use async_trait::async_trait;
use life_engine_plugin_sdk::prelude::*;
use life_engine_plugin_sdk::types::Capability;

pub struct Plugin;

impl Plugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Plugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CorePlugin for Plugin {
    fn id(&self) -> &str {
        "com.life-engine.__ID__"
    }

    fn display_name(&self) -> &str {
        "__NAME__"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![]
    }

    async fn on_load(&mut self, ctx: &PluginContext) -> Result<()> {
        tracing::info!(plugin_id = ctx.plugin_id(), "plugin loaded");
        Ok(())
    }

    async fn on_unload(&mut self) -> Result<()> {
        tracing::info!("plugin unloaded");
        Ok(())
    }

    fn routes(&self) -> Vec<PluginRoute> {
        vec![
            PluginRoute {
                method: HttpMethod::Post,
                path: "/example".into(),
            },
        ]
    }

    async fn handle_event(&self, _event: &CoreEvent) -> Result<()> {
        Ok(())
    }
}
