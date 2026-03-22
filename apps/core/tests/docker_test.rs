//! Integration tests for deployment configuration files.
//!
//! These tests verify that all deployment artefacts exist and contain the
//! required directives.  They parse files in-process — no Docker daemon
//! needed.

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

// ── Dockerfile ──────────────────────────────────────────────────────

#[test]
fn dockerfile_exists_and_contains_required_directives() {
    let path = repo_root().join("apps/core/Dockerfile");
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    let required = ["FROM", "COPY", "CMD", "EXPOSE"];
    for directive in &required {
        assert!(
            content.contains(directive),
            "Dockerfile missing required directive: {directive}"
        );
    }

    // Multi-stage build should have at least two FROM statements.
    let from_count = content.lines().filter(|l| l.starts_with("FROM")).count();
    assert!(
        from_count >= 2,
        "Dockerfile should be multi-stage (found {from_count} FROM)"
    );

    // Must expose port 3750.
    assert!(
        content.contains("3750"),
        "Dockerfile should expose port 3750"
    );
}

// ── docker-compose.yml ──────────────────────────────────────────────

#[test]
fn docker_compose_yml_is_valid_yaml_with_core_service() {
    let path = repo_root().join("deploy/docker-compose.yml");
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    let doc: serde_yaml::Value =
        serde_yaml::from_str(&content).expect("docker-compose.yml is not valid YAML");

    let services = doc.get("services").expect("missing 'services' key");
    assert!(
        services.get("core").is_some(),
        "docker-compose.yml must define a 'core' service"
    );
}

// ── docker-compose.full.yml ─────────────────────────────────────────

#[test]
fn docker_compose_full_yml_has_core_and_pocket_id() {
    let path = repo_root().join("deploy/docker-compose.full.yml");
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    let doc: serde_yaml::Value =
        serde_yaml::from_str(&content).expect("docker-compose.full.yml is not valid YAML");

    let services = doc.get("services").expect("missing 'services' key");
    assert!(
        services.get("core").is_some(),
        "docker-compose.full.yml must define a 'core' service"
    );
    assert!(
        services.get("pocket-id").is_some(),
        "docker-compose.full.yml must define a 'pocket-id' service"
    );
}

// ── systemd unit ────────────────────────────────────────────────────

#[test]
fn systemd_unit_has_required_sections() {
    let path = repo_root().join("deploy/systemd/life-engine-core.service");
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    for section in &["[Unit]", "[Service]", "[Install]"] {
        assert!(
            content.contains(section),
            "systemd unit missing section: {section}"
        );
    }

    assert!(
        content.contains("ExecStart="),
        "systemd unit missing ExecStart"
    );
    assert!(
        content.contains("life-engine-core"),
        "systemd unit should reference life-engine-core binary"
    );
}

// ── launchd plist ───────────────────────────────────────────────────

#[test]
fn launchd_plist_is_valid_xml() {
    let path = repo_root().join("deploy/launchd/com.life-engine.core.plist");
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    assert!(
        content.contains("<?xml"),
        "plist should start with XML declaration"
    );
    assert!(
        content.contains("com.life-engine.core"),
        "plist should contain the service label"
    );
    assert!(
        content.contains("<key>KeepAlive</key>"),
        "plist should configure KeepAlive"
    );
    assert!(
        content.contains("<key>RunAtLoad</key>"),
        "plist should configure RunAtLoad"
    );
    assert!(
        content.contains("life-engine-core"),
        "plist should reference the binary"
    );
}

// ── install.sh ──────────────────────────────────────────────────────

#[test]
fn install_script_is_executable_shell() {
    let path = repo_root().join("deploy/install.sh");
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    assert!(
        content.starts_with("#!/"),
        "install.sh should start with a shebang"
    );
    assert!(
        content.contains("set -euo pipefail"),
        "install.sh should use strict mode"
    );
    assert!(
        content.contains("detect_os"),
        "install.sh should detect the OS"
    );
}

