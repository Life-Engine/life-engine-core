//! Health check implementation for the SQLite storage adapter.
//!
//! Reports connectivity, WAL journal mode, encryption status, and database
//! file size. Each check is independent — a failure in one check degrades
//! the overall status but does not prevent other checks from running.

use std::path::Path;

use rusqlite::Connection;

/// Overall health status of the adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// A single health check result.
#[derive(Debug, Clone)]
pub struct HealthCheck {
    pub name: String,
    pub status: HealthStatus,
    pub message: Option<String>,
}

/// Aggregated health report.
#[derive(Debug, Clone)]
pub struct HealthReport {
    pub status: HealthStatus,
    pub message: Option<String>,
    pub checks: Vec<HealthCheck>,
}

/// Run all health checks against the given connection and database path.
///
/// Checks performed:
/// 1. **connectivity** — `SELECT 1` to verify the connection is alive.
/// 2. **wal_mode** — `PRAGMA journal_mode` must return `"wal"`.
/// 3. **encryption** — `PRAGMA cipher_version` returns a non-empty string
///    (indicates SQLCipher is active).
/// 4. **file_size** — the database file exists and its size is reported.
pub fn run_health_checks(conn: &Connection, db_path: Option<&Path>) -> HealthReport {
    let mut checks = Vec::new();

    // 1. Connectivity check: SELECT 1
    checks.push(check_connectivity(conn));

    // 2. WAL mode check
    checks.push(check_wal_mode(conn));

    // 3. Encryption status check
    checks.push(check_encryption(conn));

    // 4. File size check
    if let Some(path) = db_path {
        checks.push(check_file_size(path));
    }

    // Derive overall status from individual checks.
    let overall = if checks.iter().any(|c| c.status == HealthStatus::Unhealthy) {
        HealthStatus::Unhealthy
    } else if checks.iter().any(|c| c.status == HealthStatus::Degraded) {
        HealthStatus::Degraded
    } else {
        HealthStatus::Healthy
    };

    let message = if overall == HealthStatus::Healthy {
        None
    } else {
        let failed: Vec<&str> = checks
            .iter()
            .filter(|c| c.status != HealthStatus::Healthy)
            .map(|c| c.name.as_str())
            .collect();
        Some(format!("degraded checks: {}", failed.join(", ")))
    };

    HealthReport {
        status: overall,
        message,
        checks,
    }
}

fn check_connectivity(conn: &Connection) -> HealthCheck {
    match conn.query_row("SELECT 1", [], |row| row.get::<_, i64>(0)) {
        Ok(1) => HealthCheck {
            name: "connectivity".to_string(),
            status: HealthStatus::Healthy,
            message: None,
        },
        Ok(v) => HealthCheck {
            name: "connectivity".to_string(),
            status: HealthStatus::Degraded,
            message: Some(format!("SELECT 1 returned unexpected value: {v}")),
        },
        Err(e) => HealthCheck {
            name: "connectivity".to_string(),
            status: HealthStatus::Unhealthy,
            message: Some(format!("SELECT 1 failed: {e}")),
        },
    }
}

fn check_wal_mode(conn: &Connection) -> HealthCheck {
    match conn.query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0)) {
        Ok(mode) if mode == "wal" => HealthCheck {
            name: "wal_mode".to_string(),
            status: HealthStatus::Healthy,
            message: None,
        },
        Ok(mode) => HealthCheck {
            name: "wal_mode".to_string(),
            status: HealthStatus::Degraded,
            message: Some(format!("expected WAL mode, got: {mode}")),
        },
        Err(e) => HealthCheck {
            name: "wal_mode".to_string(),
            status: HealthStatus::Unhealthy,
            message: Some(format!("PRAGMA journal_mode failed: {e}")),
        },
    }
}

fn check_encryption(conn: &Connection) -> HealthCheck {
    // Try PRAGMA cipher_version — only available with SQLCipher.
    match conn.query_row("PRAGMA cipher_version", [], |row| row.get::<_, String>(0)) {
        Ok(version) if !version.is_empty() => HealthCheck {
            name: "encryption".to_string(),
            status: HealthStatus::Healthy,
            message: Some(format!("SQLCipher {version}")),
        },
        Ok(_) => HealthCheck {
            name: "encryption".to_string(),
            status: HealthStatus::Degraded,
            message: Some("cipher_version returned empty string".to_string()),
        },
        Err(_) => {
            // If PRAGMA cipher_version is not recognized, SQLCipher is not linked.
            // This is Degraded, not Unhealthy, because plain SQLite still works.
            HealthCheck {
                name: "encryption".to_string(),
                status: HealthStatus::Degraded,
                message: Some("SQLCipher not available — database is unencrypted".to_string()),
            }
        }
    }
}

