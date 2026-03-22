#!/usr/bin/env bash
set -euo pipefail

# Tests for validate-adrs.sh

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
VALIDATOR="$SCRIPT_DIR/validate-adrs.sh"

passed=0
failed=0

assert_pass() {
  local desc="$1"
  shift
  if "$@" > /dev/null 2>&1; then
    echo "PASS: $desc"
    passed=$((passed + 1))
  else
    echo "FAIL: $desc (expected exit 0, got exit $?)"
    failed=$((failed + 1))
  fi
}

assert_fail() {
  local desc="$1"
  shift
  if "$@" > /dev/null 2>&1; then
    echo "FAIL: $desc (expected exit 1, got exit 0)"
    failed=$((failed + 1))
  else
    echo "PASS: $desc"
    passed=$((passed + 1))
  fi
}

TMPDIR_ROOT=$(mktemp -d)
trap 'rm -rf "$TMPDIR_ROOT"' EXIT

write_valid_adr() {
  local dir="$1"
  local name="${2:-ADR-999-test.md}"
  cat > "$dir/$name" << 'ADEOF'
# ADR-999: Test ADR

## Status
Accepted

## Context
Some context here.

## Decision
We decided to do this.

## Consequences
Positive and negative results.

## Alternatives Considered
Other options.
ADEOF
}

# Test 1: Valid ADR passes
dir1=$(mktemp -d "$TMPDIR_ROOT/test1.XXXXXX")
write_valid_adr "$dir1"
assert_pass "valid ADR with all required sections passes" bash "$VALIDATOR" "$dir1"

# Test 2: Missing ## Context fails
dir2=$(mktemp -d "$TMPDIR_ROOT/test2.XXXXXX")
cat > "$dir2/ADR-001-bad.md" << 'EOF'
# ADR-001: Bad

## Status
Accepted

## Decision
We decided.

## Consequences
Results.
EOF
assert_fail "ADR missing ## Context fails" bash "$VALIDATOR" "$dir2"

# Test 3: Missing ## Decision fails
dir3=$(mktemp -d "$TMPDIR_ROOT/test3.XXXXXX")
cat > "$dir3/ADR-001-bad.md" << 'EOF'
# ADR-001: Bad

## Status
Accepted

## Context
Context here.

## Consequences
Results.
EOF
assert_fail "ADR missing ## Decision fails" bash "$VALIDATOR" "$dir3"

# Test 4: Missing ## Consequences fails
dir4=$(mktemp -d "$TMPDIR_ROOT/test4.XXXXXX")
cat > "$dir4/ADR-001-bad.md" << 'EOF'
# ADR-001: Bad

## Status
Accepted

## Context
Context here.

## Decision
Decided.
EOF
assert_fail "ADR missing ## Consequences fails" bash "$VALIDATOR" "$dir4"

# Test 5: Missing ## Status fails
dir5=$(mktemp -d "$TMPDIR_ROOT/test5.XXXXXX")
cat > "$dir5/ADR-001-bad.md" << 'EOF'
# ADR-001: Bad

## Context
Context here.

## Decision
Decided.

## Consequences
Results.
EOF
assert_fail "ADR missing ## Status fails" bash "$VALIDATOR" "$dir5"

# Test 6: Invalid Status value fails
dir6=$(mktemp -d "$TMPDIR_ROOT/test6.XXXXXX")
cat > "$dir6/ADR-001-bad.md" << 'EOF'
# ADR-001: Bad

## Status
InvalidStatus

## Context
Context here.

## Decision
Decided.

## Consequences
Results.
EOF
assert_fail "ADR with invalid Status value fails" bash "$VALIDATOR" "$dir6"

# Test 7: Malformed title fails
dir7=$(mktemp -d "$TMPDIR_ROOT/test7.XXXXXX")
cat > "$dir7/ADR-001-bad.md" << 'EOF'
# Bad Title Without ADR Number

## Status
Accepted

## Context
Context here.

## Decision
Decided.

## Consequences
Results.
EOF
assert_fail "ADR with malformed title fails" bash "$VALIDATOR" "$dir7"

# Test 8: Empty directory returns exit 0
dir8=$(mktemp -d "$TMPDIR_ROOT/test8.XXXXXX")
assert_pass "empty directory returns exit 0" bash "$VALIDATOR" "$dir8"

echo ""
echo "Results: $passed passed, $failed failed"

if [ "$failed" -gt 0 ]; then
  exit 1
fi
exit 0
