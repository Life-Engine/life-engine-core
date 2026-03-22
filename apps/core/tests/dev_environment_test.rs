//! Integration tests for the developer environment setup.
//!
//! Verifies required tools, Docker Compose configurations, nx project targets,
//! and devcontainer config. The Docker startup test is `#[ignore]` because it
//! requires a running Docker daemon and takes up to 5 minutes.

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

// ── Required CLI tools ──────────────────────────────────────────────

#[test]
fn required_tools_are_installed() {
    let tools = ["cargo", "rustc", "node", "pnpm", "docker"];
    for tool in &tools {
        let output = Command::new(tool)
            .arg("--version")
            .output()
            .unwrap_or_else(|e| panic!("{tool} is not installed or not on PATH: {e}"));
        assert!(
            output.status.success(),
            "{tool} --version failed with status {}. Install {tool} before developing.",
            output.status
        );
    }
}

// ── docker-compose.yml (dev services) ───────────────────────────────

#[test]
fn docker_compose_dev_services_are_valid() {
    let path = repo_root().join("docker-compose.yml");
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    let doc: serde_yaml::Value =
        serde_yaml::from_str(&content).expect("docker-compose.yml is not valid YAML");

    let services = doc.get("services").expect("missing 'services' key in docker-compose.yml");

    let expected = ["pocket-id", "greenmail", "radicale", "minio"];
    for svc in &expected {
        let service = services.get(*svc);
        assert!(
            service.is_some(),
            "docker-compose.yml must define a '{svc}' service"
        );
        assert!(
            service.unwrap().get("ports").is_some(),
            "docker-compose.yml service '{svc}' must have a 'ports' mapping"
        );
    }
}

// ── docker-compose.test.yml (test services) ─────────────────────────

#[test]
fn docker_compose_test_services_are_valid() {
    let path = repo_root().join("docker-compose.test.yml");
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    let doc: serde_yaml::Value =
        serde_yaml::from_str(&content).expect("docker-compose.test.yml is not valid YAML");

    let services = doc
        .get("services")
        .expect("missing 'services' key in docker-compose.test.yml");

    let expected = ["greenmail", "radicale", "minio"];
    for svc in &expected {
        assert!(
            services.get(*svc).is_some(),
            "docker-compose.test.yml must define a '{svc}' service"
        );
    }

    // Verify test ports differ from dev ports (test compose uses remapped ports)
    let test_content = content;
    // Dev ports: 3025, 3143, 5232, 9000
    // Test ports should NOT contain these exact host mappings
    assert!(
        test_content.contains("4025") || test_content.contains("4143"),
        "docker-compose.test.yml should remap GreenMail ports to avoid dev conflicts"
    );
}

// ── project.json (nx targets) ───────────────────────────────────────

#[test]
fn project_json_defines_required_targets() {
    let path = repo_root().join("project.json");
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    let doc: serde_json::Value =
        serde_json::from_str(&content).expect("project.json is not valid JSON");

    let targets = doc
        .get("targets")
        .and_then(|v| v.as_object())
        .expect("project.json must have a 'targets' object");

    let required = [
        "dev-core", "dev-app", "dev-all", "cargo-test", "cargo-lint", "fmt", "new-plugin",
    ];
    for target in &required {
        assert!(
            targets.contains_key(*target),
            "project.json must define the '{target}' target"
        );
    }
}

// ── devcontainer.json ───────────────────────────────────────────────

#[test]
fn devcontainer_config_is_valid() {
    let path = repo_root().join(".devcontainer/devcontainer.json");
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    let doc: serde_json::Value =
        serde_json::from_str(&content).expect("devcontainer.json is not valid JSON");

    // Should specify a Rust-based image
    let image = doc
        .get("image")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let build_dockerfile = doc
        .get("build")
        .and_then(|b| b.get("dockerfile"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        image.contains("rust") || build_dockerfile.contains("rust") || doc.get("features").is_some(),
        "devcontainer.json should reference Rust (via image, build, or features)"
    );

    // Should have a postCreateCommand
    assert!(
        doc.get("postCreateCommand").is_some(),
        "devcontainer.json should define postCreateCommand for setup"
    );
}

// ── Docker Compose startup (slow, requires Docker) ──────────────────

#[test]
#[ignore]
fn docker_compose_services_start_within_timeout() {
    use std::net::{SocketAddr, TcpStream};
    use std::time::{Duration, Instant};

    struct DockerCleanup {
        root: PathBuf,
    }
    impl Drop for DockerCleanup {
        fn drop(&mut self) {
            let _ = Command::new("docker")
                .args(["compose", "down", "-v"])
                .current_dir(&self.root)
                .output();
        }
    }

    let root = repo_root();
    let _cleanup = DockerCleanup {
        root: root.clone(),
    };

    let start = Instant::now();
    let timeout = Duration::from_secs(300); // 5 minutes

    // Start services
    let up = Command::new("docker")
        .args(["compose", "up", "-d"])
        .current_dir(&root)
        .output()
        .expect("failed to run docker compose up");
    assert!(
        up.status.success(),
        "docker compose up failed: {}",
        String::from_utf8_lossy(&up.stderr)
    );

    // Wait for all services to become reachable
    let services = [
        ("Pocket ID", "127.0.0.1:3751"),
        ("GreenMail SMTP", "127.0.0.1:3025"),
        ("GreenMail IMAP", "127.0.0.1:3143"),
        ("Radicale", "127.0.0.1:5232"),
        ("MinIO API", "127.0.0.1:9000"),
    ];

    for (name, addr) in &services {
        let socket_addr: SocketAddr = addr.parse().unwrap();
        let mut connected = false;
        while start.elapsed() < timeout {
            if TcpStream::connect_timeout(&socket_addr, Duration::from_secs(1)).is_ok() {
                connected = true;
                break;
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        assert!(
            connected,
            "{name} ({addr}) did not become reachable within {:.0}s",
            timeout.as_secs_f64()
        );
    }

    let elapsed = start.elapsed();
    assert!(
        elapsed < timeout,
        "Dev environment setup took {:.0}s, exceeding the 5-minute budget",
        elapsed.as_secs_f64()
    );
}
