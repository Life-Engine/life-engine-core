#!/usr/bin/env bash
set -euo pipefail

BINARY_NAME="life-engine-core"
INSTALL_DIR="/usr/local/bin"
DATA_DIR_LINUX="/var/lib/life-engine"
DATA_DIR_MACOS="$HOME/Library/Application Support/Life Engine"
LOG_DIR_MACOS="$HOME/Library/Logs/life-engine"
SERVICE_USER="life-engine"
SERVICE_GROUP="life-engine"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

info() { echo "[INFO] $*"; }
error() { echo "[ERROR] $*" >&2; exit 1; }

detect_os() {
    case "$(uname -s)" in
        Linux*)  echo "linux" ;;
        Darwin*) echo "macos" ;;
        *)       error "Unsupported OS: $(uname -s)" ;;
    esac
}

install_binary() {
    local binary_path="$1"
    if [ ! -f "$binary_path" ]; then
        error "Binary not found at $binary_path. Build it first with: cargo build --release -p life-engine-core"
    fi
    info "Installing binary to $INSTALL_DIR/$BINARY_NAME"
    sudo install -m 755 "$binary_path" "$INSTALL_DIR/$BINARY_NAME"
}

install_linux() {
    local binary_path="${1:-$SCRIPT_DIR/../target/release/$BINARY_NAME}"

    # Create service user if it does not exist.
    if ! id "$SERVICE_USER" >/dev/null 2>&1; then
        info "Creating service user $SERVICE_USER"
        sudo useradd --system --no-create-home --shell /usr/sbin/nologin "$SERVICE_USER"
    fi

    install_binary "$binary_path"

    # Create data directory.
    info "Creating data directory $DATA_DIR_LINUX"
    sudo mkdir -p "$DATA_DIR_LINUX"
    sudo chown "$SERVICE_USER:$SERVICE_GROUP" "$DATA_DIR_LINUX"

    # Install systemd service.
    info "Installing systemd service"
    sudo cp "$SCRIPT_DIR/systemd/life-engine-core.service" /etc/systemd/system/
    sudo systemctl daemon-reload
    sudo systemctl enable life-engine-core
    sudo systemctl start life-engine-core

    info "Life Engine Core installed and started (systemd)"
    info "Check status: systemctl status life-engine-core"
}

install_macos() {
    local binary_path="${1:-$SCRIPT_DIR/../target/release/$BINARY_NAME}"

    install_binary "$binary_path"

    # Create data and log directories.
    info "Creating data directory: $DATA_DIR_MACOS"
    mkdir -p "$DATA_DIR_MACOS"

    info "Creating log directory: $LOG_DIR_MACOS"
    mkdir -p "$LOG_DIR_MACOS"

    # Install launchd plist.
    local plist_dest="$HOME/Library/LaunchAgents/com.life-engine.core.plist"
    info "Installing launchd plist to $plist_dest"
    cp "$SCRIPT_DIR/launchd/com.life-engine.core.plist" "$plist_dest"

    launchctl bootstrap "gui/$(id -u)" "$plist_dest"

    info "Life Engine Core installed and started (launchd)"
    info "Check status: launchctl print gui/$(id -u)/com.life-engine.core"
}

main() {
    local os
    os="$(detect_os)"
    info "Detected OS: $os"

    case "$os" in
        linux) install_linux "${1:-}" ;;
        macos) install_macos "${1:-}" ;;
    esac
}

main "$@"
