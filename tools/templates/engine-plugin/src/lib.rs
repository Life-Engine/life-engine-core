use anyhow::Result;
use async_trait::async_trait;
use life_engine_plugin_sdk::{
    Capability, CoreEvent, CorePlugin, HttpMethod, PluginContext, PluginRoute,
};

pub struct MyPlugin {
    id: String,
}

impl MyPlugin {
    pub fn new() -> Self {
        Self {
            id: "com.example.my-plugin".to_string(),
        }
    }
}

#[async_trait]
impl CorePlugin for MyPlugin {
    fn id(&self) -> &str {
        &self.id
    }

    fn display_name(&self) -> &str {
        "My Plugin"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![Capability::StorageRead, Capability::Logging]
    }

    async fn on_load(&mut self, _ctx: &PluginContext) -> Result<()> {
        Ok(())
    }

    async fn on_unload(&mut self) -> Result<()> {
        Ok(())
    }

    fn routes(&self) -> Vec<PluginRoute> {
        vec![PluginRoute {
            method: HttpMethod::Get,
            path: "/hello".to_string(),
        }]
    }

    async fn handle_event(&self, _event: &CoreEvent) -> Result<()> {
        Ok(())
    }
}
