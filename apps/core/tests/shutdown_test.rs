//! Integration tests for graceful shutdown (WP 9.10).
//!
//! Spawns the Core binary as a subprocess with a minimal configuration
//! (SQLite unencrypted, no custom transports), sends SIGTERM, and
//! verifies orderly teardown: clean exit code, no error output, and
//! shutdown log messages in the expected reverse-startup order.
//!
//! Marked `#[ignore]` because they require the binary to be built
//! first (`cargo build -p life-engine-core`).

#[cfg(unix)]
mod unix_tests {
    use std::io::{BufRead, BufReader};
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    fn binary_path() -> PathBuf {
        // CARGO_BIN_EXE_<name> is set by cargo for integration tests
        // when the crate defines a [[bin]] target.
        if let Ok(p) = std::env::var("CARGO_BIN_EXE_life-engine-core") {
            return PathBuf::from(p);
        }
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/debug/life-engine-core")
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("failed to resolve repo root")
    }

    /// Write a minimal YAML config that starts Core without encryption
    /// and on port 0 (OS-assigned) so parallel tests don't collide.
    fn write_minimal_config(dir: &tempfile::TempDir) -> PathBuf {
        let config_path = dir.path().join("config.yaml");
        let data_dir = dir.path().join("data");
        std::fs::create_dir_all(&data_dir).unwrap();

        let yaml = format!(
            r#"core:
  host: "127.0.0.1"
  port: 0
  log_level: "info"
  log_format: "json"
  data_dir: "{data_dir}"
storage:
  backend: "sqlite"
  encryption: false
auth:
  provider: "local-token"
"#,
            data_dir = data_dir.display(),
        );
        std::fs::write(&config_path, yaml).unwrap();
        config_path
    }

