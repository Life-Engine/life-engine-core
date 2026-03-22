//! Graceful shutdown handler for the Core binary.
//!
//! Listens for SIGTERM and SIGINT, then orchestrates an orderly
//! shutdown: stop accepting connections, wait for in-flight requests,
//! unload all plugins, and exit.

use crate::plugin_loader::PluginLoader;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// Default timeout for graceful shutdown (seconds).
///
/// Intentionally hardcoded: 5 seconds is sufficient for unloading plugins
/// during graceful shutdown, and making this configurable would add
/// complexity with little practical benefit.
const SHUTDOWN_TIMEOUT_SECS: u64 = 5;

/// Creates a future that completes when a shutdown signal is received.
///
/// Handles both SIGTERM (production) and SIGINT (Ctrl-C in development).
pub async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install SIGINT handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {
            info!("received SIGINT, initiating graceful shutdown");
        }
        () = terminate => {
            info!("received SIGTERM, initiating graceful shutdown");
        }
    }
}

/// Run the graceful shutdown sequence: unload all plugins within the timeout.
pub async fn graceful_shutdown(plugin_loader: Arc<Mutex<PluginLoader>>) {
    info!("shutting down: unloading plugins");

    let unload = async {
        let mut loader = plugin_loader.lock().await;
        loader.unload_all().await;
    };

    let timeout = Duration::from_secs(SHUTDOWN_TIMEOUT_SECS);
    match tokio::time::timeout(timeout, unload).await {
        Ok(()) => {
            info!("all plugins unloaded successfully");
        }
        Err(_) => {
            warn!(
                timeout_secs = SHUTDOWN_TIMEOUT_SECS,
                "plugin unload timed out, forcing shutdown"
            );
        }
    }

    info!("shutdown complete");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_loader::PluginLoader;

    #[tokio::test]
    async fn graceful_shutdown_with_empty_loader() {
        let loader = Arc::new(Mutex::new(PluginLoader::new()));
        // Should complete without error.
        graceful_shutdown(loader).await;
    }

    #[tokio::test]
    async fn graceful_shutdown_unloads_plugins() {
        use async_trait::async_trait;
        use life_engine_plugin_sdk::types::{Capability, CoreEvent, PluginContext, PluginRoute};
        use life_engine_plugin_sdk::{CorePlugin, Result};
        use std::sync::atomic::{AtomicBool, Ordering};

        static UNLOADED: AtomicBool = AtomicBool::new(false);

        struct TrackingPlugin;

        #[async_trait]
        impl CorePlugin for TrackingPlugin {
            fn id(&self) -> &str {
                "com.test.tracking"
            }
            fn display_name(&self) -> &str {
                "Tracking Plugin"
            }
            fn version(&self) -> &str {
                "1.0.0"
            }
            fn capabilities(&self) -> Vec<Capability> {
                vec![]
            }
            async fn on_load(&mut self, _ctx: &PluginContext) -> Result<()> {
                Ok(())
            }
            async fn on_unload(&mut self) -> Result<()> {
                UNLOADED.store(true, Ordering::SeqCst);
                Ok(())
            }
            fn routes(&self) -> Vec<PluginRoute> {
                vec![]
            }
            async fn handle_event(&self, _event: &CoreEvent) -> Result<()> {
                Ok(())
            }
        }

        UNLOADED.store(false, Ordering::SeqCst);

        let mut loader = PluginLoader::new();
        loader.register(Box::new(TrackingPlugin)).unwrap();
        loader.load_all().await;
        assert_eq!(loader.loaded_count(), 1);

        let loader = Arc::new(Mutex::new(loader));
        graceful_shutdown(loader).await;

        assert!(UNLOADED.load(Ordering::SeqCst));
    }
}
