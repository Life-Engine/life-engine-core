//! `install-service` CLI subcommand — installs Life Engine Core as a system service.
//!
//! - Linux: copies the systemd unit file, creates user/group, enables the service.
//! - macOS: copies the launchd plist, creates data directory, loads the agent.

use std::fs;
use std::path::Path;
use std::process::Command;

/// Run the install-service subcommand. Exits the process when done.
pub fn run() {
    if cfg!(target_os = "linux") {
        install_systemd();
    } else if cfg!(target_os = "macos") {
        install_launchd();
    } else {
        eprintln!("install-service is not supported on this platform.");
        eprintln!("Supported platforms: Linux (systemd), macOS (launchd).");
        std::process::exit(1);
    }
}

// ── Linux (systemd) ─────────────────────────────────────────────────────

fn install_systemd() {
    // Require root.
    if !is_root() {
        eprintln!("Error: install-service on Linux requires root privileges.");
        eprintln!("Re-run with: sudo life-engine-core install-service");
        std::process::exit(1);
    }

    let service_src = find_service_file("deploy/systemd/life-engine-core.service");
    let service_dest = Path::new("/etc/systemd/system/life-engine-core.service");
    let data_dir = Path::new("/var/lib/life-engine");
    let config_dir = Path::new("/etc/life-engine");

    // Step 1: Create life-engine user/group if they don't exist.
    println!("[1/5] Creating life-engine user and group...");
    if !user_exists("life-engine") {
        run_cmd("groupadd", &["--system", "life-engine"]);
        run_cmd(
            "useradd",
            &[
                "--system",
                "--gid",
                "life-engine",
                "--home-dir",
                "/var/lib/life-engine",
                "--shell",
                "/usr/sbin/nologin",
                "--no-create-home",
                "life-engine",
            ],
        );
        println!("  Created user and group: life-engine");
    } else {
        println!("  User life-engine already exists, skipping.");
    }

    // Step 2: Create data directory.
    println!("[2/5] Creating data directory at {}...", data_dir.display());
    fs::create_dir_all(data_dir).unwrap_or_else(|e| {
        eprintln!("Error creating {}: {e}", data_dir.display());
        std::process::exit(1);
    });
    run_cmd("chown", &["life-engine:life-engine", "/var/lib/life-engine"]);
    run_cmd("chmod", &["750", "/var/lib/life-engine"]);

    // Create config directory.
    fs::create_dir_all(config_dir).unwrap_or_else(|e| {
        eprintln!("Error creating {}: {e}", config_dir.display());
        std::process::exit(1);
    });
    println!("  Created {}", data_dir.display());

    // Step 3: Copy service file.
    println!(
        "[3/5] Installing service file to {}...",
        service_dest.display()
    );
    fs::copy(&service_src, service_dest).unwrap_or_else(|e| {
        eprintln!(
            "Error copying {} to {}: {e}",
            service_src.display(),
            service_dest.display()
        );
        std::process::exit(1);
    });
    println!("  Installed {}", service_dest.display());

    // Step 4: Reload systemd.
    println!("[4/5] Reloading systemd daemon...");
    run_cmd("systemctl", &["daemon-reload"]);

    // Step 5: Enable the service.
    println!("[5/5] Enabling life-engine-core service...");
    run_cmd("systemctl", &["enable", "life-engine-core"]);

    println!();
    println!("Life Engine Core service installed successfully.");
    println!();
    println!("Next steps:");
    println!("  1. Set the storage passphrase:");
    println!("     sudo systemctl edit life-engine-core");
    println!("     Add: Environment=LIFE_ENGINE_STORAGE_PASSPHRASE=<your-passphrase>");
    println!();
    println!("  2. Place your config at /etc/life-engine/config.toml");
    println!();
    println!("  3. Start the service:");
    println!("     sudo systemctl start life-engine-core");
    println!();
    println!("  4. Check status:");
    println!("     sudo systemctl status life-engine-core");
    println!("     journalctl -u life-engine-core -f");
}

// ── macOS (launchd) ─────────────────────────────────────────────────────

