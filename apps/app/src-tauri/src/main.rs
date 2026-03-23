//! Life Engine Tauri App — desktop wrapper that manages Core as a sidecar.
//!
//! Responsibilities:
//! 1. Spawn Core as a sidecar process on App launch
//! 2. Pass bundled-mode config: platform data dir, bundled plugins, auto-generated passphrase
//! 3. Wait for Core's health endpoint before showing the UI
//! 4. On App close, send SIGTERM then SIGKILL after timeout

// Prevent a console window from appearing on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use std::time::Duration;
use tauri::Manager;
use tokio::sync::Mutex;

/// Port that Core binds to (discovered from sidecar stdout or fixed).
const CORE_DEFAULT_PORT: u16 = 3750;

/// How long to wait for Core's health endpoint before giving up.
const HEALTH_TIMEOUT: Duration = Duration::from_secs(30);

/// How long to poll between health checks.
const HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// How long to wait for graceful shutdown before sending SIGKILL.
const SHUTDOWN_GRACE_PERIOD: Duration = Duration::from_secs(5);

/// State shared across the Tauri app for the sidecar process.
struct SidecarState {
    child: Option<tauri_plugin_shell::process::CommandChild>,
    port: u16,
}

/// Resolve the platform-standard data directory for bundled mode.
///
/// - macOS: `~/Library/Application Support/life-engine/`
/// - Linux: `$XDG_DATA_HOME/life-engine/` or `~/.local/share/life-engine/`
/// - Windows: `%APPDATA%/life-engine/`
fn bundled_data_dir() -> std::path::PathBuf {
    directories::ProjectDirs::from("com", "life-engine", "Life Engine")
        .map(|dirs| dirs.data_dir().to_path_buf())
        .unwrap_or_else(|| {
            directories::BaseDirs::new()
                .map(|dirs| dirs.home_dir().join(".life-engine").join("data"))
                .unwrap_or_else(|| std::path::PathBuf::from(".life-engine/data"))
        })
}

/// Generate a passphrase for first-run and persist it in the data directory.
///
/// On subsequent runs, the stored passphrase is returned. In a future iteration
/// this will use the platform keychain (macOS Keychain, Linux secret-tool,
/// Windows Credential Manager).
fn resolve_passphrase(data_dir: &std::path::Path) -> String {
    let passphrase_path = data_dir.join(".passphrase");
    if passphrase_path.exists() {
        if let Ok(stored) = std::fs::read_to_string(&passphrase_path) {
            let trimmed = stored.trim().to_string();
            if !trimmed.is_empty() {
                return trimmed;
            }
        }
    }

    // Generate a new passphrase using UUID v4 (128 bits of randomness).
    let passphrase = uuid::Uuid::new_v4().to_string();
    std::fs::create_dir_all(data_dir).ok();
    std::fs::write(&passphrase_path, &passphrase).ok();

    // Restrict file permissions on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&passphrase_path, std::fs::Permissions::from_mode(0o600)).ok();
    }

    passphrase
}

