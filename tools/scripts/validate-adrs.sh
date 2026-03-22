#!/usr/bin/env bash
set -euo pipefail

# Validate ADR files contain required sections per the format in docs/adrs/README.md.
# Usage: validate-adrs.sh [directory]
# Defaults to docs/adrs/ relative to the repo root.

ADR_DIR="${1:-$(git rev-parse --show-toplevel)/docs/adrs}"

if [ ! -d "$ADR_DIR" ]; then
  echo "ERROR: directory not found: $ADR_DIR"
  exit 1
fi

REQUIRED_SECTIONS=("## Status" "## Context" "## Decision" "## Consequences")
VALID_STATUS_PATTERN="^(Accepted|Proposed|Deprecated|Superseded by ADR-[0-9]+)$"

failures=0
checked=0

while IFS= read -r -d '' file; do
  name=$(basename "$file")
  checked=$((checked + 1))
  failed=0

  # Check title line matches # ADR-NNN: ...
  if ! grep -qE '^# ADR-[0-9]+: ' "$file"; then
    echo "FAIL: $name — malformed title (expected '# ADR-NNN: ...')"
    failed=1
  fi

  # Check required sections
  for section in "${REQUIRED_SECTIONS[@]}"; do
    if ! grep -q "^${section}$" "$file"; then
      echo "FAIL: $name — missing section: $section"
      failed=1
    fi
  done

  # Check Status value
  if grep -q "^## Status$" "$file"; then
    status_line=$(awk '/^## Status$/{found=1; next} found && /^[^#]/ && NF{print; exit}' "$file")
    if [ -n "$status_line" ]; then
      if ! echo "$status_line" | grep -qE "$VALID_STATUS_PATTERN"; then
        echo "FAIL: $name — invalid Status value: '$status_line'"
        failed=1
      fi
    fi
  fi

  if [ "$failed" -eq 0 ]; then
    echo "PASS: $name"
  else
    failures=$((failures + 1))
  fi
done < <(find "$ADR_DIR" -name "ADR-*.md" -print0 | sort -z)

if [ "$checked" -eq 0 ]; then
  echo "No ADR files found in $ADR_DIR"
  exit 0
fi

echo ""
echo "$checked files checked, $failures failed"

if [ "$failures" -gt 0 ]; then
  exit 1
fi
exit 0
