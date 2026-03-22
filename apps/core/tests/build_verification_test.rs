//! Build verification integration tests.
//!
//! These tests assert that the monorepo's Rust workspace and the Tauri app
//! compile cleanly.  Build tests are marked `#[ignore]` because they are slow;
//! run them explicitly with `cargo test -- --ignored`.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Returns the repository root (two levels above `apps/core/`).
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("failed to resolve repo root")
}

// ── Workspace cargo check ──────────────────────────────────────────

#[test]
#[ignore]
fn workspace_cargo_check_succeeds() {
    let output = Command::new("cargo")
        .args(["check", "--workspace"])
        .current_dir(repo_root())
        .output()
        .expect("failed to execute cargo check --workspace");

    assert!(
        output.status.success(),
        "cargo check --workspace failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── Core binary build ──────────────────────────────────────────────

#[test]
#[ignore]
fn core_binary_builds_successfully() {
    let output = Command::new("cargo")
        .args(["build", "-p", "life-engine-core"])
        .current_dir(repo_root())
        .output()
        .expect("failed to execute cargo build -p life-engine-core");

    assert!(
        output.status.success(),
        "cargo build -p life-engine-core failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let binary = repo_root().join("target/debug/life-engine-core");
    assert!(
        binary.exists(),
        "Core binary not found at {path}. \
         Expected `cargo build -p life-engine-core` to produce this artifact.",
        path = binary.display()
    );
}

// ── Tauri app cargo check ──────────────────────────────────────────

#[test]
#[ignore]
fn tauri_app_cargo_check_succeeds() {
    let tauri_dir = repo_root().join("apps/app/src-tauri");
    assert!(
        tauri_dir.exists(),
        "Tauri project directory not found at {}",
        tauri_dir.display()
    );

    let output = Command::new("cargo")
        .args(["check"])
        .current_dir(&tauri_dir)
        .output()
        .expect("failed to execute cargo check in apps/app/src-tauri");

    assert!(
        output.status.success(),
        "cargo check in apps/app/src-tauri failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── Workspace members match expected set ───────────────────────────

#[test]
fn workspace_members_match_expected_set() {
    let cargo_toml_path = repo_root().join("Cargo.toml");
    let content = fs::read_to_string(&cargo_toml_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", cargo_toml_path.display()));

    let expected_members = [
        "apps/core",
        "packages/types",
        "packages/plugin-sdk-rs",
        "packages/test-utils",
        "packages/test-fixtures",
        "packages/dav-utils",
        "plugins/engine/connector-email",
        "plugins/engine/connector-filesystem",
        "plugins/engine/connector-contacts",
        "plugins/engine/connector-calendar",
        "plugins/engine/api-caldav",
        "plugins/engine/api-carddav",
        "plugins/engine/webhook-receiver",
        "plugins/engine/webhook-sender",
        "plugins/engine/backup",
    ];

    for member in &expected_members {
        assert!(
            content.contains(member),
            "Root Cargo.toml is missing expected workspace member: {member}. \
             This may indicate an accidental removal. \
             Check the [workspace] members list in Cargo.toml."
        );
    }

    // Verify the Tauri app is excluded from the workspace (it has its own Cargo project).
    assert!(
        content.contains("apps/app/src-tauri"),
        "Root Cargo.toml should reference apps/app/src-tauri in the exclude list. \
         The Tauri app is a separate Cargo project and must not be a workspace member."
    );
}
