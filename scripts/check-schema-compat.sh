#!/usr/bin/env bash
#
# check-schema-compat.sh — Verify additive-only schema changes between versions
#
# Compares JSON Schema files in .odm/doc/schemas/ between the current working
# tree and a previous git ref (tag, branch, or commit). Detects breaking changes
# and allows additive-only evolution.
#
# Usage:
#   ./scripts/check-schema-compat.sh <previous-ref>
#
# Example:
#   ./scripts/check-schema-compat.sh v0.1.0
#
# Breaking changes (exit 1):
#   - Required field removed
#   - Field type changed
#   - Enum value removed
#   - Field removed entirely
#
# Allowed changes (exit 0):
#   - New optional field added
#   - New enum value added
#   - New collection (schema file) added
#   - Description changes
#
# Requires: jq, git

set -euo pipefail

SCHEMA_DIR=".odm/doc/schemas"
SCHEMA_GLOB="*.schema.json"

# --- helpers ----------------------------------------------------------------

die() { echo "ERROR: $1" >&2; exit 1; }

usage() {
  echo "Usage: $0 <previous-git-ref>"
  echo ""
  echo "Compare canonical JSON Schema files against a previous version."
  echo "Exit 0 if changes are additive-only, exit 1 if breaking changes found."
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "'$1' is required but not installed."
}

# --- pre-flight checks ------------------------------------------------------

require_cmd jq
require_cmd git

