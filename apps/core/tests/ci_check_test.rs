//! Integration tests for the CI check script.
//!
//! These tests verify that `tools/scripts/ci-check.sh` exists, is executable,
//! and contains the expected CI check commands.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

/// Returns the repository root (two levels above `apps/core/`).
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("failed to resolve repo root")
}

#[test]
fn ci_check_script_exists() {
    let script = repo_root().join("tools/scripts/ci-check.sh");
    assert!(
        script.exists(),
        "CI check script not found at {}",
        script.display()
    );
}

#[test]
fn ci_check_script_is_executable() {
    let script = repo_root().join("tools/scripts/ci-check.sh");
    let metadata = fs::metadata(&script)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", script.display()));
    let mode = metadata.permissions().mode();
    assert!(
        mode & 0o111 != 0,
        "CI check script is not executable (mode: {mode:o})"
    );
}

#[test]
fn ci_check_script_has_valid_shebang() {
    let script = repo_root().join("tools/scripts/ci-check.sh");
    let content = fs::read_to_string(&script)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", script.display()));
    assert!(
        content.starts_with("#!/usr/bin/env bash"),
        "CI check script should start with #!/usr/bin/env bash"
    );
}

#[test]
fn ci_check_script_contains_expected_commands() {
    let script = repo_root().join("tools/scripts/ci-check.sh");
    let content = fs::read_to_string(&script)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", script.display()));

    let expected = [
        "cargo check --workspace",
        "cargo clippy --workspace -- -D warnings",
        "cargo fmt --workspace -- --check",
        "cargo test --workspace",
    ];

    for cmd in &expected {
        assert!(
            content.contains(cmd),
            "CI check script missing expected command: {cmd}"
        );
    }
}

#[test]
fn ci_check_script_has_no_syntax_errors() {
    let script = repo_root().join("tools/scripts/ci-check.sh");
    let output = Command::new("bash")
        .args(["-n", &script.to_string_lossy()])
        .output()
        .expect("failed to run bash -n");

    assert!(
        output.status.success(),
        "CI check script has syntax errors:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore]
fn ci_check_script_passes_on_current_codebase() {
    let script = repo_root().join("tools/scripts/ci-check.sh");
    let output = Command::new("bash")
        .args([&script.to_string_lossy().to_string(), "--rust-only"])
        .current_dir(repo_root())
        .output()
        .expect("failed to run ci-check.sh");

    assert!(
        output.status.success(),
        "CI check script failed on current codebase (known-good state):\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
