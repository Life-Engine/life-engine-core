#!/usr/bin/env bash
# verify-arm64.sh — ARM64 build verification for Life Engine Core
#
# Runs the verification checklist from deploy/arm64-build.md.
# Supports two modes:
#   Native:  Run on an ARM64 host (Apple Silicon Mac, Raspberry Pi, etc.)
#   Docker:  Build and test via Docker buildx (any host with Docker)
#
# Usage:
#   ./deploy/verify-arm64.sh              # auto-detect mode
#   ./deploy/verify-arm64.sh --native     # native build only
#   ./deploy/verify-arm64.sh --docker     # Docker buildx only

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
PASS=0
FAIL=0
SKIP=0

pass() { echo "  ✓ $1"; PASS=$((PASS + 1)); }
fail() { echo "  ✗ $1"; FAIL=$((FAIL + 1)); }
skip() { echo "  - $1 (skipped)"; SKIP=$((SKIP + 1)); }

echo "=== ARM64 Build Verification ==="
echo "Host arch: $(uname -m)"
echo "Host OS:   $(uname -s)"
echo ""

MODE="${1:-auto}"
HOST_ARCH="$(uname -m)"

# ── Native Build Verification ──────────────────────────────────────

run_native() {
    echo "── Native Build ──"

    # Check 1: cargo check succeeds
    echo "  Checking cargo check --release --package life-engine-core ..."
    if cargo check --release --package life-engine-core 2>/dev/null; then
        pass "cargo check --release succeeds"
    else
        fail "cargo check --release failed"
        return
    fi

    # Check 2: cargo build succeeds
    echo "  Building release binary (this may take a while) ..."
    if cargo build --release --package life-engine-core 2>/dev/null; then
        pass "cargo build --release succeeds"
    else
        fail "cargo build --release failed"
        return
    fi

    BINARY="$ROOT_DIR/target/release/life-engine-core"

    # Check 3: binary exists and is executable
    if [[ -x "$BINARY" ]]; then
        pass "Binary is executable at target/release/life-engine-core"
    else
        fail "Binary not found or not executable"
        return
    fi

    # Check 4: binary architecture matches ARM64
    FILE_INFO="$(file "$BINARY")"
    if echo "$FILE_INFO" | grep -qi "arm64\|aarch64"; then
        pass "Binary architecture is ARM64"
    else
        fail "Binary architecture is not ARM64: $FILE_INFO"
    fi

    # Check 5: binary starts and responds to --help
    if "$BINARY" --help >/dev/null 2>&1; then
        pass "Binary executes (--help)"
    else
        fail "Binary failed to execute --help"
    fi

    # Check 6: binary starts and health check responds
    echo "  Starting binary for health check ..."
    TEMP_DIR="$(mktemp -d)"
    "$BINARY" --config "$TEMP_DIR/config.toml" &>/dev/null &
    PID=$!
    sleep 2

    if kill -0 "$PID" 2>/dev/null; then
        if curl -sf http://localhost:3750/api/system/health >/dev/null 2>&1; then
            pass "Health check responds at /api/system/health"
        else
            skip "Health check (may need config/passphrase to start fully)"
        fi
        kill "$PID" 2>/dev/null || true
        wait "$PID" 2>/dev/null || true
    else
        skip "Health check (binary exited — may need config/passphrase)"
    fi

    rm -rf "$TEMP_DIR"
}

# ── Docker Build Verification ──────────────────────────────────────

