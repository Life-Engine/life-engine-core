#!/usr/bin/env bash
set -euo pipefail

# Run the same checks as CI locally. Mirrors .github/workflows/ci.yml.
# Usage: ci-check.sh [--quick]

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

QUICK=false

for arg in "$@"; do
  case "$arg" in
    --quick) QUICK=true ;;
    --help|-h)
      echo "Usage: ci-check.sh [--quick]"
      echo "  --quick  Skip cargo test (fast pre-commit check)"
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

# Secret scan — pattern is split so this file does not self-match
step "secret scan" bash -c '
  PATTERN="BEGIN.*(PRIV""ATE KEY|OPENSSH PRIV""ATE KEY)"
  if git grep -lE "$PATTERN" -- ":(exclude).githooks" ":(exclude)tools/scripts/ci-check.sh" 2>/dev/null; then
    echo "Private key content found in tracked files"
    exit 1
  fi
  if git ls-files | grep -iE "\.(pem|key|p12|pfx|jks|keystore)$"; then
    echo "Files with sensitive extensions found in repo"
    exit 1
  fi
  echo "No secrets detected"
'

# Rust checks
step "cargo check" cargo check --workspace
step "cargo clippy" cargo clippy --workspace -- -D warnings
step "cargo fmt" cargo fmt --workspace -- --check

if [ "$QUICK" = false ]; then
  step "cargo test" cargo test --workspace
fi

echo ""
echo "Summary: $passed passed, $failed failed"
exit 0
