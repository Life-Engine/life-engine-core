#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
SCAFFOLD="$SCRIPT_DIR/scaffold-plugin.sh"

PASS=0
FAIL=0

pass() { echo "  PASS: $1"; PASS=$((PASS + 1)); }
fail() { echo "  FAIL: $1"; FAIL=$((FAIL + 1)); }

# Save Cargo.toml for restoration
cp "$REPO_ROOT/Cargo.toml" "$REPO_ROOT/Cargo.toml.bak"

cleanup() {
  rm -rf "$REPO_ROOT/plugins/engine/test-scaffold-eng"
  mv "$REPO_ROOT/Cargo.toml.bak" "$REPO_ROOT/Cargo.toml"
}
trap cleanup EXIT

echo "Testing scaffold-plugin.sh"
echo "=========================="

# ── Test 1: missing name argument rejected ───────────────────────
echo ""
echo "Test: missing name argument rejected"
OUTPUT=$(bash "$SCAFFOLD" 2>&1 || true)
if echo "$OUTPUT" | grep -q "Usage:"; then
  pass "missing name prints usage"
else
  fail "missing name should print usage"
fi

# ── Test 2: engine plugin scaffolding ────────────────────────────
echo ""
echo "Test: engine plugin scaffolding"
bash "$SCAFFOLD" test-scaffold-eng

if [[ -d "$REPO_ROOT/plugins/engine/test-scaffold-eng" ]]; then
  pass "engine directory created"
else
  fail "engine directory not created"
fi

if grep -q 'name = "test-scaffold-eng"' "$REPO_ROOT/plugins/engine/test-scaffold-eng/Cargo.toml"; then
  pass "engine Cargo.toml has correct name"
else
  fail "engine Cargo.toml placeholder not substituted"
fi

if grep -q "TestScaffoldEngPlugin" "$REPO_ROOT/plugins/engine/test-scaffold-eng/src/lib.rs"; then
  pass "engine lib.rs has PascalCase struct name"
else
  fail "engine lib.rs struct name not substituted"
fi

if ! grep -q '{{plugin-name}}' "$REPO_ROOT/plugins/engine/test-scaffold-eng/Cargo.toml"; then
  pass "engine Cargo.toml has no remaining placeholders"
else
  fail "engine Cargo.toml still has template placeholders"
fi

if grep -q "test-scaffold-eng" "$REPO_ROOT/Cargo.toml"; then
  pass "engine plugin added to workspace members"
else
  fail "engine plugin not added to workspace members"
fi

# ── Test 3: duplicate directory rejected ─────────────────────────
echo ""
echo "Test: duplicate directory rejected"
OUTPUT=$(bash "$SCAFFOLD" test-scaffold-eng 2>&1 || true)
if echo "$OUTPUT" | grep -q "already exists"; then
  pass "duplicate engine directory rejected"
else
  fail "duplicate engine directory should be rejected"
fi

# ── Summary ──────────────────────────────────────────────────────
echo ""
echo "=========================="
echo "Results: $PASS passed, $FAIL failed"

if [[ $FAIL -gt 0 ]]; then
  exit 1
fi
