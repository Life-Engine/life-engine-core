use super::*;

#[test]
fn plugin_id_is_correct() {
    let plugin = Plugin::new();
    assert_eq!(plugin.id(), "com.life-engine.__ID__");
}

#[test]
fn plugin_display_name() {
    let plugin = Plugin::new();
    assert_eq!(plugin.display_name(), "__NAME__");
}

#[test]
fn plugin_version() {
    let plugin = Plugin::new();
    assert_eq!(plugin.version(), "0.1.0");
}

#[test]
fn plugin_routes() {
    use life_engine_plugin_sdk::prelude::*;
    let plugin = Plugin::new();
    let routes = plugin.routes();
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].path, "/example");
}

#[tokio::test]
async fn plugin_lifecycle() {
    use life_engine_plugin_sdk::prelude::*;
    let mut plugin = Plugin::new();
    let ctx = PluginContext::new(plugin.id());
    plugin.on_load(&ctx).await.expect("on_load should succeed");
    plugin.on_unload().await.expect("on_unload should succeed");
}

#[tokio::test]
async fn handle_event_returns_ok() {
    life_engine_test_utils::plugin_test_helpers::test_handle_event_ok(&Plugin::new()).await;
}

#[test]
fn default_impl() {
    use life_engine_plugin_sdk::prelude::*;
    let plugin = Plugin::default();
    assert_eq!(plugin.id(), "com.life-engine.__ID__");
}
