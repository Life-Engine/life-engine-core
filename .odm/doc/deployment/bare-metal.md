# Bare-metal Installation

Build Life Engine Core from source and install it as a system service on Linux (systemd) or macOS (launchd). The repository ships service definitions and an install script that automates the process.

## Prerequisites

- Rust toolchain (stable, 1.83 or later) -- install via [rustup.rs](https://rustup.rs)
- `cargo` in your `$PATH`
- On Linux: `sudo` access to create a system user and install a systemd service
- On macOS: user-level install (no root required for launchd)

## Build from source

Clone the repository and build the release binary:

```bash
git clone https://github.com/life-engine-org/life-engine.git
cd life-engine
cargo build --release --package life-engine-core
```

The binary is written to `target/release/life-engine-core`. It is statically linked on musl targets or dynamically linked against the system libc. No other runtime files are needed.

## Automated install

The `deploy/install.sh` script detects the OS and installs the binary, creates the data directory, and registers the service. Run it from the repository root after building:

```bash
bash deploy/install.sh
```

### What the script does on Linux

- Creates a `life-engine` system user and group if they do not exist.
- Copies the binary to `/usr/local/bin/life-engine-core`.
- Creates the data directory `/var/lib/life-engine` owned by `life-engine:life-engine`.
- Installs `deploy/systemd/life-engine-core.service` to `/etc/systemd/system/`.
- Runs `systemctl daemon-reload`, `systemctl enable`, and `systemctl start`.

### What the script does on macOS

- Copies the binary to `/usr/local/bin/life-engine-core`.
- Creates `~/Library/Application Support/Life Engine` and `~/Library/Logs/life-engine`.
- Copies `deploy/launchd/com.life-engine.core.plist` to `~/Library/LaunchAgents/`.
- Runs `launchctl load` to start the service immediately and at login.

## Manual Linux install (systemd)

If you prefer to install manually, follow these steps.

### Create the service user

```bash
sudo useradd --system --no-create-home --shell /usr/sbin/nologin life-engine
```

### Install the binary

```bash
sudo install -m 755 target/release/life-engine-core /usr/local/bin/life-engine-core
```

### Create the data directory

```bash
sudo mkdir -p /var/lib/life-engine
sudo chown life-engine:life-engine /var/lib/life-engine
```

### Install the systemd service unit

The service file at `deploy/systemd/life-engine-core.service` contains:

```ini
[Unit]
Description=Life Engine Core
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=life-engine
Group=life-engine
ExecStart=/usr/local/bin/life-engine-core
Environment=LIFE_ENGINE_CORE_HOST=127.0.0.1
Environment=LIFE_ENGINE_CORE_PORT=3750
Environment=LIFE_ENGINE_CORE_LOG_LEVEL=info
Environment=LIFE_ENGINE_CORE_DATA_DIR=/var/lib/life-engine
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=multi-user.target
```

Copy it into place and enable:

```bash
sudo cp deploy/systemd/life-engine-core.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable life-engine-core
sudo systemctl start life-engine-core
```

Check status:

```bash
systemctl status life-engine-core
journalctl -u life-engine-core -f
```

### Override environment variables

To override settings without editing the service file, create a systemd drop-in:

```bash
sudo systemctl edit life-engine-core
```

This opens a drop-in file where you can add environment variables:

```ini
[Service]
Environment=LIFE_ENGINE_CORE_LOG_LEVEL=debug
Environment=LIFE_ENGINE_AUTH_PROVIDER=oidc
Environment=LIFE_ENGINE_OIDC_ISSUER_URL=http://localhost:3751
```

## Manual macOS install (launchd)

### Install the binary

```bash
sudo install -m 755 target/release/life-engine-core /usr/local/bin/life-engine-core
```

### Create directories

```bash
mkdir -p "$HOME/Library/Application Support/Life Engine"
mkdir -p "$HOME/Library/Logs/life-engine"
```

### Install the launchd plist

The plist at `deploy/launchd/com.life-engine.core.plist` configures the service to:

- Run `/usr/local/bin/life-engine-core`
- Bind to `127.0.0.1:3750`
- Store data in `~/Library/Application Support/Life Engine`
- Write stdout to `~/Library/Logs/life-engine/core.log`
- Write stderr to `~/Library/Logs/life-engine/core.error.log`
- Start at login (`RunAtLoad`) and restart if it exits (`KeepAlive`)

Copy it into place:

```bash
cp deploy/launchd/com.life-engine.core.plist \
  ~/Library/LaunchAgents/com.life-engine.core.plist
```

Load and start:

```bash
launchctl load ~/Library/LaunchAgents/com.life-engine.core.plist
```

Check status:

```bash
launchctl list | grep life-engine
```

View logs:

```bash
tail -f ~/Library/Logs/life-engine/core.log
```

## Configuration

After installing, configure Core using a YAML file or environment variables. The default config file location is `~/.life-engine/config.yaml`. See [configuration.md](configuration.md) for all available options.

## Updating

To update to a new version:

1. Pull the latest code and rebuild:

```bash
git pull
cargo build --release --package life-engine-core
```

2. Stop the service:

```bash
# Linux
sudo systemctl stop life-engine-core

# macOS
launchctl unload ~/Library/LaunchAgents/com.life-engine.core.plist
```

3. Replace the binary:

```bash
sudo install -m 755 target/release/life-engine-core /usr/local/bin/life-engine-core
```

4. Start the service:

```bash
# Linux
sudo systemctl start life-engine-core

# macOS
launchctl load ~/Library/LaunchAgents/com.life-engine.core.plist
```

Core performs any required database migrations on startup, so no manual migration steps are needed.
