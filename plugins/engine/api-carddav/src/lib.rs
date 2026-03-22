//! CardDAV server API plugin for Life Engine Core.
//!
//! Exposes the `contacts` collection as a CardDAV address book, allowing
//! native contacts apps (iOS Contacts, Thunderbird, GNOME Contacts) to
//! connect to Core as a contacts server.
//!
//! # Architecture
//!
//! - `serializer` — CDM `Contact` to vCard serialisation
//! - `protocol` — CardDAV protocol handlers (PROPFIND, REPORT, GET, PUT, DELETE)
//! - `discovery` — `.well-known/carddav` service discovery

pub mod discovery;
pub mod protocol;
pub mod serializer;

use anyhow::Result;
use async_trait::async_trait;
use life_engine_plugin_sdk::prelude::*;
use life_engine_plugin_sdk::types::Capability;

/// The CardDAV server API plugin.
///
/// Exposes Core's `contacts` collection as a CardDAV-compatible address
/// book that native contacts clients can subscribe to and sync with.
pub struct CardDavApiPlugin;

impl CardDavApiPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CardDavApiPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CorePlugin for CardDavApiPlugin {
    fn id(&self) -> &str {
        "com.life-engine.api-carddav"
    }

    fn display_name(&self) -> &str {
        "CardDAV Server"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![
            Capability::StorageRead,
            Capability::StorageWrite,
            Capability::Logging,
        ]
    }

    async fn on_load(&mut self, ctx: &PluginContext) -> Result<()> {
        tracing::info!(
            plugin_id = ctx.plugin_id(),
            "CardDAV server API plugin loaded"
        );
        Ok(())
    }

    async fn on_unload(&mut self) -> Result<()> {
        tracing::info!("CardDAV server API plugin unloaded");
        Ok(())
    }

    fn routes(&self) -> Vec<PluginRoute> {
        vec![
            PluginRoute {
                method: HttpMethod::Get,
                path: "/addressbooks/default".into(),
            },
            PluginRoute {
                method: HttpMethod::Get,
                path: "/addressbooks/default/{uid}.vcf".into(),
            },
            PluginRoute {
                method: HttpMethod::Put,
                path: "/addressbooks/default/{uid}.vcf".into(),
            },
            PluginRoute {
                method: HttpMethod::Delete,
                path: "/addressbooks/default/{uid}.vcf".into(),
            },
            PluginRoute {
                method: HttpMethod::Get,
                path: "/.well-known/carddav".into(),
            },
        ]
    }

    async fn handle_event(&self, _event: &CoreEvent) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use life_engine_plugin_sdk::types::PluginContext;

    #[test]
    fn plugin_id() {
        let plugin = CardDavApiPlugin::new();
        assert_eq!(plugin.id(), "com.life-engine.api-carddav");
    }

    #[test]
    fn plugin_display_name() {
        let plugin = CardDavApiPlugin::new();
        assert_eq!(plugin.display_name(), "CardDAV Server");
    }

    #[test]
    fn plugin_version() {
        let plugin = CardDavApiPlugin::new();
        assert_eq!(plugin.version(), "0.1.0");
    }

    #[test]
    fn plugin_capabilities() {
        let plugin = CardDavApiPlugin::new();
        let caps = plugin.capabilities();
        assert!(caps.contains(&Capability::StorageRead));
        assert!(caps.contains(&Capability::StorageWrite));
        assert!(caps.contains(&Capability::Logging));
    }

    #[test]
    fn plugin_routes_registered() {
        let plugin = CardDavApiPlugin::new();
        let routes = plugin.routes();
        assert!(!routes.is_empty());
        let paths: Vec<&str> = routes.iter().map(|r| r.path.as_str()).collect();
        assert!(paths.contains(&"/addressbooks/default"));
        assert!(paths.contains(&"/.well-known/carddav"));
    }

    #[tokio::test]
    async fn plugin_lifecycle() {
        let mut plugin = CardDavApiPlugin::new();
        let ctx = PluginContext::new(plugin.id());
        plugin.on_load(&ctx).await.expect("on_load should succeed");
        plugin.on_unload().await.expect("on_unload should succeed");
    }

    #[test]
    fn default_impl() {
        let plugin = CardDavApiPlugin::default();
        assert_eq!(plugin.id(), "com.life-engine.api-carddav");
    }
}