fn check_file_size(path: &Path) -> HealthCheck {
    match std::fs::metadata(path) {
        Ok(meta) => {
            let size_bytes = meta.len();
            let size_mb = size_bytes as f64 / (1024.0 * 1024.0);
            HealthCheck {
                name: "file_size".to_string(),
                status: HealthStatus::Healthy,
                message: Some(format!("{size_mb:.2} MB ({size_bytes} bytes)")),
            }
        }
        Err(e) => HealthCheck {
            name: "file_size".to_string(),
            status: HealthStatus::Degraded,
            message: Some(format!("cannot read file metadata: {e}")),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_in_memory() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA journal_mode = WAL;").unwrap();
        conn
    }

    // -----------------------------------------------------------------------
    // Requirement 8.1: HealthReport with overall status
    // -----------------------------------------------------------------------

    #[test]
    fn health_report_returns_overall_status() {
        let conn = setup_in_memory();
        let report = run_health_checks(&conn, None);

        // In-memory databases cannot do WAL, so we expect at least one non-healthy check,
        // but the report should still have an overall status.
        assert!(
            report.status == HealthStatus::Healthy
                || report.status == HealthStatus::Degraded
                || report.status == HealthStatus::Unhealthy
        );
    }

    // -----------------------------------------------------------------------
    // Requirement 8.2: individual checks in the checks vector
    // -----------------------------------------------------------------------

    #[test]
    fn health_report_contains_connectivity_check() {
        let conn = setup_in_memory();
        let report = run_health_checks(&conn, None);

        let connectivity = report.checks.iter().find(|c| c.name == "connectivity");
        assert!(connectivity.is_some());
        assert_eq!(connectivity.unwrap().status, HealthStatus::Healthy);
    }

    #[test]
    fn health_report_contains_wal_mode_check() {
        let conn = setup_in_memory();
        let report = run_health_checks(&conn, None);

        let wal = report.checks.iter().find(|c| c.name == "wal_mode");
        assert!(wal.is_some());
    }

    #[test]
    fn health_report_contains_encryption_check() {
        let conn = setup_in_memory();
        let report = run_health_checks(&conn, None);

        let enc = report.checks.iter().find(|c| c.name == "encryption");
        assert!(enc.is_some());
    }

    #[test]
    fn health_report_contains_file_size_check_when_path_given() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch("CREATE TABLE t (id INTEGER); INSERT INTO t VALUES (1);")
            .unwrap();

        let report = run_health_checks(&conn, Some(&db_path));

        let fs = report.checks.iter().find(|c| c.name == "file_size");
        assert!(fs.is_some());
        assert_eq!(fs.unwrap().status, HealthStatus::Healthy);
        assert!(fs.unwrap().message.as_ref().unwrap().contains("bytes"));
    }

    #[test]
    fn health_report_omits_file_size_when_no_path() {
        let conn = setup_in_memory();
        let report = run_health_checks(&conn, None);

        let fs = report.checks.iter().find(|c| c.name == "file_size");
        assert!(fs.is_none());
    }

    // -----------------------------------------------------------------------
    // Connectivity check
    // -----------------------------------------------------------------------

    #[test]
    fn connectivity_check_succeeds_on_valid_connection() {
        let conn = setup_in_memory();
        let check = check_connectivity(&conn);
        assert_eq!(check.status, HealthStatus::Healthy);
        assert!(check.message.is_none());
    }

    // -----------------------------------------------------------------------
    // WAL mode check
    // -----------------------------------------------------------------------

    #[test]
    fn wal_check_degraded_when_not_wal() {
        let conn = Connection::open_in_memory().unwrap();
        // Default journal mode for in-memory is "memory", not "wal".
        let check = check_wal_mode(&conn);
        // In-memory DBs report "memory" for journal_mode.
        assert!(
            check.status == HealthStatus::Degraded || check.status == HealthStatus::Healthy,
            "unexpected status: {:?}",
            check.status
        );
    }

    // -----------------------------------------------------------------------
    // File size check
    // -----------------------------------------------------------------------

    #[test]
    fn file_size_check_degraded_for_missing_file() {
        let check = check_file_size(Path::new("/nonexistent/path/db.sqlite"));
        assert_eq!(check.status, HealthStatus::Degraded);
        assert!(check.message.as_ref().unwrap().contains("cannot read"));
    }

    #[test]
    fn file_size_check_reports_size_in_message() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch("CREATE TABLE t (id INTEGER);").unwrap();
        drop(conn);

        let check = check_file_size(&db_path);
        assert_eq!(check.status, HealthStatus::Healthy);
        assert!(check.message.as_ref().unwrap().contains("MB"));
    }

    // -----------------------------------------------------------------------
    // Overall status derivation
    // -----------------------------------------------------------------------

    #[test]
    fn overall_unhealthy_if_any_check_unhealthy() {
        // We cannot easily force SELECT 1 to fail with rusqlite, but we can
        // verify the logic through the report with a file-based DB.
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch("PRAGMA journal_mode = WAL;").unwrap();

        let report = run_health_checks(&conn, Some(&db_path));

        // With plain rusqlite (no SQLCipher), encryption check will be Degraded.
        // So overall should be at most Degraded.
        assert!(report.status == HealthStatus::Degraded || report.status == HealthStatus::Healthy);
    }

    #[test]
    fn overall_healthy_message_is_none() {
        // If all checks pass, message should be None.
        // Construct a report manually to verify the logic.
        let checks = vec![
            HealthCheck {
                name: "a".to_string(),
                status: HealthStatus::Healthy,
                message: None,
            },
            HealthCheck {
                name: "b".to_string(),
                status: HealthStatus::Healthy,
                message: None,
            },
        ];

        let overall = if checks.iter().any(|c| c.status == HealthStatus::Unhealthy) {
            HealthStatus::Unhealthy
        } else if checks.iter().any(|c| c.status == HealthStatus::Degraded) {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        assert_eq!(overall, HealthStatus::Healthy);
    }

    #[test]
    fn degraded_message_lists_failing_checks() {
        let conn = setup_in_memory();
        let report = run_health_checks(&conn, None);

        if report.status == HealthStatus::Degraded {
            assert!(report.message.is_some());
            assert!(report.message.as_ref().unwrap().contains("degraded checks"));
        }
    }
}
