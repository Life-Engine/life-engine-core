#!/usr/bin/env bash
set -euo pipefail

# Run the same checks as CI locally. Mirrors .github/workflows/ci.yml.
# Usage: ci-check.sh [--quick] [--rust-only]

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

QUICK=false
RUST_ONLY=false

for arg in "$@"; do
  case "$arg" in
    --quick) QUICK=true ;;
    --rust-only) RUST_ONLY=true ;;
    --help|-h)
      echo "Usage: ci-check.sh [--quick] [--rust-only]"
      echo "  --quick      Skip cargo test (fast pre-commit check)"
      echo "  --rust-only  Skip JS/TS checks"
      exit 0
      ;;
    *) echo "Unknown flag: $arg"; exit 1 ;;
  esac
done

passed=0
failed=0

step() {
  local name="$1"
  shift
  echo ""
  echo "[CI] $name"
  if "$@"; then
    echo "[CI] PASS: $name"
    passed=$((passed + 1))
  else
    echo "[CI] FAIL: $name"
    failed=$((failed + 1))
    echo ""
    echo "Summary: $passed passed, $failed failed"
    exit 1
  fi
}

# Rust checks
step "cargo check" cargo check --workspace
step "cargo clippy" cargo clippy --workspace -- -D warnings
step "cargo fmt" cargo fmt --workspace -- --check

if [ "$QUICK" = false ]; then
  step "cargo test" cargo test --workspace
fi

# JS/TS checks
if [ "$RUST_ONLY" = false ] && command -v pnpm > /dev/null 2>&1; then
  step "pnpm install" pnpm install --frozen-lockfile
  step "pnpm lint" pnpm lint
  step "pnpm type-check" pnpm type-check
  if [ "$QUICK" = false ]; then
    step "pnpm test" pnpm test
  fi
fi

echo ""
echo "Summary: $passed passed, $failed failed"
exit 0