fn install_launchd() {
    let plist_src = find_service_file("deploy/launchd/com.life-engine.core.plist");
    let home = std::env::var("HOME").unwrap_or_else(|_| {
        eprintln!("Error: HOME environment variable not set.");
        std::process::exit(1);
    });
    let launch_agents_dir = Path::new(&home).join("Library/LaunchAgents");
    let plist_dest = launch_agents_dir.join("com.life-engine.core.plist");
    let data_dir = Path::new(&home).join("Library/Application Support/life-engine");
    let log_dir = Path::new(&home).join("Library/Logs/life-engine");

    // Step 1: Create data directory.
    println!(
        "[1/3] Creating data directory at {}...",
        data_dir.display()
    );
    fs::create_dir_all(&data_dir).unwrap_or_else(|e| {
        eprintln!("Error creating {}: {e}", data_dir.display());
        std::process::exit(1);
    });
    fs::create_dir_all(&log_dir).unwrap_or_else(|e| {
        eprintln!("Error creating {}: {e}", log_dir.display());
        std::process::exit(1);
    });
    println!("  Created {}", data_dir.display());

    // Step 2: Copy plist file.
    println!("[2/3] Installing plist to {}...", plist_dest.display());
    fs::create_dir_all(&launch_agents_dir).unwrap_or_else(|e| {
        eprintln!(
            "Error creating {}: {e}",
            launch_agents_dir.display()
        );
        std::process::exit(1);
    });
    fs::copy(&plist_src, &plist_dest).unwrap_or_else(|e| {
        eprintln!(
            "Error copying {} to {}: {e}",
            plist_src.display(),
            plist_dest.display()
        );
        std::process::exit(1);
    });
    println!("  Installed {}", plist_dest.display());

    // Step 3: Load the agent.
    println!("[3/3] Loading launchd agent...");
    let status = Command::new("launchctl")
        .args(["load", plist_dest.to_str().unwrap()])
        .status();
    match status {
        Ok(s) if s.success() => {}
        Ok(s) => {
            // launchctl load returns non-zero if already loaded — not fatal.
            eprintln!(
                "  Warning: launchctl load exited with {}. The agent may already be loaded.",
                s.code().unwrap_or(-1)
            );
        }
        Err(e) => {
            eprintln!("Error running launchctl: {e}");
            std::process::exit(1);
        }
    }

    println!();
    println!("Life Engine Core agent installed successfully.");
    println!();
    println!("Next steps:");
    println!("  1. Set the storage passphrase in the plist:");
    println!("     Edit {}", plist_dest.display());
    println!("     Set LIFE_ENGINE_STORAGE_PASSPHRASE to your passphrase.");
    println!();
    println!(
        "  2. Place your config at {}/config.toml",
        data_dir.display()
    );
    println!();
    println!("  3. The agent will start automatically on login.");
    println!("     To start now: launchctl start com.life-engine.core");
    println!("     To check logs: tail -f ~/Library/Logs/life-engine/core.log");
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Find a service file relative to the binary location or the current directory.
fn find_service_file(relative_path: &str) -> std::path::PathBuf {
    // Try relative to the executable first.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let candidate = exe_dir.join(relative_path);
            if candidate.exists() {
                return candidate;
            }
            // Also try one level up (common in cargo builds).
            if let Some(parent) = exe_dir.parent() {
                let candidate = parent.join(relative_path);
                if candidate.exists() {
                    return candidate;
                }
            }
        }
    }
    // Try current directory.
    let candidate = Path::new(relative_path);
    if candidate.exists() {
        return candidate.to_path_buf();
    }
    eprintln!("Error: could not find {relative_path}");
    eprintln!("Make sure you run this command from the project root or that the file is next to the binary.");
    std::process::exit(1);
}

/// Check if the current process is running as root.
fn is_root() -> bool {
    Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim() == "0")
        .unwrap_or(false)
}

/// Check if a system user exists (Linux).
fn user_exists(name: &str) -> bool {
    Command::new("id")
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Run a system command, exiting on failure.
fn run_cmd(cmd: &str, args: &[&str]) {
    let status = Command::new(cmd).args(args).status();
    match status {
        Ok(s) if s.success() => {}
        Ok(s) => {
            eprintln!(
                "Error: `{} {}` exited with code {}",
                cmd,
                args.join(" "),
                s.code().unwrap_or(-1)
            );
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error running `{cmd}`: {e}");
            std::process::exit(1);
        }
    }
}
