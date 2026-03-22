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
  rm -rf "$REPO_ROOT/plugins/life/test-scaffold-van"
  rm -rf "$REPO_ROOT/plugins/life/test-scaffold-lit"
  mv "$REPO_ROOT/Cargo.toml.bak" "$REPO_ROOT/Cargo.toml"
}
trap cleanup EXIT

echo "Testing scaffold-plugin.sh"
echo "=========================="

# ── Test 1: invalid type rejected ────────────────────────────────
echo ""
echo "Test: invalid type rejected"
OUTPUT=$(bash "$SCAFFOLD" test-plugin badtype 2>&1 || true)
if echo "$OUTPUT" | grep -q "unknown plugin type"; then
  pass "invalid type prints error"
else
  fail "invalid type should print error"
fi

# ── Test 2: engine plugin scaffolding ────────────────────────────
echo ""
echo "Test: engine plugin scaffolding"
bash "$SCAFFOLD" test-scaffold-eng engine

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
OUTPUT=$(bash "$SCAFFOLD" test-scaffold-eng engine 2>&1 || true)
if echo "$OUTPUT" | grep -q "already exists"; then
  pass "duplicate engine directory rejected"
else
  fail "duplicate engine directory should be rejected"
fi

# ── Test 4: vanilla plugin scaffolding ───────────────────────────
echo ""
echo "Test: vanilla plugin scaffolding"
bash "$SCAFFOLD" test-scaffold-van life-vanilla

if [[ -d "$REPO_ROOT/plugins/life/test-scaffold-van" ]]; then
  pass "vanilla directory created"
else
  fail "vanilla directory not created"
fi

if grep -q '"com.life-engine.test-scaffold-van"' "$REPO_ROOT/plugins/life/test-scaffold-van/plugin.json"; then
  pass "vanilla plugin.json has correct ID"
else
  fail "vanilla plugin.json ID not substituted"
fi

if grep -q '"test-scaffold-van"' "$REPO_ROOT/plugins/life/test-scaffold-van/plugin.json"; then
  pass "vanilla plugin.json has correct element name"
else
  fail "vanilla plugin.json element name not substituted"
fi

if ! grep -q 'com\.example\.my-plugin' "$REPO_ROOT/plugins/life/test-scaffold-van/plugin.json"; then
  pass "vanilla plugin.json has no remaining template placeholders"
else
  fail "vanilla plugin.json still has template placeholders"
fi

# ── Test 5: lit plugin scaffolding ───────────────────────────────
echo ""
echo "Test: lit plugin scaffolding"
bash "$SCAFFOLD" test-scaffold-lit life-lit

if [[ -d "$REPO_ROOT/plugins/life/test-scaffold-lit" ]]; then
  pass "lit directory created"
else
  fail "lit directory not created"
fi

if grep -q '"com.life-engine.test-scaffold-lit"' "$REPO_ROOT/plugins/life/test-scaffold-lit/plugin.json"; then
  pass "lit plugin.json has correct ID"
else
  fail "lit plugin.json ID not substituted"
fi

if ! grep -q 'com\.example\.my-lit-plugin' "$REPO_ROOT/plugins/life/test-scaffold-lit/plugin.json"; then
  pass "lit plugin.json has no remaining template placeholders"
else
  fail "lit plugin.json still has template placeholders"
fi

if grep -q 'TestScaffoldLitPlugin' "$REPO_ROOT/plugins/life/test-scaffold-lit/src/index.js"; then
  pass "lit index.js has PascalCase class name"
else
  fail "lit index.js class name not substituted"
fi

# ── Summary ──────────────────────────────────────────────────────
echo ""
echo "=========================="
echo "Results: $PASS passed, $FAIL failed"

if [[ $FAIL -gt 0 ]]; then
  exit 1
fi
