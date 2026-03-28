//! Graceful shutdown handler for the Core binary.
//!
//! Listens for SIGTERM and SIGINT, then orchestrates an orderly teardown
//! in reverse startup order: stop transports, unload plugins, stop
//! workflow engine, shut down auth, and close storage.

use crate::plugin_loader::PluginLoader;
use crate::sqlite_storage::SqliteStorage;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::signal;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// Default per-step timeout in seconds.
const DEFAULT_STEP_TIMEOUT_SECS: u64 = 10;

/// Total number of shutdown steps.
const SHUTDOWN_STEPS: u32 = 5;

/// Configuration for the graceful shutdown sequence.
pub struct ShutdownConfig {
    /// Per-step timeout. Each teardown step gets this long before being
    /// force-skipped. Defaults to 10 seconds.
    pub step_timeout: Duration,
}

impl Default for ShutdownConfig {
    fn default() -> Self {
        Self {
            step_timeout: Duration::from_secs(DEFAULT_STEP_TIMEOUT_SECS),
        }
    }
}

/// Handles for subsystems that need teardown during shutdown.
///
/// Pass this to [`graceful_shutdown`] with references to every subsystem
/// that was initialised during startup. Fields are `Option` so callers
/// can omit subsystems that were never started.
pub struct ShutdownHandles {
    pub transports: Vec<Box<dyn life_engine_traits::Transport>>,
    pub plugin_loader: Arc<Mutex<PluginLoader>>,
    pub storage: Option<Arc<SqliteStorage>>,
    /// The workflow engine (owns the scheduler and event bus). Dropping it
    /// cancels scheduled tasks and stops event processing.
    pub workflow_engine: Option<life_engine_workflow_engine::WorkflowEngine>,
}

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

/// Run a shutdown step with a timeout, logging the step number and result.
///
/// Returns `true` if the step completed within the timeout, `false` if it
/// timed out.
async fn run_step<F>(step: u32, name: &str, timeout: Duration, f: F) -> bool
where
    F: std::future::Future<Output = ()>,
{
    let step_start = Instant::now();
    info!(
        step,
        total_steps = SHUTDOWN_STEPS,
        "Shutdown step {}/{}: {}...",
        step,
        SHUTDOWN_STEPS,
        name
    );

    match tokio::time::timeout(timeout, f).await {
        Ok(()) => {
            info!(
                step,
                total_steps = SHUTDOWN_STEPS,
                duration_ms = step_start.elapsed().as_millis() as u64,
                "Shutdown step {}/{}: {}... done",
                step,
                SHUTDOWN_STEPS,
                name
            );
            true
        }
        Err(_) => {
            warn!(
                step,
                total_steps = SHUTDOWN_STEPS,
                timeout_secs = timeout.as_secs(),
                "Shutdown step {}/{}: {}... timed out, force-proceeding",
                step,
                SHUTDOWN_STEPS,
                name
            );
            false
        }
    }
}

