# Life Engine — Development Commands

# Run the Core backend with auto-restart on source changes
dev-core:
    @echo "Starting Life Engine Core (cargo-watch)..."
    cargo watch -w apps/core/ -w packages/ -x 'run --bin life-engine-core'

# Run the Admin UI development server
dev-app:
    @echo "Starting Admin UI (vite)..."
    cd apps/admin && pnpm dev

# Run Core and Admin UI concurrently
dev-all:
    @echo "Starting Life Engine Core + Admin UI..."
    just dev-core & just dev-app & wait

# Run all workspace tests
test:
    cargo test --workspace

# Run clippy across all crates, treating warnings as errors
lint:
    cargo clippy --workspace -- -D warnings

# Scaffold a new plugin from the template
new-plugin name:
    #!/usr/bin/env bash
    set -euo pipefail
    id="{{name}}"
    crate_name=$(echo "{{name}}" | tr '-' '_')
    dest="plugins/engine/{{name}}"
    template="tools/templates/plugin"
    if [ ! -d "$template" ]; then
        echo "Error: Plugin template not found at $template"
        echo "Run WP 1.8 to create the plugin scaffold template first."
        exit 1
    fi
    if [ -d "$dest" ]; then
        echo "Error: Plugin directory $dest already exists."
        exit 1
    fi
    cp -r "$template" "$dest"
    # Replace placeholders in all files
    find "$dest" -type f -exec sed -i '' "s/{{{{name}}}}/$id/g; s/{{{{id}}}}/$id/g" {} +
    # Add to Cargo.toml workspace members
    sed -i '' "/^]$/i\\
    \"$dest\",
    " Cargo.toml
    echo "Plugin scaffolded at $dest"
    echo "Next steps:"
    echo "  1. Edit $dest/manifest.toml with your plugin metadata"
    echo "  2. Implement your actions in $dest/src/steps/"
    echo "  3. Run 'just test' to verify compilation"