    /// Spawn Core, wait for it to start, send SIGTERM, and return the
    /// collected stderr lines and exit status.
    fn run_shutdown_test() -> (Vec<String>, std::process::ExitStatus) {
        let dir = tempfile::tempdir().unwrap();
        let config_path = write_minimal_config(&dir);

        let mut child = Command::new(binary_path())
            .arg("--config")
            .arg(&config_path)
            .current_dir(repo_root())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn Core binary — is it built?");

        let stderr = child.stderr.take().unwrap();
        let (line_tx, line_rx) = mpsc::channel::<String>();

        // Read stderr in a background thread.
        let reader_handle = std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            let mut lines = Vec::new();
            for line in reader.lines().map_while(Result::ok) {
                let _ = line_tx.send(line.clone());
                lines.push(line);
            }
            lines
        });

        // Wait for Core to emit "listening" or "all systems ready",
        // indicating startup is complete.
        let deadline = Instant::now() + Duration::from_secs(120);
        let mut started = false;
        while Instant::now() < deadline {
            match line_rx.recv_timeout(Duration::from_millis(500)) {
                Ok(line) => {
                    if line.contains("listening") || line.contains("all systems ready") {
                        started = true;
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        if !started {
            let _ = child.kill();
            let _ = child.wait();
            let lines = reader_handle.join().unwrap_or_default();
            panic!(
                "Core did not start within timeout. Captured stderr:\n{}",
                lines.join("\n")
            );
        }

        // Send SIGTERM.
        let pid = child.id() as i32;
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }

        // Wait for clean exit (up to 30 seconds for the shutdown sequence).
        let exit_deadline = Instant::now() + Duration::from_secs(30);
        let status = loop {
            match child.try_wait() {
                Ok(Some(s)) => break s,
                Ok(None) => {
                    if Instant::now() > exit_deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        panic!("Core did not exit within 30 seconds after SIGTERM");
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => {
                    let _ = child.kill();
                    panic!("error waiting for Core process: {e}");
                }
            }
        };

        // Collect all lines from the reader thread.
        let all_lines = reader_handle.join().expect("stderr reader thread panicked");

        (all_lines, status)
    }

    // -----------------------------------------------------------------------
    // 1. Process exits with code 0 after SIGTERM
    // -----------------------------------------------------------------------
    #[test]
    #[ignore]
    fn sigterm_exits_with_code_zero() {
        let (_lines, status) = run_shutdown_test();
        assert!(
            status.success(),
            "expected exit code 0, got: {status}"
        );
    }

    // -----------------------------------------------------------------------
    // 2. No error-level output on stderr during clean shutdown
    // -----------------------------------------------------------------------
    #[test]
    #[ignore]
    fn sigterm_no_error_output() {
        let (lines, _status) = run_shutdown_test();

        // In JSON log format, error-level entries have `"level":"ERROR"`.
        let error_lines: Vec<&String> = lines
            .iter()
            .filter(|l| {
                l.contains("\"level\":\"ERROR\"")
                    || l.contains("\"level\": \"ERROR\"")
            })
            .collect();

        assert!(
            error_lines.is_empty(),
            "unexpected ERROR log lines during shutdown:\n{}",
            error_lines
                .iter()
                .map(|l| l.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    // -----------------------------------------------------------------------
    // 3. Shutdown log messages appear in expected reverse-startup order
    // -----------------------------------------------------------------------
    #[test]
    #[ignore]
    fn shutdown_steps_in_order() {
        let (lines, _status) = run_shutdown_test();

        // The shutdown handler logs steps 1-5 in order:
        //   1. Stopping transports
        //   2. Unloading plugins
        //   3. Stopping workflow engine
        //   4. Shutting down auth
        //   5. Closing storage
        let shutdown_keywords = [
            "Stopping transports",
            "Unloading plugins",
            "Stopping workflow engine",
            "Shutting down auth",
            "Closing storage",
        ];

        // Find the line index of each shutdown step.
        let mut positions: Vec<(usize, &str)> = Vec::new();
        for keyword in &shutdown_keywords {
            if let Some(pos) = lines.iter().position(|l| l.contains(keyword)) {
                positions.push((pos, keyword));
            }
        }

        // We expect at least the first, last, and a middle step to appear.
        assert!(
            positions.len() >= 3,
            "expected at least 3 shutdown step log messages, found {}: {:?}\nAll lines:\n{}",
            positions.len(),
            positions.iter().map(|(_, k)| *k).collect::<Vec<_>>(),
            lines.join("\n")
        );

        // Verify they appear in ascending order (correct reverse-startup sequence).
        for window in positions.windows(2) {
            assert!(
                window[0].0 < window[1].0,
                "shutdown steps out of order: '{}' (line {}) should come before '{}' (line {})",
                window[0].1,
                window[0].0,
                window[1].1,
                window[1].0,
            );
        }
    }

    // -----------------------------------------------------------------------
    // 4. Database is properly closed (no stale WAL file after shutdown)
    // -----------------------------------------------------------------------
    #[test]
    #[ignore]
    fn database_wal_cleaned_after_shutdown() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = write_minimal_config(&dir);
        let db_path = dir.path().join("data/life-engine.db");
        let wal_path = dir.path().join("data/life-engine.db-wal");

        let mut child = Command::new(binary_path())
            .arg("--config")
            .arg(&config_path)
            .current_dir(repo_root())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn Core binary");

        let stderr = child.stderr.take().unwrap();
        let (line_tx, line_rx) = mpsc::channel::<String>();

        let reader_handle = std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                let _ = line_tx.send(line);
            }
        });

        // Wait for startup.
        let deadline = Instant::now() + Duration::from_secs(120);
        let mut started = false;
        while Instant::now() < deadline {
            match line_rx.recv_timeout(Duration::from_millis(500)) {
                Ok(line) => {
                    if line.contains("listening") || line.contains("all systems ready") {
                        started = true;
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        if !started {
            let _ = child.kill();
            let _ = child.wait();
            panic!("Core did not start within timeout");
        }

        // Send SIGTERM and wait for exit.
        let pid = child.id() as i32;
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }

        let exit_deadline = Instant::now() + Duration::from_secs(30);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {
                    if Instant::now() > exit_deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        panic!("Core did not exit within 30 seconds after SIGTERM");
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => {
                    let _ = child.kill();
                    panic!("error waiting for Core process: {e}");
                }
            }
        }

        let _ = reader_handle.join();

        // After clean shutdown, the database file should exist.
        assert!(
            db_path.exists(),
            "database file should exist at {}",
            db_path.display()
        );

        // The WAL file should either not exist or be empty (checkpointed).
        if wal_path.exists() {
            let wal_size = std::fs::metadata(&wal_path)
                .map(|m| m.len())
                .unwrap_or(0);
            assert_eq!(
                wal_size, 0,
                "WAL file should be empty after TRUNCATE checkpoint, but has {wal_size} bytes"
            );
        }
        // If the WAL file doesn't exist, that's also acceptable — it means
        // the checkpoint completed and SQLite removed it.
    }
}
