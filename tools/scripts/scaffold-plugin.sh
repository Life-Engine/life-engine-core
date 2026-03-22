#!/usr/bin/env bash
set -euo pipefail

# Scaffold a new Life Engine Core plugin from a template.
# Usage: scaffold-plugin.sh <name>
#   name  — kebab-case plugin name (e.g. my-tasks)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

if [[ $# -lt 1 ]]; then
  echo "Usage: scaffold-plugin.sh <name>"
  exit 1
fi

NAME="$1"

# Validate plugin name: lowercase alphanumeric and hyphens only, must start with a letter
if [[ ! "$NAME" =~ ^[a-z][a-z0-9-]*$ ]]; then
  echo "Error: plugin name must be lowercase alphanumeric with hyphens (e.g. my-plugin)"
  echo "       Must start with a letter. Got: '$NAME'"
  exit 1
fi

TARGET_DIR="$REPO_ROOT/plugins/engine/$NAME"
TEMPLATE_DIR="$REPO_ROOT/tools/templates/engine-plugin"

# Check target doesn't already exist
if [[ -d "$TARGET_DIR" ]]; then
  echo "Error: directory already exists: $TARGET_DIR"
  exit 1
fi

# Capitalize first letter of a word (portable, no bash 4 needed)
capitalize() {
  local word="$1"
  local first
  first=$(echo "${word:0:1}" | tr '[:lower:]' '[:upper:]')
  echo "${first}${word:1}"
}

# Derive PascalCase name
IFS='-' read -ra PARTS <<< "$NAME"
PASCAL_NAME=""
for part in "${PARTS[@]}"; do
  PASCAL_NAME+="$(capitalize "$part")"
done

# Copy template
cp -r "$TEMPLATE_DIR" "$TARGET_DIR"

# Apply substitutions to all files in the target
find "$TARGET_DIR" -type f | while read -r file; do
  tmp="$file.tmp"
  sed "s/{{plugin-name}}/$NAME/g; s/MyPlugin/${PASCAL_NAME}Plugin/g" "$file" > "$tmp"
  mv "$tmp" "$file"
done

# Add to Cargo.toml workspace members
MEMBER="    \"plugins/engine/$NAME\","
# Insert before the closing ] of the members array
sed_tmp="$REPO_ROOT/Cargo.toml.tmp"
awk -v member="$MEMBER" '
  /^]/ && in_members {
    print member
    in_members = 0
  }
  /^\[workspace\]/ { in_workspace = 1 }
  in_workspace && /^members/ { in_members = 1 }
  { print }
' "$REPO_ROOT/Cargo.toml" > "$sed_tmp"
mv "$sed_tmp" "$REPO_ROOT/Cargo.toml"

echo "Created engine plugin at: $TARGET_DIR"
echo "Next steps:"
echo "  cd plugins/engine/$NAME && cargo check"