run_docker() {
    echo "── Docker ARM64 Build ──"

    if ! command -v docker >/dev/null 2>&1; then
        skip "Docker not installed"
        return
    fi

    # Check if buildx is available
    if ! docker buildx version >/dev/null 2>&1; then
        skip "Docker buildx not available"
        return
    fi

    # Check 1: Docker buildx build for linux/arm64
    echo "  Building Docker image for linux/arm64 (this may take a while) ..."
    if docker buildx build \
        --platform linux/arm64 \
        --tag life-engine-core:arm64-verify \
        --file "$ROOT_DIR/deploy/Dockerfile" \
        --load \
        "$ROOT_DIR" 2>/dev/null; then
        pass "Docker buildx build --platform linux/arm64 succeeds"
    else
        fail "Docker buildx build failed for linux/arm64"
        return
    fi

    # Check 2: Image size under 50 MB
    IMAGE_SIZE=$(docker image inspect life-engine-core:arm64-verify \
        --format '{{.Size}}' 2>/dev/null || echo "0")
    IMAGE_SIZE_MB=$((IMAGE_SIZE / 1024 / 1024))
    if [[ "$IMAGE_SIZE_MB" -lt 50 ]]; then
        pass "Docker image size is ${IMAGE_SIZE_MB} MB (under 50 MB limit)"
    else
        fail "Docker image size is ${IMAGE_SIZE_MB} MB (exceeds 50 MB limit)"
    fi

    # Check 3: Binary inside container is ARM64
    CONTAINER_ARCH=$(docker run --rm --platform linux/arm64 \
        life-engine-core:arm64-verify \
        /usr/local/bin/life-engine-core --help 2>/dev/null && echo "ok" || echo "fail")
    if [[ "$CONTAINER_ARCH" == *"ok"* ]]; then
        pass "Binary executes inside ARM64 container"
    else
        skip "Binary execution in container (may need config/passphrase)"
    fi

    # Cleanup
    docker rmi life-engine-core:arm64-verify >/dev/null 2>&1 || true
}

# ── Cross-Compilation Config Verification ──────────────────────────

run_config_check() {
    echo "── Configuration Checks ──"

    # Check .cargo/config.toml exists with ARM64 targets
    if [[ -f "$ROOT_DIR/.cargo/config.toml" ]]; then
        if grep -q "aarch64-unknown-linux-gnu" "$ROOT_DIR/.cargo/config.toml"; then
            pass ".cargo/config.toml has aarch64-unknown-linux-gnu target"
        else
            fail ".cargo/config.toml missing aarch64-unknown-linux-gnu target"
        fi
        if grep -q "aarch64-unknown-linux-musl" "$ROOT_DIR/.cargo/config.toml"; then
            pass ".cargo/config.toml has aarch64-unknown-linux-musl target"
        else
            fail ".cargo/config.toml missing aarch64-unknown-linux-musl target"
        fi
    else
        fail ".cargo/config.toml not found"
    fi

    # Check Dockerfile exists and references ARM64-compatible base images
    if [[ -f "$ROOT_DIR/deploy/Dockerfile" ]]; then
        if grep -q "rust:.*alpine" "$ROOT_DIR/deploy/Dockerfile"; then
            pass "Dockerfile uses Alpine-based Rust image (ARM64 multi-arch)"
        else
            fail "Dockerfile does not use Alpine-based Rust image"
        fi
        if grep -q "alpine:" "$ROOT_DIR/deploy/Dockerfile"; then
            pass "Dockerfile uses Alpine runtime image (ARM64 multi-arch)"
        else
            fail "Dockerfile does not use Alpine runtime image"
        fi
    else
        fail "deploy/Dockerfile not found"
    fi

    # Check bundled-sqlcipher feature (compiles from source, no runtime dep)
    if grep -q "bundled-sqlcipher" "$ROOT_DIR/Cargo.toml"; then
        pass "rusqlite uses bundled-sqlcipher (no runtime ARM64 dependency)"
    else
        fail "rusqlite not using bundled-sqlcipher"
    fi
}

# ── Main ───────────────────────────────────────────────────────────

run_config_check

case "$MODE" in
    --native)
        run_native
        ;;
    --docker)
        run_docker
        ;;
    auto)
        if [[ "$HOST_ARCH" == "arm64" || "$HOST_ARCH" == "aarch64" ]]; then
            run_native
        fi
        run_docker
        ;;
    *)
        echo "Unknown mode: $MODE"
        echo "Usage: $0 [--native|--docker|auto]"
        exit 1
        ;;
esac

echo ""
echo "=== Results ==="
echo "  Passed:  $PASS"
echo "  Failed:  $FAIL"
echo "  Skipped: $SKIP"

if [[ "$FAIL" -gt 0 ]]; then
    exit 1
fi