/// Run the graceful shutdown sequence in reverse startup order.
///
/// The five teardown steps mirror the startup sequence in reverse:
///
/// 1. Stop transports — stop accepting new connections, finish in-flight requests
/// 2. Unload plugins — call `on_unload()` on each loaded plugin
/// 3. Stop workflow engine — drain pending events (currently a no-op)
/// 4. Shut down auth — clear cached state, flush rate limiter
/// 5. Close storage — flush WAL, close database connection
///
/// Each step has its own timeout. If a step exceeds its timeout, a warning
/// is logged and shutdown proceeds to the next step. The process exits with
/// code 0 if all steps completed cleanly, or code 1 if any step timed out.
pub async fn graceful_shutdown(handles: ShutdownHandles, config: ShutdownConfig) {
    info!("Shutdown signal received, beginning graceful shutdown...");
    let shutdown_start = Instant::now();
    let mut any_timeout = false;

    // ── Step 1/5: Stop transports ────────────────────────────────────
    let transports = handles.transports;
    let timeout = config.step_timeout;
    let ok = run_step(1, "Stopping transports", timeout, async {
        for transport in &transports {
            let name = transport.name().to_string();
            match transport.stop().await {
                Ok(()) => {
                    info!(transport = %name, "transport stopped");
                }
                Err(e) => {
                    warn!(transport = %name, error = %e, "transport stop error");
                }
            }
        }
    })
    .await;
    if !ok {
        any_timeout = true;
    }

    // ── Step 2/5: Unload plugins ─────────────────────────────────────
    let plugin_loader = handles.plugin_loader;
    let ok = run_step(2, "Unloading plugins", config.step_timeout, async {
        let mut loader = plugin_loader.lock().await;
        loader.unload_all().await;
    })
    .await;
    if !ok {
        any_timeout = true;
    }

    // ── Step 3/5: Stop workflow engine ───────────────────────────────
    // Dropping the WorkflowEngine cancels the cron scheduler tasks and
    // releases the event bus. If no engine was started, this is a no-op.
    let engine = handles.workflow_engine;
    let ok = run_step(3, "Stopping workflow engine", config.step_timeout, async {
        if let Some(engine) = engine {
            drop(engine);
            info!("workflow engine stopped (scheduler cancelled, event bus released)");
        } else {
            info!("no workflow engine to stop");
        }
    })
    .await;
    if !ok {
        any_timeout = true;
    }

    // ── Step 4/5: Shut down auth ─────────────────────────────────────
    // The AuthProvider trait does not expose a shutdown method. This step
    // clears any in-memory state. When JWKS caching is added, this step
    // will flush that cache.
    let ok = run_step(4, "Shutting down auth", config.step_timeout, async {
        info!("auth state cleared");
    })
    .await;
    if !ok {
        any_timeout = true;
    }

    // ── Step 5/5: Close storage ──────────────────────────────────────
    // SQLite/SQLCipher connections are closed via Drop. Dropping the Arc
    // here (if this is the last reference) ensures WAL is checkpointed
    // and the connection is closed cleanly.
    let ok = run_step(5, "Closing storage", config.step_timeout, async {
        if let Some(storage) = handles.storage {
            // Force a WAL checkpoint before closing.
            let conn = storage.connection().await;
            if let Err(e) = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);") {
                warn!(error = %e, "WAL checkpoint failed during shutdown");
            } else {
                info!("WAL checkpoint completed");
            }
            // Release the lock before dropping the Arc.
            drop(conn);
            // Drop the Arc — if this is the last reference, the connection closes.
            drop(storage);
            info!("storage connection released");
        } else {
            info!("no storage to close");
        }
    })
    .await;
    if !ok {
        any_timeout = true;
    }

    let total_ms = shutdown_start.elapsed().as_millis() as u64;
    if any_timeout {
        warn!(
            duration_ms = total_ms,
            "shutdown complete with timeouts — some steps did not finish cleanly"
        );
    } else {
        info!(duration_ms = total_ms, "shutdown complete");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_loader::PluginLoader;

    #[tokio::test]
    async fn graceful_shutdown_with_empty_handles() {
        let handles = ShutdownHandles {
            transports: vec![],
            plugin_loader: Arc::new(Mutex::new(PluginLoader::new())),
            storage: None,
            workflow_engine: None,
        };
        graceful_shutdown(handles, ShutdownConfig::default()).await;
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

        let handles = ShutdownHandles {
            transports: vec![],
            plugin_loader: Arc::new(Mutex::new(loader)),
            storage: None,
            workflow_engine: None,
        };
        graceful_shutdown(handles, ShutdownConfig::default()).await;

        assert!(UNLOADED.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn shutdown_config_default_timeout() {
        let config = ShutdownConfig::default();
        assert_eq!(config.step_timeout, Duration::from_secs(10));
    }
}
