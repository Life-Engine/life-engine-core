//! CalDAV server API plugin for Life Engine Core.
//!
//! Exposes the `events` collection as a CalDAV calendar, allowing native
//! calendar apps (iOS Calendar, Thunderbird, GNOME Calendar) to connect
//! to Core as a calendar server.
//!
//! # Architecture
//!
//! - `serializer` — CDM `CalendarEvent` to iCalendar VEVENT serialisation
//! - `protocol` — CalDAV protocol handlers (PROPFIND, REPORT, GET, PUT, DELETE)
//! - `discovery` — `.well-known/caldav` service discovery

pub mod discovery;
pub mod error;
pub mod protocol;
pub mod serializer;

use anyhow::Result;
use async_trait::async_trait;
use life_engine_plugin_sdk::prelude::*;
use life_engine_plugin_sdk::types::Capability;

/// The CalDAV server API plugin.
///
/// Exposes Core's `events` collection as a CalDAV-compatible calendar
/// that native calendar clients can subscribe to and sync with.
pub struct CalDavApiPlugin;

impl CalDavApiPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CalDavApiPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for CalDavApiPlugin {
    fn id(&self) -> &str {
        "com.life-engine.api-caldav"
    }

    fn display_name(&self) -> &str {
        "CalDAV Server"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn actions(&self) -> Vec<Action> {
        vec![
            Action::new("propfind", "Handle CalDAV PROPFIND requests"),
            Action::new("report", "Handle CalDAV REPORT requests"),
            Action::new("get_event", "Retrieve a calendar event as iCalendar VEVENT"),
            Action::new("put_event", "Create or update a calendar event"),
            Action::new("delete_event", "Delete a calendar event"),
        ]
    }

    fn execute(
        &self,
        action: &str,
        input: PipelineMessage,
    ) -> std::result::Result<PipelineMessage, Box<dyn EngineError>> {
        match action {
            "propfind" | "report" | "get_event" | "put_event" | "delete_event" => Ok(input),
            other => Err(Box::new(
                crate::error::CalDavError::UnknownAction(other.to_string()),
            )),
        }
    }
}

life_engine_plugin_sdk::register_plugin!(CalDavApiPlugin);

#[async_trait]
impl CorePlugin for CalDavApiPlugin {
    fn id(&self) -> &str {
        "com.life-engine.api-caldav"
    }

    fn display_name(&self) -> &str {
        "CalDAV Server"
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
            "CalDAV server API plugin loaded"
        );
        Ok(())
    }

    async fn on_unload(&mut self) -> Result<()> {
        tracing::info!("CalDAV server API plugin unloaded");
        Ok(())
    }

    fn routes(&self) -> Vec<PluginRoute> {
        vec![
            // CalDAV protocol endpoints
            PluginRoute {
                method: HttpMethod::Get,
                path: "/calendars/default".into(),
            },
            PluginRoute {
                method: HttpMethod::Get,
                path: "/calendars/default/{uid}.ics".into(),
            },
            PluginRoute {
                method: HttpMethod::Put,
                path: "/calendars/default/{uid}.ics".into(),
            },
            PluginRoute {
                method: HttpMethod::Delete,
                path: "/calendars/default/{uid}.ics".into(),
            },
            // Service discovery — RFC 6764 well-known redirect
            PluginRoute {
                method: HttpMethod::Get,
                path: "/.well-known/caldav".into(),
            },
            PluginRoute {
                method: HttpMethod::Propfind,
                path: "/.well-known/caldav".into(),
            },
            // WebDAV/CalDAV protocol methods
            PluginRoute {
                method: HttpMethod::Propfind,
                path: "/calendars/default".into(),
            },
            PluginRoute {
                method: HttpMethod::Report,
                path: "/calendars/default".into(),
            },
            // OPTIONS — advertise DAV: 1, calendar-access (RFC 4791 Section 5.1)
            PluginRoute {
                method: HttpMethod::Options,
                path: "/calendars/default".into(),
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
        let plugin = CalDavApiPlugin::new();
        assert_eq!(plugin.id(), "com.life-engine.api-caldav");
    }

    #[test]
    fn plugin_display_name() {
        let plugin = CalDavApiPlugin::new();
        assert_eq!(plugin.display_name(), "CalDAV Server");
    }

    #[test]
    fn plugin_version() {
        let plugin = CalDavApiPlugin::new();
        assert_eq!(plugin.version(), "0.1.0");
    }

    #[test]
    fn plugin_capabilities() {
        let plugin = CalDavApiPlugin::new();
        let caps = plugin.capabilities();
        assert!(caps.contains(&Capability::StorageRead));
        assert!(caps.contains(&Capability::StorageWrite));
        assert!(caps.contains(&Capability::Logging));
    }

    #[test]
    fn plugin_routes_registered() {
        let plugin = CalDavApiPlugin::new();
        let routes = plugin.routes();
        assert!(!routes.is_empty());
        let paths: Vec<&str> = routes.iter().map(|r| r.path.as_str()).collect();
        assert!(paths.contains(&"/calendars/default"));
        assert!(paths.contains(&"/.well-known/caldav"));
    }

    #[test]
    fn well_known_propfind_route_registered() {
        let plugin = CalDavApiPlugin::new();
        let routes = plugin.routes();
        let has_propfind_wellknown = routes.iter().any(|r| {
            matches!(r.method, HttpMethod::Propfind) && r.path == "/.well-known/caldav"
        });
        assert!(has_propfind_wellknown, "PROPFIND on /.well-known/caldav must be registered");
    }

    #[test]
    fn options_route_registered() {
        let plugin = CalDavApiPlugin::new();
        let routes = plugin.routes();
        let has_options = routes.iter().any(|r| {
            matches!(r.method, HttpMethod::Options) && r.path == "/calendars/default"
        });
        assert!(has_options, "OPTIONS on /calendars/default must be registered");
    }

    #[tokio::test]
    async fn plugin_lifecycle() {
        let mut plugin = CalDavApiPlugin::new();
        let ctx = PluginContext::new(plugin.id());
        plugin.on_load(&ctx).await.expect("on_load should succeed");
        plugin.on_unload().await.expect("on_unload should succeed");
    }

    #[test]
    fn default_impl() {
        let plugin = CalDavApiPlugin::default();
        assert_eq!(plugin.id(), "com.life-engine.api-caldav");
    }
}