// ── nginx config ────────────────────────────────────────────────────

#[test]
fn nginx_config_contains_proxy_pass() {
    let path = repo_root().join("deploy/nginx/life-engine.conf");
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    assert!(
        content.contains("proxy_pass"),
        "nginx config must contain proxy_pass directive"
    );
    assert!(
        content.contains("3750"),
        "nginx config should proxy to port 3750"
    );
    assert!(
        content.contains("ssl"),
        "nginx config should include TLS configuration"
    );
}

// ── Caddyfile ───────────────────────────────────────────────────────

#[test]
fn caddyfile_contains_reverse_proxy() {
    let path = repo_root().join("deploy/caddy/Caddyfile");
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    assert!(
        content.contains("reverse_proxy"),
        "Caddyfile must contain reverse_proxy directive"
    );
    assert!(
        content.contains("3750"),
        "Caddyfile should proxy to port 3750"
    );
}

// ── Docker Compose healthcheck ──────────────────────────────────────

#[test]
fn docker_compose_core_has_healthcheck() {
    let path = repo_root().join("deploy/docker-compose.yml");
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    assert!(
        content.contains("healthcheck"),
        "docker-compose.yml core service should have a healthcheck"
    );
    assert!(
        content.contains("/api/system/health"),
        "healthcheck should hit /api/system/health"
    );
}

// ── Docker Compose volume persistence ───────────────────────────────

#[test]
fn docker_compose_defines_persistent_volume() {
    let path = repo_root().join("deploy/docker-compose.yml");
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    let doc: serde_yaml::Value =
        serde_yaml::from_str(&content).expect("docker-compose.yml is not valid YAML");

    assert!(
        doc.get("volumes").is_some(),
        "docker-compose.yml should define named volumes"
    );

    let volumes = doc["volumes"].as_mapping().expect("volumes should be a mapping");
    assert!(
        volumes.contains_key(serde_yaml::Value::String("core-data".to_string())),
        "docker-compose.yml should define a 'core-data' volume"
    );
}

// -- Docker image size --

/// Builds the Docker image and asserts the final image is under 50 MB.
///
/// This test is `#[ignore]` because it requires a running Docker daemon and
/// takes significant time to complete (full Rust release build inside Docker).
///
/// Run manually with:
/// ```sh
/// cargo test --package life-engine-core --test docker_test -- --ignored
/// ```
#[test]
#[ignore]
fn docker_image_is_under_50mb() {
    let root = repo_root();
    let tag = "life-engine-core:test";

    // Build the Docker image from the repo root.
    let build = Command::new("docker")
        .args(["build", "-t", tag, "-f", "apps/core/Dockerfile", "."])
        .current_dir(&root)
        .output()
        .expect("failed to execute docker build");

    assert!(
        build.status.success(),
        "docker build failed:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );

    // Inspect the image size in bytes.
    let inspect = Command::new("docker")
        .args(["image", "inspect", tag, "--format", "{{.Size}}"])
        .output()
        .expect("failed to execute docker image inspect");

    assert!(
        inspect.status.success(),
        "docker image inspect failed:\n{}",
        String::from_utf8_lossy(&inspect.stderr)
    );

    let size_str = String::from_utf8_lossy(&inspect.stdout);
    let size_bytes: u64 = size_str
        .trim()
        .parse()
        .unwrap_or_else(|e| panic!("failed to parse image size '{size_str}': {e}"));

    let max_bytes: u64 = 50 * 1024 * 1024;
    assert!(
        size_bytes < max_bytes,
        "Docker image is too large: {size_bytes} bytes ({:.1} MB), must be under 50 MB",
        size_bytes as f64 / (1024.0 * 1024.0)
    );

    // Best-effort cleanup — don't panic if removal fails.
    let _ = Command::new("docker").args(["rmi", tag]).output();
}