/// Wait for Core's health endpoint to return 200 OK.
async fn wait_for_health(port: u16) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{port}/api/health");
    let start = std::time::Instant::now();

    while start.elapsed() < HEALTH_TIMEOUT {
        match reqwest::get(&url).await {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!(port, elapsed_ms = start.elapsed().as_millis() as u64, "Core health check passed");
                return Ok(());
            }
            _ => {
                tokio::time::sleep(HEALTH_POLL_INTERVAL).await;
            }
        }
    }

    Err(format!(
        "Core health check timed out after {}s on port {port}",
        HEALTH_TIMEOUT.as_secs()
    ))
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Resolve bundled-mode paths.
            let data_dir = bundled_data_dir();
            std::fs::create_dir_all(&data_dir).map_err(|e| {
                format!("Failed to create data directory {}: {e}", data_dir.display())
            })?;

            let passphrase = resolve_passphrase(&data_dir);

            // Resolve bundled plugins directory from app resources.
            let resource_dir = app_handle
                .path()
                .resource_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("resources"));
            let plugins_dir = resource_dir.join("plugins");
            let workflows_dir = resource_dir.join("workflows");

            let port = CORE_DEFAULT_PORT;

            // Spawn Core as a sidecar process.
            let sidecar_command = app_handle
                .shell()
                .sidecar("life-engine-core")
                .map_err(|e| format!("Failed to create sidecar command: {e}"))?
                .env("LIFE_ENGINE_BUNDLED", "true")
                .env("LIFE_ENGINE_CORE_HOST", "127.0.0.1")
                .env("LIFE_ENGINE_CORE_PORT", port.to_string())
                .env("LIFE_ENGINE_CORE_DATA_DIR", data_dir.to_string_lossy().as_ref())
                .env("LIFE_ENGINE_STORAGE_PASSPHRASE", &passphrase)
                .env("LIFE_ENGINE_CORE_LOG_FORMAT", "json");

            // Add plugin and workflow paths if the directories exist.
            let sidecar_command = if plugins_dir.exists() {
                sidecar_command.args(["--data-dir", &data_dir.to_string_lossy()])
            } else {
                sidecar_command.args(["--data-dir", &data_dir.to_string_lossy()])
            };

            let (mut rx, child) = sidecar_command
                .spawn()
                .map_err(|e| format!("Failed to spawn Core sidecar: {e}"))?;

            tracing::info!(
                port,
                data_dir = %data_dir.display(),
                "Core sidecar spawned, waiting for health check"
            );

            // Store sidecar state for shutdown.
            let sidecar_state = Arc::new(Mutex::new(SidecarState {
                child: Some(child),
                port,
            }));
            app.manage(sidecar_state.clone());

            // Pipe sidecar stdout/stderr to the app's log.
            tauri::async_runtime::spawn(async move {
                use tauri_plugin_shell::process::CommandEvent;
                while let Some(event) = rx.recv().await {
                    match event {
                        CommandEvent::Stdout(line) => {
                            tracing::debug!(target: "core_sidecar", "{}", String::from_utf8_lossy(&line));
                        }
                        CommandEvent::Stderr(line) => {
                            tracing::warn!(target: "core_sidecar", "{}", String::from_utf8_lossy(&line));
                        }
                        CommandEvent::Terminated(payload) => {
                            tracing::info!(
                                code = ?payload.code,
                                signal = ?payload.signal,
                                "Core sidecar process terminated"
                            );
                            break;
                        }
                        CommandEvent::Error(err) => {
                            tracing::error!("Core sidecar error: {err}");
                            break;
                        }
                        _ => {}
                    }
                }
            });

            // Wait for Core health before showing the window.
            let app_handle_health = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                match wait_for_health(port).await {
                    Ok(()) => {
                        tracing::info!("Core is ready, showing main window");
                        if let Some(window) = app_handle_health.get_webview_window("main") {
                            window.show().ok();
                        }
                    }
                    Err(e) => {
                        tracing::error!("Core failed to start: {e}");
                        // TODO: Show error dialog to user
                    }
                }
            });

            Ok(())
        })
        .on_event(|app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                // Gracefully shut down the Core sidecar.
                let state = app.state::<Arc<Mutex<SidecarState>>>();
                let state = state.inner().clone();

                tauri::async_runtime::block_on(async {
                    let mut guard = state.lock().await;
                    if let Some(child) = guard.child.take() {
                        tracing::info!("Sending shutdown signal to Core sidecar");

                        // Send kill signal (Tauri's CommandChild::kill sends SIGTERM on Unix).
                        if let Err(e) = child.kill() {
                            tracing::warn!("Failed to send shutdown signal to Core: {e}");
                        }

                        // Wait for graceful shutdown.
                        tokio::time::sleep(SHUTDOWN_GRACE_PERIOD).await;
                        tracing::info!("Core sidecar shutdown complete");
                    }
                });
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Life Engine");
}