[[ $# -ge 1 ]] || usage

PREVIOUS_REF="$1"

# Verify the ref exists
git rev-parse --verify "$PREVIOUS_REF" >/dev/null 2>&1 \
  || die "Git ref '$PREVIOUS_REF' does not exist."

# --- state -------------------------------------------------------------------

BREAKING=0
ADDED_FIELDS=()
ADDED_ENUMS=()
NEW_COLLECTIONS=()
BREAKING_MESSAGES=()

# --- schema comparison functions --------------------------------------------

# Get the list of schema files at a given ref
schemas_at_ref() {
  git ls-tree -r --name-only "$1" -- "$SCHEMA_DIR" 2>/dev/null \
    | grep '\.schema\.json$' || true
}

# Read a schema file at a given ref
schema_content_at_ref() {
  git show "$1:$2" 2>/dev/null
}

# Extract all property paths from a schema (top-level properties only)
extract_properties() {
  echo "$1" | jq -r '.properties // {} | keys[]' 2>/dev/null || true
}

# Extract required fields
extract_required() {
  echo "$1" | jq -r '.required // [] | .[]' 2>/dev/null || true
}

# Extract the type of a property
extract_property_type() {
  echo "$1" | jq -r ".properties[\"$2\"].type // empty" 2>/dev/null || true
}

# Extract the $ref of a property
extract_property_ref() {
  echo "$1" | jq -r ".properties[\"$2\"].\"\$ref\" // empty" 2>/dev/null || true
}

# Extract enum values from a $defs entry or inline enum
extract_enum_values() {
  local schema="$1"
  local def_name="$2"
  echo "$schema" | jq -r ".\"\$defs\"[\"$def_name\"].enum // [] | .[]" 2>/dev/null || true
}

# Extract all $defs names
extract_defs() {
  echo "$1" | jq -r '."$defs" // {} | keys[]' 2>/dev/null || true
}

# Extract inline enum values for a property
extract_inline_enum() {
  echo "$1" | jq -r ".properties[\"$2\"].enum // [] | .[]" 2>/dev/null || true
}

record_breaking() {
  BREAKING=1
  BREAKING_MESSAGES+=("$1")
}

# --- main comparison --------------------------------------------------------

echo "Comparing schemas: $PREVIOUS_REF → current"
echo "Schema directory: $SCHEMA_DIR"
echo ""

OLD_SCHEMAS=$(schemas_at_ref "$PREVIOUS_REF")
CURRENT_SCHEMAS=$(ls "$SCHEMA_DIR"/$SCHEMA_GLOB 2>/dev/null \
  | sed "s|^./||" || true)

# Normalise current schemas to relative paths
CURRENT_SCHEMAS_REL=""
for f in $CURRENT_SCHEMAS; do
  rel="${f#./}"
  # Make path relative to repo root if it's absolute
  rel="${rel#"$PWD/"}"
  # If it already starts with .odm, keep it; otherwise prepend
  if [[ "$rel" == "$SCHEMA_DIR"/* ]]; then
    CURRENT_SCHEMAS_REL="$CURRENT_SCHEMAS_REL $rel"
  else
    basename=$(basename "$rel")
    CURRENT_SCHEMAS_REL="$CURRENT_SCHEMAS_REL $SCHEMA_DIR/$basename"
  fi
done

# Check for new collections (schema files added)
for current_path in $CURRENT_SCHEMAS_REL; do
  basename=$(basename "$current_path")
  found=false
  for old_path in $OLD_SCHEMAS; do
    if [[ "$(basename "$old_path")" == "$basename" ]]; then
      found=true
      break
    fi
  done
  if [[ "$found" == "false" ]]; then
    collection="${basename%.schema.json}"
    NEW_COLLECTIONS+=("$collection")
  fi
done

# Check for removed collections (schema files deleted — breaking)
for old_path in $OLD_SCHEMAS; do
  basename=$(basename "$old_path")
  found=false
  for current_path in $CURRENT_SCHEMAS_REL; do
    if [[ "$(basename "$current_path")" == "$basename" ]]; then
      found=true
      break
    fi
  done
  if [[ "$found" == "false" ]]; then
    collection="${basename%.schema.json}"
    record_breaking "Collection '$collection' removed ($basename deleted)"
  fi
done

# Compare each schema that exists in both versions
for old_path in $OLD_SCHEMAS; do
  basename=$(basename "$old_path")
  collection="${basename%.schema.json}"

  # Find matching current file
  current_file=""
  for current_path in $CURRENT_SCHEMAS_REL; do
    if [[ "$(basename "$current_path")" == "$basename" ]]; then
      current_file="$current_path"
      break
    fi
  done

  [[ -n "$current_file" ]] || continue

  old_content=$(schema_content_at_ref "$PREVIOUS_REF" "$old_path")
  new_content=$(cat "$current_file")

  # 1. Check for removed required fields
  old_required=$(extract_required "$old_content")
  new_required=$(extract_required "$new_content")

  for field in $old_required; do
    if ! echo "$new_required" | grep -qx "$field"; then
      record_breaking "[$collection] Required field '$field' removed"
    fi
  done

  # 2. Check for removed properties
  old_props=$(extract_properties "$old_content")
  new_props=$(extract_properties "$new_content")

  for prop in $old_props; do
    if ! echo "$new_props" | grep -qx "$prop"; then
      record_breaking "[$collection] Field '$prop' removed"
    fi
  done

  # 3. Check for added properties (additive)
  for prop in $new_props; do
    if ! echo "$old_props" | grep -qx "$prop"; then
      ADDED_FIELDS+=("[$collection] $prop")
    fi
  done

  # 4. Check for type changes on existing properties
  for prop in $old_props; do
    echo "$new_props" | grep -qx "$prop" || continue

    old_type=$(extract_property_type "$old_content" "$prop")
    new_type=$(extract_property_type "$new_content" "$prop")

    old_ref=$(extract_property_ref "$old_content" "$prop")
    new_ref=$(extract_property_ref "$new_content" "$prop")

    if [[ -n "$old_type" && -n "$new_type" && "$old_type" != "$new_type" ]]; then
      record_breaking "[$collection] Field '$prop' type changed: '$old_type' → '$new_type'"
    fi

    if [[ -n "$old_ref" && -n "$new_ref" && "$old_ref" != "$new_ref" ]]; then
      record_breaking "[$collection] Field '$prop' \$ref changed: '$old_ref' → '$new_ref'"
    fi

    # Check inline enum changes
    old_inline_enum=$(extract_inline_enum "$old_content" "$prop")
    new_inline_enum=$(extract_inline_enum "$new_content" "$prop")

    if [[ -n "$old_inline_enum" ]]; then
      for val in $old_inline_enum; do
        if ! echo "$new_inline_enum" | grep -qx "$val"; then
          record_breaking "[$collection] Enum value '$val' removed from field '$prop'"
        fi
      done
      for val in $new_inline_enum; do
        if ! echo "$old_inline_enum" | grep -qx "$val"; then
          ADDED_ENUMS+=("[$collection] $prop: $val")
        fi
      done
    fi
  done

  # 5. Check $defs enum value changes
  old_defs=$(extract_defs "$old_content")
  new_defs=$(extract_defs "$new_content")

  for def in $old_defs; do
    echo "$new_defs" | grep -qx "$def" || continue

    old_enum=$(extract_enum_values "$old_content" "$def")
    new_enum=$(extract_enum_values "$new_content" "$def")

    [[ -n "$old_enum" ]] || continue

    for val in $old_enum; do
      if ! echo "$new_enum" | grep -qx "$val"; then
        record_breaking "[$collection] Enum value '$val' removed from \$defs/$def"
      fi
    done

    for val in $new_enum; do
      if ! echo "$old_enum" | grep -qx "$val"; then
        ADDED_ENUMS+=("[$collection] \$defs/$def: $val")
      fi
    done
  done
done

# --- summary ----------------------------------------------------------------

echo "=== Schema Compatibility Report ==="
echo ""

if [[ ${#NEW_COLLECTIONS[@]} -gt 0 ]]; then
  echo "New collections added:"
  for c in "${NEW_COLLECTIONS[@]}"; do
    echo "  + $c"
  done
  echo ""
fi

if [[ ${#ADDED_FIELDS[@]} -gt 0 ]]; then
  echo "New fields added:"
  for f in "${ADDED_FIELDS[@]}"; do
    echo "  + $f"
  done
  echo ""
fi

if [[ ${#ADDED_ENUMS[@]} -gt 0 ]]; then
  echo "New enum values added:"
  for e in "${ADDED_ENUMS[@]}"; do
    echo "  + $e"
  done
  echo ""
fi

if [[ $BREAKING -eq 1 ]]; then
  echo "BREAKING CHANGES DETECTED:"
  for msg in "${BREAKING_MESSAGES[@]}"; do
    echo "  ✗ $msg"
  done
  echo ""
  echo "Schema changes are NOT backward-compatible."
  exit 1
fi

echo "No breaking changes detected. All changes are additive."
exit 0
