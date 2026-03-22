#!/usr/bin/env bash
set -euo pipefail

# Scaffold a new Life Engine plugin from a template.
# Usage: scaffold-plugin.sh <name> <type>
#   name  — kebab-case plugin name (e.g. my-tasks)
#   type  — engine | life-vanilla | life-lit

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

if [[ $# -lt 2 ]]; then
  echo "Usage: scaffold-plugin.sh <name> <type>"
  echo "Types: engine, life-vanilla, life-lit"
  exit 1
fi

NAME="$1"
TYPE="$2"

# Validate type
case "$TYPE" in
  engine|life-vanilla|life-lit) ;;
  *)
    echo "Error: unknown plugin type '$TYPE'"
    echo "Valid types: engine, life-vanilla, life-lit"
    exit 1
    ;;
esac

# Derive target directory
case "$TYPE" in
  engine)       TARGET_DIR="$REPO_ROOT/plugins/engine/$NAME" ;;
  life-vanilla) TARGET_DIR="$REPO_ROOT/plugins/life/$NAME" ;;
  life-lit)     TARGET_DIR="$REPO_ROOT/plugins/life/$NAME" ;;
esac

# Check target doesn't already exist
if [[ -d "$TARGET_DIR" ]]; then
  echo "Error: directory already exists: $TARGET_DIR"
  exit 1
fi

# Derive template directory
case "$TYPE" in
  engine)       TEMPLATE_DIR="$REPO_ROOT/tools/templates/engine-plugin" ;;
  life-vanilla) TEMPLATE_DIR="$REPO_ROOT/tools/templates/life-plugin-vanilla" ;;
  life-lit)     TEMPLATE_DIR="$REPO_ROOT/tools/templates/life-plugin-lit" ;;
esac

# Capitalize first letter of a word (portable, no bash 4 needed)
capitalize() {
  local word="$1"
  local first
  first=$(echo "${word:0:1}" | tr '[:lower:]' '[:upper:]')
  echo "${first}${word:1}"
}

# Derive display name (title case from kebab-case)
DISPLAY_NAME=""
IFS='-' read -ra PARTS <<< "$NAME"
for part in "${PARTS[@]}"; do
  DISPLAY_NAME+="$(capitalize "$part") "
done
DISPLAY_NAME="${DISPLAY_NAME% }"

# Derive PascalCase name
PASCAL_NAME=""
for part in "${PARTS[@]}"; do
  PASCAL_NAME+="$(capitalize "$part")"
done

PLUGIN_ID="com.life-engine.$NAME"

# Copy template
cp -r "$TEMPLATE_DIR" "$TARGET_DIR"

# Apply substitutions to all files in the target
find "$TARGET_DIR" -type f | while read -r file; do
  # Create temp file for portable sed
  tmp="$file.tmp"

  case "$TYPE" in
    engine)
      sed "s/{{plugin-name}}/$NAME/g; s/MyPlugin/${PASCAL_NAME}Plugin/g" "$file" > "$tmp"
      ;;
    life-vanilla)
      sed "s/com\.example\.my-plugin/$PLUGIN_ID/g; s/My Plugin/$DISPLAY_NAME/g; s/MyPlugin/${PASCAL_NAME}Plugin/g; s/my-plugin/$NAME/g" "$file" > "$tmp"
      ;;
    life-lit)
      sed "s/com\.example\.my-lit-plugin/$PLUGIN_ID/g; s/My Lit Plugin/$DISPLAY_NAME/g; s/MyLitPlugin/${PASCAL_NAME}Plugin/g; s/my-lit-plugin/$NAME/g" "$file" > "$tmp"
      ;;
  esac

  mv "$tmp" "$file"
done

# For engine plugins, add to Cargo.toml workspace members
if [[ "$TYPE" == "engine" ]]; then
  MEMBER="    \"plugins/engine/$NAME\","
  # Insert before the closing ] of the members array
  # Find the last line matching a member entry and append after it
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
fi

echo "Created $TYPE plugin at: $TARGET_DIR"
case "$TYPE" in
  engine)
    echo "Next steps:"
    echo "  cd plugins/engine/$NAME && cargo check"
    ;;
  life-vanilla|life-lit)
    echo "Next steps:"
    echo "  cd plugins/life/$NAME"
    ;;
esac
